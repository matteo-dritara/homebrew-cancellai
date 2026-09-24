"""Tests for the OpenCode reviewer agents' shell permissions (E34-S02).

OpenCode turns an agent's `permission.bash` map into an ordered rule list and applies the last
rule whose wildcard matches the command. These tests read the committed agent files and evaluate
commands the same way, so an allow that re-admits a denied operation through an allowed
interpreter - round 1's `python3 -m pip install` - fails here rather than in a review.
"""

from __future__ import annotations

import fnmatch
import json
import os
import re
import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
AGENTS = ("verifier", "pre-reviewer")
RULE_RE = re.compile(r'^    "(.+)": (allow|deny|ask)\s*$')


def bash_rules(agent: str) -> list[tuple[str, str]]:
    text = (ROOT / ".opencode" / "agents" / f"{agent}.md").read_text(encoding="utf-8")
    frontmatter = text.split("---", 2)[1]
    rules, inside = [], False
    for line in frontmatter.splitlines():
        if line.startswith("  bash:"):
            inside = True
        elif inside and line.startswith("    "):
            match = RULE_RE.match(line)
            if match:
                rules.append((match.group(1), match.group(2)))
        elif inside and not line.startswith("    #"):
            break
    return rules


def decide(rules: list[tuple[str, str]], command: str) -> str:
    decision = "ask"  # what OpenCode does when nothing matches
    for pattern, action in rules:
        if fnmatch.fnmatchcase(command, pattern):
            decision = action
    return decision


DENIED = (
    # Round 1's reproductions: denied directly, re-admitted through `python3 *`.
    "python3 -m pip install example",
    "python3 -c \"import os; os.remove('file')\"",
    "python3 -c \"import urllib.request; urllib.request.urlopen('https://example.com')\"",
    "git commit -qm x",
    "git push origin HEAD:main",
    "git tag v9",
    "git reset --hard HEAD~1",
    "pip install example",
    "rm -rf project",
    "curl https://example.com",
    "cargo install cargo-evil",
    "python3 scripts/../project/evidence/x.py",
    "find . -name '*.md' -delete",
    "find . -exec sh -c 'x' ;",
    "cat README.md; rm README.md",
    "ls && rm README.md",
    "cat x.sh | sh",
    "echo $(rm README.md)",
    "python3 -m pytest tests > /tmp/out",
    "brew install x",
    "npm install x",
    # Round 2: an allowed script whose subcommand reaches the web through gh.
    "python3 scripts/check_agent_toolchain.py updates",
    "python3 scripts/check_platforms.py check",
    "python3 scripts/release.py finalize 2.0.0",
    "python3 scripts/release.py verify-formula",
    "python3 scripts/new_script.py check",
    "python3 scripts/project_os.py brief E34-S01 --role executor; curl x",
    "cargo test --config net.offline=false",
    "CARGO_NET_OFFLINE=false cargo test",
    # Self-review of round 2: writes to a file named in the arguments, and background jobs.
    "sed -n w/tmp/x README.md",
    "sort -o /tmp/x README.md",
    "uniq README.md /tmp/x",
    "git diff HEAD~1 --output=/tmp/x",
    "git log --output=/tmp/x",
    "python3 -m pytest tests/test_x.py &",
    "rg --pre ./x pattern",
    "find . -fprint /tmp/x",
    "touch /tmp/x",
    "mkdir /tmp/x",
    "chmod 000 README.md",
)
ALLOWED = (
    "python3 -m pytest tests -q",
    "python3 scripts/project_os.py check",
    "cargo test --workspace",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "git diff HEAD~1",
    "git show HEAD:project/epics/E34.json",
    "grep -rn review_round scripts",
    "head -40 scripts/review_round.py",
)


# A script an agent may run is either free of anything that reaches the network, or listed here
# with the reason the allowed subcommand never reaches it.
NETWORK_MARKERS = ("urllib.request", 'which("gh")', '"curl"', "socket.", "http.client")
REVIEWED_SCRIPTS = {
    "check_agent_toolchain.py": "gh is called only by `updates`; `check` and `report` never reach it",
    "release.py": "urlopen is reached only from finalize and verify-formula; `check` never reaches it",
}
SCRIPT_RULE = re.compile(r"^python3 scripts/(\S+\.py)(?: |$)")


class ReviewerShellPermissionTests(unittest.TestCase):
    def test_every_allowed_script_is_an_exact_reviewed_subcommand(self) -> None:
        for agent in AGENTS:
            for pattern, action in bash_rules(agent):
                match = SCRIPT_RULE.match(pattern)
                if action != "allow" or not match:
                    continue
                with self.subTest(agent=agent, pattern=pattern):
                    self.assertNotIn("*", pattern)
                    source = (ROOT / "scripts" / match.group(1)).read_text(encoding="utf-8")
                    if any(marker in source for marker in NETWORK_MARKERS):
                        self.assertIn(match.group(1), REVIEWED_SCRIPTS)

    # Round 3 (E34-S02): OpenCode's own defaults start with `"*": allow`, so a permission category
    # the agent file did not name - a tool added in a later OpenCode - was allowed.
    def test_every_unnamed_permission_category_is_denied_first(self) -> None:
        for agent in AGENTS:
            text = (ROOT / ".opencode" / "agents" / f"{agent}.md").read_text(encoding="utf-8")
            block = text.split("permission:\n", 1)[1]
            first = next(line for line in block.splitlines() if line.strip() and not line.strip().startswith("#"))
            self.assertEqual(first, '  "*": deny', agent)

    @unittest.skipUnless(shutil.which("opencode"), "OpenCode is not installed")
    def test_opencode_itself_denies_what_the_file_does_not_name(self) -> None:
        """The same question put to OpenCode's own engine, not to a model of it."""
        for agent in AGENTS:
            result = subprocess.run(  # noqa: S603
                ["opencode", "debug", "agent", agent],  # noqa: S607
                cwd=ROOT,
                capture_output=True,
                text=True,
                check=False,
                env={**os.environ, "OPENCODE_DISABLE_AUTOUPDATE": "1", "OPENCODE_DISABLE_CLAUDE_CODE": "1"},
                timeout=120,
            )
            if result.returncode != 0:
                self.skipTest(f"opencode debug could not run here: {result.stderr.strip()[:200]}")
            rules = json.loads(result.stdout)["permission"]
            for category, expected in (("some_future_tool", "deny"), ("webfetch", "deny"), ("task", "deny"), ("read", "allow")):
                decision = None
                for rule in rules:
                    if fnmatch.fnmatchcase(category, rule["permission"]) and fnmatch.fnmatchcase("x", rule["pattern"]):
                        decision = rule["action"]
                with self.subTest(agent=agent, category=category):
                    self.assertEqual(decision, expected)

    def test_cargo_runs_offline_in_the_reviewer_environment(self) -> None:
        import tempfile

        from scripts import review_round

        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(review_round.reviewer_env(Path(tmp))["CARGO_NET_OFFLINE"], "true")

    def test_the_agents_declare_their_rules(self) -> None:
        for agent in AGENTS:
            rules = bash_rules(agent)
            self.assertEqual(rules[0], ("*", "deny"), agent)
            self.assertGreater(len(rules), 20, agent)

    def test_denied_operations_stay_denied_through_allowed_commands(self) -> None:
        for agent in AGENTS:
            rules = bash_rules(agent)
            for command in DENIED:
                with self.subTest(agent=agent, command=command):
                    self.assertEqual(decide(rules, command), "deny")

    def test_the_commands_a_reviewer_needs_are_allowed(self) -> None:
        for agent in AGENTS:
            rules = bash_rules(agent)
            for command in ALLOWED:
                with self.subTest(agent=agent, command=command):
                    self.assertEqual(decide(rules, command), "allow")

    def test_nothing_is_left_to_a_prompt(self) -> None:
        # A non-interactive run cannot answer one (AC3): every command reaches allow or deny.
        for agent in AGENTS:
            self.assertEqual(decide(bash_rules(agent), "some-unknown-tool --flag"), "deny")


if __name__ == "__main__":
    unittest.main()

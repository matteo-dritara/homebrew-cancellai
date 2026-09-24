"""Tests for the OpenCode reviewer agents' shell permissions (E34-S02).

OpenCode turns an agent's `permission.bash` map into an ordered rule list and applies the last
rule whose wildcard matches the command. These tests read the committed agent files and evaluate
commands the same way, so an allow that re-admits a denied operation through an allowed
interpreter - round 1's `python3 -m pip install` - fails here rather than in a review.
"""

from __future__ import annotations

import fnmatch
import re
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
)
ALLOWED = (
    "python3 -m pytest tests -q",
    "python3 scripts/project_os.py check",
    "cargo test --workspace",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "git diff HEAD~1",
    "git show HEAD:project/epics/E34.json",
    "grep -rn review_round scripts",
    "sed -n 1,40p scripts/review_round.py",
)


class ReviewerShellPermissionTests(unittest.TestCase):
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

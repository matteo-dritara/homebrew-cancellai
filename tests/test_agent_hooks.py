"""Tests for the generated-document edit guard (E24-S02).

The hook is a convenience guard, not a safety boundary: `scripts/project_os.py check` and
`scripts/gen_docs.py --check` remain the authority on drift. What matters here is that it
refuses exactly the generated documents, refuses nothing else, and - most importantly - fails
*open* on any input it does not understand. A guard that can halt a session on a malformed
payload would be worse than the mistake it prevents.
"""

from __future__ import annotations

import json
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HOOK = ROOT / ".claude" / "hooks" / "guard-generated-docs.sh"

REFUSED = 2
ALLOWED = 0


def run_hook(payload: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(  # noqa: S603
        [str(HOOK)],
        input=payload,
        capture_output=True,
        text=True,
        timeout=20,
        check=False,
    )


def edit_of(path: str) -> str:
    return json.dumps({"tool_name": "Edit", "tool_input": {"file_path": path}})


class GuardRefusesGeneratedDocumentsTests(unittest.TestCase):
    def setUp(self):
        if sys.platform.startswith("win"):
            self.skipTest("the hook is a POSIX shell script; Windows sessions use the CI drift check")
        self.assertTrue(HOOK.is_file(), f"{HOOK} is missing")

    def test_the_hook_is_executable(self):
        import os

        self.assertTrue(os.access(HOOK, os.X_OK), "the hook must be executable or the harness cannot run it")

    def test_the_generated_planning_documents_are_refused(self):
        for relative in ("docs/DECISION_REGISTER.md", "docs/ROADMAP.md", "docs/BACKLOG.md"):
            with self.subTest(relative=relative):
                result = run_hook(edit_of(str(ROOT / relative)))
                self.assertEqual(REFUSED, result.returncode)
                self.assertIn("scripts/project_os.py generate", result.stderr)

    def test_anything_under_project_generated_is_refused(self):
        result = run_hook(edit_of(str(ROOT / "project" / "generated" / "PROJECT_STATUS.md")))
        self.assertEqual(REFUSED, result.returncode)
        self.assertIn("scripts/project_os.py generate", result.stderr)

    def test_the_generated_cli_reference_names_its_own_generator(self):
        result = run_hook(edit_of(str(ROOT / "docs" / "CLI.md")))
        self.assertEqual(REFUSED, result.returncode)
        self.assertIn("scripts/gen_docs.py", result.stderr)

    def test_the_refusal_names_the_source_to_edit_instead(self):
        result = run_hook(edit_of(str(ROOT / "docs" / "BACKLOG.md")))
        self.assertIn("project/epics/*.json", result.stderr)

    def test_every_spelling_of_the_same_file_is_refused(self):
        """Reproductions from the independent review of E24-S02.

        `docs/BACKLOG.md`, `docs/backlog.md`, `DOCS/BACKLOG.md`, `docs//BACKLOG.md` and
        `docs/adrs/../BACKLOG.md` are one inode on an APFS volume. README.md states the same
        rule for protected provider names, and for the same reason: a barrier must not depend
        on how a name happens to be spelled. The first implementation matched a `case` glob on
        the raw string and let four of these through.
        """
        spellings = (
            str(ROOT / "docs" / "BACKLOG.md"),
            str(ROOT / "docs" / "backlog.md"),
            str(ROOT / "DOCS" / "BACKLOG.md"),
            f"{ROOT}/docs//BACKLOG.md",
            f"{ROOT}/docs/adrs/../BACKLOG.md",
            "docs/BACKLOG.md",
            "./docs/BACKLOG.md",
        )
        for spelling in spellings:
            with self.subTest(spelling=spelling):
                self.assertEqual(REFUSED, run_hook(edit_of(spelling)).returncode)

    def test_the_generated_cli_reference_is_refused_case_insensitively(self):
        self.assertEqual(REFUSED, run_hook(edit_of(str(ROOT / "docs" / "cli.md"))).returncode)


class GuardAllowsEverythingElseTests(unittest.TestCase):
    def setUp(self):
        if sys.platform.startswith("win"):
            self.skipTest("the hook is a POSIX shell script; Windows sessions use the CI drift check")

    def test_a_rust_source_file_is_allowed(self):
        result = run_hook(edit_of(str(ROOT / "rust" / "crates" / "cancellai-safety" / "src" / "lib.rs")))
        self.assertEqual(ALLOWED, result.returncode)

    def test_the_hand_written_sources_of_the_generated_documents_are_allowed(self):
        for relative in ("project/roadmap.json", "project/decisions.json", "project/epics/E24.json"):
            with self.subTest(relative=relative):
                self.assertEqual(ALLOWED, run_hook(edit_of(str(ROOT / relative))).returncode)

    def test_a_document_whose_name_merely_resembles_a_generated_one_is_allowed(self):
        # docs/CLI_RUST.md is hand-maintained; only docs/CLI.md is generated.
        self.assertEqual(ALLOWED, run_hook(edit_of(str(ROOT / "docs" / "CLI_RUST.md"))).returncode)

    def test_a_path_outside_the_project_is_never_refused(self):
        """AGENTS.md requires tests to build synthetic trees. A guard that refuses any path on
        the machine ending in `/docs/BACKLOG.md` would block the repository's own test policy -
        which the first implementation did, because it matched a suffix rather than a
        project-relative path."""
        outside = (
            "/tmp/pytest-of-someone/test_tree0/docs/BACKLOG.md",
            "/tmp/t/project/generated/PROJECT_STATUS.md",
            "/var/folders/x/T/tmpabc/docs/ROADMAP.md",
        )
        for path in outside:
            with self.subTest(path=path):
                self.assertEqual(ALLOWED, run_hook(edit_of(path)).returncode)

    def test_a_lookalike_directory_inside_the_project_is_allowed(self):
        for path in (ROOT / "rust" / "docs" / "BACKLOG.md", ROOT / "my-docs" / "BACKLOG.md"):
            with self.subTest(path=str(path)):
                self.assertEqual(ALLOWED, run_hook(edit_of(str(path))).returncode)


class GuardFailsOpenTests(unittest.TestCase):
    """Every input the guard cannot interpret must let the tool call through."""

    def setUp(self):
        if sys.platform.startswith("win"):
            self.skipTest("the hook is a POSIX shell script; Windows sessions use the CI drift check")

    def test_malformed_json_is_allowed(self):
        self.assertEqual(ALLOWED, run_hook("{not json at all").returncode)

    def test_empty_input_is_allowed(self):
        self.assertEqual(ALLOWED, run_hook("").returncode)

    def test_a_payload_without_a_file_path_is_allowed(self):
        self.assertEqual(ALLOWED, run_hook(json.dumps({"tool_name": "Bash", "tool_input": {"command": "ls"}})).returncode)

    def test_a_payload_that_is_not_an_object_is_allowed(self):
        self.assertEqual(ALLOWED, run_hook(json.dumps(["docs/BACKLOG.md"])).returncode)

    def test_a_null_file_path_is_allowed(self):
        self.assertEqual(ALLOWED, run_hook(json.dumps({"tool_input": {"file_path": None}})).returncode)

    def test_a_non_string_file_path_is_allowed(self):
        for value in (42, ["docs/BACKLOG.md"], {"path": "docs/BACKLOG.md"}, ""):
            with self.subTest(value=value):
                self.assertEqual(ALLOWED, run_hook(json.dumps({"tool_input": {"file_path": value}})).returncode)

    def test_a_large_payload_naming_a_generated_document_still_refuses(self):
        payload = json.dumps({"pad": "x" * 1_000_000, "tool_input": {"file_path": str(ROOT / "docs" / "BACKLOG.md")}})
        self.assertEqual(REFUSED, run_hook(payload).returncode)


class GuardCoverageIsHonestlyBoundedTests(unittest.TestCase):
    """The matcher covers structured file-writing tools only.

    A write performed through Bash never reaches this hook, and matching those would mean
    parsing shell. The limit is real, so it is pinned here and stated in the hook's own header
    and in `.claude/skills/README.md` rather than left for a reader to discover.
    """

    def test_the_matcher_names_the_structured_writing_tools(self):
        settings = json.loads((ROOT / ".claude" / "settings.json").read_text(encoding="utf-8"))
        matchers = [entry["matcher"] for entry in settings["hooks"]["PreToolUse"]]
        self.assertIn("Edit|Write|NotebookEdit", matchers)

    def test_the_hook_documents_that_bash_writes_are_not_covered(self):
        header = (ROOT / ".claude" / "hooks" / "guard-generated-docs.sh").read_text(encoding="utf-8")
        self.assertIn("Bash", header)
        self.assertIn("remain the", header.replace("\n#     ", " ").replace("\n#   ", " "))


class GuardDoesNotReplaceTheDriftCheckTests(unittest.TestCase):
    def test_the_authority_on_drift_is_still_the_governance_checker(self):
        # The hook prevents one class of mistake earlier; it is not what proves the generated
        # documents are current. That remains project_os.py, which runs with no hook involved.
        result = subprocess.run(  # noqa: S603
            [sys.executable, str(ROOT / "scripts" / "project_os.py"), "check"],
            capture_output=True,
            text=True,
            timeout=120,
            check=False,
        )
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()

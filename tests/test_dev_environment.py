"""A fresh clone can run every gate after the documented setup, and nothing more (E28-S01).

The skill-content gate refuses outright when its scanner is absent - deliberately, because a gate
reporting "clean" without having scanned is worse than no gate. That makes the scanner a real
dependency of the repository rather than a preference on one machine, and it has two failure modes
that no other check here would catch:

* the scanner is pinned in `scripts/check_skill_content.py` and installed from
  `requirements-dev.txt`, and the two can drift. Bump one and forget the other and every fresh
  clone fails the gate with a version mismatch, on a machine whose owner did exactly what the
  documentation said;
* the documented setup and what CI actually runs can drift, so `pre-commit` passes for everyone who
  already has the tool and fails for everyone who followed the instructions.

These are the cases that produce "it works on my machine", which is the one outcome a repository of
gates cannot tolerate: a gate nobody else can run is a gate nobody else is bound by.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path
from typing import ClassVar

from scripts import check_skill_content as gate

ROOT = Path(__file__).resolve().parent.parent
REQUIREMENTS = ROOT / "requirements-dev.txt"
CONTRIBUTING = ROOT / ".github" / "CONTRIBUTING.md"
WORKFLOWS = ROOT / ".github" / "workflows"

SKILLSPECTOR_REQUIREMENT = re.compile(r"^skillspector\s*@\s*git\+\S+@v([0-9][0-9A-Za-z.\-+]*)\s*(?:;\s*(.+?))?\s*$", re.MULTILINE)
PYTHON_MARKER = re.compile(r"""python_version\s*>=\s*['"](\d+)\.(\d+)['"]""")


class TheScannerPinAgreesWithItself(unittest.TestCase):
    TEXT: ClassVar[str] = REQUIREMENTS.read_text(encoding="utf-8")

    def test_the_scanner_is_installed_by_the_documented_setup(self) -> None:
        """It used to be a separate install step, which is a step someone does not take."""
        self.assertIsNotNone(SKILLSPECTOR_REQUIREMENT.search(self.TEXT), "requirements-dev.txt must install the scanner")

    def test_the_requirement_and_the_gate_pin_the_same_version(self) -> None:
        match = SKILLSPECTOR_REQUIREMENT.search(self.TEXT)
        assert match is not None
        self.assertEqual(
            match.group(1),
            gate.PINNED_SCANNER,
            "requirements-dev.txt and check_skill_content.py must pin the same scanner; a drift "
            "fails every fresh clone with a version mismatch and passes on any machine that "
            "installed the tool earlier",
        )

    def test_the_requirement_is_skipped_below_the_scanner_s_minimum_python(self) -> None:
        """Without this marker, `pip install -r requirements-dev.txt` fails outright on the 3.10 leg
        of the test matrix - the scanner declares requires-python >=3.12. Making the tooling
        installable by one command must not make it uninstallable on a supported interpreter."""
        match = SKILLSPECTOR_REQUIREMENT.search(self.TEXT)
        assert match is not None
        marker = match.group(2)
        self.assertIsNotNone(marker, "the requirement needs a python_version marker")
        assert marker is not None
        version = PYTHON_MARKER.search(marker)
        self.assertIsNotNone(version, f"marker {marker!r} does not bound python_version")
        assert version is not None
        self.assertEqual((int(version.group(1)), int(version.group(2))), gate.MINIMUM_PYTHON)

    def test_the_gate_names_the_interpreter_requirement_when_the_scanner_is_absent(self) -> None:
        """On Python 3.10 the scanner is legitimately absent; the message must say so rather than
        read as a setup someone skipped."""
        message = gate.provenance_errors(None)[0]
        self.assertIn(f"{gate.MINIMUM_PYTHON[0]}.{gate.MINIMUM_PYTHON[1]}", message)
        self.assertIn("requirements-dev.txt", message)

    def test_the_requirement_is_pinned_to_a_tag_not_a_branch(self) -> None:
        """`@main` would let the scanner's detections change under a passing run."""
        self.assertNotIn("@main", self.TEXT)
        self.assertNotIn("@master", self.TEXT)


class TheDocumentedSetupIsWhatCiRuns(unittest.TestCase):
    def test_contributing_installs_the_requirements_file(self) -> None:
        text = CONTRIBUTING.read_text(encoding="utf-8")
        self.assertIn("pip install -r requirements-dev.txt", text)

    def test_no_workflow_installs_the_scanner_separately(self) -> None:
        """One source of truth. A second install step drifts from the requirements file silently."""
        for workflow in sorted(WORKFLOWS.glob("*.yml")):
            text = workflow.read_text(encoding="utf-8")
            self.assertNotIn(
                "pip install 'git+https://github.com/NVIDIA/SkillSpector",
                text,
                f"{workflow.name} installs the scanner outside requirements-dev.txt",
            )

    def test_every_workflow_running_pre_commit_installs_the_requirements(self) -> None:
        """pre-commit runs the skill-content hook, which refuses without the scanner."""
        for workflow in sorted(WORKFLOWS.glob("*.yml")):
            text = workflow.read_text(encoding="utf-8")
            if "pre_commit run" in text or "pre-commit run" in text:
                self.assertIn(
                    "pip install -r requirements-dev.txt",
                    text,
                    f"{workflow.name} runs pre-commit without installing the tooling it needs",
                )


class NoGateNeedsAnUndocumentedBinary(unittest.TestCase):
    """The gates may shell out, but only to things the documented setup or the platform provides."""

    # `git` is the repository itself; `cargo` and `gh` are documented in AGENTS.md and both degrade
    # truthfully when absent, which was verified by running each gate with them off PATH. Only
    # `skillspector` refuses, and it is the one this file exists to keep installable.
    PERMITTED: ClassVar[frozenset[str]] = frozenset({"git", "gh", "cargo", "skillspector", "pre-commit"})

    def test_no_script_shells_out_to_an_undeclared_binary(self) -> None:
        pattern = re.compile(r'shutil\.which\(\s*"([\w.-]+)"\s*\)')
        for script in sorted((ROOT / "scripts").glob("*.py")):
            for name in pattern.findall(script.read_text(encoding="utf-8")):
                self.assertIn(
                    name,
                    self.PERMITTED,
                    f"{script.name} looks for {name!r}, which no setup step installs and no "
                    "document names - add it to the documented setup or to this list with the "
                    "argument for why it is safe to assume",
                )


if __name__ == "__main__":
    unittest.main()

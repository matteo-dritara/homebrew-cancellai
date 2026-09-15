"""Tests for the method-defect record (E28-S03).

Every rule in AGENTS.md worth having was written after something went wrong, and the record of
what went wrong is nowhere: a session notices a rule is missing or unreachable, the correction
happens in conversation, the session ends. This section carries that forward in the packet where
the work already lives.

The design constraint is the one that rejected the package the idea came from: it generates skill
edits, and a skill generated from an observation is a second copy of the contract produced without
review. So the mechanism proposes and never writes, and the last test asserts that property rather
than trusting the author of the next change to remember it.
"""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts import check_evidence as evidence

ROOT = Path(__file__).resolve().parent.parent


def packet(section: str) -> str:
    return f"# Evidence\n\n## Method defects\n\n{section}\n\n## Residual risks\n\n- none\n"


ENTRY = (
    "**What happened**: the executor read windows-sys 0.59 while the workspace compiles 0.61.2 "
    "**Prevented by**: none exists **Disposition**: proposed"
)


class Parsing(unittest.TestCase):
    def test_none_is_not_an_entry(self) -> None:
        self.assertEqual(evidence.method_defect_entries(packet("- none")), [])

    def test_a_missing_section_is_not_an_entry(self) -> None:
        self.assertEqual(evidence.method_defect_entries("# Evidence\n\n## Residual risks\n\n- none\n"), [])

    def test_a_real_entry_is_found(self) -> None:
        self.assertEqual(len(evidence.method_defect_entries(packet(f"- {ENTRY}"))), 1)

    def test_entries_inside_a_fenced_example_do_not_count(self) -> None:
        """The same rule the acceptance-criteria rows already follow: an example is not evidence."""
        text = packet(f"```\n- {ENTRY}\n```")
        self.assertEqual(evidence.method_defect_entries(text), [])


class Disposition(unittest.TestCase):
    def test_an_entry_without_a_disposition_is_refused(self) -> None:
        text = packet("- **What happened**: something **Prevented by**: none exists")
        problems = evidence.method_defect_problems("E00-S01", text)
        self.assertEqual(len(problems), 1)
        self.assertIn("Disposition", problems[0])

    def test_each_permitted_disposition_is_accepted(self) -> None:
        for value in ("proposed", "accepted 2026-09-15", "declined 2026-09-15 - not worth a rule"):
            text = packet(f"- **What happened**: x **Prevented by**: none exists **Disposition**: {value}")
            self.assertEqual(evidence.method_defect_problems("E00-S01", text), [], value)

    def test_an_invented_disposition_is_refused(self) -> None:
        text = packet("- **What happened**: x **Prevented by**: none exists **Disposition**: maybe later")
        self.assertEqual(len(evidence.method_defect_problems("E00-S01", text)), 1)

    def test_an_entry_that_points_nowhere_is_refused(self) -> None:
        """Naming what would have prevented it, or saying none exists, is the actionable half."""
        text = packet("- **What happened**: something **Disposition**: proposed")
        problems = evidence.method_defect_problems("E00-S01", text)
        self.assertEqual(len(problems), 1)
        self.assertIn("prevented", problems[0].lower())

    def test_a_declined_entry_stays_in_the_packet(self) -> None:
        """A decline is a decision worth keeping, so a later session does not re-propose it."""
        text = packet("- **What happened**: x **Prevented by**: AGENTS.md **Disposition**: declined 2026-09-15 - too narrow")
        self.assertEqual(evidence.method_defect_problems("E00-S01", text), [])
        self.assertEqual(len(evidence.method_defect_entries(text)), 1)


class TheMechanismProposesAndNeverWrites(unittest.TestCase):
    """The reason the source package was rejected, asserted rather than remembered."""

    def test_no_path_in_the_gate_writes_anywhere(self) -> None:
        source = (ROOT / "scripts" / "check_evidence.py").read_text(encoding="utf-8")
        for forbidden in ("write_text", "open(", "mkdir", "shutil", "subprocess"):
            self.assertNotIn(forbidden, source, f"the evidence gate must not {forbidden}; it reads and reports")

    def test_the_template_carries_the_section(self) -> None:
        template = (ROOT / "project" / "templates" / "EVIDENCE_PACKET.md").read_text(encoding="utf-8")
        self.assertIn("## Method defects", template)
        self.assertIn("Disposition", template)


class AProposalIsAgedNotForgotten(unittest.TestCase):
    """`proposed` is the right resting state and also one an entry can sit in forever (E29-S04)."""

    TODAY = __import__("datetime").date(2026, 9, 15)

    def aged(self, disposition: str):
        text = packet(f"- **What happened**: x **Prevented by**: none exists **Disposition**: {disposition}")
        return evidence.aged_proposals("E00-S01", text, self.TODAY)

    def test_a_proposal_past_the_cadence_is_reported(self) -> None:
        reported = self.aged("proposed 2026-01-01")
        self.assertEqual(len(reported), 1)
        self.assertIn("unexamined, not invalid", reported[0])

    def test_a_recent_proposal_is_not_reported(self) -> None:
        self.assertEqual(self.aged("proposed 2026-09-01"), [])

    def test_an_aged_proposal_is_not_an_error(self) -> None:
        """Ageing must not convert a proposal into a defect."""
        text = packet("- **What happened**: x **Prevented by**: none exists **Disposition**: proposed 2026-01-01")
        self.assertEqual(evidence.method_defect_problems("E00-S01", text), [])

    def test_a_disposed_entry_is_never_aged(self) -> None:
        for disposition in ("accepted 2026-01-01", "declined 2026-01-01 - too narrow"):
            self.assertEqual(self.aged(disposition), [], disposition)

    def test_an_undated_proposal_says_its_age_cannot_be_reported(self) -> None:
        reported = self.aged("proposed")
        self.assertEqual(len(reported), 1)
        self.assertIn("no date", reported[0])

    def test_every_committed_proposal_carries_a_date(self) -> None:
        from pathlib import Path as _P

        for pkt in sorted((_P(__file__).resolve().parent.parent / "project" / "evidence").glob("*/EVIDENCE.md")):
            for warning in evidence.aged_proposals(pkt.parent.name, pkt.read_text(encoding="utf-8"), self.TODAY):
                self.assertNotIn("no date", warning, f"{pkt.parent.name} has an undated proposal")


if __name__ == "__main__":
    unittest.main()

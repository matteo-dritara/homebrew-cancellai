from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts import check_process


class ProcessConventionTests(unittest.TestCase):
    def test_repository_process_conventions_hold(self) -> None:
        check_process.check_process()

    def test_review_rounds_are_bounded(self) -> None:
        errors: list[str] = []
        warnings: list[str] = []
        check_process.check_review_rounds(errors, warnings)
        # E00 and E07 are the recorded exceptions (E22-S06): both exceed the ceiling only
        # once story-scoped review records are counted against their epic, and both predate
        # the counting fix / ADR-0014.
        self.assertEqual(errors, [])
        self.assertTrue(any("E00" in w for w in warnings))
        self.assertTrue(any("E07" in w for w in warnings))
        self.assertEqual(check_process.MAX_REVIEW_ROUNDS, 2)

    def test_an_unexcepted_epic_cannot_exceed_the_review_ceiling(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            for round_number in range(1, check_process.MAX_REVIEW_ROUNDS + 2):
                (base / f"E11-VERIFIER-REVIEW-ROUND{round_number}.md").write_text("x", encoding="utf-8")
            errors: list[str] = []
            warnings: list[str] = []
            with mock.patch.object(check_process, "EVIDENCE", base):
                check_process.check_review_rounds(errors, warnings)
            self.assertTrue(errors)
            self.assertEqual(warnings, [])

    def test_a_story_scoped_review_record_counts_against_its_epics_ceiling(self) -> None:
        # E22-S06 (CR-TE companion): before this story, a filename like
        # `E12-S01-VERIFIER-REVIEW.md` matched no pattern this check looked for at all, so an
        # epic could carry an unbounded number of story-scoped review rounds - exactly the
        # gap that made E07 read as one round here while four were actually run.
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            (base / "E12-VERIFIER-REVIEW.md").write_text("x", encoding="utf-8")
            (base / "E12-VERIFIER-REVIEW-ROUND2.md").write_text("x", encoding="utf-8")
            (base / "E12-S01-VERIFIER-REVIEW.md").write_text("x", encoding="utf-8")
            errors: list[str] = []
            warnings: list[str] = []
            with mock.patch.object(check_process, "EVIDENCE", base):
                check_process.check_review_rounds(errors, warnings)
            self.assertTrue(errors, "a third round, even a story-scoped one, must fail an epic with no exception")
            self.assertEqual(warnings, [])
            # AC3: the failure message names which records were counted.
            self.assertIn("E12-S01-VERIFIER-REVIEW.md", errors[0])
            self.assertIn("E12-VERIFIER-REVIEW.md", errors[0])
            self.assertIn("E12-VERIFIER-REVIEW-ROUND2.md", errors[0])

    def test_reference_freeze_marker_present_in_the_real_repo(self) -> None:
        errors: list[str] = []
        check_process.check_reference_freeze_marker(errors)
        self.assertEqual([], errors)

    def test_reference_freeze_marker_missing_is_flagged(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            (base / "AGENTS.md").write_text("# AGENTS\n\nNothing about a freeze here.\n", encoding="utf-8")
            (base / "docs" / "development").mkdir(parents=True)
            (base / "docs" / "development" / "MIGRATION_PYTHON_RUST.md").write_text("gate\nrollback\n", encoding="utf-8")
            errors: list[str] = []
            with mock.patch.object(check_process, "ROOT", base):
                check_process.check_reference_freeze_marker(errors)
            self.assertTrue(any(check_process.REFERENCE_FREEZE_MARKER in e for e in errors), errors)

    def test_migration_doc_missing_gate_or_rollback_is_flagged(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            (base / "AGENTS.md").write_text(f"# AGENTS\n\n## {check_process.REFERENCE_FREEZE_MARKER}\n", encoding="utf-8")
            (base / "docs" / "development").mkdir(parents=True)
            (base / "docs" / "development" / "MIGRATION_PYTHON_RUST.md").write_text("neither concept mentioned\n", encoding="utf-8")
            errors: list[str] = []
            with mock.patch.object(check_process, "ROOT", base):
                check_process.check_reference_freeze_marker(errors)
            self.assertTrue(any("rollback" in e for e in errors), errors)
            self.assertTrue(any("gate" in e for e in errors), errors)

    def test_conventional_commit_subjects(self) -> None:
        valid = [
            "feat: add a thing",
            "fix(safety): close a barrier",
            "docs: explain the thing",
            "refactor!: rename a public function",
            "Merge branch 'main' into topic",
            'Revert "feat: add a thing"',
        ]
        for subject in valid:
            self.assertEqual(check_process.validate_commit_subject(subject), [], subject)

        invalid = {
            "added a thing": "no type",
            "feat add a thing": "no colon",
            "wip: something": "unknown type",
            "feat: add a thing.": "trailing period",
            "feat: " + "x" * 200: "too long",
        }
        for subject, reason in invalid.items():
            self.assertNotEqual(check_process.validate_commit_subject(subject), [], reason)

    def test_commit_message_requires_a_blank_line_after_the_subject(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "COMMIT_EDITMSG"
            path.write_text("feat: add a thing\nbody glued to subject\n", encoding="utf-8")
            with self.assertRaises(check_process.ProcessError):
                check_process.check_commit_message(path)

            path.write_text("feat: add a thing\n\nproper body\n", encoding="utf-8")
            check_process.check_commit_message(path)

            # Comment lines are what the editor adds; they must not count as the subject.
            path.write_text("# please enter a message\nfeat: add a thing\n", encoding="utf-8")
            check_process.check_commit_message(path)

    def test_empty_commit_message_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "COMMIT_EDITMSG"
            path.write_text("# only comments\n", encoding="utf-8")
            with self.assertRaises(check_process.ProcessError):
                check_process.check_commit_message(path)


if __name__ == "__main__":
    unittest.main(verbosity=2)

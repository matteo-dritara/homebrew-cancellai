from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts import check_process


class ProcessConventionTests(unittest.TestCase):
    def test_repository_process_conventions_hold(self) -> None:
        check_process.check_process()

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

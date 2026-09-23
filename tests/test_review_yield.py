"""Tests for what the review-yield measurement can see (E29-S01).

ADR-0025 makes another review round mandatory while a round's yield is at or above 10%, and this
measurement computes that fraction. E28's first round found four defects across four of five
stories and the table reported nothing at all, for three independent reasons: the records were
story-scoped and skipped with a bare `continue`, their verdicts were not in a table the parser
reads, and only `FAIL` counted - so a reviewer who repairs a defect instead of failing the story
measures zero having found something.

The cases below are those three, each asserted against the behaviour rather than the constant. The
last group is the one that matters most: a measurement whose answer depends on how the reviewer
chose to write, rather than on what the reviewer found, cannot decide whether another round is
needed.
"""

from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

from scripts import process_metrics as pm

ROOT = Path(__file__).resolve().parent.parent


def table(*rows: tuple[str, str]) -> str:
    body = "\n".join(f"| {story} | {verdict} | note |" for story, verdict in rows)
    return "| Story | Verdict | Evidence |\n| --- | --- | --- |\n" + body + "\n"


class FindingIsNotRejecting(unittest.TestCase):
    """The two questions ADR-0025 and the rejection rate ask are different questions."""

    def test_a_repaired_defect_counts_as_a_finding(self) -> None:
        record = pm.Round(number=1, path=Path("x"), independent=True, verdicts={"E00-S01": "REPAIRED"})
        self.assertEqual(record.found, {"E00-S01"})

    def test_a_repaired_defect_is_not_a_rejection(self) -> None:
        """The story was not sent back, so the first-pass rejection rate must not count it."""
        record = pm.Round(number=1, path=Path("x"), independent=True, verdicts={"E00-S01": "REPAIRED"})
        self.assertEqual(record.rejected, set())

    def test_a_failure_is_both(self) -> None:
        record = pm.Round(number=1, path=Path("x"), independent=True, verdicts={"E00-S01": "FAIL"})
        self.assertEqual(record.found, {"E00-S01"})
        self.assertEqual(record.rejected, {"E00-S01"})

    def test_a_clean_pass_is_neither(self) -> None:
        for verdict in ("PASS", "PASS_WITH_RESIDUALS"):
            record = pm.Round(number=1, path=Path("x"), independent=True, verdicts={"E00-S01": verdict})
            self.assertEqual(record.found, set(), verdict)
            self.assertEqual(record.rejected, set(), verdict)

    def test_the_e28_round_one_shape_is_no_longer_invisible(self) -> None:
        """Four of five stories with a repaired defect is 80%, not 0% - the number that decides."""
        verdicts = {f"E28-S0{n}": "REPAIRED" for n in range(1, 5)} | {"E28-S05": "PASS_WITH_RESIDUALS"}
        record = pm.Round(number=1, path=Path("x"), independent=True, verdicts=verdicts)
        self.assertEqual(len(record.found), 4)
        self.assertEqual(round(100 * len(record.found) / len(record.verdicts)), 80)
        self.assertGreaterEqual(100 * len(record.found) / len(record.verdicts), 10, "above ADR-0025's threshold")

    # Records written after E29-S01 introduced REPAIRED, and so free to use it.
    WRITTEN_AFTER_REPAIRED = frozenset({"E16-VERIFIER-REVIEW-ROUND2.md"})

    def test_the_new_verdict_changes_no_historical_number(self) -> None:
        """No record older than REPAIRED uses it, so every figure computed before it is unchanged."""
        for records in pm.load_rounds().values():
            for record in records:
                if record.path.name.startswith("E29-") or record.path.name in self.WRITTEN_AFTER_REPAIRED:
                    continue
                self.assertEqual(record.found, record.rejected, f"{record.path} predates the distinction")


class RecordsThisToolDoesNotCount(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = TemporaryDirectory()
        self.evidence = Path(self._tmp.name) / "project" / "evidence"
        (self.evidence / "E00-S01").mkdir(parents=True)
        patcher = mock.patch.multiple(pm, ROOT=Path(self._tmp.name), EVIDENCE=self.evidence)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.addCleanup(self._tmp.cleanup)

    def write(self, name: str, text: str, *, subdir: str = "") -> None:
        directory = self.evidence / subdir if subdir else self.evidence
        directory.mkdir(parents=True, exist_ok=True)
        (directory / name).write_text(text, encoding="utf-8")

    def test_a_skipped_record_is_named_rather_than_dropped(self) -> None:
        """It used to `continue` in silence, which is the failure this story exists to remove."""
        self.write("E00-S01-VERIFIER-REVIEW.md", table(("E00-S01", "FAIL")), subdir="E00-S01")
        pm.load_rounds()
        self.assertEqual(len(pm.NOT_COUNTED), 1)
        self.assertIn("E00-S01-VERIFIER-REVIEW.md", pm.NOT_COUNTED[0])

    def test_a_skipped_record_still_produces_no_round(self) -> None:
        """Naming it must not silently start counting rounds that never happened."""
        self.write("E00-S01-VERIFIER-REVIEW.md", table(("E00-S01", "FAIL")), subdir="E00-S01")
        self.assertEqual(pm.load_rounds(), {})

    def test_a_record_declaring_epic_scope_is_counted_whatever_its_filename(self) -> None:
        """A round record is identified by what it declares, not by where its file sits."""
        self.write(
            "E00-S01-VERIFIER-REVIEW.md",
            "Review-Scope: epic\nRound: 2\n\n" + table(("E00-S01", "REPAIRED")),
            subdir="E00-S01",
        )
        rounds = pm.load_rounds()
        self.assertEqual(list(rounds), ["E00"])
        self.assertEqual(rounds["E00"][0].number, 2)
        self.assertEqual(rounds["E00"][0].found, {"E00-S01"})
        self.assertEqual(pm.NOT_COUNTED, [])

    def test_a_declaration_inside_a_fenced_example_does_not_count(self) -> None:
        """The same rule the verdict rows already follow: an example is not a declaration."""
        self.write(
            "E00-S01-VERIFIER-REVIEW.md",
            "```\nReview-Scope: epic\n```\n\n" + table(("E00-S01", "FAIL")),
            subdir="E00-S01",
        )
        self.assertEqual(pm.load_rounds(), {})
        self.assertEqual(len(pm.NOT_COUNTED), 1)

    def test_an_unclassifiable_record_is_still_reported_separately(self) -> None:
        """Two different states: one the tool could not read, one it read and chose not to count."""
        self.write("notes-REVIEW.md", "whatever")
        pm.load_rounds()
        self.assertEqual(len(pm.UNCLASSIFIABLE), 1)
        self.assertEqual(pm.NOT_COUNTED, [])


class TheCommittedReportSaysBoth(unittest.TestCase):
    def test_the_report_names_the_records_it_did_not_count(self) -> None:
        report = (ROOT / "project" / "generated" / "PROCESS_METRICS.md").read_text(encoding="utf-8")
        self.assertIn("recognised and did not count", report)

    def test_the_yield_table_separates_found_from_rejected(self) -> None:
        report = (ROOT / "project" / "generated" / "PROCESS_METRICS.md").read_text(encoding="utf-8")
        self.assertIn("| Epic | Round | Reviewer | Stories judged | Found | Rejected | Yield |", report)


if __name__ == "__main__":
    unittest.main()

"""Tests for work-item citations in prose (E31-S01).

`check_docs.py` validated every local link and every `SI-0xx` a document names, and did not read
the reference class these documents lean on hardest. Measuring first found 424 distinct work-item
ids across the non-generated documentation and two that resolve to nothing - both survivors of the
E07/E20 split, both cited deliberately as history.

That is the whole design constraint. A gate that refuses `E20-S04 (formerly E07-S06)` is wrong, and
a gate that cannot tell it apart from a typo is worthless, so a retired identifier is recorded once
with what it became. The check is about existence and says so: the stale claim that motivated its
sibling story - RELEASE_GATES.md naming a dependency that had closed - would pass here, because the
id existed.
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from typing import ClassVar

from scripts import check_docs

ROOT = Path(__file__).resolve().parent.parent


class TheCommittedCorpus(unittest.TestCase):
    def test_every_citation_resolves(self) -> None:
        self.assertEqual(check_docs.work_item_reference_errors(check_docs.markdown_files()), [])

    def test_an_invented_id_is_refused(self) -> None:
        """Shown failing on a real document before the corpus is shown passing."""
        target = ROOT / "docs" / "development" / "ENGINEERING_SYSTEM.md"
        original = target.read_text(encoding="utf-8")
        try:
            target.write_text(original + "\nA planted reference to E99-S99.\n", encoding="utf-8")
            errors = [e for e in check_docs.work_item_reference_errors(check_docs.markdown_files()) if "E99-S99" in e]
            self.assertEqual(len(errors), 1)
            self.assertIn("ENGINEERING_SYSTEM.md", errors[0])
        finally:
            target.write_text(original, encoding="utf-8")

    def test_a_cancelled_work_item_may_still_be_cited(self) -> None:
        """Seven documents explain why E07-S07 was cancelled; cancelling it does not unwrite them."""
        cancelled = "E07-S07"
        self.assertIn(cancelled, check_docs.work_item_ids())
        citing = [p for p in check_docs.markdown_files() if cancelled in p.read_text(encoding="utf-8")]
        self.assertGreaterEqual(len(citing), 2, "this case needs real citations to be meaningful")
        self.assertEqual(check_docs.work_item_reference_errors(citing), [])


class RetiredIdentifiers(unittest.TestCase):
    RETIRED: ClassVar[dict] = json.loads((ROOT / "project" / "retired_work_items.json").read_text(encoding="utf-8"))

    def test_a_retired_id_passes_wherever_it_is_cited(self) -> None:
        known = check_docs.work_item_ids()
        for entry in self.RETIRED["retired"]:
            self.assertNotIn(entry["id"], known, f"{entry['id']} still exists; it is not retired")

    def test_every_record_names_what_it_became_and_when(self) -> None:
        for entry in self.RETIRED["retired"]:
            self.assertTrue(entry.get("became"), f"{entry['id']} records no successor")
            self.assertTrue(entry.get("reason", "").strip(), f"{entry['id']} records no reason")
            self.assertRegex(entry.get("recorded", ""), r"^\d{4}-\d{2}-\d{2}$")

    def test_a_successor_that_does_not_resolve_is_refused(self) -> None:
        """A retirement record that rots is the defect the file exists to prevent."""
        path = ROOT / "project" / "retired_work_items.json"
        original = path.read_text(encoding="utf-8")
        try:
            broken = json.loads(original)
            broken["retired"][0]["became"] = "E77-S77"
            path.write_text(json.dumps(broken, indent=2) + "\n", encoding="utf-8")
            errors = [e for e in check_docs.work_item_reference_errors(check_docs.markdown_files()) if "rots" in e]
            self.assertEqual(len(errors), 1)
        finally:
            path.write_text(original, encoding="utf-8")

    def test_a_retirement_without_a_successor_or_date_is_refused(self) -> None:
        path = ROOT / "project" / "retired_work_items.json"
        original = path.read_text(encoding="utf-8")
        try:
            broken = json.loads(original)
            broken["retired"][0].pop("became")
            broken["retired"][0].pop("recorded")
            path.write_text(json.dumps(broken, indent=2) + "\n", encoding="utf-8")
            errors = check_docs.work_item_reference_errors(check_docs.markdown_files())
            self.assertTrue(any("records no successor" in error for error in errors))
            self.assertTrue(any("records no valid date" in error for error in errors))
        finally:
            path.write_text(original, encoding="utf-8")

    def test_a_retirement_cannot_be_recorded_twice(self) -> None:
        path = ROOT / "project" / "retired_work_items.json"
        original = path.read_text(encoding="utf-8")
        try:
            broken = json.loads(original)
            broken["retired"].append(broken["retired"][0].copy())
            path.write_text(json.dumps(broken, indent=2) + "\n", encoding="utf-8")
            errors = check_docs.work_item_reference_errors(check_docs.markdown_files())
            self.assertTrue(any("recorded more than once" in error for error in errors))
        finally:
            path.write_text(original, encoding="utf-8")

    def test_each_successor_resolves_today(self) -> None:
        known = check_docs.work_item_ids()
        for entry in self.RETIRED["retired"]:
            self.assertIn(entry["became"], known, f"{entry['id']} points at a successor that does not exist")


class WhatIsDeliberatelyNotChecked(unittest.TestCase):
    def test_generated_documents_are_excluded(self) -> None:
        """They are produced from the control plane, so checking them checks the generator twice."""
        self.assertIn("docs/BACKLOG.md", check_docs.GENERATED_DOCS)
        self.assertIn("docs/ROADMAP.md", check_docs.GENERATED_DOCS)

    def test_existence_is_checked_and_currency_is_not(self) -> None:
        """The defect that motivated E30-S01 would pass here, because the id existed.

        RELEASE_GATES.md named E16-S05 as an outstanding dependency long after it closed. This gate
        would not have objected, and stating that plainly is better than implying otherwise.
        """
        self.assertIn("E16-S05", check_docs.work_item_ids())


if __name__ == "__main__":
    unittest.main()

"""Tests for the coverage ratchet (E27-S02).

The ratchet's whole value is that it can fail. A coverage gate that reports a number and accepts
every number is a dashboard, so these concentrate on the directions it must refuse and on the
noise it must tolerate without either becoming a way through.
"""

from __future__ import annotations

import json
import unittest
from typing import Any, ClassVar

from scripts import check_coverage as coverage


class RatchetTests(unittest.TestCase):
    BASE: ClassVar[dict[str, Any]] = {"crates": dict.fromkeys(coverage.RATCHETED, 90.0)}

    def measured(self, **overrides: float) -> dict[str, float]:
        values = dict.fromkeys(coverage.RATCHETED, 90.0)
        values.update(overrides)
        return values

    def test_holding_the_line_passes(self):
        errors, _ = coverage.evaluate(self.measured(), self.BASE)
        self.assertEqual([], errors)

    def test_a_real_fall_is_refused_and_names_both_numbers(self):
        errors, _ = coverage.evaluate(self.measured(**{"cancellai-sealedfs": 80.0}), self.BASE)
        self.assertEqual(1, len(errors))
        self.assertIn("80.00%", errors[0])
        self.assertIn("90.00%", errors[0])

    def test_noise_below_the_tolerance_is_not_a_regression(self):
        # Region counts shift when a function is split or a match arm is reordered, with no test
        # lost. Failing on that teaches people to re-record reflexively, which is how a ratchet
        # becomes a rubber stamp.
        errors, _ = coverage.evaluate(self.measured(**{"cancellai-safety": 89.6}), self.BASE)
        self.assertEqual([], errors)

    def test_the_tolerance_is_not_a_way_through(self):
        errors, _ = coverage.evaluate(self.measured(**{"cancellai-safety": 89.4}), self.BASE)
        self.assertTrue(errors)

    def test_a_ratcheted_crate_with_no_floor_is_refused(self):
        errors, _ = coverage.evaluate(self.measured(), {"crates": {}})
        self.assertEqual(len(coverage.RATCHETED), len(errors))
        self.assertTrue(all("no recorded floor" in error for error in errors))

    def test_a_ratcheted_crate_that_stopped_building_is_refused(self):
        measured = self.measured()
        del measured["cancellai-sealedfs"]
        errors, _ = coverage.evaluate(measured, self.BASE)
        self.assertTrue(any("absent from the coverage report" in error for error in errors))

    def test_improvement_is_reported_rather_than_recorded_silently(self):
        # Re-recording is an explicit, reviewable diff. A ratchet that tightens itself is a number
        # nobody chose; a ratchet that loosens itself is worse.
        errors, notes = coverage.evaluate(self.measured(**{"cancellai-model": 99.0}), self.BASE)
        self.assertEqual([], errors)
        self.assertTrue(any("above its recorded" in note for note in notes))

    def test_an_unratcheted_crate_is_reported_and_never_fails(self):
        measured = self.measured()
        measured["cancellai-tui"] = 1.0
        errors, notes = coverage.evaluate(measured, self.BASE)
        self.assertEqual([], errors)
        self.assertTrue(any("cancellai-tui" in note and "not ratcheted" in note for note in notes))


class BaselineTests(unittest.TestCase):
    def test_the_committed_baseline_covers_every_ratcheted_crate(self):
        recorded = coverage.load_baseline()["crates"]
        for crate in coverage.RATCHETED:
            with self.subTest(crate=crate):
                self.assertIn(crate, recorded)
                self.assertGreater(float(recorded[crate]), 0.0)

    def test_the_baseline_is_valid_json_with_a_schema_version(self):
        data = json.loads(coverage.BASELINE.read_text(encoding="utf-8"))
        self.assertEqual(1, data["schema_version"])

    def test_the_kernel_ring_is_ratcheted(self):
        # The ring ADR-0019 defines, plus the two crates that decide eligibility and completeness.
        for crate in ("cancellai-model", "cancellai-safety", "cancellai-platform", "cancellai-sealedfs"):
            with self.subTest(crate=crate):
                self.assertIn(crate, coverage.RATCHETED)

    def test_a_declared_skeleton_is_not_ratcheted(self):
        # `cancellai-guardian` is 16 lines that print "not yet implemented" (E02-S01). Its 0% is
        # correct, and a gate that treated it as a crisis would be measuring the wrong thing.
        self.assertNotIn("cancellai-guardian", coverage.RATCHETED)

    def test_crate_of_reads_a_workspace_path(self):
        self.assertEqual("cancellai-safety", coverage.crate_of("/x/rust/crates/cancellai-safety/src/lib.rs"))
        self.assertIsNone(coverage.crate_of("/x/rust/build.rs"))


if __name__ == "__main__":
    unittest.main()

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
        # Version 2 added `provenance`: a baseline that does not say what measured it cannot be
        # compared against anything (E27-S07).
        self.assertEqual(2, data["schema_version"])

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


class ProvenanceTests(unittest.TestCase):
    """A measurement taken with different tools is not a comparison (E27-S07).

    The same unchanged workspace measures `cancellai-platform` at 95.83% on stable and 63.85% on
    nightly. Region counting depends on the compiler that produced the instrumentation, so a
    baseline without provenance compares one number against a different number and reports the
    difference as a coverage regression - a gate describing the machine it ran on, which is the
    defect class this repository fixed in the risk gate for shallow clones.
    """

    RECORDED: ClassVar[dict[str, str]] = {
        "toolchain": "stable",
        "rustc": "rustc 1.94.0 (4a4ef493e 2026-03-02)",
        "cargo_llvm_cov": "cargo-llvm-cov 0.9.0",
    }

    def test_matching_provenance_is_accepted(self):
        self.assertEqual([], coverage.provenance_errors(dict(self.RECORDED), self.RECORDED))

    def test_a_different_compiler_is_refused_rather_than_read_as_a_fall(self):
        now = dict(self.RECORDED, rustc="rustc 1.100.0-nightly (4b6d04e70 2026-09-13)")
        errors = coverage.provenance_errors(now, self.RECORDED)
        self.assertEqual(1, len(errors))
        self.assertIn("not comparable", errors[0])
        self.assertIn("nightly", errors[0])

    def test_a_different_coverage_tool_is_refused(self):
        now = dict(self.RECORDED, cargo_llvm_cov="cargo-llvm-cov 0.6.21")
        self.assertTrue(coverage.provenance_errors(now, self.RECORDED))

    def test_a_baseline_with_no_provenance_is_refused(self):
        errors = coverage.provenance_errors(dict(self.RECORDED), {})
        self.assertTrue(any("records no measurement provenance" in error for error in errors))

    def test_the_committed_baseline_carries_provenance(self):
        recorded = coverage.load_baseline().get("provenance", {})
        for key in ("toolchain", "rustc", "cargo_llvm_cov"):
            with self.subTest(key=key):
                self.assertIn(key, recorded)
                self.assertNotEqual("unknown", recorded[key], f"{key} was not probed successfully")

    def test_the_measurement_names_its_toolchain_rather_than_taking_the_default(self):
        # `cargo llvm-cov` with whatever rustup happens to default to is how the same checkout
        # produced three different numbers for one unchanged crate.
        self.assertEqual("stable", coverage.MEASUREMENT_TOOLCHAIN)

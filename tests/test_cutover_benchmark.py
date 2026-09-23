"""Tests for the cutover performance self-budget (E06-S08).

The benchmark's own numbers depend on the machine; what can be pinned here is the judgement it
applies to them, and its refusal to time a corpus the engines do not see - a fast run over
nothing is how a performance gate stays green while measuring nothing.
"""

from __future__ import annotations

import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import cutover_benchmark as bench  # noqa: E402

MIB = 1024 * 1024


def measurement(rust: float, reference: float, rss: int | None = 10 * MIB) -> bench.Measurement:
    return bench.Measurement("status", rust, reference, rss, 30 * MIB)


class BudgetViolationTests(unittest.TestCase):
    def test_rust_no_slower_than_the_reference_passes(self) -> None:
        self.assertEqual(bench.budget_violations([measurement(0.2, 0.4)]), [])

    def test_rust_within_the_noise_tolerance_passes(self) -> None:
        allowed = 0.4 * bench.TOLERANCE_RATIO + bench.TOLERANCE_SECONDS
        self.assertEqual(bench.budget_violations([measurement(allowed, 0.4)]), [])

    def test_rust_slower_than_the_tolerance_fails(self) -> None:
        allowed = 0.4 * bench.TOLERANCE_RATIO + bench.TOLERANCE_SECONDS
        violations = bench.budget_violations([measurement(allowed + 0.001, 0.4)])
        self.assertEqual(len(violations), 1)
        self.assertIn("status", violations[0])

    def test_rss_over_the_ceiling_fails_even_when_fast(self) -> None:
        violations = bench.budget_violations([measurement(0.1, 0.4, bench.RSS_CEILING_BYTES + 1)])
        self.assertEqual(len(violations), 1)
        self.assertIn("RSS", violations[0])

    def test_rss_at_the_ceiling_passes(self) -> None:
        self.assertEqual(bench.budget_violations([measurement(0.1, 0.4, bench.RSS_CEILING_BYTES)]), [])

    def test_unmeasured_rss_is_not_a_violation_and_not_a_pass_of_the_ceiling(self) -> None:
        # Windows has no wait4: RSS is reported as unmeasured. The gate only runs on Linux, so
        # this is a report-only case - it must not invent a number either way.
        self.assertEqual(bench.budget_violations([measurement(0.1, 0.4, None)]), [])

    def test_nothing_measured_is_a_violation(self) -> None:
        self.assertEqual(bench.budget_violations([]), ["no command was measured"])


@unittest.skipIf(os.name == "nt", "the fake engine is a POSIX shell script")
class CorpusLivenessTests(unittest.TestCase):
    def test_a_rust_engine_that_sees_nothing_is_refused_before_anything_is_timed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            fake = Path(tmp) / "empty-plan"
            fake.write_text("#!/bin/sh\necho '{\"actions\": []}'\n", encoding="utf-8")
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            results, errors = bench.measure(fake, sessions=5, runs=1)
        self.assertEqual(results, [])
        self.assertEqual(len(errors), 1)
        self.assertIn("Rust engine proposed 0 deletions", errors[0])

    def test_the_reference_sees_exactly_the_corpus_it_was_built_for(self) -> None:
        sessions = 5
        expected = 2 * (sessions - bench.KEEP_LATEST)
        actions = ", ".join(['{"action_class": "delete"}'] * expected)
        with tempfile.TemporaryDirectory() as tmp:
            fake = Path(tmp) / "exact-plan"
            fake.write_text(f"#!/bin/sh\necho '{{\"actions\": [{actions}]}}'\n", encoding="utf-8")
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            home = Path(tmp) / "home"
            bench.build_corpus(home, sessions)
            # The fake Rust side is exact, so any error here is the reference's count.
            self.assertEqual(bench.prove_corpus_is_live(fake, bench.child_env(home), expected), [])
            # And a count the corpus was not built for is refused on both sides.
            errors = bench.prove_corpus_is_live(fake, bench.child_env(home), expected + 1)
        self.assertEqual(len(errors), 2)


if __name__ == "__main__":
    unittest.main()

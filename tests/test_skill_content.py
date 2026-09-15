"""Tests for the skill-content gate (E28-S01).

The gate's whole value is in its failure path, and the failure path is where a naive version of it
would have been wrong. Three of the cases below are direct consequences of measuring SkillSpector
rather than reading its README: on a skill planted with SSH-key exfiltration the process exits 0
and the report's top-level `risk_recommendation` reads `SAFE`, and `finding_id` is regenerated on
every run while `match_fingerprint` is not. A gate built on any of those three would pass a skill
that steals keys, so each has a test asserting the gate does not read it.
"""

from __future__ import annotations

import unittest
from typing import Any, ClassVar

from scripts import check_skill_content as gate


def issue(**overrides: Any) -> dict[str, Any]:
    base = {
        "match_fingerprint": "fp-benign",
        "finding_id": "finding-regenerated-every-run",
        "severity": "LOW",
        "category": "Rogue Agent",
        "pattern": "Session Persistence",
        "location": {"file": "SKILL.md", "start_line": 12},
        "explanation": "something",
    }
    base.update(overrides)
    return base


def report(*issues: dict[str, Any], **top: Any) -> dict[str, Any]:
    base: dict[str, Any] = {
        "skills": [{"name": "example", "issues": list(issues)}],
        "analysis_completeness": {"status": "complete"},
    }
    base.update(top)
    return base


def waivers(*entries: dict[str, Any]) -> dict[str, Any]:
    return {"schema_version": 1, "scanner": gate.PINNED_SCANNER, "waivers": list(entries)}


class SeverityLadder(unittest.TestCase):
    def test_refusal_severity_and_above_refuse(self) -> None:
        for severity in ("HIGH", "CRITICAL"):
            found = gate.findings(report(issue(severity=severity)))
            refusals, _, _ = gate.evaluate(found, waivers())
            self.assertEqual(len(refusals), 1, f"{severity} must refuse")

    def test_below_refusal_severity_is_reported_not_refused(self) -> None:
        for severity in ("INFO", "LOW", "MEDIUM"):
            found = gate.findings(report(issue(severity=severity)))
            refusals, _, _ = gate.evaluate(found, waivers())
            self.assertEqual(refusals, [], f"{severity} must not refuse")

    def test_an_unknown_severity_is_treated_as_the_worst(self) -> None:
        """A scanner that grows a new level must not introduce it silently below the line."""
        found = gate.findings(report(issue(severity="CATASTROPHIC")))
        refusals, _, _ = gate.evaluate(found, waivers())
        self.assertEqual(len(refusals), 1)


class WhatTheGateRefusesToRead(unittest.TestCase):
    """The three fields that lie, each asserted not to change the verdict."""

    def test_a_safe_top_level_recommendation_does_not_rescue_a_high_finding(self) -> None:
        # Measured on 2026-09-15: a skill exfiltrating ~/.ssh/id_rsa reported risk_recommendation
        # SAFE while the repository's own clean pack reported CAUTION.
        found = gate.findings(report(issue(severity="HIGH"), risk_recommendation="SAFE", max_risk_score=0))
        refusals, _, _ = gate.evaluate(found, waivers())
        self.assertEqual(len(refusals), 1)

    def test_a_cautious_top_level_recommendation_does_not_condemn_a_clean_pack(self) -> None:
        found = gate.findings(report(issue(severity="LOW"), risk_recommendation="CAUTION", max_risk_score=99))
        refusals, _, _ = gate.evaluate(found, waivers())
        self.assertEqual(refusals, [])

    def test_waivers_key_on_fingerprint_not_on_the_per_run_finding_id(self) -> None:
        """`finding_id` differs between two scans of an unchanged pack; a waiver on it would rot."""
        entry = {"fingerprint": "finding-regenerated-every-run", "reason": "keyed on the wrong field", "date": "2026-09-15"}
        found = gate.findings(report(issue(severity="HIGH")))
        refusals, waived, _ = gate.evaluate(found, waivers(entry))
        self.assertEqual(len(refusals), 1, "a waiver keyed on finding_id must not suppress anything")
        self.assertEqual(waived, [])


class Waivers(unittest.TestCase):
    ENTRY: ClassVar[dict[str, Any]] = {"fingerprint": "fp-waived", "reason": "argued", "date": "2026-09-15"}

    def test_a_waiver_suppresses_exactly_the_finding_it_names(self) -> None:
        found = gate.findings(
            report(
                issue(severity="HIGH", match_fingerprint="fp-waived"),
                issue(severity="HIGH", match_fingerprint="fp-other"),
            )
        )
        refusals, waived, _ = gate.evaluate(found, waivers(self.ENTRY))
        self.assertEqual([f["fingerprint"] for f in refusals], ["fp-other"])
        self.assertEqual([f["fingerprint"] for f in waived], ["fp-waived"])

    def test_a_waiver_that_matches_nothing_is_reported_as_stale(self) -> None:
        """Either the finding was fixed and the waiver should go, or the fingerprint drifted and
        the finding is back - both need saying, and neither is silence."""
        found = gate.findings(report(issue(severity="LOW", match_fingerprint="fp-present")))
        _, _, stale = gate.evaluate(found, waivers(self.ENTRY))
        self.assertEqual(len(stale), 1)
        self.assertIn("fp-waived", stale[0])

    def test_the_committed_waivers_all_carry_a_reason_and_a_date(self) -> None:
        for entry in gate.load_waivers()["waivers"]:
            self.assertTrue(entry.get("reason", "").strip(), f"{entry.get('fingerprint')} has no reason")
            self.assertTrue(entry.get("date", "").strip(), f"{entry.get('fingerprint')} has no date")
            self.assertIn("fingerprint", entry)


class ScannerProvenance(unittest.TestCase):
    def test_an_absent_scanner_refuses_rather_than_reporting_a_clean_pack(self) -> None:
        errors = gate.provenance_errors(None)
        self.assertEqual(len(errors), 1)
        self.assertIn("not installed", errors[0])

    def test_a_different_scanner_version_refuses_before_scanning(self) -> None:
        errors = gate.provenance_errors("9.9.9")
        self.assertEqual(len(errors), 1)
        self.assertIn(gate.PINNED_SCANNER, errors[0])

    def test_the_pinned_version_passes(self) -> None:
        self.assertEqual(gate.provenance_errors(gate.PINNED_SCANNER), [])

    def test_the_scan_never_enables_the_provider_backed_analysers(self) -> None:
        """--no-llm is the reason no file content leaves the machine; it is not optional here."""
        import inspect

        source = inspect.getsource(gate.scan)
        self.assertIn('"--no-llm"', source)


class Reporting(unittest.TestCase):
    def test_incomplete_inspection_is_stated_rather_than_implied(self) -> None:
        lines = gate.describe(
            {
                "skills_scanned": 8,
                "skills_omitted": 0,
                "analysis_completeness": {
                    "status": "partial",
                    "fully_inspected_files": 0,
                    "partially_inspected_files": 8,
                    "entirely_uninspected_files": 0,
                },
            }
        )
        joined = "\n".join(lines)
        self.assertIn("partial", joined)
        self.assertIn("semantic analysis: not run", joined)


if __name__ == "__main__":
    unittest.main()

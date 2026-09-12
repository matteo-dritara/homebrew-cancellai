"""Tests for the four checkers E25-S06..S11 and E26-S02/S03 added (gate sensitivity, safety
oracle, EARS classification, toolchain updates and usage).

Each of these makes a claim about the *rest* of the system, so the cases below concentrate on the
ways such a claim can be confidently wrong: a mutant that silently stops mutating, a predicate that
reuses the code it is supposed to check independently, a classifier whose enforcement never binds,
and a usage record that reads absence as idleness.
"""

from __future__ import annotations

import datetime as dt
import json
import tempfile
import unittest
from pathlib import Path

from scripts import check_agent_toolchain as toolchain
from scripts import check_ears as ears
from scripts import gate_sensitivity as sensitivity
from scripts import safety_oracle as oracle


class MutantIntegrityTests(unittest.TestCase):
    """A mutant whose anchor has moved is testing nothing, and must say so."""

    def test_every_mutant_targets_a_file_that_exists(self):
        for mutant in sensitivity.MUTANTS:
            with self.subTest(mutant=mutant.identifier):
                self.assertTrue((sensitivity.ROOT / mutant.path).is_file(), mutant.path)

    def test_every_mutant_anchor_is_still_present(self):
        # The check that keeps this harness honest as the code moves underneath it.
        for mutant in sensitivity.MUTANTS:
            with self.subTest(mutant=mutant.identifier):
                text = (sensitivity.ROOT / mutant.path).read_text(encoding="utf-8")
                self.assertIn(mutant.find, text)

    def test_every_mutant_actually_changes_the_file(self):
        for mutant in sensitivity.MUTANTS:
            with self.subTest(mutant=mutant.identifier):
                self.assertNotEqual(mutant.find, mutant.replace)

    def test_every_named_gate_exists(self):
        for mutant in sensitivity.MUTANTS:
            for gate in mutant.gates:
                with self.subTest(mutant=mutant.identifier, gate=gate):
                    self.assertIn(gate, sensitivity.GATES)

    def test_a_missing_anchor_raises_rather_than_reporting_no_gate_caught_it(self):
        # Reporting a stale mutant as "no gate caught it" would be wrong in the dangerous
        # direction; reporting it as a pass would be worse.
        with tempfile.TemporaryDirectory() as tmp:
            tree = Path(tmp)
            (tree / "x.md").write_text("nothing to find here\n", encoding="utf-8")
            stale = sensitivity.Mutant("stale", "SI-000", "claim", "x.md", "ABSENT", "y", ("docs",))
            with self.assertRaises(sensitivity.SensitivityError):
                sensitivity.apply_mutant(tree, stale)

    def test_the_committed_report_is_current(self):
        report = (sensitivity.OUTPUT).read_text(encoding="utf-8")
        self.assertIn("Gate Sensitivity", report)
        self.assertIn("Do not edit by hand", report)
        for mutant in sensitivity.MUTANTS:
            with self.subTest(mutant=mutant.identifier):
                self.assertIn(mutant.identifier, report)


class SafetyOracleTests(unittest.TestCase):
    """The predicates must be independent of the implementation, and must be able to fail."""

    def test_a_protected_name_is_protected_in_every_spelling(self):
        protected = {"settings.json", "auth.json"}
        for spelling in ("settings.json", "SETTINGS.JSON", "Settings.Json", "sEtTiNgS.jSoN"):
            with self.subTest(spelling=spelling):
                self.assertTrue(oracle.protected_by_predicate(spelling, protected))

    def test_a_name_that_merely_resembles_one_is_not_protected(self):
        protected = {"settings.json"}
        for spelling in ("settings.json.bak", "xsettings.json", "settings_json", "setting.json"):
            with self.subTest(spelling=spelling):
                self.assertFalse(oracle.protected_by_predicate(spelling, protected))

    def test_only_the_default_root_permits_mutation(self):
        self.assertTrue(oracle.root_permits_mutation_by_predicate("~/.claude", "~/.claude"))
        self.assertTrue(oracle.root_permits_mutation_by_predicate("~/.claude/", "~/.claude"))
        for other in ("~/.claude2", "~/.Claude-backup", "~/.claude/sub", "/tmp/.claude", "~"):
            with self.subTest(root=other):
                self.assertFalse(oracle.root_permits_mutation_by_predicate(other, "~/.claude"))

    def test_keep_latest_protects_regardless_of_age(self):
        # The README's promise, stated as a property rather than an example.
        for age in (0, 8, 100, 10_000):
            with self.subTest(age=age):
                self.assertFalse(oracle.eligible_by_predicate(age, rank_newest_first=0, days=7, keep_latest=2))
                self.assertFalse(oracle.eligible_by_predicate(age, rank_newest_first=1, days=7, keep_latest=2))

    def test_the_age_cutoff_is_never_bypassed(self):
        for age in range(0, 8):
            with self.subTest(age=age):
                self.assertFalse(oracle.eligible_by_predicate(age, rank_newest_first=9, days=7, keep_latest=2))
        self.assertTrue(oracle.eligible_by_predicate(8, rank_newest_first=9, days=7, keep_latest=2))

    def test_the_predicate_can_fail(self):
        # A predicate that cannot fail proves nothing. This is the shape of the failure it detects.
        self.assertNotEqual(
            oracle.eligible_by_predicate(100, 0, 7, 2),
            oracle.eligible_by_predicate(100, 5, 7, 2),
        )

    def test_the_real_reference_satisfies_every_predicate(self):
        self.assertEqual([], oracle.run())

    def test_every_predicate_quotes_the_text_it_encodes(self):
        for predicate in oracle.PREDICATES:
            with self.subTest(predicate=predicate.identifier):
                self.assertGreater(len(predicate.quotes), 80)
                self.assertTrue(predicate.invariant)


class EarsTests(unittest.TestCase):
    def test_the_five_patterns_are_recognised(self):
        cases = {
            "The system shall report the total size.": "ubiquitous",
            "When a provider process is running, the system shall refuse.": "event-driven",
            "While a scan is incomplete, the system shall withhold deletion.": "state-driven",
            "If a path cannot be read, then the system shall report a lower bound.": "unwanted",
            "Where the TUI is enabled, the system shall render the atlas.": "optional",
        }
        for text, expected in cases.items():
            with self.subTest(text=text):
                self.assertEqual(expected, ears.classify(text))

    def test_unwanted_behaviour_wins_over_a_bare_when(self):
        # "When X fails" is an unwanted-behaviour requirement wearing an event keyword.
        self.assertEqual("unwanted", ears.classify("When the identity probe fails, the system shall refuse."))

    def test_a_cr4_story_with_no_unwanted_criterion_is_an_error(self):
        stories = {
            "E99-S01": {
                "id": "E99-S01",
                "epic": "E99",
                "status": "done",
                "change_risk": "CR4",
                "acceptance_criteria": ["The system shall delete the artifact."],
            }
        }
        errors, _, _ = ears.evaluate(stories, {})
        self.assertTrue(any("no acceptance criterion describes unwanted behaviour" in e for e in errors))

    def test_the_same_story_at_cr1_is_not(self):
        stories = {
            "E99-S01": {
                "id": "E99-S01",
                "epic": "E99",
                "status": "done",
                "change_risk": "CR1",
                "acceptance_criteria": ["The system shall render a table."],
            }
        }
        self.assertEqual([], ears.evaluate(stories, {})[0])

    def test_a_baselined_story_warns_rather_than_failing(self):
        stories = {
            "E99-S01": {
                "id": "E99-S01",
                "epic": "E99",
                "status": "done",
                "change_risk": "CR4",
                "acceptance_criteria": ["The system shall delete the artifact."],
            }
        }
        errors, warnings, _ = ears.evaluate(stories, {"E99-S01": "predates the rule"})
        self.assertEqual([], errors)
        self.assertTrue(warnings)

    def test_a_planned_story_is_judged_too(self):
        # The rule binds at authoring time, which is the only moment a contract can still be
        # written differently. Skipping `planned` meant the gate never bound where it mattered -
        # an independent review pointed that out.
        stories = {
            "E99-S01": {
                "id": "E99-S01",
                "epic": "E99",
                "status": "planned",
                "change_risk": "CR4",
                "acceptance_criteria": ["The system shall delete the artifact."],
            }
        }
        errors, _, _ = ears.evaluate(stories, {})
        self.assertTrue(errors)

    def test_a_cancelled_story_is_not_judged(self):
        stories = {
            "E99-S01": {
                "id": "E99-S01",
                "epic": "E99",
                "status": "cancelled",
                "change_risk": "CR4",
                "acceptance_criteria": ["The system shall delete the artifact."],
            }
        }
        self.assertEqual([], ears.evaluate(stories, {})[0])

    def test_an_incidental_if_does_not_satisfy_the_rule(self):
        # "the user is notified if verbose" is not a hazard requirement.
        self.assertEqual("ubiquitous", ears.classify("The plan is applied and the user is notified if verbose"))
        self.assertEqual("unwanted", ears.classify("If the root is not the default, then the system shall refuse."))

    def test_every_baseline_entry_names_a_real_story(self):
        stories = ears.load_stories()
        for story_id in ears.load_baseline():
            with self.subTest(story=story_id):
                self.assertIn(story_id, stories)

    def test_the_repository_passes(self):
        self.assertEqual(0, ears.main(["check"]))


class ToolchainUsageTests(unittest.TestCase):
    """Absence of a usage record must read as unknown, never as unused."""

    def component(self, decided="2026-09-01"):
        return {"id": "thing", "decision": {"date": decided}}

    def test_no_record_is_unknown(self):
        self.assertEqual("unknown", toolchain.usage_since_decision(self.component(), {"components": {}}))

    def test_a_record_older_than_the_decision_is_zero_since_decision(self):
        usage = {"components": {"thing": {"invocations": 5, "last": "2026-08-01"}}}
        self.assertEqual("0 since decision", toolchain.usage_since_decision(self.component(), usage))

    def test_a_record_after_the_decision_reports_its_count(self):
        usage = {"components": {"thing": {"invocations": 3, "last": "2026-09-05"}}}
        self.assertEqual("3", toolchain.usage_since_decision(self.component(), usage))

    def test_recording_writes_identity_and_a_count_and_nothing_else(self):
        original = toolchain.USAGE
        managed = toolchain.load_manifest()["components"][0]["id"]
        with tempfile.TemporaryDirectory() as tmp:
            toolchain.USAGE = Path(tmp) / "usage.json"
            try:
                self.assertEqual(0, toolchain.record_usage(managed, dt.date(2026, 9, 12)))
                data = json.loads(toolchain.USAGE.read_text(encoding="utf-8"))
            finally:
                toolchain.USAGE = original
        entry = data["components"][managed]
        self.assertEqual({"invocations", "last"}, set(entry))
        self.assertEqual(1, entry["invocations"])

    def test_recording_an_unmanaged_component_is_refused(self):
        # A usage record for something nobody decided to carry would make the retirement report
        # describe a component the manifest has never heard of.
        original = toolchain.USAGE
        with tempfile.TemporaryDirectory() as tmp:
            toolchain.USAGE = Path(tmp) / "usage.json"
            try:
                self.assertEqual(2, toolchain.record_usage("not-a-component", dt.date(2026, 9, 12)))
                self.assertFalse(toolchain.USAGE.exists())
            finally:
                toolchain.USAGE = original

    def test_a_malformed_last_date_reads_as_unknown_not_as_used(self):
        usage = {"components": {"thing": {"invocations": 9, "last": "yesterday"}}}
        self.assertEqual("unknown", toolchain.usage_since_decision(self.component(), usage))

    def test_an_upstream_is_parsed_only_from_a_github_source(self):
        self.assertEqual(("trailofbits", "skills"), toolchain.upstream("github:trailofbits/skills"))
        self.assertIsNone(toolchain.upstream(".claude/skills"))
        self.assertIsNone(toolchain.upstream("marketplace:claude-plugins-official"))
        self.assertIsNone(toolchain.upstream("github:malformed"))


if __name__ == "__main__":
    unittest.main()

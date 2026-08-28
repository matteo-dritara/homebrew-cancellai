"""Tests for the differential comparison harness (E01-S05).

scripts/diff_harness.py's own selftest() is the "harness self-test catches intentionally
injected divergence" verification the story names; this file exposes it to pytest/CI and
adds a couple of direct unit checks of the underlying comparator.
"""

from __future__ import annotations

import copy
import unittest

from scripts import diff_harness


class DiffHarnessSelfTestTests(unittest.TestCase):
    def test_selftest_passes(self):
        self.assertEqual([], diff_harness.selftest())


class DiffHarnessUnitTests(unittest.TestCase):
    def test_compare_documents_of_different_types_is_always_a_divergence(self):
        plan = diff_harness._load_golden("plan.golden.json")
        result = diff_harness._load_golden("result.golden.json")
        errors = diff_harness.compare_documents(plan, result)
        self.assertTrue(errors)

    def test_top_level_set_field_ignores_order(self):
        plan_a = diff_harness._load_golden("plan.golden.json")
        plan_b = copy.deepcopy(plan_a)
        plan_b["safety_invariant_refs"] = list(reversed(plan_a["safety_invariant_refs"]))
        self.assertEqual([], diff_harness.compare_documents(plan_a, plan_b))

    def test_a_changed_set_field_member_is_caught(self):
        plan_a = diff_harness._load_golden("plan.golden.json")
        plan_b = copy.deepcopy(plan_a)
        plan_b["safety_invariant_refs"] = ["SI-013"]  # dropped SI-016
        errors = diff_harness.compare_documents(plan_a, plan_b)
        self.assertTrue(any("safety_invariant_refs" in e for e in errors), errors)

    def test_artifact_index_resolves_cross_engine_target_artifact_ids(self):
        # Build a minimal plan doc referencing an artifact by a *different* opaque id on
        # each side, and prove that supplying the identity_token index makes the actions
        # match anyway - the whole point of E01-S05 adding identity_token to the inventory
        # contract.
        base = {
            "schema_version": 1,
            "document_type": "plan",
            "generated_at": "2026-01-01T00:00:00Z",
            "generator": {"name": "x", "version": "1"},
            "plan_id": "p",
            "inventory_snapshot_id": "inv",
            "provider_roots": [],
            "actions": [
                {
                    "action_id": "a1",
                    "target_artifact_ids": ["PLACEHOLDER"],
                    "action_class": "QUARANTINE",
                    "reason": "old",
                    "authority": "QUARANTINE",
                    "reversibility": "QUARANTINABLE",
                    "evidence_ids": ["e1"],
                    "execution_preconditions": [{"kind": "root_identity_token", "expected": "x"}],
                }
            ],
            "notes": [],
            "safety_invariant_refs": [],
        }
        side_a = copy.deepcopy(base)
        side_a["actions"][0]["target_artifact_ids"] = ["python-artifact-1"]
        side_b = copy.deepcopy(base)
        side_b["actions"][0]["target_artifact_ids"] = ["rust-artifact-1"]

        index_a = {"python-artifact-1": "shared-identity-token"}
        index_b = {"rust-artifact-1": "shared-identity-token"}

        # Without the index, the two sides' opaque artifact ids do not resolve to the same
        # key, so the harness (correctly) cannot pair the actions and reports both as unmatched.
        without_index = diff_harness.compare_documents(side_a, side_b)
        self.assertTrue(any("present only in side A" in e for e in without_index))
        self.assertTrue(any("present only in side B" in e for e in without_index))

        # With the index, both resolve to the same identity_token and the actions pair up
        # cleanly - the ids themselves are never compared.
        with_index = diff_harness.compare_documents(side_a, side_b, artifact_index_a=index_a, artifact_index_b=index_b)
        self.assertEqual([], with_index)


class DiffHarnessActionCorrelationRegressionTests(unittest.TestCase):
    """E01-S05 round-one review (project/evidence/E01-VERIFIER-REVIEW.md): the comparator
    keyed explanation/result records by opaque action_id, so renaming only the id on
    semantically identical records produced false divergences. Reproduces the review's exact
    repro and proves the fix, using plan_a/plan_b to resolve action_id to a content-derived key.
    """

    def setUp(self):
        self.plan = diff_harness._load_golden("plan.golden.json")
        self.explanation = diff_harness._load_golden("explanation.golden.json")
        self.result = diff_harness._load_golden("result.golden.json")

    def _renamed_plan(self):
        renamed = copy.deepcopy(self.plan)
        renamed["plan_id"] = "rust-plan-id"
        for action in renamed["actions"]:
            action["action_id"] = f"rust-{action['action_id']}"
        return renamed

    def test_review_repro_explanation_id_only_rename_no_longer_diverges(self):
        renamed_plan = self._renamed_plan()
        renamed = copy.deepcopy(self.explanation)
        renamed["plan_id"] = "rust-plan-id"
        for item in renamed["explanations"]:
            item["action_id"] = f"rust-{item['action_id']}"
        errors = diff_harness.compare_documents(self.explanation, renamed, plan_a=self.plan, plan_b=renamed_plan)
        self.assertEqual([], errors)

    def test_review_repro_result_id_only_rename_no_longer_diverges(self):
        renamed_plan = self._renamed_plan()
        renamed = copy.deepcopy(self.result)
        renamed["plan_id"] = "rust-plan-id"
        for item in renamed["action_results"]:
            item["action_id"] = f"rust-{item['action_id']}"
        errors = diff_harness.compare_documents(self.result, renamed, plan_a=self.plan, plan_b=renamed_plan)
        self.assertEqual([], errors)

    def test_explanation_comparison_without_plan_context_refuses_rather_than_falling_back(self):
        errors = diff_harness.compare_documents(self.explanation, copy.deepcopy(self.explanation))
        self.assertTrue(any("requires plan_a and plan_b" in e for e in errors), errors)

    def test_result_comparison_without_plan_context_refuses_rather_than_falling_back(self):
        errors = diff_harness.compare_documents(self.result, copy.deepcopy(self.result))
        self.assertTrue(any("requires plan_a and plan_b" in e for e in errors), errors)

    def test_a_real_divergence_is_still_caught_alongside_a_renamed_id(self):
        renamed_plan = self._renamed_plan()
        renamed = copy.deepcopy(self.explanation)
        renamed["plan_id"] = "rust-plan-id"
        for item in renamed["explanations"]:
            item["action_id"] = f"rust-{item['action_id']}"
        renamed["explanations"][0]["final_authority"] = "AUTOPILOT"  # golden value is QUARANTINE
        errors = diff_harness.compare_documents(self.explanation, renamed, plan_a=self.plan, plan_b=renamed_plan)
        self.assertTrue(any("fields differ" in e for e in errors), errors)

    def test_action_id_field_itself_is_not_compared_once_records_are_matched(self):
        # Matched-by-content-key records legitimately carry different opaque action_id
        # values; the field-level comparison must not reintroduce that as a false divergence.
        renamed_plan = self._renamed_plan()
        renamed = copy.deepcopy(self.result)
        renamed["plan_id"] = "rust-plan-id"
        for item in renamed["action_results"]:
            item["action_id"] = f"different-{item['action_id']}"
        errors = diff_harness.compare_documents(self.result, renamed, plan_a=self.plan, plan_b=renamed_plan)
        self.assertFalse(any("action_id" in e for e in errors), errors)


if __name__ == "__main__":
    unittest.main()

"""Tests for the versioned JSON document contracts (E01-S03).

Mirrors tests/test_fixtures.py's approach: prove the real corpus is valid, then prove the
checker actually rejects the violations docs/architecture/JSON_CONTRACTS.md forbids - a
checker with no failing case proves nothing about itself.
"""

from __future__ import annotations

import copy
import json
import unittest

from scripts import check_schemas


def load(name: str) -> dict:
    path = check_schemas.GOLDEN_DIR / name
    return json.loads(path.read_text(encoding="utf-8"))


class SchemaContractTests(unittest.TestCase):
    def test_golden_corpus_is_valid(self):
        self.assertEqual([], check_schemas.validate())

    def test_all_four_document_types_are_covered(self):
        types = {
            json_doc["document_type"] for json_doc in (load(f"{name}.golden.json") for name in ("inventory", "plan", "explanation", "result"))
        }
        self.assertEqual({"inventory", "plan", "explanation", "result"}, types)

    # --- AC1: explicit version field -----------------------------------------

    def test_checker_flags_a_missing_schema_version(self):
        doc = copy.deepcopy(load("plan.golden.json"))
        del doc["schema_version"]
        errors = check_schemas.validate_document(doc, "synthetic")
        self.assertTrue(any("schema_version" in e for e in errors), errors)

    def test_checker_flags_an_unrecognized_schema_version(self):
        doc = copy.deepcopy(load("plan.golden.json"))
        doc["schema_version"] = 999
        errors = check_schemas.validate_document(doc, "synthetic")
        self.assertTrue(any("schema_version must be 1" in e for e in errors), errors)

    def test_checker_flags_envelope_keys_out_of_order(self):
        doc = copy.deepcopy(load("plan.golden.json"))
        # Swap the first two envelope keys - order is checked, not merely presence.
        reordered = dict(doc)
        keys = list(reordered.keys())
        keys[0], keys[1] = keys[1], keys[0]
        doc = {k: reordered[k] for k in keys}
        errors = check_schemas.validate_document(doc, "synthetic")
        self.assertTrue(any("envelope keys must appear first" in e for e in errors), errors)

    # --- AC3: every destructive action carries reason/authority/reversibility/preconditions --

    def test_checker_flags_a_mutating_action_with_no_preconditions(self):
        doc = copy.deepcopy(load("plan.golden.json"))
        mutating = next(a for a in doc["actions"] if a["action_class"] != "OBSERVE")
        mutating["execution_preconditions"] = []
        errors = check_schemas.validate_document(doc, "synthetic")
        self.assertTrue(any("requires at least one execution precondition" in e for e in errors), errors)

    def test_checker_allows_observe_action_with_no_preconditions(self):
        doc = copy.deepcopy(load("plan.golden.json"))
        observe = next(a for a in doc["actions"] if a["action_class"] == "OBSERVE")
        self.assertEqual([], observe["execution_preconditions"])
        errors = check_schemas.validate_document(doc, "synthetic")
        self.assertEqual([], errors)

    def test_checker_flags_an_action_missing_reason(self):
        doc = copy.deepcopy(load("plan.golden.json"))
        del doc["actions"][0]["reason"]
        errors = check_schemas.validate_document(doc, "synthetic")
        self.assertTrue(any("reason" in e for e in errors), errors)

    def test_checker_flags_an_empty_reason(self):
        doc = copy.deepcopy(load("plan.golden.json"))
        doc["actions"][0]["reason"] = "   "
        errors = check_schemas.validate_document(doc, "synthetic")
        self.assertTrue(any("reason must be a non-empty string" in e for e in errors), errors)

    def test_checker_flags_an_unrecognized_authority_value(self):
        doc = copy.deepcopy(load("plan.golden.json"))
        doc["actions"][0]["authority"] = "SUPERUSER"
        errors = check_schemas.validate_document(doc, "synthetic")
        self.assertTrue(any("authority" in e and "SUPERUSER" in e for e in errors), errors)

    def test_checker_flags_missing_evidence_ids(self):
        doc = copy.deepcopy(load("plan.golden.json"))
        doc["actions"][0]["evidence_ids"] = []
        errors = check_schemas.validate_document(doc, "synthetic")
        self.assertTrue(any("evidence_ids must be a non-empty list" in e for e in errors), errors)

    # --- explanation: final_authority must be derived, not asserted ----------

    def test_checker_flags_final_authority_that_does_not_match_the_last_step(self):
        doc = copy.deepcopy(load("explanation.golden.json"))
        doc["explanations"][0]["final_authority"] = "AUTOPILOT"
        errors = check_schemas.validate_document(doc, "synthetic")
        self.assertTrue(any("must equal the last step's resulting_authority" in e for e in errors), errors)

    # --- result: skipped is not success (SI-014) ------------------------------

    def test_checker_flags_a_summary_that_folds_skipped_into_succeeded(self):
        doc = copy.deepcopy(load("result.golden.json"))
        # A summary that hides the skip inside "succeeded" must be rejected, not just one
        # that has the wrong raw number.
        doc["summary"]["succeeded"] = 2
        doc["summary"]["safely_skipped"] = 0
        errors = check_schemas.validate_document(doc, "synthetic")
        self.assertTrue(any("SI-014" in e for e in errors), errors)

    def test_checker_flags_an_unrecognized_result_status(self):
        doc = copy.deepcopy(load("result.golden.json"))
        doc["action_results"][0]["status"] = "maybe"
        errors = check_schemas.validate_document(doc, "synthetic")
        self.assertTrue(any("status" in e and "maybe" in e for e in errors), errors)

    def test_checker_flags_an_unknown_document_type(self):
        errors = check_schemas.validate_document({"document_type": "nonsense"}, "synthetic")
        self.assertTrue(any("unknown or missing document_type" in e for e in errors), errors)


if __name__ == "__main__":
    unittest.main()

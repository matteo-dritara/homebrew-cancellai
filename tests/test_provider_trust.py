"""Tests for the community provider verification workflow (E16-S06).

Mirrors tests/test_schemas.py's approach: prove the real corpus is valid, then prove the
checker actually rejects the spoofed-metadata and self-promotion-without-evidence cases its
own acceptance criteria name - a checker with no failing case proves nothing about itself.
"""

from __future__ import annotations

import copy
import json
import os
import tempfile
import unittest
from pathlib import Path

from scripts import check_provider_trust as cpt


def real_manifests() -> dict[str, dict]:
    return {path.stem: json.loads(path.read_text(encoding="utf-8")) for path in sorted(cpt.MANIFEST_DIR.glob("*.json"))}


def real_registry() -> dict:
    return json.loads(cpt.REGISTRY_PATH.read_text(encoding="utf-8"))


def promoted_entry(fixture_references: list[str]) -> dict:
    return {
        "provider_id": "opencode",
        "tier": "builtin_verified",
        "verified_by": "maintainer-review-2026-09-23",
        "fixture_references": fixture_references,
    }


class ProviderTrustWorkflowTests(unittest.TestCase):
    def test_the_real_corpus_is_valid(self):
        self.assertEqual([], cpt.validate())

    def test_every_committed_manifest_has_exactly_one_registry_entry(self):
        manifest_ids = {doc["provider_id"] for doc in real_manifests().values()}
        registry_ids = [entry["provider_id"] for entry in real_registry()["publishers"]]
        self.assertEqual(manifest_ids, set(registry_ids))
        self.assertEqual(len(registry_ids), len(set(registry_ids)))

    # --- AC1 / spoofed-metadata: a manifest cannot smuggle a trust/capability claim ----------

    def test_manifest_lint_flags_a_smuggled_top_level_trust_field(self):
        doc = copy.deepcopy(next(iter(real_manifests().values())))
        doc["trust"] = "builtin_verified"
        errors: list[str] = []
        cpt.lint_manifest(doc, "synthetic", errors)
        self.assertTrue(any("trust" in e for e in errors), errors)

    def test_manifest_lint_flags_a_smuggled_capability_field(self):
        doc = copy.deepcopy(next(iter(real_manifests().values())))
        doc["capabilities"] = ["native_delete_capability"]
        errors: list[str] = []
        cpt.lint_manifest(doc, "synthetic", errors)
        self.assertTrue(any("capabilities" in e for e in errors), errors)

    def test_manifest_lint_flags_a_smuggled_field_nested_inside_a_root(self):
        doc = copy.deepcopy(next(iter(real_manifests().values())))
        doc["roots"][0]["authority"] = "autopilot"
        errors: list[str] = []
        cpt.lint_manifest(doc, "synthetic", errors)
        self.assertTrue(any("authority" in e for e in errors), errors)

    def test_manifest_lint_flags_a_smuggled_field_nested_inside_an_artifact(self):
        doc = copy.deepcopy(next(iter(real_manifests().values())))
        doc["artifacts"][0]["verified_by"] = "self"
        errors: list[str] = []
        cpt.lint_manifest(doc, "synthetic", errors)
        self.assertTrue(any("verified_by" in e for e in errors), errors)

    def test_manifest_lint_flags_a_smuggled_field_nested_inside_a_marker(self):
        doc = copy.deepcopy(next(iter(real_manifests().values())))
        doc["roots"][0]["markers"][0]["trusted"] = True
        errors: list[str] = []
        cpt.lint_manifest(doc, "synthetic", errors)
        self.assertTrue(any("trusted" in e for e in errors), errors)

    # --- AC1/AC2: a registry entry cannot self-promote without maintainer-owned evidence -----

    def test_checker_flags_builtin_verified_with_no_evidence_at_all(self):
        registry = copy.deepcopy(real_registry())
        registry["publishers"][0]["tier"] = "builtin_verified"
        errors: list[str] = []
        provider_id = cpt.validate_registry_entry(registry["publishers"][0], "synthetic", errors)
        self.assertIsNotNone(provider_id)
        self.assertTrue(any("verified_by" in e for e in errors), errors)
        self.assertTrue(any("fixture_references" in e for e in errors), errors)

    def test_checker_flags_builtin_verified_with_a_verifier_but_no_fixtures(self):
        registry = copy.deepcopy(real_registry())
        registry["publishers"][0]["tier"] = "builtin_verified"
        registry["publishers"][0]["verified_by"] = "someone"
        errors: list[str] = []
        cpt.validate_registry_entry(registry["publishers"][0], "synthetic", errors)
        self.assertTrue(any("fixture_references" in e for e in errors), errors)

    def test_checker_flags_a_whitespace_only_verified_by_as_self_attested(self):
        registry = copy.deepcopy(real_registry())
        registry["publishers"][0]["tier"] = "community_verified"
        registry["publishers"][0]["verified_by"] = "   "
        registry["publishers"][0]["fixture_references"] = ["tests/fixtures/x"]
        errors: list[str] = []
        cpt.validate_registry_entry(registry["publishers"][0], "synthetic", errors)
        self.assertTrue(any("verified_by" in e for e in errors), errors)

    def test_a_promotion_with_real_evidence_is_accepted(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "tests" / "fixtures" / "opencode" / "v1-layout").mkdir(parents=True)
            (root / "tests" / "fixtures" / "opencode" / "session.json").write_text("{}", encoding="utf-8")
            entry = promoted_entry(["tests/fixtures/opencode/v1-layout", "tests/fixtures/opencode/session.json"])
            errors: list[str] = []
            cpt.validate_registry_entry(entry, "synthetic", errors, root)
            self.assertEqual([], errors)

    # --- E16-S08: listed fixture evidence must exist inside the repository --------------------

    def assert_reference_refused(self, reference: str, root: Path, needle: str) -> None:
        errors: list[str] = []
        cpt.validate_registry_entry(promoted_entry([reference]), "synthetic", errors, root)
        self.assertTrue(any(repr(reference) in e and needle in e for e in errors), errors)

    def test_a_nonexistent_fixture_reference_is_refused(self):
        # The exact counterexample the E16 round-1 review recorded as a residual.
        with tempfile.TemporaryDirectory() as tmp:
            self.assert_reference_refused("tests/fixtures/opencode/v1-layout", Path(tmp), "does not exist")

    def test_a_nonexistent_reference_is_refused_even_beside_a_real_one(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "tests" / "fixtures").mkdir(parents=True)
            errors: list[str] = []
            entry = promoted_entry(["tests/fixtures", "tests/fixtures/missing"])
            cpt.validate_registry_entry(entry, "synthetic", errors, root)
            self.assertEqual(1, len(errors), errors)
            self.assertIn("'tests/fixtures/missing'", errors[0])

    def test_an_absolute_fixture_reference_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "real").mkdir()
            self.assert_reference_refused(str((root / "real").resolve()), root, "absolute")
            self.assert_reference_refused("C:\\fixtures", root, "absolute")

    def test_a_parent_directory_reference_is_refused_even_when_it_lands_inside(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "tests" / "fixtures").mkdir(parents=True)
            self.assert_reference_refused("tests/../tests/fixtures", root, "parent-directory")
            self.assert_reference_refused("..\\outside", root, "parent-directory")

    @unittest.skipIf(os.name == "nt", "symlink creation needs privileges on Windows")
    def test_a_symlink_escaping_the_repository_is_refused(self):
        with tempfile.TemporaryDirectory() as outside, tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "tests").mkdir()
            (root / "tests" / "escape").symlink_to(Path(outside), target_is_directory=True)
            self.assert_reference_refused("tests/escape", root, "outside the repository")

    def test_an_empty_fixture_reference_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assert_reference_refused("  ", Path(tmp), "is empty")

    def test_an_untrusted_entry_does_not_need_fixtures_to_exist(self):
        errors: list[str] = []
        entry = {"provider_id": "opencode", "tier": "untrusted", "verified_by": None, "fixture_references": ["nope"]}
        with tempfile.TemporaryDirectory() as tmp:
            cpt.validate_registry_entry(entry, "synthetic", errors, Path(tmp))
        self.assertEqual([], errors)

    def test_the_whole_registry_check_applies_the_fixture_rule(self):
        with tempfile.TemporaryDirectory() as tmp:
            registry = {"schema_version": 1, "publishers": [promoted_entry(["tests/fixtures/ghost"])]}
            errors = cpt.validate_registry(registry, {"opencode": "synthetic-manifest"}, "synthetic", Path(tmp))
            self.assertTrue(any("ghost" in e and "does not exist" in e for e in errors), errors)

    def test_checker_flags_an_invalid_tier_value(self):
        registry = copy.deepcopy(real_registry())
        registry["publishers"][0]["tier"] = "fully_trusted"
        errors: list[str] = []
        cpt.validate_registry_entry(registry["publishers"][0], "synthetic", errors)
        self.assertTrue(any("tier" in e for e in errors), errors)

    # --- completeness / phantom-provider checks ----------------------------------------------

    def test_cross_check_flags_a_registry_entry_naming_a_nonexistent_provider(self):
        registry = {
            "schema_version": 1,
            "publishers": [
                {
                    "provider_id": "does-not-exist",
                    "tier": "untrusted",
                    "verified_by": None,
                    "fixture_references": [],
                }
            ],
        }
        errors = cpt.validate_registry(registry, {"opencode": "synthetic-manifest"}, "synthetic")
        self.assertTrue(any("does-not-exist" in e and "names no manifest" in e for e in errors), errors)
        self.assertTrue(any("manifest(s) with no registry entry" in e for e in errors), errors)

    def test_cross_check_flags_a_manifest_missing_from_the_registry(self):
        registry = {"schema_version": 1, "publishers": []}
        errors = cpt.validate_registry(registry, {"opencode": "synthetic-manifest"}, "synthetic")
        self.assertTrue(any("manifest(s) with no registry entry" in e for e in errors), errors)

    def test_cross_check_accepts_a_matching_registry_and_manifest_set(self):
        registry = {
            "schema_version": 1,
            "publishers": [{"provider_id": "opencode", "tier": "untrusted", "verified_by": None, "fixture_references": []}],
        }
        errors = cpt.validate_registry(registry, {"opencode": "synthetic-manifest"}, "synthetic")
        self.assertEqual([], errors)

    def test_registry_rejects_an_unrecognized_top_level_field(self):
        registry = copy.deepcopy(real_registry())
        registry["maintainer_override"] = True
        errors: list[str] = []
        cpt._check_no_unknown_keys(registry, {"schema_version", "publishers"}, "synthetic", errors)
        self.assertTrue(any("maintainer_override" in e for e in errors), errors)

    def test_cli_reports_success_for_the_real_corpus(self):
        self.assertEqual(0, cpt.main(["check"]))


if __name__ == "__main__":
    unittest.main()

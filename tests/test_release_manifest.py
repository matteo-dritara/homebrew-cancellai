"""Tests for the release artifact manifest contract (E17-S01).

Mirrors tests/test_schemas.py's approach: prove the golden corpus is valid, then prove the
checker actually rejects what project/schemas/release_manifest.schema.json forbids - a checker
with no failing case proves nothing about itself. Also covers the checksum round-trip the
story's verification contract names explicitly.
"""

from __future__ import annotations

import copy
import hashlib
import json
import unittest

from scripts import release_manifest


def load_golden() -> dict:
    path = release_manifest.GOLDEN_DIR / "manifest.golden.json"
    return json.loads(path.read_text(encoding="utf-8"))


class ReleaseManifestSchemaTests(unittest.TestCase):
    def test_golden_corpus_is_valid(self):
        self.assertEqual([], release_manifest.check())

    def test_schema_file_is_valid_json(self):
        json.loads(release_manifest.SCHEMA_PATH.read_text(encoding="utf-8"))

    # --- AC1: machine-verifiable and versioned --------------------------------

    def test_checker_flags_a_missing_schema_version(self):
        doc = copy.deepcopy(load_golden())
        del doc["schema_version"]
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("schema_version" in e for e in errors), errors)

    def test_checker_flags_an_unrecognized_schema_version(self):
        doc = copy.deepcopy(load_golden())
        doc["schema_version"] = 999
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("schema_version must be 1" in e for e in errors), errors)

    def test_checker_flags_a_wrong_document_type(self):
        doc = copy.deepcopy(load_golden())
        doc["document_type"] = "something_else"
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("document_type" in e for e in errors), errors)

    def test_checker_flags_an_unknown_top_level_key(self):
        doc = copy.deepcopy(load_golden())
        doc["extra"] = "nope"
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("unknown key 'extra'" in e for e in errors), errors)

    def test_checker_flags_a_non_semver_version(self):
        doc = copy.deepcopy(load_golden())
        doc["version"] = "v1.0"
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("SemVer" in e for e in errors), errors)

    def test_checker_flags_an_unrecognized_channel(self):
        doc = copy.deepcopy(load_golden())
        doc["channel"] = "canary"
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("channel" in e and "canary" in e for e in errors), errors)

    def test_checker_flags_a_malformed_source_sha(self):
        doc = copy.deepcopy(load_golden())
        doc["source_sha"] = "not-a-sha"
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("source_sha" in e for e in errors), errors)

    def test_checker_flags_a_short_source_sha(self):
        doc = copy.deepcopy(load_golden())
        doc["source_sha"] = "abc123"
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("source_sha" in e for e in errors), errors)

    def test_checker_flags_missing_build_identity_fields(self):
        doc = copy.deepcopy(load_golden())
        del doc["build_identity"]["run_id"]
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("run_id" in e for e in errors), errors)

    def test_checker_flags_an_inverted_knowledge_compatibility_range(self):
        doc = copy.deepcopy(load_golden())
        doc["knowledge_compatibility"] = {"min_schema_version": 5, "max_schema_version": 1}
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("must not exceed" in e for e in errors), errors)

    def test_checker_flags_a_non_integer_knowledge_version(self):
        doc = copy.deepcopy(load_golden())
        doc["knowledge_compatibility"]["min_schema_version"] = "1"
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("min_schema_version" in e for e in errors), errors)

    # --- AC2: every distributed binary is represented exactly once ------------

    def test_checker_flags_a_duplicate_artifact_name(self):
        doc = copy.deepcopy(load_golden())
        duplicate = copy.deepcopy(doc["artifacts"][0])
        duplicate["target_triple"] = "a-different-triple"
        doc["artifacts"].append(duplicate)
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("exactly once" in e for e in errors), errors)

    def test_checker_allows_two_artifacts_sharing_a_target_triple(self):
        # An archive and an installer for the same platform are two distinct, distinguishably
        # named binaries - only the name is required to be unique.
        doc = copy.deepcopy(load_golden())
        installer = copy.deepcopy(doc["artifacts"][0])
        installer["name"] = installer["name"] + "-installer"
        doc["artifacts"].append(installer)
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertEqual([], errors)

    def test_checker_flags_an_empty_artifacts_list(self):
        doc = copy.deepcopy(load_golden())
        doc["artifacts"] = []
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("artifacts must be a non-empty list" in e for e in errors), errors)

    def test_checker_flags_a_malformed_artifact_checksum(self):
        doc = copy.deepcopy(load_golden())
        doc["artifacts"][0]["sha256"] = "deadbeef"
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("sha256" in e for e in errors), errors)

    def test_checker_flags_an_uppercase_artifact_checksum(self):
        doc = copy.deepcopy(load_golden())
        doc["artifacts"][0]["sha256"] = "AB" + "0" * 62
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("sha256" in e for e in errors), errors)

    def test_checker_flags_an_invalid_artifact_name(self):
        doc = copy.deepcopy(load_golden())
        doc["artifacts"][0]["name"] = "-leading-dash-not-allowed"
        errors = release_manifest.validate_document(doc, "synthetic")
        self.assertTrue(any("does not match" in e for e in errors), errors)

    def test_checker_flags_an_unknown_document_type_document(self):
        errors = release_manifest.validate_document({"document_type": "nonsense"}, "synthetic")
        self.assertTrue(any("document_type" in e for e in errors), errors)

    def test_checker_flags_a_non_object_document(self):
        errors = release_manifest.validate_document(["not", "an", "object"], "synthetic")
        self.assertTrue(any("must be a JSON object" in e for e in errors), errors)


class ChecksumRoundTripTests(unittest.TestCase):
    """Verification contract: 'Manifest schema and checksum round-trip tests.'"""

    def test_checksum_matches_a_freshly_hashed_payload(self):
        data = b"cancellai release artifact bytes"
        digest = hashlib.sha256(data).hexdigest()
        self.assertEqual(digest, release_manifest.sha256_of(data))
        self.assertTrue(release_manifest.checksum_matches(data, digest))

    def test_checksum_matches_is_case_insensitive_on_the_expected_value(self):
        data = b"cancellai release artifact bytes"
        digest = release_manifest.sha256_of(data)
        self.assertTrue(release_manifest.checksum_matches(data, digest.upper()))

    def test_checksum_matches_rejects_tampered_bytes(self):
        data = b"cancellai release artifact bytes"
        digest = release_manifest.sha256_of(data)
        self.assertFalse(release_manifest.checksum_matches(data + b"\x00", digest))

    def test_verify_artifact_checksums_round_trips_real_bytes_into_a_manifest(self):
        payload = b"a fake canonical binary, for the purposes of this test"
        doc = release_manifest.build_manifest(
            version="0.0.0",
            channel="nightly",
            source_sha="0" * 40,
            repository="matteo-dritara/homebrew-cancellai",
            workflow="release.yml",
            run_id="1",
            knowledge_min=1,
            knowledge_max=1,
            artifacts=[("cancellai-x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", release_manifest.sha256_of(payload))],
        )
        errors = release_manifest.verify_artifact_checksums(doc, {"cancellai-x86_64-unknown-linux-gnu": payload})
        self.assertEqual([], errors)

    def test_verify_artifact_checksums_detects_a_mismatch(self):
        doc = copy.deepcopy(load_golden())
        name = doc["artifacts"][0]["name"]
        errors = release_manifest.verify_artifact_checksums(doc, {name: b"bytes that do not match the declared checksum"})
        self.assertTrue(any("checksum mismatch" in e for e in errors), errors)

    def test_verify_artifact_checksums_flags_bytes_for_an_artifact_not_in_the_manifest(self):
        doc = copy.deepcopy(load_golden())
        errors = release_manifest.verify_artifact_checksums(doc, {"not-a-listed-artifact": b"whatever"})
        self.assertTrue(any("no matching manifest entry" in e for e in errors), errors)


class BuildManifestTests(unittest.TestCase):
    def test_build_manifest_produces_a_document_that_validates_cleanly(self):
        doc = release_manifest.build_manifest(
            version="2.0.0",
            channel="stable",
            source_sha="a" * 40,
            repository="matteo-dritara/homebrew-cancellai",
            workflow="release.yml",
            run_id="42",
            knowledge_min=1,
            knowledge_max=2,
            artifacts=[("cancellai-x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", "b" * 64)],
        )
        self.assertEqual([], release_manifest.validate_document(doc, "synthetic"))

    def test_build_manifest_refuses_a_duplicate_artifact_name(self):
        with self.assertRaises(release_manifest.ReleaseManifestError):
            release_manifest.build_manifest(
                version="2.0.0",
                channel="stable",
                source_sha="a" * 40,
                repository="matteo-dritara/homebrew-cancellai",
                workflow="release.yml",
                run_id="42",
                knowledge_min=1,
                knowledge_max=1,
                artifacts=[
                    ("cancellai-x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", "b" * 64),
                    ("cancellai-x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", "c" * 64),
                ],
            )

    def test_build_manifest_refuses_an_invalid_channel(self):
        with self.assertRaises(release_manifest.ReleaseManifestError):
            release_manifest.build_manifest(
                version="2.0.0",
                channel="canary",
                source_sha="a" * 40,
                repository="matteo-dritara/homebrew-cancellai",
                workflow="release.yml",
                run_id="42",
                knowledge_min=1,
                knowledge_max=1,
                artifacts=[("cancellai-x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", "b" * 64)],
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)

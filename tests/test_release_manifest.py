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
import tempfile
import unittest
from pathlib import Path

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


class BuildManifestFromFilesTests(unittest.TestCase):
    """E17-S02: generating a manifest from real, already-built artifact files."""

    def test_builds_a_valid_manifest_from_a_real_file_on_disk(self):
        with tempfile.TemporaryDirectory() as tmp:
            archive = Path(tmp) / "cancellai-cli-x86_64-unknown-linux-gnu.tar.gz"
            payload = b"a fake canonical archive, for the purposes of this test"
            archive.write_bytes(payload)
            doc = release_manifest.build_manifest_from_files(
                version="9.9.9",
                channel="nightly",
                source_sha="a" * 40,
                repository="matteo-dritara/homebrew-cancellai",
                workflow="release.yml",
                run_id="1",
                knowledge_min=1,
                knowledge_max=1,
                artifact_files=[("cancellai-cli-x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", archive)],
            )
            self.assertEqual([], release_manifest.validate_document(doc, "synthetic"))
            self.assertEqual(doc["artifacts"][0]["sha256"], release_manifest.sha256_of(payload))

    def test_refuses_to_generate_a_manifest_for_a_missing_artifact_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            missing = Path(tmp) / "does-not-exist.tar.gz"
            with self.assertRaises(release_manifest.ReleaseManifestError):
                release_manifest.build_manifest_from_files(
                    version="9.9.9",
                    channel="nightly",
                    source_sha="a" * 40,
                    repository="matteo-dritara/homebrew-cancellai",
                    workflow="release.yml",
                    run_id="1",
                    knowledge_min=1,
                    knowledge_max=1,
                    artifact_files=[("cancellai-cli-x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", missing)],
                )


class VerifyChecksumsAgainstDirectoryTests(unittest.TestCase):
    """E17-S02's publish-time guard: refuse to publish on a checksum mismatch."""

    def test_matches_real_files_named_after_each_artifact(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            payload = b"a fake canonical archive, for the purposes of this test"
            (directory / "cancellai-cli-x86_64-unknown-linux-gnu.tar.gz").write_bytes(payload)
            doc = release_manifest.build_manifest(
                version="9.9.9",
                channel="nightly",
                source_sha="a" * 40,
                repository="matteo-dritara/homebrew-cancellai",
                workflow="release.yml",
                run_id="1",
                knowledge_min=1,
                knowledge_max=1,
                artifacts=[("cancellai-cli-x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", release_manifest.sha256_of(payload))],
            )
            self.assertEqual([], release_manifest.verify_checksums_against_directory(doc, directory))

    def test_flags_a_missing_artifact_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            doc = copy.deepcopy(load_golden())
            errors = release_manifest.verify_checksums_against_directory(doc, Path(tmp))
            # The message names the suffixes it wanted and what it found instead, because the one
            # time this fired for real the answer was "an SBOM", and "no file matching" would not
            # have said so.
            self.assertTrue(any(".tar.gz" in e and "found: nothing" in e for e in errors), errors)

    def test_flags_a_real_file_that_does_not_match_the_declared_checksum(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            (directory / "cancellai-cli-x86_64-unknown-linux-gnu.tar.gz").write_bytes(b"tampered bytes")
            doc = release_manifest.build_manifest(
                version="9.9.9",
                channel="nightly",
                source_sha="a" * 40,
                repository="matteo-dritara/homebrew-cancellai",
                workflow="release.yml",
                run_id="1",
                knowledge_min=1,
                knowledge_max=1,
                artifacts=[("cancellai-cli-x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", "0" * 64)],
            )
            errors = release_manifest.verify_checksums_against_directory(doc, directory)
            self.assertTrue(any("checksum mismatch" in e for e in errors), errors)


class CliSubcommandTests(unittest.TestCase):
    """The `generate` and `verify-checksums` subcommands end to end (E17-S02)."""

    def test_generate_then_verify_checksums_round_trips_through_the_cli(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            archive = directory / "cancellai-cli-9.9.9-x86_64-unknown-linux-gnu.tar.gz"
            archive.write_bytes(b"a fake canonical archive, for the purposes of this test")
            out = directory / "release-manifest.json"

            exit_code = release_manifest.main(
                [
                    "generate",
                    "--version",
                    "9.9.9",
                    "--channel",
                    "nightly",
                    "--source-sha",
                    "a" * 40,
                    "--repository",
                    "matteo-dritara/homebrew-cancellai",
                    "--workflow",
                    "release.yml",
                    "--run-id",
                    "1",
                    "--knowledge-min",
                    "1",
                    "--knowledge-max",
                    "1",
                    "--artifact",
                    "cancellai-cli-9.9.9-x86_64-unknown-linux-gnu",
                    "x86_64-unknown-linux-gnu",
                    str(archive),
                    "--out",
                    str(out),
                ]
            )
            self.assertEqual(0, exit_code)
            doc = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual([], release_manifest.validate_document(doc, "synthetic"))

            exit_code = release_manifest.main(["verify-checksums", str(out), str(directory)])
            self.assertEqual(0, exit_code)

    def test_verify_checksums_cli_fails_on_a_tampered_artifact(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            (directory / "cancellai-cli-x86_64-unknown-linux-gnu.tar.gz").write_bytes(b"tampered")
            manifest_path = directory / "release-manifest.json"
            doc = release_manifest.build_manifest(
                version="9.9.9",
                channel="nightly",
                source_sha="a" * 40,
                repository="matteo-dritara/homebrew-cancellai",
                workflow="release.yml",
                run_id="1",
                knowledge_min=1,
                knowledge_max=1,
                artifacts=[("cancellai-cli-x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", "0" * 64)],
            )
            manifest_path.write_text(json.dumps(doc), encoding="utf-8")
            exit_code = release_manifest.main(["verify-checksums", str(manifest_path), str(directory)])
            self.assertEqual(2, exit_code)

    def test_bare_command_still_checks_the_golden_corpus(self):
        self.assertEqual(0, release_manifest.main([]))
        self.assertEqual(0, release_manifest.main(["check"]))


class ArchiveSelectionTests(unittest.TestCase):
    """The publish-time checksum guard has to hash the archive, not whatever sorts first.

    E17-S03 started dropping `<name>.cdx.json` beside `<name>.tar.gz`, and the guard globbed
    `<name>.*` and took the first match alphabetically - the SBOM. Every checksum in the v1.13.1
    manifest was then compared against a JSON document describing the archive, and all four
    mismatched. The defect was latent from the day the SBOM landed: the two releases between then
    and now failed before this job ever ran.
    """

    def artifact_dir(self, stack, files):
        directory = Path(stack)
        for name, payload in files.items():
            (directory / name).write_bytes(payload)
        return directory

    def test_the_archive_is_chosen_over_the_sbom_that_sorts_before_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = self.artifact_dir(
                tmp,
                {
                    "cancellai-cli-1.0.0-x86_64-apple-darwin.cdx.json": b'{"bomFormat":"CycloneDX"}',
                    "cancellai-cli-1.0.0-x86_64-apple-darwin.tar.gz": b"archive bytes",
                    "cancellai-cli-1.0.0-x86_64-apple-darwin.tar.gz.sha256": b"deadbeef\n",
                },
            )
            archive, problem = release_manifest.archive_for("cancellai-cli-1.0.0-x86_64-apple-darwin", directory)
            self.assertIsNone(problem)
            self.assertIsNotNone(archive)
            self.assertEqual(b"archive bytes", archive.read_bytes())

    def test_a_zip_archive_is_recognised(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = self.artifact_dir(
                tmp,
                {"a-b.cdx.json": b"{}", "a-b.zip": b"zip bytes", "a-b.zip.sha256": b"x\n"},
            )
            archive, problem = release_manifest.archive_for("a-b", directory)
            self.assertIsNone(problem)
            self.assertEqual(b"zip bytes", archive.read_bytes())

    def test_no_archive_is_an_error_that_says_what_was_there(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = self.artifact_dir(tmp, {"a-b.cdx.json": b"{}"})
            archive, problem = release_manifest.archive_for("a-b", directory)
            self.assertIsNone(archive)
            self.assertIn("a-b.cdx.json", problem)

    def test_two_archives_are_refused_rather_than_resolved_by_picking(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = self.artifact_dir(tmp, {"a-b.tar.gz": b"one", "a-b.zip": b"two"})
            archive, problem = release_manifest.archive_for("a-b", directory)
            self.assertIsNone(archive)
            self.assertIn("cannot tell which", problem)

    def test_the_round_trip_passes_with_the_sbom_present(self):
        # The end-to-end shape of the v1.13.1 failure: generate from the archive, verify against a
        # directory that also holds the SBOM and the checksum sidecar.
        with tempfile.TemporaryDirectory() as tmp:
            directory = self.artifact_dir(
                tmp,
                {
                    "cancellai-cli-1.0.0-x86_64-unknown-linux-gnu.cdx.json": b'{"bomFormat":"CycloneDX"}',
                    "cancellai-cli-1.0.0-x86_64-unknown-linux-gnu.tar.gz": b"real archive",
                    "cancellai-cli-1.0.0-x86_64-unknown-linux-gnu.tar.gz.sha256": b"stale\n",
                },
            )
            doc = release_manifest.build_manifest_from_files(
                version="1.0.0",
                channel="stable",
                source_sha="a" * 40,
                repository="owner/repo",
                workflow="release.yml",
                run_id="1",
                knowledge_min=1,
                knowledge_max=1,
                artifact_files=[
                    (
                        "cancellai-cli-1.0.0-x86_64-unknown-linux-gnu",
                        "x86_64-unknown-linux-gnu",
                        directory / "cancellai-cli-1.0.0-x86_64-unknown-linux-gnu.tar.gz",
                    )
                ],
            )
            self.assertEqual([], release_manifest.verify_checksums_against_directory(doc, directory))


if __name__ == "__main__":
    unittest.main(verbosity=2)

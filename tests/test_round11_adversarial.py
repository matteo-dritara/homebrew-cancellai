"""Independent round-11 counterexamples for release adoption."""

import hashlib
import json
import unittest
from unittest import mock

from scripts import release, release_manifest


class DuplicateTargetTest(unittest.TestCase):
    def test_manifest_with_second_name_for_same_target_must_refuse(self) -> None:
        version = "2.0.0"
        commit = "1" * 40
        archive = b"verified engine"
        digest = hashlib.sha256(archive).hexdigest()
        artifacts = [
            {
                "name": f"cancellai-cli-{version}-{target}",
                "target_triple": target,
                "sha256": digest,
            }
            for target in release.ENGINE_TARGETS
        ]
        artifacts.append(
            {
                "name": "second-macos-arm-archive",
                "target_triple": release.ENGINE_TARGETS[0],
                "sha256": digest,
            }
        )
        manifest = {
            "schema_version": 1,
            "document_type": "release_manifest",
            "version": version,
            "channel": "stable",
            "source_sha": commit,
            "build_identity": {
                "repository": release.REPO,
                "workflow": release.RELEASE_WORKFLOW,
                "run_id": "1",
            },
            "knowledge_compatibility": {
                "min_schema_version": 1,
                "max_schema_version": 1,
            },
            "artifacts": artifacts,
        }
        self.assertEqual(release_manifest.validate_document(manifest, "test manifest"), [])
        assets = {
            release.RELEASE_MANIFEST_ASSET.format(repo=release.REPO, version=version): json.dumps(manifest).encode(),
            release.FORMULA_ASSET.format(repo=release.REPO, version=version): release.render_formula(
                version,
                "f" * 64,
                release.published_engines(version, dict.fromkeys(release.ENGINE_TARGETS, digest)),
            ).encode(),
        }
        for target in release.ENGINE_TARGETS:
            assets[release.ENGINE_ASSET.format(repo=release.REPO, version=version, target=target)] = archive
        with (
            mock.patch.object(release, "download", side_effect=lambda url, cap: assets[url]),
            mock.patch.object(release, "archive_sha256", return_value="f" * 64),
            mock.patch.object(release, "tag_commit", return_value=commit),
            mock.patch.object(release, "verify_provenance", return_value=None),
            self.assertRaises(release.ReleaseError),
        ):
            release.adoptable_formula(version)

"""Independent release counterexample for E06-S14."""

import hashlib
import json
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

from scripts import release


def test_manifest_source_commit_must_bind_archive_provenance() -> None:
    version = "2.0.0"
    archive = b"archive attested from a different commit of the same tag"
    digest = hashlib.sha256(archive).hexdigest()
    manifest = {
        "document_type": "release_manifest",
        "version": version,
        "source_sha": "0" * 40,
        "build_identity": {"repository": release.REPO, "workflow": release.RELEASE_WORKFLOW, "run_id": "1"},
        "artifacts": [
            {"name": f"cancellai-cli-{version}-{target}", "target_triple": target, "sha256": digest} for target in release.ENGINE_TARGETS
        ],
    }
    source_digest = "f" * 64
    formula = release.render_formula(
        version,
        source_digest,
        release.published_engines(version, dict.fromkeys(release.ENGINE_TARGETS, digest)),
    )

    def downloaded(url: str, _cap: int) -> bytes:
        if url.endswith("release-manifest.json"):
            return json.dumps(manifest).encode()
        if url.endswith("cancellai.rb"):
            return formula.encode()
        return archive

    # A same-repository release.yml attestation from refs/tags/v2.0.0 is allowed by
    # provenance_command even when its certificate's source digest differs from the
    # manifest's source_sha. The mock represents gh accepting that valid attestation.
    # (Executor, after round 10: provenance_command now also takes the tag's commit; only this call
    # changed. The counterexample below is the reviewer's, unchanged.)
    command = release.provenance_command("gh", Path("archive.tar.gz"), version, "1" * 40)
    assert "--source-ref" in command
    with TemporaryDirectory() as tmp:
        live = Path(tmp) / "cancellai.rb"
        live.write_text("previous formula\n", encoding="utf-8")
        before = live.read_bytes()
        evidence = Path(tmp) / "RELEASE.md"
        evidence.write_text("synthetic release evidence\n", encoding="utf-8")
        versions = release.Versions(source=version, packaging=version, formula="1.21.0", engine=version)
        with (
            mock.patch.object(release, "FORMULA", live),
            mock.patch.object(release, "current_versions", return_value=versions),
            mock.patch.object(release, "release_evidence_path", return_value=evidence),
            mock.patch.object(release, "cutover_authorization_problems", return_value=[]),
            mock.patch.object(release, "download", side_effect=downloaded),
            mock.patch.object(release, "archive_sha256", return_value=source_digest),
            mock.patch.object(release, "verify_provenance"),
            mock.patch.object(release, "check", return_value=[]),
        ):
            refused = False
            try:
                release.finalize(version, adopt_cutover=True)
            except release.ReleaseError:
                refused = True
        assert refused, "finalize accepted a manifest whose source_sha disagreed with archive provenance"
        assert live.read_bytes() == before

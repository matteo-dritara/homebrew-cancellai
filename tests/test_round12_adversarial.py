"""Independent simulated-release checks of the E06-S14 adoption boundary."""

import hashlib
import json
import tempfile
import unittest
from contextlib import ExitStack
from pathlib import Path
from unittest import mock

from scripts import release, release_manifest


class FormulaAdoptionBoundaryTest(unittest.TestCase):
    VERSION = "2.0.0"
    COMMIT = "a" * 40
    RUN = "12345"
    SOURCE_DIGEST = "b" * 64

    def assets(self, directory: Path) -> tuple[dict[str, bytes], bytes]:
        archives = []
        assets = {}
        for target, extension in release.RELEASE_ARCHIVES:
            name = f"cancellai-cli-{self.VERSION}-{target}"
            path = directory / f"{name}.{extension}"
            path.write_bytes(f"independent archive for {target}".encode())
            archives.append((name, target, path))
            url = release.ARCHIVE_ASSET.format(repo=release.REPO, version=self.VERSION, target=target, ext=extension)
            assets[url] = path.read_bytes()

        document = release_manifest.build_manifest_from_files(
            version=self.VERSION,
            channel="stable",
            source_sha=self.COMMIT,
            repository=release.REPO,
            workflow="release.yml",
            run_id=self.RUN,
            knowledge_min=1,
            knowledge_max=1,
            artifact_files=archives,
        )
        manifest_url = release.RELEASE_MANIFEST_ASSET.format(repo=release.REPO, version=self.VERSION)
        assets[manifest_url] = (json.dumps(document, indent=2) + "\n").encode()
        digests = {
            target: hashlib.sha256(
                assets[release.ARCHIVE_ASSET.format(repo=release.REPO, version=self.VERSION, target=target, ext=ext)]
            ).hexdigest()
            for target, ext in release.RELEASE_ARCHIVES
        }
        formula = release.render_formula(
            self.VERSION,
            self.SOURCE_DIGEST,
            release.published_engines(self.VERSION, {target: digests[target] for target in release.ENGINE_TARGETS}),
        ).encode()
        assets[release.FORMULA_ASSET.format(repo=release.REPO, version=self.VERSION)] = formula
        return assets, formula

    def finalize_with(self, directory: Path, assets: dict[str, bytes], *, provenance_fails: bool = False) -> bytes:
        formula = directory / "cancellai.rb"
        # (Executor, after the 2.0.0 cutover: the live formula now carries the engine, so the
        # starting point is a pre-cutover Python formula built here, not the repository's own.
        # Only this line changed; the counterexamples are the reviewer's.)
        original = release.render_formula("1.21.1", "a" * 64, None).encode()
        formula.write_bytes(original)
        evidence = directory / "release-evidence.md"
        evidence.write_text("synthetic release", encoding="utf-8")

        def download(url: str, cap: int) -> bytes:
            if url not in assets:
                raise release.ReleaseError(f"missing asset: {url}")
            if len(assets[url]) > cap:
                raise release.ReleaseError(f"oversized asset: {url}")
            return assets[url]

        def provenance(data: bytes, name: str, version: str, commit: str) -> str:
            if provenance_fails:
                raise release.ReleaseError(f"invalid provenance: {name}")
            self.assertEqual((version, commit), (self.VERSION, self.COMMIT))
            return self.RUN

        def gh_json(*args: str) -> object:
            if args[1].endswith(f"/commits/v{self.VERSION}"):
                return {"sha": self.COMMIT}
            return {
                "path": ".github/workflows/release.yml",
                "head_sha": self.COMMIT,
                "head_branch": f"v{self.VERSION}",
                "event": "push",
                "conclusion": "success",
            }

        versions = release.Versions(source=self.VERSION, packaging=self.VERSION, formula=self.VERSION, engine=self.VERSION)
        with ExitStack() as stack:
            for name, value in (
                ("FORMULA", formula),
                ("current_versions", mock.Mock(return_value=versions)),
                ("release_evidence_path", mock.Mock(return_value=evidence)),
                ("cutover_authorization_problems", mock.Mock(return_value=[])),
                ("download", mock.Mock(side_effect=download)),
                ("verify_provenance", mock.Mock(side_effect=provenance)),
                ("archive_sha256", mock.Mock(return_value=self.SOURCE_DIGEST)),
                ("tag_commit", mock.Mock(return_value=self.COMMIT)),
                ("gh_json", mock.Mock(side_effect=gh_json)),
                ("check", mock.Mock(return_value=[])),
            ):
                stack.enter_context(mock.patch.object(release, name, value))
            try:
                release.finalize(self.VERSION, adopt_cutover=True)
            except release.ReleaseError:
                self.assertEqual(formula.read_bytes(), original)
                raise
        return formula.read_bytes()

    def test_valid_release_adopts_and_each_fault_refuses_without_touching_formula(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            assets, formula = self.assets(directory)
            self.assertEqual(self.finalize_with(directory, assets), formula)

            manifest_url = release.RELEASE_MANIFEST_ASSET.format(repo=release.REPO, version=self.VERSION)
            formula_url = release.FORMULA_ASSET.format(repo=release.REPO, version=self.VERSION)
            archive_urls = [
                release.ARCHIVE_ASSET.format(repo=release.REPO, version=self.VERSION, target=target, ext=ext)
                for target, ext in release.RELEASE_ARCHIVES
            ]
            for label, url in [
                ("manifest", manifest_url),
                ("formula", formula_url),
                *[(f"archive {i}", url) for i, url in enumerate(archive_urls)],
            ]:
                with self.subTest(label):
                    damaged = dict(assets)
                    del damaged[url]
                    with self.assertRaises(release.ReleaseError):
                        self.finalize_with(directory, damaged)

            for label, url in [("formula byte", formula_url), *[(f"archive byte {i}", url) for i, url in enumerate(archive_urls)]]:
                with self.subTest(label):
                    damaged = dict(assets)
                    damaged[url] += b"x"
                    with self.assertRaises(release.ReleaseError):
                        self.finalize_with(directory, damaged)

            for label, mutate in (
                ("other version", lambda doc: doc.update(version="2.0.1")),
                ("missing target", lambda doc: doc["artifacts"].pop(0)),
                ("duplicate target", lambda doc: doc["artifacts"].append({**doc["artifacts"][0], "name": "another-arm-archive"})),
                ("changed manifest digest", lambda doc: doc["artifacts"][0].update(sha256="0" * 64)),
            ):
                with self.subTest(label):
                    damaged = dict(assets)
                    document = json.loads(damaged[manifest_url])
                    mutate(document)
                    damaged[manifest_url] = (json.dumps(document, indent=2) + "\n").encode()
                    with self.assertRaises(release.ReleaseError):
                        self.finalize_with(directory, damaged)

            with self.assertRaises(release.ReleaseError):
                self.finalize_with(directory, assets, provenance_fails=True)

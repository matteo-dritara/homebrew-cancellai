from __future__ import annotations

import re
import unittest
from typing import ClassVar
from unittest import mock

from scripts import release


class ReleaseConsistencyTests(unittest.TestCase):
    def test_repository_release_state_is_consistent(self) -> None:
        # Versions agree across source, packaging and formula, and no closed epic is
        # sitting unreleased (PD-021).
        self.assertEqual(release.check(), [])

    def test_source_and_packaging_versions_always_agree(self) -> None:
        versions = release.current_versions()
        self.assertEqual(versions.source, versions.packaging)

    def test_the_formula_never_lags_by_more_than_the_in_flight_window(self) -> None:
        # Between `prepare` and `finalize` the formula legitimately points at the previous
        # *published* release, because the archive checksum cannot exist before the tag does -
        # not necessarily cut[1]: a version cut and never published (release.outcome --state no)
        # is never a valid target either, and formula_should_point_at skips past it the same way
        # release.py check does. Anything else means shipping a build nobody verified.
        versions = release.current_versions()
        cut = release.released_versions()
        self.assertEqual(versions.source, cut[0])
        if versions.formula == versions.source:
            return  # fully finalized; nothing in flight
        expected = release.formula_should_point_at(versions.source, cut, release.release_outcomes())
        self.assertEqual(versions.formula, expected)

    def test_a_closed_epic_is_at_least_a_minor_release(self) -> None:
        # An epic changes what the tool is willing to do; that is never a patch.
        self.assertEqual(release.suggest_version("1.0.2"), "1.1.0")
        self.assertEqual(release.suggest_version("2.7.0"), "2.8.0")

    def test_semantic_version_parsing_rejects_junk(self) -> None:
        for bad in ("1.0", "v1.0.0", "1.0.0-rc1", ""):
            with self.assertRaises(release.ReleaseError, msg=bad):
                release.parse(bad)

    def test_every_done_epic_has_release_evidence(self) -> None:
        covered = release.released_epics()
        for epic_id in release.epic_ids(status="done"):
            self.assertIn(epic_id, covered, epic_id)

    def test_changelog_unreleased_section_is_readable(self) -> None:
        # The section must be parseable even when empty; `prepare` is what rejects an
        # empty one, and it should fail with a clear message rather than a traceback.
        release.unreleased_body()


class FixReleaseTests(unittest.TestCase):
    """A release that closes no epic - the shape this repository could not express.

    v1.12.0 and v1.13.0 both failed in the release workflow, and a published tag is immutable
    history. The fix had to ship as a new version, and `prepare` could only cut a version that
    closed an epic. These tests pin the shape that was added and the two ways it could be abused:
    claiming a feature number for work that closed nothing, and crediting an epic by prose.
    """

    def test_a_fix_release_takes_the_next_patch_number(self) -> None:
        self.assertTrue(release.is_patch_of("1.13.1", "1.13.0"))
        for wrong in ("1.14.0", "2.0.0", "1.13.2", "1.13.0"):
            with self.subTest(version=wrong):
                self.assertFalse(release.is_patch_of(wrong, "1.13.0"))

    def test_prepare_refuses_both_shapes_at_once(self) -> None:
        with self.assertRaises(release.ReleaseError) as caught:
            release.prepare("9.9.9", epic_id="E22", reason="also a fix")
        self.assertIn("exactly one", str(caught.exception))

    def test_prepare_refuses_neither_shape(self) -> None:
        with self.assertRaises(release.ReleaseError) as caught:
            release.prepare("9.9.9")
        self.assertIn("exactly one", str(caught.exception))

    def test_a_fix_release_may_not_claim_a_feature_number(self) -> None:
        current = release.current_versions().source
        major, minor, _patch = release.parse(current)
        with self.assertRaises(release.ReleaseError) as caught:
            release.prepare(f"{major}.{minor + 1}.0", reason="a fix")
        self.assertIn("closes no epic", str(caught.exception))

    def test_a_fix_release_needs_a_stated_reason(self) -> None:
        current = release.current_versions().source
        major, minor, patch = release.parse(current)
        with self.assertRaises(release.ReleaseError) as caught:
            release.prepare(f"{major}.{minor}.{patch + 1}", reason="   ")
        self.assertIn("needs a reason", str(caught.exception))

    def test_a_fix_release_packet_declares_no_epic(self) -> None:
        # The safety property: a packet that closes nothing must not be able to satisfy PD-021
        # for some epic that happens to appear in the changelog text it embeds.
        section = release.included_work(None, "the tagged release workflow failed at verify-rust")
        self.assertEqual([], release.EPIC_DECLARATION_RE.findall(section))
        self.assertIn("closes no epic", section.lower())

    def test_an_epic_release_packet_declares_exactly_that_epic(self) -> None:
        section = release.included_work("E22", None)
        self.assertEqual(["E22"], release.EPIC_DECLARATION_RE.findall(section))


class EpicCoverageTests(unittest.TestCase):
    """Being mentioned is not being released."""

    def test_an_epic_named_only_in_prose_is_not_credited(self) -> None:
        prose = "This release fixes what E99 introduced, and mentions E98 in passing.\n"
        self.assertEqual([], release.EPIC_DECLARATION_RE.findall(prose))

    def test_a_declared_epic_is_credited_wherever_the_line_is_indented(self) -> None:
        packet = "## Included work\n\n- Epic: E09 - Atlas TUI\n  - Epic: E10 - Storage\n"
        self.assertEqual(["E09", "E10"], release.EPIC_DECLARATION_RE.findall(packet))

    def test_every_committed_packet_declares_at_least_one_epic_or_says_it_closes_none(self) -> None:
        for path in sorted(release.EVIDENCE.glob("RELEASE-v*.md")):
            with self.subTest(packet=path.name):
                text = path.read_text(encoding="utf-8")
                declared = release.EPIC_DECLARATION_RE.findall(text)
                self.assertTrue(declared or "closes no epic" in text.lower(), path.name)


class EmbeddedLinkTests(unittest.TestCase):
    """A packet embeds the changelog, and the changelog's links are written from the root."""

    def test_a_root_relative_link_is_rewritten_for_the_packet_directory(self):
        self.assertEqual("[a](../../docs/x.md)", release.relocate_links("[a](docs/x.md)"))

    def test_absolute_anchor_and_already_relative_links_are_left_alone(self):
        for text in ("[b](https://example.test/x)", "[c](#anchor)", "[d](../../docs/x.md)", "[e](/abs)"):
            with self.subTest(text=text):
                self.assertEqual(text, release.relocate_links(text))

    def test_rewriting_is_idempotent(self):
        once = release.relocate_links("[a](docs/x.md)")
        self.assertEqual(once, release.relocate_links(once))

    def test_every_committed_packet_link_resolves_from_where_it_lives(self):
        # The defect this came from: `prepare` copied the changelog verbatim, so v1.13.1's packet
        # pointed at `docs/adrs/...` from inside `project/evidence/`, where that is nothing.
        for path in sorted(release.EVIDENCE.glob("RELEASE-v*.md")):
            for target in re.findall(r"\]\((?!https?://|#|mailto:)([^)]+)\)", path.read_text(encoding="utf-8")):
                with self.subTest(packet=path.name, target=target):
                    self.assertTrue((path.parent / target.split("#")[0]).exists(), target)


class ReleaseOutcomeTests(unittest.TestCase):
    """A cut version whose release never published is a state, not folklore.

    Four of them exist - v1.10.0, v1.12.0, v1.13.0, v1.13.1 - each tagged with an evidence packet
    and a cut changelog section, none published. Before E17-S10 the only record was whoever
    remembered, and the formula-lag rule had to be walked around by hand.
    """

    def test_the_formula_may_skip_a_version_that_never_published(self):
        cut = ["1.13.2", "1.13.1", "1.13.0"]
        outcomes = {"1.13.1": ("no", "workflow failed"), "1.13.0": ("yes", "")}
        self.assertEqual("1.13.0", release.formula_should_point_at("1.13.2", cut, outcomes))

    def test_it_is_exactly_the_old_rule_where_nothing_failed(self):
        cut = ["1.13.2", "1.13.1", "1.13.0"]
        outcomes = {"1.13.1": ("yes", ""), "1.13.0": ("yes", "")}
        self.assertEqual("1.13.1", release.formula_should_point_at("1.13.2", cut, outcomes))

    def test_a_pending_outcome_does_not_let_the_formula_skip(self):
        # Unknown is not permission. Only a recorded failure moves the expected pointer back.
        cut = ["1.13.2", "1.13.1"]
        self.assertEqual("1.13.1", release.formula_should_point_at("1.13.2", cut, {}))

    def test_a_source_that_is_not_the_newest_cut_version_has_no_window(self):
        self.assertIsNone(release.formula_should_point_at("1.13.1", ["1.13.2", "1.13.1"], {}))

    def test_an_unpublished_release_must_say_what_failed(self):
        with self.assertRaises(release.ReleaseError) as caught:
            release.record_outcome("1.13.1", "no", "   ")
        self.assertIn("--reason is required", str(caught.exception))

    def test_recording_against_a_version_with_no_packet_is_refused(self):
        with self.assertRaises(release.ReleaseError) as caught:
            release.record_outcome("9.9.9", "yes", None)
        self.assertIn("nothing to record against", str(caught.exception))

    def test_an_unknown_state_is_refused(self):
        with self.assertRaises(release.ReleaseError):
            release.record_outcome("1.13.2", "maybe", None)

    def test_the_report_names_every_unpublished_version_and_its_reason(self):
        outcomes = {"1.12.0": ("no", "windows packaging"), "1.13.2": ("yes", "")}
        lines = release.unpublished_report(outcomes, "1.13.2")
        self.assertEqual(1, len(lines))
        self.assertIn("windows packaging", lines[0])

    def test_the_report_asks_about_an_older_version_with_no_outcome(self):
        lines = release.unpublished_report({"1.12.0": ("pending", "")}, "1.13.2")
        self.assertTrue(any("no recorded outcome" in line for line in lines))

    def test_every_committed_packet_records_an_outcome(self):
        # The backfill is the corpus this story was written from; a packet without a marker would
        # silently read as pending and make the report noisy rather than wrong. The newest cut
        # version is exempt: `prepare` writes `pending` and nobody can know the answer until the
        # release workflow has run. Anything older than that has had its chance.
        newest = release.released_versions()[0]
        for version, (state, reason) in release.release_outcomes().items():
            with self.subTest(version=version):
                self.assertIn(state, release.PUBLISHED_STATES)
                if version != newest:
                    self.assertNotEqual("pending", state, f"v{version} has no recorded outcome")
                if state == "no":
                    self.assertTrue(reason, f"v{version} says it failed and does not say why")

    def test_the_four_known_failures_are_recorded_as_such(self):
        outcomes = release.release_outcomes()
        for version in ("1.10.0", "1.12.0", "1.13.0", "1.13.1"):
            with self.subTest(version=version):
                self.assertEqual("no", outcomes[version][0])


if __name__ == "__main__":
    unittest.main(verbosity=2)


class EngineCutoverTests(unittest.TestCase):
    """E06-S04: the Rust engine's version moves with the release, and the formula is generated whole
    by release.render_formula and compared byte for byte (the answer to review rounds 6-8)."""

    DIGESTS: ClassVar[dict[str, str]] = {target: f"{index:064x}" for index, target in enumerate(release.ENGINE_TARGETS, 1)}

    def _engine_formula(self, version: str = "2.0.0") -> str:
        return release.render_formula(version, "f" * 64, release.published_engines(version, self.DIGESTS))

    def test_the_engine_version_agrees_with_the_source(self) -> None:
        versions = release.current_versions()
        self.assertEqual(versions.engine, versions.source)

    def test_internal_path_dependencies_require_the_workspace_version(self) -> None:
        engine = release.current_versions().engine
        for manifest in (release.RUST / "crates").glob("*/Cargo.toml"):
            for match in release.RUST_PATH_DEP_RE.finditer(release.read(manifest)):
                self.assertIn(f'version = "{engine}"', match.group(0), manifest)

    def test_the_live_formula_is_exactly_the_rendered_one(self) -> None:
        versions = release.current_versions()
        self.assertEqual(release.live_formula_problems(release.read(release.FORMULA), versions.formula), [])

    def test_a_rendered_engine_formula_is_accepted_and_names_each_archive_in_its_platform_block(self) -> None:
        text = self._engine_formula()
        self.assertEqual(release.live_formula_problems(text, "2.0.0"), [])
        arm = text.index("aarch64-apple-darwin.tar.gz")
        self.assertLess(text.index("on_arm do"), arm)
        self.assertLess(arm, text.index("on_intel do"))
        self.assertIn('bin.install "cancellai-cli" => "cancellai"', text)
        self.assertIn('bin.install "cancellai.py" => "cancellai-legacy"', text)

    def test_round_eight_counterexamples_are_refused(self) -> None:
        text = self._engine_formula()
        cases = {
            "python-only formula at 2.0.0": release.render_formula("2.0.0", "f" * 64, None),
            "resource moved out of its CPU block": text.replace("    on_intel do\n      resource", "    resource", 1),
            "engine install line removed": text.replace('    resource("engine").stage { bin.install "cancellai-cli" => "cancellai" }\n', ""),
            "archives swapped between platforms": text.replace("aarch64-apple-darwin", "@@")
            .replace("x86_64-apple-darwin", "aarch64-apple-darwin")
            .replace("@@", "x86_64-apple-darwin"),
            "engine at a pre-cutover version": self._engine_formula("1.22.0"),
        }
        for label, candidate in cases.items():
            version = "1.22.0" if "pre-cutover" in label else "2.0.0"
            self.assertTrue(release.live_formula_problems(candidate, version), label)

    def test_render_formula_refuses_a_wrong_set_of_targets(self) -> None:
        with self.assertRaises(release.ReleaseError):
            release.render_formula("2.0.0", "f" * 64, {"riscv64-unknown-linux-gnu": ("u", "0" * 64)})

    def _finalize_as(self, version: str, *, adopt: bool, story_status: str = "in_progress") -> str:
        import tempfile
        from pathlib import Path
        from unittest import mock

        with tempfile.TemporaryDirectory() as tmp:
            formula = Path(tmp) / "cancellai.rb"
            formula.write_text(release.read(release.FORMULA), encoding="utf-8")
            evidence = Path(tmp) / "RELEASE.md"
            evidence.write_text("x", encoding="utf-8")
            versions = release.Versions(source=version, packaging=version, formula=version, engine=version)
            with (
                mock.patch.object(release, "FORMULA", formula),
                mock.patch.object(release, "current_versions", return_value=versions),
                mock.patch.object(release, "release_evidence_path", return_value=evidence),
                mock.patch.object(release, "cutover_story_status", return_value=story_status),
                mock.patch.object(release, "check", return_value=[]),
            ):
                release.finalize(version, sha256="b" * 64, engine_digests=self.DIGESTS, adopt_cutover=adopt)
            return formula.read_text(encoding="utf-8")

    def test_a_release_at_or_after_the_cutover_cannot_skip_the_engine(self) -> None:
        with self.assertRaises(release.ReleaseError):
            self._finalize_as("2.0.0", adopt=False)

    def test_adoption_needs_the_owner_accepted_cutover_story(self) -> None:
        with self.assertRaises(release.ReleaseError):
            self._finalize_as("2.0.0", adopt=True, story_status="verification")
        adopted = self._finalize_as("2.0.0", adopt=True, story_status="done")
        self.assertEqual(release.live_formula_problems(adopted, "2.0.0"), [])

    def test_adopt_cutover_is_refused_before_the_cutover_version(self) -> None:
        with self.assertRaises(release.ReleaseError):
            self._finalize_as("1.22.0", adopt=True, story_status="done")

    def test_a_failed_post_write_check_restores_the_formula(self) -> None:
        import tempfile
        from pathlib import Path
        from unittest import mock

        with tempfile.TemporaryDirectory() as tmp:
            formula = Path(tmp) / "cancellai.rb"
            formula.write_text(release.read(release.FORMULA), encoding="utf-8")
            before = formula.read_text(encoding="utf-8")
            version = release.current_versions().source
            with (
                mock.patch.object(release, "FORMULA", formula),
                mock.patch.object(release, "check", return_value=["drift"]),
                self.assertRaises(release.ReleaseError),
            ):
                release.finalize(version, sha256="f" * 64)
            self.assertEqual(formula.read_text(encoding="utf-8"), before)

    def test_verify_installed_demands_exact_versions(self) -> None:
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as tmp:
            bin_dir = Path(tmp) / "bin"
            bin_dir.mkdir()
            for name, output in (("cancellai", "cancellai-cli 2.0.0"), ("cancellai-legacy", "cancellai 12.0.0")):
                script = bin_dir / name
                script.write_text(f"#!/bin/sh\necho '{output}'\n", encoding="utf-8")
                script.chmod(0o755)
            with self.assertRaises(release.ReleaseError):
                release.verify_installed("2.0.0", Path(tmp))
            (bin_dir / "cancellai-legacy").write_text("#!/bin/sh\necho 'cancellai 2.0.0'\n", encoding="utf-8")
            self.assertEqual(len(release.verify_installed("2.0.0", Path(tmp))), 2)


class PublishedEngineEvidenceTests(unittest.TestCase):
    """E06 review round 9: a formula whose engine digests matched the `.sha256` sidecars was
    accepted while the archives' own bytes hashed to something else, and no archive was ever
    downloaded. The archive, its sidecar and the release manifest must now agree."""

    VERSION = "2.0.0"

    def published(self, *, archive: bytes = b"engine", sidecar: str | None = None, manifest: str | None = None, manifest_version: str = VERSION):
        import hashlib
        import json

        real = hashlib.sha256(b"engine").hexdigest()
        assets = {}
        artifacts = []
        for target in release.ENGINE_TARGETS:
            url = release.ENGINE_ASSET.format(repo=release.REPO, version=self.VERSION, target=target)
            assets[url] = archive
            assets[url + ".sha256"] = f"{sidecar or real}  cancellai-cli-{self.VERSION}-{target}.tar.gz\n".encode()
            artifacts.append({"name": f"cancellai-cli-{self.VERSION}-{target}", "target_triple": target, "sha256": manifest or real})
        doc = {"document_type": "release_manifest", "version": manifest_version, "artifacts": artifacts}
        assets[release.RELEASE_MANIFEST_ASSET.format(repo=release.REPO, version=self.VERSION)] = json.dumps(doc).encode()
        return assets, real

    def fetch(self, assets):
        def download(url, cap):
            if url not in assets:
                raise release.ReleaseError(f"could not download {url}")
            return assets[url]

        return mock.patch.object(release, "download", side_effect=download)

    def formula(self, digest: str) -> str:
        return release.render_formula(
            self.VERSION, "f" * 64, release.published_engines(self.VERSION, dict.fromkeys(release.ENGINE_TARGETS, digest))
        )

    def problems(self, assets, digest: str):
        with self.fetch(assets), mock.patch.object(release, "archive_sha256", return_value="f" * 64):
            return release.published_digest_problems(self.formula(digest), self.VERSION)

    def test_agreeing_evidence_passes(self) -> None:
        assets, real = self.published()
        self.assertEqual(self.problems(assets, real), [])

    def test_an_archive_whose_bytes_differ_from_sidecar_and_manifest_is_refused(self) -> None:
        # Round 9's probe: sidecar and manifest both say `aa…`, the archive is other bytes.
        assets, _ = self.published(archive=b"other bytes", sidecar="a" * 64, manifest="a" * 64)
        with self.assertRaisesRegex(release.ReleaseError, "disagree"):
            self.problems(assets, "a" * 64)

    def test_a_sidecar_that_disagrees_is_refused(self) -> None:
        assets, real = self.published(sidecar="b" * 64)
        with self.assertRaisesRegex(release.ReleaseError, "disagree"):
            self.problems(assets, real)

    def test_a_manifest_that_disagrees_is_refused(self) -> None:
        assets, real = self.published(manifest="c" * 64)
        with self.assertRaisesRegex(release.ReleaseError, "disagree"):
            self.problems(assets, real)

    def test_a_manifest_for_another_version_is_refused(self) -> None:
        assets, real = self.published(manifest_version="1.99.0")
        with self.assertRaisesRegex(release.ReleaseError, "not the release manifest"):
            self.problems(assets, real)

    def test_missing_evidence_is_refused_not_skipped(self) -> None:
        for missing in ("release-manifest.json", ".sha256", ".tar.gz"):
            assets, real = self.published()
            assets = {url: data for url, data in assets.items() if not url.endswith(missing)}
            with self.subTest(missing=missing), self.assertRaises(release.ReleaseError):
                self.problems(assets, real)

    def test_a_formula_naming_other_bytes_than_the_agreed_archive_is_reported(self) -> None:
        assets, _ = self.published()
        self.assertTrue(self.problems(assets, "d" * 64))

    def test_finalize_takes_its_digests_from_the_archives(self) -> None:
        assets, real = self.published()
        with self.fetch(assets):
            self.assertEqual(release.engine_sha256s(self.VERSION), dict.fromkeys(release.ENGINE_TARGETS, real))

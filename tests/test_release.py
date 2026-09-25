from __future__ import annotations

import re
import unittest
from typing import ClassVar
from unittest import mock

from scripts import release

# A live formula from before the cutover. The finalize tests start from it rather than from the
# repository's own formula, which carries the engine since 2.0.0 - they depended on the tree's
# release state, and every one failed the day the cutover landed.
PRE_CUTOVER_FORMULA = release.render_formula("1.21.1", "a" * 64, None)


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

    def test_set_versions_moves_source_packaging_and_engine_together(self) -> None:
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = {
                "CANCELLAI": (root / "cancellai.py", release.read(release.CANCELLAI)),
                "PYPROJECT": (root / "pyproject.toml", release.read(release.PYPROJECT)),
                "RUST_WORKSPACE": (root / "Cargo.toml", release.read(release.RUST_WORKSPACE)),
                "RUST_LOCK": (root / "Cargo.lock", release.read(release.RUST_LOCK)),
            }
            for path, text in files.values():
                path.write_text(text, encoding="utf-8")
            patches = [mock.patch.object(release, name, path) for name, (path, _) in files.items()]
            patches.append(mock.patch.object(release, "RUST", root / "no-crates"))
            for patch in patches:
                patch.start()
            try:
                release.set_versions("2.0.0")
                versions = release.current_versions()
            finally:
                for patch in patches:
                    patch.stop()
            self.assertEqual((versions.source, versions.packaging, versions.engine), ("2.0.0", "2.0.0", "2.0.0"))

    def test_render_formula_refuses_a_wrong_set_of_targets(self) -> None:
        with self.assertRaises(release.ReleaseError):
            release.render_formula("2.0.0", "f" * 64, {"riscv64-unknown-linux-gnu": ("u", "0" * 64)})

    def _finalize_as(self, version: str, *, adopt: bool, authorized: bool = False) -> str:
        import tempfile
        from pathlib import Path
        from unittest import mock

        with tempfile.TemporaryDirectory() as tmp:
            formula = Path(tmp) / "cancellai.rb"
            formula.write_text(PRE_CUTOVER_FORMULA, encoding="utf-8")
            evidence = Path(tmp) / "RELEASE.md"
            evidence.write_text("x", encoding="utf-8")
            versions = release.Versions(source=version, packaging=version, formula=version, engine=version)
            engine = release.parse(version) >= release.CUTOVER_VERSION
            with (
                mock.patch.object(release, "FORMULA", formula),
                mock.patch.object(release, "current_versions", return_value=versions),
                mock.patch.object(release, "release_evidence_path", return_value=evidence),
                mock.patch.object(release, "cutover_authorization_problems", return_value=[] if authorized else ["no owner authorization"]),
                mock.patch.object(release, "adoptable_formula", return_value=self._engine_formula(version)),
                mock.patch.object(release, "check", return_value=[]),
            ):
                release.finalize(version, sha256=None if engine else "b" * 64, adopt_cutover=adopt)
            return formula.read_text(encoding="utf-8")

    def test_sha256_cannot_stand_in_for_the_published_release_after_the_cutover(self) -> None:
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as tmp:
            formula = Path(tmp) / "cancellai.rb"
            formula.write_text(PRE_CUTOVER_FORMULA, encoding="utf-8")
            before = formula.read_text(encoding="utf-8")
            evidence = Path(tmp) / "RELEASE.md"
            evidence.write_text("x", encoding="utf-8")
            versions = release.Versions(source="2.0.0", packaging="2.0.0", formula="2.0.0", engine="2.0.0")
            with (
                mock.patch.object(release, "FORMULA", formula),
                mock.patch.object(release, "current_versions", return_value=versions),
                mock.patch.object(release, "release_evidence_path", return_value=evidence),
                mock.patch.object(release, "cutover_authorization_problems", return_value=[]),
                self.assertRaisesRegex(release.ReleaseError, "cannot stand in"),
            ):
                release.finalize("2.0.0", sha256="b" * 64, adopt_cutover=True)
            self.assertEqual(formula.read_text(encoding="utf-8"), before)

    def test_a_refused_adoption_leaves_the_live_formula_unchanged(self) -> None:
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as tmp:
            formula = Path(tmp) / "cancellai.rb"
            formula.write_text(PRE_CUTOVER_FORMULA, encoding="utf-8")
            before = formula.read_text(encoding="utf-8")
            evidence = Path(tmp) / "RELEASE.md"
            evidence.write_text("x", encoding="utf-8")
            versions = release.Versions(source="2.0.0", packaging="2.0.0", formula="2.0.0", engine="2.0.0")
            with (
                mock.patch.object(release, "FORMULA", formula),
                mock.patch.object(release, "current_versions", return_value=versions),
                mock.patch.object(release, "release_evidence_path", return_value=evidence),
                mock.patch.object(release, "cutover_authorization_problems", return_value=[]),
                mock.patch.object(release, "adoptable_formula", side_effect=release.ReleaseError("provenance")),
                self.assertRaises(release.ReleaseError),
            ):
                release.finalize("2.0.0", adopt_cutover=True)
            self.assertEqual(formula.read_text(encoding="utf-8"), before)

    def test_a_release_at_or_after_the_cutover_cannot_skip_the_engine(self) -> None:
        with self.assertRaises(release.ReleaseError):
            self._finalize_as("2.0.0", adopt=False)

    def test_adoption_needs_the_owners_authorization(self) -> None:
        with self.assertRaisesRegex(release.ReleaseError, "not authorized"):
            self._finalize_as("2.0.0", adopt=True, authorized=False)
        adopted = self._finalize_as("2.0.0", adopt=True, authorized=True)
        self.assertEqual(release.live_formula_problems(adopted, "2.0.0"), [])

    def test_adopt_cutover_is_refused_before_the_cutover_version(self) -> None:
        with self.assertRaises(release.ReleaseError):
            self._finalize_as("1.22.0", adopt=True, authorized=True)

    def test_a_failed_post_write_check_restores_the_formula(self) -> None:
        import tempfile
        from pathlib import Path
        from unittest import mock

        with tempfile.TemporaryDirectory() as tmp:
            formula = Path(tmp) / "cancellai.rb"
            formula.write_text(PRE_CUTOVER_FORMULA, encoding="utf-8")
            before = formula.read_text(encoding="utf-8")
            version = "1.21.2"
            evidence = Path(tmp) / "RELEASE.md"
            evidence.write_text("x", encoding="utf-8")
            versions = release.Versions(source=version, packaging=version, formula="1.21.1", engine=version)
            with (
                mock.patch.object(release, "FORMULA", formula),
                mock.patch.object(release, "current_versions", return_value=versions),
                mock.patch.object(release, "release_evidence_path", return_value=evidence),
                mock.patch.object(release, "check", return_value=["drift"]),
                self.assertRaisesRegex(release.ReleaseError, "restored"),
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
    """E06-S14 (ADR-0040) after rounds 9-11 and the design consultation that followed: nothing the
    release says about itself is trusted. The manifest must be byte-identical to what the generator
    writes - first from its own values, then from the tag's commit, the run the attestations name
    and the digests of the downloaded bytes - and the published formula byte-identical to the one
    rendered from it. Each earlier round's counterexample is one case below."""

    VERSION = "2.0.0"
    SOURCE = "f" * 64
    COMMIT = "1" * 40
    RUN = "4242"

    def digests_of(self, archives: dict[str, bytes]) -> dict[str, str]:
        import hashlib

        return {target: hashlib.sha256(data).hexdigest() for target, data in archives.items()}

    def published(self, *, archives: dict[str, bytes] | None = None, manifest: str | None = None, run: str = RUN, commit: str = COMMIT):
        archives = archives or {target: f"engine {target}".encode() for target, _ in release.RELEASE_ARCHIVES}
        digests = self.digests_of(archives)
        text = manifest if manifest is not None else release.expected_manifest_text(self.VERSION, commit, run, digests)
        assets = {release.RELEASE_MANIFEST_ASSET.format(repo=release.REPO, version=self.VERSION): text.encode()}
        for target, extension in release.RELEASE_ARCHIVES:
            assets[release.ARCHIVE_ASSET.format(repo=release.REPO, version=self.VERSION, target=target, ext=extension)] = archives[target]
        formula = release.render_formula(
            self.VERSION, self.SOURCE, release.published_engines(self.VERSION, {t: digests[t] for t in release.ENGINE_TARGETS})
        )
        assets[release.FORMULA_ASSET.format(repo=release.REPO, version=self.VERSION)] = formula.encode()
        return assets, formula

    def network(self, assets, *, runs: dict[str, str] | None = None, remote: str = COMMIT, run_record=None, provenance_fails: bool = False):
        from contextlib import ExitStack

        def download(url, cap):
            if url not in assets:
                raise release.ReleaseError(f"could not download {url}")
            return assets[url]

        def verify_provenance(data, name, version, commit):
            if provenance_fails:
                raise release.ReleaseError(f"{name} failed build-provenance verification")
            return (runs or {}).get(name, self.RUN)

        record = run_record or {
            "path": ".github/workflows/release.yml",
            "head_sha": self.COMMIT,
            "head_branch": f"v{self.VERSION}",
            "event": "push",
            "conclusion": "success",
        }

        def gh_json(*args):
            if "/commits/v" in args[1]:
                return {"sha": remote}
            if "/actions/runs/" in args[1]:
                return record
            raise AssertionError(args)

        stack = ExitStack()
        stack.enter_context(mock.patch.object(release, "download", side_effect=download))
        stack.enter_context(mock.patch.object(release, "verify_provenance", side_effect=verify_provenance))
        stack.enter_context(mock.patch.object(release, "archive_sha256", return_value=self.SOURCE))
        stack.enter_context(mock.patch.object(release, "tag_commit", return_value=self.COMMIT))
        stack.enter_context(mock.patch.object(release, "gh_json", side_effect=gh_json))
        return stack

    def refused(self, assets, **network) -> None:
        with self.network(assets, **network), self.assertRaises(release.ReleaseError):
            release.adoptable_formula(self.VERSION)

    def test_the_release_workflow_writes_what_finalize_expects(self) -> None:
        """The generator's inputs in release.yml are the ones `expected_manifest_text` assumes."""
        workflow = (release.ROOT / ".github" / "workflows" / "release.yml").read_text(encoding="utf-8")
        generate = workflow[workflow.index("python3 scripts/release_manifest.py generate") :]
        generate = generate[: generate.index("--out release-manifest.json")]
        listed = re.findall(
            r'--artifact "cancellai-cli-\$\{version\}-([\w-]+)" ([\w-]+) "dist/cancellai-cli-\$\{version\}-[\w-]+\.([\w.]+)"', generate
        )
        self.assertEqual([(target, extension) for target, triple, extension in listed], list(release.RELEASE_ARCHIVES))
        self.assertTrue(all(target == triple for target, triple, _ in listed))
        for flag in (
            "--channel stable",
            "--workflow release.yml",
            '--run-id "$GITHUB_RUN_ID"',
            '--source-sha "$GITHUB_SHA"',
            "--knowledge-min 1 --knowledge-max 1",
        ):
            self.assertIn(flag, generate)

    def test_agreeing_evidence_adopts_the_published_formula(self) -> None:
        assets, formula = self.published()
        with self.network(assets):
            self.assertEqual(release.adoptable_formula(self.VERSION), formula)
            self.assertEqual(release.published_digest_problems(formula, self.VERSION), [])

    def test_round_eleven_a_second_name_for_a_target_is_refused(self) -> None:
        import json

        assets, _ = self.published()
        url = release.RELEASE_MANIFEST_ASSET.format(repo=release.REPO, version=self.VERSION)
        doc = json.loads(assets[url])
        doc["artifacts"].append({**doc["artifacts"][0], "name": "second-macos-arm-archive"})
        assets[url] = (json.dumps(doc, indent=2) + "\n").encode()
        self.refused(assets)

    def test_any_departure_from_the_generated_manifest_is_refused(self) -> None:
        import json

        cases = {
            "missing Windows archive": lambda doc: doc["artifacts"].pop(),
            "fifth archive": lambda doc: doc["artifacts"].append(
                {"name": "cancellai-cli-2.0.0-riscv", "target_triple": "riscv", "sha256": "0" * 64}
            ),
            "reordered archives": lambda doc: doc["artifacts"].reverse(),
            "renamed archive": lambda doc: doc["artifacts"][1].update(name="cancellai-cli-2.0.0-intel"),
            "relabelled triple": lambda doc: doc["artifacts"][1].update(target_triple="x86_64-unknown-linux-gnu"),
            "beta channel": lambda doc: doc.update(channel="beta"),
            "widened knowledge range": lambda doc: doc["knowledge_compatibility"].update(max_schema_version=2),
            "unknown nested field": lambda doc: doc["build_identity"].update(extra="x"),
            "another workflow": lambda doc: doc["build_identity"].update(workflow="experiment.yml"),
        }
        for label, mutate in cases.items():
            assets, _ = self.published()
            url = release.RELEASE_MANIFEST_ASSET.format(repo=release.REPO, version=self.VERSION)
            doc = json.loads(assets[url])
            mutate(doc)
            assets[url] = (json.dumps(doc, indent=2) + "\n").encode()
            with self.subTest(label):
                self.refused(assets)

    def test_a_manifest_serialized_differently_is_refused(self) -> None:
        import json

        assets, _ = self.published()
        url = release.RELEASE_MANIFEST_ASSET.format(repo=release.REPO, version=self.VERSION)
        text = assets[url].decode()
        for label, variant in {
            "compact": json.dumps(json.loads(text)),
            "duplicate key": text.replace('"channel": "stable",', '"channel": "beta",\n  "channel": "stable",', 1),
            "no final newline": text.rstrip("\n"),
        }.items():
            assets[url] = variant.encode()
            with self.subTest(label):
                self.refused(assets)

    def test_round_ten_a_manifest_naming_another_commit_is_refused(self) -> None:
        assets, _ = self.published(commit="0" * 40)
        self.refused(assets)

    def test_a_local_tag_github_does_not_share_is_refused(self) -> None:
        assets, _ = self.published()
        self.refused(assets, remote="2" * 40)

    def test_round_nine_an_archive_byte_changed_is_refused(self) -> None:
        assets, _ = self.published()
        url = release.ARCHIVE_ASSET.format(repo=release.REPO, version=self.VERSION, target="x86_64-pc-windows-msvc", ext="zip")
        assets[url] = assets[url] + b"x"
        self.refused(assets)

    def test_archives_from_different_runs_or_a_manifest_naming_another_run_are_refused(self) -> None:
        assets, _ = self.published()
        split = self.network(assets, runs={"cancellai-cli-2.0.0-x86_64-pc-windows-msvc.zip": "9999"})
        with split, self.assertRaisesRegex(release.ReleaseError, "different runs"):
            release.adoptable_formula(self.VERSION)
        assets, _ = self.published(run="9999")
        self.refused(assets)

    def test_a_run_that_is_not_the_tags_successful_release_run_is_refused(self) -> None:
        base = {
            "path": ".github/workflows/release.yml",
            "head_sha": self.COMMIT,
            "head_branch": "v2.0.0",
            "event": "push",
            "conclusion": "success",
        }
        for key, value in (
            ("conclusion", "failure"),
            ("head_sha", "2" * 40),
            ("head_branch", "main"),
            ("event", "workflow_dispatch"),
            ("path", ".github/workflows/x.yml"),
        ):
            assets, _ = self.published()
            with self.subTest(key):
                self.refused(assets, run_record={**base, key: value})

    def test_a_provenance_failure_is_refused(self) -> None:
        assets, _ = self.published()
        self.refused(assets, provenance_fails=True)

    def test_a_published_formula_one_byte_off_is_refused(self) -> None:
        assets, formula = self.published()
        assets[release.FORMULA_ASSET.format(repo=release.REPO, version=self.VERSION)] = (formula + " ").encode()
        self.refused(assets)

    def test_missing_evidence_is_refused_not_skipped(self) -> None:
        for missing in ("release-manifest.json", ".tar.gz", ".zip", "cancellai.rb"):
            assets, _ = self.published()
            assets = {url: data for url, data in assets.items() if not url.endswith(missing)}
            with self.subTest(missing=missing):
                self.refused(assets)

    def test_the_workflow_and_finalize_render_the_same_formula(self) -> None:
        import tempfile
        from pathlib import Path

        assets, formula = self.published()
        text = assets[release.RELEASE_MANIFEST_ASSET.format(repo=release.REPO, version=self.VERSION)].decode()
        with tempfile.TemporaryDirectory() as tmp:
            manifest = Path(tmp) / "release-manifest.json"
            manifest.write_text(text, encoding="utf-8")
            self.assertEqual(release.render_release_formula(self.VERSION, manifest, self.SOURCE), formula)
            manifest.write_text(text.replace('"stable"', '"beta"'), encoding="utf-8")
            with self.assertRaises(release.ReleaseError):
                release.render_release_formula(self.VERSION, manifest, self.SOURCE)

    def test_a_live_formula_that_is_not_the_published_one_is_reported(self) -> None:
        assets, formula = self.published()
        with self.network(assets):
            self.assertTrue(release.published_digest_problems(formula.replace("f" * 64, "e" * 64, 1), self.VERSION))

    def test_provenance_is_bound_to_the_workflow_tag_commit_and_hosted_runners(self) -> None:
        from pathlib import Path

        command = release.provenance_command("gh", Path("a.tar.gz"), "2.0.0", "1" * 40)
        self.assertEqual(command[command.index("--signer-workflow") + 1], f"{release.REPO}/.github/workflows/release.yml")
        self.assertEqual(command[command.index("--source-ref") + 1], "refs/tags/v2.0.0")
        self.assertEqual(command[command.index("--source-digest") + 1], "1" * 40)
        self.assertIn("--deny-self-hosted-runners", command)
        self.assertEqual(command[command.index("--format") + 1], "json")

    def test_the_attested_run_is_read_from_the_certificate(self) -> None:
        def entry(uri):
            return {"verificationResult": {"signature": {"certificate": {"runInvocationURI": uri}}}}

        good = f"https://github.com/{release.REPO}/actions/runs/4242/attempts/1"
        self.assertEqual(release.attested_run_ids([entry(good)]), {"4242"})
        self.assertEqual(release.attested_run_ids([entry(good), entry(good.replace("4242", "7"))]), {"4242", "7"})
        self.assertEqual(release.attested_run_ids([entry("https://github.com/someone/else/actions/runs/4242/attempts/1")]), {""})
        self.assertEqual(release.attested_run_ids([{}]), set())

    # E06-S16: the rehearsal's check, read-only, for the engine formula and the Python-only one.
    def test_verify_release_accepts_a_verified_release_and_refuses_a_wrong_formula_asset(self) -> None:
        assets, formula = self.published()
        with self.network(assets):
            self.assertEqual(len(release.verify_release(self.VERSION)), 3)
        assets[release.FORMULA_ASSET.format(repo=release.REPO, version=self.VERSION)] = (formula + "\n").encode()
        self.refused_by(release.verify_release, assets)

    def test_verify_release_checks_a_pre_cutover_release_against_the_python_formula(self) -> None:
        version = "1.21.1"
        archives = {target: f"engine {target}".encode() for target, _ in release.RELEASE_ARCHIVES}
        digests = self.digests_of(archives)
        assets = {
            release.RELEASE_MANIFEST_ASSET.format(repo=release.REPO, version=version): release.expected_manifest_text(
                version, self.COMMIT, self.RUN, digests
            ).encode(),
            release.FORMULA_ASSET.format(repo=release.REPO, version=version): release.render_formula(version, self.SOURCE, None).encode(),
        }
        for target, extension in release.RELEASE_ARCHIVES:
            assets[release.ARCHIVE_ASSET.format(repo=release.REPO, version=version, target=target, ext=extension)] = archives[target]
        record = {
            "path": ".github/workflows/release.yml",
            "head_sha": self.COMMIT,
            "head_branch": "v1.21.1",
            "event": "push",
            "conclusion": "success",
        }
        with self.network(assets, run_record=record):
            self.assertIn("Python-only", release.verify_release(version)[-1])
        with self.network(assets, run_record={**record, "conclusion": "failure"}), self.assertRaises(release.ReleaseError):
            release.verify_release(version)

    def refused_by(self, check, assets, **network) -> None:
        with self.network(assets, **network), self.assertRaises(release.ReleaseError):
            check(self.VERSION)

    def test_verify_provenance_refuses_without_gh(self) -> None:
        with mock.patch.object(release.shutil, "which", return_value=None), self.assertRaisesRegex(release.ReleaseError, "gh"):
            release.verify_provenance(b"x", "a.tar.gz", "2.0.0", "1" * 40)


class CutoverAuthorizationTests(unittest.TestCase):
    """E06-S15 (ADR-0040): `finalize --adopt-cutover` read E06-S04's `done` status as the owner's
    acceptance, so a status edit stood in for a decision. The authorization is now a committed file
    bound to the version and to the SHA-256 of the Safety Verdict the owner accepted."""

    PASSING = "## Round 10\n\nVerifier: Codex\n\nPASS\n"

    def check(self, *, verdict: str | None = PASSING, authorization: str | None = "", version: str = "2.0.0"):
        import hashlib
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            verdict_path = root / "SAFETY_VERDICT.md"
            authorization_path = root / "CUTOVER_AUTHORIZATION.md"
            if verdict is not None:
                verdict_path.write_text(verdict, encoding="utf-8")
            if authorization is not None:
                if authorization == "":
                    digest = hashlib.sha256((verdict or "").encode()).hexdigest()
                    authorization = f"Authorized-by: @matteo-dritara\nVersion: 2.0.0\nSafety-Verdict-SHA256: {digest}\n"
                authorization_path.write_text(authorization, encoding="utf-8")
            with (
                mock.patch.object(release, "ROOT", root),
                mock.patch.object(release, "CUTOVER_AUTHORIZATION", authorization_path),
                mock.patch.object(release, "CUTOVER_SAFETY_VERDICT", verdict_path),
            ):
                return release.cutover_authorization_problems(version)

    def test_a_valid_authorization_passes(self) -> None:
        self.assertEqual(self.check(), [])

    def test_no_authorization_is_refused(self) -> None:
        self.assertTrue(self.check(authorization=None))

    def test_an_authorization_for_another_version_is_refused(self) -> None:
        self.assertTrue(any("not v2.1.0" in p for p in self.check(version="2.1.0")))

    def test_a_verdict_edited_after_the_authorization_voids_it(self) -> None:
        import hashlib

        digest = hashlib.sha256(self.PASSING.encode()).hexdigest()
        authorization = f"Authorized-by: @matteo-dritara\nVersion: 2.0.0\nSafety-Verdict-SHA256: {digest}\n"
        problems = self.check(verdict=self.PASSING + "\n## Round 11\n\nPASS\n", authorization=authorization)
        self.assertTrue(any("changed after" in p for p in problems), problems)

    def test_a_failing_final_round_is_refused_even_when_authorized(self) -> None:
        problems = self.check(verdict="## Round 10\n\nFAIL\n")
        self.assertTrue(any("does not pass" in p for p in problems), problems)

    # E06 review round 10: any `Authorized-by` value was accepted.
    def test_only_the_repository_owner_can_authorize(self) -> None:
        import hashlib

        digest = hashlib.sha256(self.PASSING.encode()).hexdigest()
        stranger = f"Authorized-by: stranger\nVersion: 2.0.0\nSafety-Verdict-SHA256: {digest}\n"
        self.assertTrue(any("repository owner" in p for p in self.check(authorization=stranger)))
        self.assertEqual(release.repository_owner(), "@matteo-dritara")

    def test_every_field_is_required_exactly_once(self) -> None:
        for authorization in ("Version: 2.0.0\n", "Authorized-by: a\nAuthorized-by: b\nVersion: 2.0.0\nSafety-Verdict-SHA256: x\n"):
            with self.subTest(authorization=authorization):
                self.assertTrue(self.check(authorization=authorization))

    def test_no_story_status_is_read(self) -> None:
        # The function has no path to a story status: with E06-S04 marked done and no file, it refuses.
        self.assertFalse(hasattr(release, "cutover_story_status"))
        self.assertTrue(self.check(authorization=None))

    # E06 review round 10 (Codex's counterexample, kept): a verdict-shaped line in a later,
    # non-round section turned a failing final round into a pass.
    def test_a_later_note_cannot_reverse_a_failed_final_round(self) -> None:
        verdict = "## Round 10\nVerifier: Codex\n\nFAIL\n\n## Owner note\n\nPASS\n"
        self.assertTrue(any("does not pass" in p for p in self.check(verdict=verdict)))

    def test_the_final_round_section_decides(self) -> None:
        import tempfile
        from pathlib import Path

        cases = {
            "## Round 9\n\nFAIL\n\n## Round 10\n\nPASS\n": True,
            "## Round 9 — 2026-09-24\n\nPASS\n\n## Round 10 — 2026-09-25\n\nFAIL\n": False,
            "## Round 10\n\nPASS_WITH_RESIDUALS\n\n### Residuals\n\nnone blocking\n": True,
            "## Round 10\n\n```\nPASS\n```\n\nFAIL\n": False,
            "## Round 10\n\nFAIL\n\n```\nPASS\n```\n": False,
            "## Round 10\n\nPASS\n\n## Owner note\n\nFAIL\n": False,
            "PASS\n": False,
            "## Round 10\n\nno verdict line\n": False,
            # E35 self-review: a round heading only inside a fence does not make the file a
            # round history, so its later owner note cannot decide it.
            "```\n## Round 1\n```\n\n## Verdict\n\nFAIL\n\n## Owner note\n\nPASS\n": False,
            "## Round 10\n\nFAIL\n\n## Owner note\n\nsee below\n\n## Verdict\n\nPASS\n": False,
            "## Round 10\n\n## Verdict\n\nPASS\n\n## Verdict\n\nPASS\n": False,
            "## Round 10\n\nFAIL\n\n## Verdict\n\nPASS\n": False,
            "## Round 2 - operative\n\n## Verdict\n\n`PASS_WITH_RESIDUALS`\n\n## Owner decision\n\naccepted\n": True,
            "## Round 2\n\n## Verdict\n\nPASS\n\n## Owner decision\n\nREJECT\n": False,
        }
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "SAFETY_VERDICT.md"
            for text, passes in cases.items():
                path.write_text(text, encoding="utf-8")
                with self.subTest(text=text):
                    self.assertEqual(release.safety_verdict_passes(path), passes)

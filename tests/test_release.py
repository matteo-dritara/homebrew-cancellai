from __future__ import annotations

import re
import unittest

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
        # release, because the archive checksum cannot exist before the tag does. Anything
        # further behind, or ahead, means shipping a build nobody verified.
        versions = release.current_versions()
        cut = release.released_versions()
        self.assertEqual(versions.source, cut[0])
        self.assertIn(versions.formula, {cut[0], cut[1]})

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


if __name__ == "__main__":
    unittest.main(verbosity=2)

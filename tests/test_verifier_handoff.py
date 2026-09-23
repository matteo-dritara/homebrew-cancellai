"""Tests for the executor/verifier handoff (E28-S02).

Automating a handoff between two roles is the most direct way to collapse them, so the cases that
matter here are the ones where the mechanism could be used to erase the separation it exists to
prove: an executor recording its own verdict, a verdict answering a document that is not the
committed brief, a verdict with no author, and a brief edited after it was rendered. Each is
asserted to be refused by the gate rather than discouraged by documentation.

The last group asserts the property the story's own acceptance criteria put hardest: that a
verifier being unavailable produces no verdict at all, and that the manual route a human has always
used stays valid.
"""

from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

from scripts import verifier_handoff as handoff

BODY = "# Verifier Brief - E00-S01 - example\n\nStatus: ready_for_review | Change Risk: CR2\n"


class Checksums(unittest.TestCase):
    def test_the_checksum_ignores_the_header_it_lives_in(self) -> None:
        """Hashing the whole file would be self-referential: the header carries the checksum."""
        text = f"Brief-Checksum: {'0' * 64}\n\n{handoff.HEADER_END}\n{BODY}"
        self.assertEqual(handoff.digest(handoff.body_of(text)), handoff.digest(BODY))

    def test_line_endings_do_not_read_as_tampering(self) -> None:
        self.assertEqual(handoff.digest(BODY), handoff.digest(BODY.replace("\n", "\r\n")))

    def test_a_one_byte_change_to_the_body_changes_the_checksum(self) -> None:
        self.assertNotEqual(handoff.digest(BODY), handoff.digest(BODY + "."))


class HandoffCases(unittest.TestCase):
    """Each case builds a scratch evidence tree and runs the real checker over it."""

    def setUp(self) -> None:
        self._tmp = TemporaryDirectory()
        root = Path(self._tmp.name)
        self.evidence = root / "project" / "evidence"
        (self.evidence / "E00-S01").mkdir(parents=True)
        patcher = mock.patch.multiple(handoff, ROOT=root, PROJECT=root / "project", EVIDENCE=self.evidence)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.addCleanup(self._tmp.cleanup)

    def write_brief(self, *, rendered_by: str = "Claude (executor)", body: str = BODY, declared: str | None = None) -> str:
        actual = handoff.digest(body)
        path = self.evidence / "E00-S01" / handoff.BRIEF
        path.write_text(
            f"Story: E00-S01\nRendered-by: {rendered_by}\nBrief-Checksum: {declared or actual}\n\n{handoff.HEADER_END}\n{body}",
            encoding="utf-8",
        )
        return actual

    def write_verdict(self, *, checksum: str | None, verifier: str | None = "Codex") -> None:
        lines = ["# Verifier review", ""]
        if verifier is not None:
            lines.append(f"Verifier: {verifier}")
        if checksum is not None:
            lines.append(f"Brief-Checksum: {checksum}")
        (self.evidence / "E00-S01" / "E00-S01-VERIFIER-REVIEW.md").write_text("\n".join(lines) + "\n", encoding="utf-8")

    def test_a_matching_pair_passes(self) -> None:
        self.write_verdict(checksum=self.write_brief())
        self.assertEqual(handoff.check_story("E00-S01"), [])

    def test_the_executor_may_not_author_the_verdict(self) -> None:
        """The rule AGENT_PROTOCOL.md states, made mechanical instead of obeyed voluntarily."""
        checksum = self.write_brief(rendered_by="Claude (executor)")
        self.write_verdict(checksum=checksum, verifier="Claude (executor)")
        problems = handoff.check_story("E00-S01")
        self.assertEqual(len(problems), 1)
        self.assertIn("collapse", problems[0])

    def test_a_verdict_answering_a_different_document_is_refused(self) -> None:
        self.write_brief()
        self.write_verdict(checksum=handoff.digest(BODY + " tampered"))
        problems = handoff.check_story("E00-S01")
        self.assertEqual(len(problems), 1)
        self.assertIn("not the committed brief", problems[0])

    def test_a_brief_edited_after_rendering_is_refused(self) -> None:
        self.write_brief(declared="f" * 64)
        problems = handoff.check_story("E00-S01")
        self.assertTrue(any("edited after it was rendered" in p for p in problems))

    def test_an_unattributed_verdict_is_refused(self) -> None:
        """A CR4 Safety Verdict must stay attributable; the field is required, not defaulted."""
        self.write_verdict(checksum=self.write_brief(), verifier=None)
        problems = handoff.check_story("E00-S01")
        self.assertEqual(len(problems), 1)
        self.assertIn("unattributed", problems[0])

    def test_a_verdict_cannot_bypass_a_rendered_brief_by_omitting_its_checksum(self) -> None:
        """The manual route is no brief at all, not a verdict that ignores a committed brief."""
        self.write_brief()
        self.write_verdict(checksum=None, verifier="Codex")
        problems = handoff.check_story("E00-S01")
        self.assertEqual(len(problems), 1)
        self.assertIn("carries no Brief-Checksum", problems[0])

    def test_a_verdict_naming_a_brief_that_does_not_exist_is_refused(self) -> None:
        self.write_verdict(checksum=handoff.digest(BODY))
        problems = handoff.check_story("E00-S01")
        self.assertEqual(len(problems), 1)
        self.assertIn("no VERIFIER_BRIEF.md", problems[0].replace(handoff.BRIEF, "VERIFIER_BRIEF.md"))


class TheSeparationSurvivesTheAutomation(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = TemporaryDirectory()
        root = Path(self._tmp.name)
        self.evidence = root / "project" / "evidence"
        (self.evidence / "E00-S01").mkdir(parents=True)
        patcher = mock.patch.multiple(handoff, ROOT=root, PROJECT=root / "project", EVIDENCE=self.evidence)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.addCleanup(self._tmp.cleanup)

    def test_an_unavailable_verifier_leaves_no_verdict_behind(self) -> None:
        """There is no path here that produces a verdict without a verifier; a brief alone passes
        and the story simply has not been reviewed yet."""
        (self.evidence / "E00-S01" / handoff.BRIEF).write_text(
            f"Rendered-by: Claude\nBrief-Checksum: {handoff.digest(BODY)}\n\n{handoff.HEADER_END}\n{BODY}",
            encoding="utf-8",
        )
        self.assertEqual(handoff.check_story("E00-S01"), [])
        self.assertEqual(list((self.evidence / "E00-S01").glob("*VERDICT*")), [])

    def test_the_manual_route_stays_valid(self) -> None:
        """The method is the separation, not the automation of it: a story with no brief is fine."""
        (self.evidence / "E00-S01" / "E00-S01-VERIFIER-REVIEW.md").write_text("# Review\n\nVerifier: Codex\n\nPASS\n", encoding="utf-8")
        self.assertEqual(handoff.check_story("E00-S01"), [])

    def test_an_epic_round_answers_each_named_story_brief(self) -> None:
        checksum = handoff.digest(BODY)
        (self.evidence / "E00-S01" / handoff.BRIEF).write_text(
            f"Rendered-by: Claude\nBrief-Checksum: {checksum}\n\n{handoff.HEADER_END}\n{BODY}",
            encoding="utf-8",
        )
        record = self.evidence / "E00-VERIFIER-REVIEW.md"
        record.write_text(
            "Review-Scope: epic\nVerifier: Codex\n\n"
            "| Story | Verdict | evidence |\n| --- | --- | --- |\n"
            f"| E00-S01 | PASS | Brief-Checksum: {checksum} |\n",
            encoding="utf-8",
        )
        self.assertEqual(handoff.check_story("E00-S01"), [])

    def test_the_module_never_writes_a_verdict(self) -> None:
        source = (Path(handoff.__file__)).read_text(encoding="utf-8")
        for name in ("VERDICT", "VERIFIER-REVIEW"):
            self.assertNotIn(f'write_text(f"{name}', source)
        self.assertEqual(source.count("write_text"), 1, "the only write is the brief itself")


class LinkRelocation(unittest.TestCase):
    def test_invariant_links_are_rewritten_for_the_evidence_directory(self) -> None:
        body = "see [ADR](../adrs/0023-x.md), [model](THREAT_MODEL.md#tm-11), [web](https://e.x), [a](#s)"
        self.assertEqual(
            handoff.relocate_links(body),
            "see [ADR](../../../docs/adrs/0023-x.md), [model](../../../docs/security/THREAT_MODEL.md#tm-11), [web](https://e.x), [a](#s)",
        )

    def test_every_relocated_link_in_a_real_brief_resolves(self) -> None:
        body = handoff.relocate_links(handoff.render_brief("E17-S07"))
        base = handoff.EVIDENCE / "E17-S07"
        for target in handoff.RELATIVE_LINK.findall(body):
            self.assertTrue((base / target.split("#")[0]).resolve().exists(), target)


if __name__ == "__main__":
    unittest.main()

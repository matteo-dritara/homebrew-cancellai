"""Tests for evidence-ledger validation (E25-S03).

The ledger is the system's memory. It had already decayed without anyone noticing - fifteen
packets past `ready_for_review` carried no per-criterion rows, two of them CR4 - so the cases
below are about the ways a packet can look complete and discharge nothing.
"""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts import check_evidence as evidence

FULL_PACKET = """# Evidence Packet - E01-S01

- Change Risk: {risk}

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | a test | PASS |
| AC2 | another test | PASS |

## Verification Commands

```text
python3 scripts/project_os.py check -> pass
```

## Residual risks

{residual}
"""


def story(**overrides):
    base = {
        "id": "E01-S01",
        "status": "ready_for_review",
        "change_risk": "CR1",
        "acceptance_criteria": ["first", "second"],
    }
    base.update(overrides)
    return base


def scratch(root: Path, story_id: str, text: str) -> None:
    directory = root / "project" / "evidence" / story_id
    directory.mkdir(parents=True, exist_ok=True)
    (directory / evidence.PACKET).write_text(text, encoding="utf-8")
    scripts = root / "scripts"
    scripts.mkdir(exist_ok=True)
    (scripts / "project_os.py").write_text("# stub\n", encoding="utf-8")


class PacketPresenceTests(unittest.TestCase):
    def test_a_story_past_ready_for_review_without_a_packet_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            problems = evidence.check_story(story(), Path(tmp))
            self.assertTrue(any("no evidence packet" in p for p in problems))

    def test_a_planned_story_is_not_considered(self):
        stories = {"E01-S01": story(status="planned")}
        errors, warnings, considered = evidence.evaluate(stories, Path("/nonexistent"))
        self.assertEqual(([], [], 0), (errors, warnings, considered))


class CriterionCoverageTests(unittest.TestCase):
    def run_check(self, text, **overrides):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            scratch(root, "E01-S01", text)
            return evidence.check_story(story(**overrides), root)

    def test_a_row_per_criterion_passes(self):
        self.assertEqual([], self.run_check(FULL_PACKET.format(risk="CR1", residual="- a real one")))

    def test_fewer_rows_than_criteria_fails(self):
        # The failure that had already happened fifteen times before anything looked.
        problems = self.run_check(FULL_PACKET.format(risk="CR1", residual="- x"), acceptance_criteria=["a", "b", "c"])
        self.assertTrue(any("no evidence row for AC3" in p for p in problems))

    def test_duplicated_row_numbers_do_not_inflate_the_count(self):
        text = FULL_PACKET.format(risk="CR1", residual="- x").replace("| AC2 | another test | PASS |", "| AC1 | again | PASS |")
        problems = self.run_check(text)
        self.assertTrue(any("no evidence row for AC2" in p for p in problems))

    def test_rows_inside_a_fenced_example_do_not_count_as_coverage(self):
        # A packet whose only AC rows sat in a fenced template was counted as fully covered.
        text = FULL_PACKET.format(risk="CR1", residual="- x")
        fenced = text.replace(
            "| AC1 | a test | PASS |\n| AC2 | another test | PASS |", "```\n| AC1 | example | PASS |\n| AC2 | example | PASS |\n```"
        )
        problems = self.run_check(fenced)
        self.assertTrue(any("no evidence row for AC1, AC2" in p for p in problems))

    def test_rows_numbered_beyond_the_criteria_do_not_satisfy_them(self):
        # AC5-AC9 satisfied a three-criterion story under a count comparison.
        text = FULL_PACKET.format(risk="CR1", residual="- x").replace("| AC1 |", "| AC8 |").replace("| AC2 |", "| AC9 |")
        problems = self.run_check(text)
        self.assertTrue(any("no evidence row for AC1, AC2" in p for p in problems))

    def test_more_rows_than_criteria_is_not_an_error(self):
        # Over-documenting is not the failure mode this guards against.
        self.assertEqual([], self.run_check(FULL_PACKET.format(risk="CR1", residual="- x"), acceptance_criteria=["only one"]))


class ResidualRiskTests(unittest.TestCase):
    def run_check(self, residual, risk):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            scratch(root, "E01-S01", FULL_PACKET.format(risk=risk, residual=residual))
            return evidence.check_story(story(change_risk=risk), root)

    def test_a_cr4_packet_with_no_residual_risk_fails(self):
        self.assertTrue(any("no residual risk" in p for p in self.run_check("- none", "CR4")))

    def test_a_cr3_packet_with_an_empty_section_fails(self):
        self.assertTrue(any("no residual risk" in p for p in self.run_check("", "CR3")))

    def test_a_cr1_packet_may_declare_none(self):
        # The rule binds where the change can mutate or holds authority, not everywhere.
        self.assertEqual([], self.run_check("- none", "CR1"))

    def test_a_cr4_packet_with_a_real_residual_passes(self):
        self.assertEqual([], self.run_check("- the harness cannot reach Windows reparse points", "CR4"))

    def test_none_is_detected_through_list_markers(self):
        for spelling in ("none", "- none", "* None", "~ NONE - nothing found"):
            with self.subTest(spelling=spelling):
                self.assertEqual("", evidence.residual_body(f"## Residual risks\n\n{spelling}\n"))


class SafetyVerdictTests(unittest.TestCase):
    def test_a_done_cr4_story_without_a_verdict_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            scratch(root, "E01-S01", FULL_PACKET.format(risk="CR4", residual="- real"))
            problems = evidence.check_story(story(change_risk="CR4", status="done"), root)
            self.assertTrue(any("no Safety Verdict" in p for p in problems))

    def test_a_ready_for_review_cr4_story_does_not_need_one_yet(self):
        # The verdict is the reviewer's output, gated at `done`, never at handover.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            scratch(root, "E01-S01", FULL_PACKET.format(risk="CR4", residual="- real"))
            self.assertEqual([], evidence.check_story(story(change_risk="CR4"), root))


class ClaimedCommandTests(unittest.TestCase):
    def test_a_claimed_script_that_does_not_exist_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            text = FULL_PACKET.format(risk="CR1", residual="- x").replace("scripts/project_os.py", "scripts/imaginary.py")
            scratch(root, "E01-S01", text)
            problems = evidence.check_story(story(), root)
            self.assertTrue(any("claims to have run scripts/imaginary.py" in p for p in problems))


class BaselineTests(unittest.TestCase):
    def test_every_baseline_entry_names_a_real_story_and_a_reason(self):
        stories = evidence.load_stories()
        for story_id, reason in evidence.BASELINE.items():
            with self.subTest(story=story_id):
                self.assertIn(story_id, stories)
                self.assertTrue(reason.strip())

    def test_a_baselined_story_warns_rather_than_failing(self):
        stories = {"E00-S01": story(id="E00-S01", status="done")}
        errors, warnings, _ = evidence.evaluate(stories, Path("/nonexistent"))
        self.assertEqual([], errors)
        self.assertTrue(any("baseline" in w for w in warnings))

    def test_a_story_outside_the_baseline_fails(self):
        stories = {"E99-S99": story(id="E99-S99", status="done")}
        errors, _, _ = evidence.evaluate(stories, Path("/nonexistent"))
        self.assertTrue(errors)


class RealRepositoryTests(unittest.TestCase):
    def test_the_committed_ledger_passes(self):
        self.assertEqual(0, evidence.main(["check"]))

    def test_no_cr3_or_cr4_packet_in_the_repository_declares_no_residual_risk(self):
        stories = evidence.load_stories()
        for story_id, entry in sorted(stories.items()):
            if entry["status"] not in evidence.ACCEPTED_STATUSES or entry["change_risk"] not in evidence.RESIDUAL_REQUIRED_AT:
                continue
            if story_id in evidence.BASELINE:
                continue
            packet = evidence.EVIDENCE / story_id / evidence.PACKET
            with self.subTest(story=story_id):
                self.assertTrue(evidence.residual_body(packet.read_text(encoding="utf-8")))


if __name__ == "__main__":
    unittest.main()

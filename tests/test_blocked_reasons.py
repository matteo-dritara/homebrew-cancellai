"""Tests for what a blocked item has to say about itself (E30-S01).

`blocked` existed as a status with ten sibling fields and none of them said why. The two stories
gating the Rust cutover - E06-S04 and E17-S07 - sat at `blocked` while every dependency they
declared was `done`, so the only honest reading of the control plane was that nothing held them.
It was false, and the reasons lived in the prose of `docs/development/RELEASE_GATES.md`, which had
already drifted: it named E16-S05 as outstanding long after E16-S05 closed.

The rule is deliberately narrow. A declared dependency that is not yet closed already says what
holds an item, and restating it in prose would create a second source of truth - the defect class
this repository keeps finding in itself. The field is required exactly where the dependencies
explain nothing.
"""

from __future__ import annotations

import unittest
from typing import Any, ClassVar

from scripts import project_os

CLOSED: set[str] = {"E99-S99"}
STATUS: dict[str, str] = {"E99-S99": "done", "E88": "planned"}


def item(**overrides: Any) -> dict[str, Any]:
    base: dict[str, Any] = {"id": "X-S01", "status": "blocked", "dependencies": ["E99-S99"]}
    base.update(overrides)
    return base


def errors(*items: dict[str, Any]) -> list[str]:
    return project_os.blocked_by_errors(list(items), "story", CLOSED, STATUS)


class ABlockNamesWhatHoldsIt(unittest.TestCase):
    def test_blocked_with_every_dependency_satisfied_and_no_reason_is_refused(self) -> None:
        """The exact shape E06-S04 and E17-S07 were in."""
        problems = errors(item())
        self.assertEqual(len(problems), 1)
        self.assertIn("nothing says what holds it", problems[0])

    def test_a_recorded_reason_makes_it_pass(self) -> None:
        reason = {
            "summary": "waiting on the Guardian runtime",
            "argument": "docs/development/RELEASE_GATES.md",
            "recorded": "2026-09-15",
        }
        self.assertEqual(errors(item(blocked_by=reason)), [])

    def test_an_open_dependency_is_reason_enough_on_its_own(self) -> None:
        """Restating it would be a second source of truth, which is the defect this avoids."""
        self.assertEqual(errors(item(dependencies=["E88"])), [])


class AStaleReasonIsWorseThanNone(unittest.TestCase):
    def test_an_unblocked_item_carrying_a_reason_is_refused(self) -> None:
        reason = {"summary": "an old reason", "argument": "AGENTS.md", "recorded": "2026-01-01"}
        for status in ("planned", "in_progress", "ready_for_review", "done"):
            problems = errors(item(status=status, blocked_by=reason))
            self.assertEqual(len(problems), 1, status)
            self.assertIn("reads as current", problems[0])

    def test_waiting_on_something_that_has_closed_is_reported(self) -> None:
        """This is how the prose went wrong: E16-S05 stayed named long after it was done."""
        reason = {"summary": "s", "argument": "AGENTS.md", "recorded": "2026-01-01", "waiting_on": ["E99-S99"]}
        problems = errors(item(blocked_by=reason))
        self.assertEqual(len(problems), 1)
        self.assertIn("has closed", problems[0])

    def test_waiting_on_something_still_open_is_fine(self) -> None:
        reason = {"summary": "s", "argument": "AGENTS.md", "recorded": "2026-09-15", "waiting_on": ["E88"]}
        self.assertEqual(errors(item(blocked_by=reason)), [])

    def test_waiting_on_a_cancelled_work_item_is_refused(self) -> None:
        reason = {"summary": "s", "argument": "AGENTS.md", "recorded": "2026-09-15", "waiting_on": ["E99-S99"]}
        problems = project_os.blocked_by_errors([item(blocked_by=reason)], "story", {"E99-S99"}, {"E99-S99": "cancelled"})
        self.assertEqual(len(problems), 1)
        self.assertIn("has closed", problems[0])

    def test_a_recorded_reason_without_an_argument_is_refused(self) -> None:
        problems = errors(item(blocked_by={"summary": "s", "recorded": "2026-09-15"}))
        self.assertEqual(len(problems), 1)
        self.assertIn("names no argument", problems[0])

    def test_an_argument_that_does_not_name_a_file_is_refused(self) -> None:
        reason = {"summary": "s", "argument": "docs/no-such-file.md", "recorded": "2026-09-15"}
        problems = errors(item(blocked_by=reason))
        self.assertEqual(len(problems), 1)
        self.assertIn("not a readable repository file", problems[0])

    def test_waiting_on_an_unknown_work_item_is_refused(self) -> None:
        reason = {"summary": "s", "argument": "AGENTS.md", "recorded": "2026-09-15", "waiting_on": ["E77-S77"]}
        problems = errors(item(blocked_by=reason))
        self.assertEqual(len(problems), 1)
        self.assertIn("unknown work item", problems[0])


class TheCommittedControlPlane(unittest.TestCase):
    MODEL: ClassVar[Any] = project_os.load_model()

    def test_every_blocked_item_is_explained(self) -> None:
        project_os.validate(self.MODEL)  # raises if any block is unexplained

    def test_the_two_cutover_stories_record_a_real_argument(self) -> None:
        stories = {s["id"]: s for s in self.MODEL.stories}
        for story_id in ("E06-S04", "E17-S07"):
            recorded = stories[story_id].get("blocked_by")
            self.assertIsNotNone(recorded, f"{story_id} is the case this story exists for")
            assert recorded is not None
            self.assertEqual(recorded["argument"], "docs/development/RELEASE_GATES.md")
            self.assertTrue(recorded["summary"].strip())

    def test_no_recorded_blocker_waits_on_something_already_closed(self) -> None:
        closed = {e["id"] for e in self.MODEL.epics if e["status"] in project_os.CLOSED_EPIC_STATUS}
        closed |= {s["id"] for s in self.MODEL.stories if s["status"] == "done"}
        for record in list(self.MODEL.epics) + list(self.MODEL.stories):
            for waiting in (record.get("blocked_by") or {}).get("waiting_on", []):
                self.assertNotIn(waiting, closed, f"{record['id']} waits on closed {waiting}")


if __name__ == "__main__":
    unittest.main()

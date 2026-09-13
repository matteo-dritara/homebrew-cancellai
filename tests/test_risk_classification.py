"""Tests for the risk-classification floor (E25-S02).

The floor exists because `change_risk` is the one field every gate is downstream of, so the
cases below are about the ways a floor can fail to bind: a pattern that is defeated by adding a
narrower one, an attribution that guesses, a baseline that quietly becomes permanent, and an
override that asserts rather than argues.
"""

from __future__ import annotations

import unittest

from scripts import check_risk_classification as risk

FLOORS = [
    {"pattern": "kernel/src/*", "level": "CR4", "reason": "decides what is permitted"},
    {"pattern": "policy/src/*", "level": "CR3", "reason": "decides eligibility"},
    {"pattern": "kernel/src/notes.md", "level": "CR0", "reason": "a narrow low pattern next to a broad high one"},
]


class FloorSelectionTests(unittest.TestCase):
    def test_a_matching_path_sets_its_floor(self):
        floor = risk.floor_for(["kernel/src/lib.rs"], FLOORS)
        self.assertEqual("CR4", floor.level)
        self.assertEqual("kernel/src/lib.rs", floor.path)

    def test_the_highest_floor_wins_not_the_most_specific(self):
        # A most-specific rule could be defeated by adding a narrow low-level pattern beside a
        # broad high-level one, which is the cheapest way to weaken a floor without touching it.
        floor = risk.floor_for(["kernel/src/notes.md"], FLOORS)
        self.assertEqual("CR4", floor.level)

    def test_the_highest_floor_across_several_paths_wins(self):
        floor = risk.floor_for(["docs/x.md", "policy/src/a.rs", "kernel/src/b.rs"], FLOORS)
        self.assertEqual("CR4", floor.level)

    def test_a_change_touching_no_authority_surface_has_no_floor(self):
        self.assertIsNone(risk.floor_for(["docs/x.md", "README.md"], FLOORS))

    def test_ranking_is_ordered_and_rejects_nonsense(self):
        self.assertLess(risk.rank("CR1"), risk.rank("CR4"))
        self.assertEqual(-1, risk.rank("CR9"))


class ConfigValidationTests(unittest.TestCase):
    def test_the_real_configuration_loads(self):
        data = risk.load_floors()
        self.assertTrue(data["floors"])

    def test_every_real_floor_states_a_reason(self):
        for entry in risk.load_floors()["floors"]:
            with self.subTest(pattern=entry["pattern"]):
                self.assertTrue(entry["reason"].strip())

    def test_every_baseline_entry_names_a_real_story(self):
        stories = risk.load_stories()
        for entry in risk.load_floors().get("baseline", []):
            with self.subTest(story=entry["story"]):
                self.assertIn(entry["story"], stories)


EVAL_STORIES = {
    "E01-S01": {"id": "E01-S01", "change_risk": "CR1"},
    "E01-S02": {"id": "E01-S02", "change_risk": "CR4"},
}
EVAL_ATTRIBUTED = {"E01-S01": {"kernel/src/lib.rs"}, "E01-S02": {"kernel/src/lib.rs"}}
SUMMARY_STORIES = {"E01-S01": {"change_risk": "CR1"}, "E01-S02": {"change_risk": "CR4"}}


class EvaluationTests(unittest.TestCase):
    def evaluate(self, **extra):
        data = {"floors": FLOORS, "overrides": [], "baseline": [], "independent_classifications": [], **extra}
        return risk.evaluate(EVAL_STORIES, EVAL_ATTRIBUTED, data)

    def test_a_declaration_below_the_floor_is_an_error(self):
        errors, warnings = self.evaluate()
        self.assertTrue(any("E01-S01" in e for e in errors))
        self.assertEqual([], warnings)

    def test_a_declaration_at_or_above_the_floor_passes(self):
        errors, _ = self.evaluate()
        self.assertFalse(any("E01-S02" in e for e in errors))

    def test_a_baseline_entry_demotes_the_error_to_a_visible_warning(self):
        # Demoted, never removed: a baseline printed on every run is a decision the owner still
        # owes, where a silent exemption is a decision nobody will ever make.
        errors, warnings = self.evaluate(baseline=[{"story": "E01-S01", "note": "recorded at introduction"}])
        self.assertEqual([], errors)
        self.assertTrue(any("E01-S01" in w and "baseline" in w for w in warnings))

    def test_an_override_with_a_reason_is_accepted_and_still_reported(self):
        errors, warnings = self.evaluate(overrides=[{"story": "E01-S01", "reason": "the file is a doc comment only"}])
        self.assertEqual([], errors)
        self.assertTrue(any("override" in w for w in warnings))

    def test_an_override_with_no_reason_is_refused(self):
        errors, _ = self.evaluate(overrides=[{"story": "E01-S01", "reason": ""}])
        self.assertTrue(any("records no reason" in e for e in errors))

    def test_an_independent_classification_must_name_its_classifier(self):
        errors, _ = self.evaluate(independent_classifications=[{"story": "E01-S02", "level": "CR4", "classifier": ""}])
        self.assertTrue(any("records no classifier" in e for e in errors))

    def test_an_independent_classification_of_an_unknown_story_is_refused(self):
        errors, _ = self.evaluate(independent_classifications=[{"story": "E99-S99", "level": "CR4", "classifier": "codex"}])
        self.assertTrue(any("unknown story" in e for e in errors))


class DisagreementSummaryTests(unittest.TestCase):
    def test_no_classifications_says_so_rather_than_reporting_agreement(self):
        # Zero recorded classifications and zero disagreements are not the same fact, and a
        # summary that conflated them would read as "the second opinion always agrees".
        summary = risk.disagreement_summary(SUMMARY_STORIES, {"independent_classifications": []})
        self.assertIn("recorded: 0", summary)
        self.assertIn("No story has been classified twice", summary)

    def test_a_disagreement_is_named_with_both_levels(self):
        data = {"independent_classifications": [{"story": "E01-S01", "level": "CR4", "classifier": "codex"}]}
        summary = risk.disagreement_summary(SUMMARY_STORIES, data)
        self.assertIn("declared CR1, independently classified CR4", summary)

    def test_a_zero_disagreement_rate_over_many_stories_is_itself_flagged(self):
        stories = {f"E01-S{n:02d}": {"change_risk": "CR2"} for n in range(1, 13)}
        data = {"independent_classifications": [{"story": sid, "level": "CR2", "classifier": "codex"} for sid in stories]}
        summary = risk.disagreement_summary(stories, data)
        self.assertIn("is not independent", summary)


def skip_without_history(case: unittest.TestCase) -> None:
    """Skip a test that reasons over git history when this checkout does not have it.

    A shallow clone is not "no history": `git log` answers, and answers wrongly - a depth-1 tip
    has no parent object, so its diff is the whole tree. The checker refuses that outright, and a
    test asserting the committed state must skip rather than read the refusal as a defect. CI now
    fetches full history for the jobs that run this gate; this keeps the suite honest anywhere
    else it is run.
    """
    if risk.is_shallow():
        case.skipTest("shallow clone; attribution needs the history it cannot see")


class AttributionTests(unittest.TestCase):
    def test_attribution_reports_what_it_could_not_attribute(self):
        skip_without_history(self)
        attributed, ambiguous = risk.attributable_paths()
        if not attributed and not ambiguous:
            # A source export has no `.git`, so there is nothing to attribute and the claim is
            # vacuous rather than false. Asserting unconditionally made the whole suite fail in a
            # released tarball - found by the gate-sensitivity control experiment, which runs the
            # suite against a git-less copy and reported `pytest` as failing on a clean tree,
            # which would have made every kill it recorded worthless.
            self.skipTest("no git history available; attribution has nothing to read")
        # This repository batches stories into commits routinely; a checker that hid that would
        # report a clean run over a third of the backlog as if it covered all of it.
        self.assertGreater(ambiguous, 0)
        self.assertGreater(len(attributed), 0)

    def test_every_attributed_story_id_has_the_expected_shape(self):
        skip_without_history(self)
        for story_id in risk.attributable_paths()[0]:
            with self.subTest(story=story_id):
                self.assertRegex(story_id, r"^E\d{2}-S\d{2}$")


class StoryIdExtractionTests(unittest.TestCase):
    """Shorthand must not make a batched commit look unambiguous."""

    def test_a_single_story_is_found(self):
        self.assertEqual({"E25-S04"}, risk.story_ids("feat(x): thing (E25-S04)"))

    def test_shorthand_expands_to_every_story_it_names(self):
        # Found by this checker on its own commit: `E25-S04/S05/S10` matched one id, so a
        # three-story commit read as unambiguous and every file in it was attributed to the first.
        self.assertEqual({"E25-S04", "E25-S05", "E25-S10"}, risk.story_ids("thing (E25-S04/S05/S10)"))

    def test_shorthand_and_full_ids_combine(self):
        self.assertEqual(
            {"E24-S01", "E25-S02", "E25-S03"},
            risk.story_ids("body mentions E24-S01 and the subject says (E25-S02/S03)"),
        )

    def test_a_trailer_is_authoritative_over_prose(self):
        # A commit mentioning another story is not a commit belonging to it. This gate refused a
        # correct commit whose body explained which earlier story added the mechanism it used.
        message = "fix(policy): a thing\n\nThis uses the override E25-S02 added.\n\nStory: E06-S05\n"
        self.assertEqual({"E06-S05"}, risk.story_ids(message))

    def test_a_trailer_may_name_several_stories(self):
        self.assertEqual({"E25-S04", "E25-S05"}, risk.story_ids("subject\n\nStory: E25-S04, E25-S05\n"))

    def test_shorthand_in_a_trailer_expands(self):
        self.assertEqual({"E25-S04", "E25-S05", "E25-S10"}, risk.story_ids("subject\n\nStory: E25-S04/S05/S10\n"))

    def test_without_a_trailer_prose_is_still_read(self):
        self.assertEqual({"E25-S04"}, risk.story_ids("feat(x): thing (E25-S04)"))

    def test_a_message_naming_nothing_yields_nothing(self):
        self.assertEqual(set(), risk.story_ids("chore: tidy up"))


class CommandTests(unittest.TestCase):
    def test_check_passes_on_the_committed_state(self):
        skip_without_history(self)
        self.assertEqual(0, risk.main(["check"]))

    def test_check_refuses_a_shallow_clone_rather_than_attributing_everything_to_one_story(self):
        # The failure this came from: CI ran this gate against a depth-1 checkout, `git log`
        # reported all 539 tracked files as the tip commit's diff, and the gate refused a kernel
        # crate's CR4 floor for a story that touched no Rust at all. The same defect pointing the
        # other way is a silent pass, which is why it refuses instead of degrading.
        original = risk.git
        try:
            risk.git = lambda *args: "true" if args[:2] == ("rev-parse", "--is-shallow-repository") else original(*args)  # type: ignore[assignment]
            with self.assertRaises(risk.RiskClassificationError) as caught:
                risk.attributable_paths()
        finally:
            risk.git = original  # type: ignore[assignment]
        self.assertIn("fetch-depth", str(caught.exception).lower())

    def test_floor_reports_a_kernel_path(self):
        self.assertEqual(0, risk.main(["floor", "rust/crates/cancellai-safety/src/lib.rs"]))

    def test_floor_reports_no_floor_for_documentation(self):
        self.assertEqual(0, risk.main(["floor", "docs/INDEX.md"]))


if __name__ == "__main__":
    unittest.main()

from __future__ import annotations

import copy
import tempfile
import unittest
from pathlib import Path

from scripts import project_os


class ProjectOSTests(unittest.TestCase):
    def test_repository_control_plane_is_valid(self) -> None:
        model = project_os.load_model()
        warnings = project_os.validate(model)
        self.assertEqual([], warnings)
        # Assert the shape of the register, not a magic count that has to be bumped by
        # every decision - a test that must be edited to accept new data tests nothing.
        decision_ids = [item["id"] for item in model.decisions["decisions"]]
        self.assertEqual(decision_ids, sorted(decision_ids))
        self.assertEqual(len(decision_ids), len(set(decision_ids)))
        self.assertGreaterEqual(len(decision_ids), 18)
        self.assertGreaterEqual(len(model.epics), 20)
        self.assertGreaterEqual(len(model.stories), 80)

    def test_generated_docs_have_no_drift(self) -> None:
        model = project_os.load_model()
        project_os.validate(model)
        project_os.check_generated(model)

    def test_duplicate_story_is_rejected(self) -> None:
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        epics[0]["stories"].append(copy.deepcopy(epics[0]["stories"][0]))
        bad = project_os.Model(model.decisions, model.roadmap, epics)
        with self.assertRaises(project_os.GovernanceError):
            project_os.validate(bad)

    def test_cr4_without_safety_obligation_is_rejected(self) -> None:
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        target = next(s for e in epics for s in e["stories"] if s["change_risk"] == "CR4")
        target["safety_obligations"] = []
        bad = project_os.Model(model.decisions, model.roadmap, epics)
        with self.assertRaises(project_os.GovernanceError):
            project_os.validate(bad)

    def test_unknown_dependency_is_rejected(self) -> None:
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        epics[0]["stories"][0]["dependencies"] = ["E99-S99"]
        bad = project_os.Model(model.decisions, model.roadmap, epics)
        with self.assertRaises(project_os.GovernanceError):
            project_os.validate(bad)

    def test_unknown_safety_obligation_is_rejected(self) -> None:
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        epics[0]["stories"][0]["safety_obligations"] = ["SI-999"]
        bad = project_os.Model(model.decisions, model.roadmap, epics)
        with self.assertRaises(project_os.GovernanceError):
            project_os.validate(bad)

    def test_dependency_cycle_is_rejected(self) -> None:
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        first = epics[0]["stories"][0]
        second = epics[0]["stories"][1]
        first["dependencies"] = [second["id"]]
        second["dependencies"] = [first["id"]]
        bad = project_os.Model(model.decisions, model.roadmap, epics)
        with self.assertRaises(project_os.GovernanceError):
            project_os.validate(bad)

    def test_ready_story_with_unfinished_dependency_is_rejected(self) -> None:
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        # Pick a story that really has story dependencies rather than relying on
        # position, so adding a story to the first epic cannot silence this test.
        target = next(
            story for epic in epics for story in epic["stories"] if any(dep.startswith(f"{epic['id']}-S") for dep in story["dependencies"])
        )
        target["status"] = "ready"
        bad = project_os.Model(model.decisions, model.roadmap, epics)
        with self.assertRaises(project_os.GovernanceError):
            project_os.validate(bad)

    def test_ready_for_review_requires_committed_executor_evidence(self) -> None:
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        # An epic with no committed evidence at all, so neither the story-level nor the
        # epic-level batch lookup can satisfy the gate.
        evidence_root = project_os.PROJECT / "evidence"
        epic = next(e for e in epics if not list(evidence_root.glob(f"{e['id']}-*.md")))
        target = epic["stories"][0]
        target["status"] = "ready_for_review"
        target["dependencies"] = []
        epic["dependencies"] = []
        bad = project_os.Model(model.decisions, model.roadmap, epics)
        with self.assertRaises(project_os.GovernanceError):
            project_os.validate(bad)

    def test_evidence_gate_rejects_an_empty_or_unrelated_file(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            empty = base / "E00-EMPTY.md"
            empty.write_text("# nothing\n", encoding="utf-8")
            unrelated = base / "E00-OTHER.md"
            unrelated.write_text("x" * (project_os.MIN_EVIDENCE_BYTES + 10), encoding="utf-8")
            good = base / "E00-GOOD.md"
            good.write_text("E00-S01 evidence. " + "y" * project_os.MIN_EVIDENCE_BYTES, encoding="utf-8")
            self.assertFalse(project_os.evidence_is_substantive(empty, "E00-S01"))
            self.assertFalse(project_os.evidence_is_substantive(unrelated, "E00-S01"))
            self.assertFalse(project_os.evidence_is_substantive(base / "missing.md", "E00-S01"))
            self.assertTrue(project_os.evidence_is_substantive(good, "E00-S01"))

    def test_ready_for_review_does_not_require_a_safety_verdict(self) -> None:
        # The Safety Verdict is the reviewer's output; demanding it at handoff would force
        # the executor to sign off on its own CR4 work.
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        cr4 = next(s for epic in epics for s in epic["stories"] if s["change_risk"] == "CR4")
        cr4["status"] = "ready_for_review"
        ready_for_review = project_os.Model(model.decisions, model.roadmap, epics)
        project_os.validate(ready_for_review)

    def test_dependency_on_later_phase_is_rejected(self) -> None:
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        p0_story = epics[0]["stories"][0]
        later_story = next(s for e in epics if e["phase"] == "P1" for s in e["stories"])
        p0_story["dependencies"] = [later_story["id"]]
        bad = project_os.Model(model.decisions, model.roadmap, epics)
        with self.assertRaises(project_os.GovernanceError):
            project_os.validate(bad)

    def test_story_lookup_returns_known_story(self) -> None:
        model = project_os.load_model()
        story = project_os.story_by_id(model, "E00-S01")
        self.assertEqual(story["id"], "E00-S01")


if __name__ == "__main__":
    unittest.main()

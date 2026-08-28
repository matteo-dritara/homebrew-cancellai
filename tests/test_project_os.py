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

    @staticmethod
    def _fully_isolated_epics(model: project_os.Model) -> list[dict]:
        """A deepcopy of the real epics with every status/dependency edge zeroed out.

        The real control plane keeps changing shape as work lands (new epics depending on
        E01 at the epic level, new stories depending on specific E01 stories, ...). A
        synthetic scenario that leaves any of that untouched is one topology change away
        from failing for an unrelated reason - this happened twice already for these two
        tests. Starting from a fully neutral copy and wiring in only the one edge under
        test is the version of this fixture that cannot break again the same way.
        """
        epics = copy.deepcopy(model.epics)
        for epic in epics:
            epic["status"] = "planned"
            epic["dependencies"] = []
            for story in epic["stories"]:
                story["status"] = "planned"
                story["dependencies"] = []
        return epics

    def test_same_epic_dependency_satisfied_by_ready_for_review(self) -> None:
        # ADR-0014: review is per epic, not story by story. A story chained to a same-epic
        # predecessor must be able to start (and reach ready_for_review itself) once that
        # predecessor is ready_for_review - it does not have to wait for independent "done",
        # since the whole epic is verified and closed together.
        model = project_os.load_model()
        epics = self._fully_isolated_epics(model)
        epic = next(e for e in epics if e["id"] == "E01")
        predecessor = next(s for s in epic["stories"] if s["id"] == "E01-S01")
        dependent = next(s for s in epic["stories"] if s["id"] == "E01-S02")
        dependent["dependencies"] = [predecessor["id"]]
        predecessor["status"] = "ready_for_review"
        # in_progress (not ready_for_review) so this exercises only the dependency gate,
        # not the separate evidence-packet requirement.
        dependent["status"] = "in_progress"
        candidate = project_os.Model(model.decisions, model.roadmap, epics)
        project_os.validate(candidate)  # must not raise

    def test_same_epic_dependency_still_blocks_when_predecessor_is_unfinished(self) -> None:
        model = project_os.load_model()
        epics = self._fully_isolated_epics(model)
        epic = next(e for e in epics if e["id"] == "E01")
        predecessor = next(s for s in epic["stories"] if s["id"] == "E01-S01")
        dependent = next(s for s in epic["stories"] if s["id"] == "E01-S02")
        dependent["dependencies"] = [predecessor["id"]]
        predecessor["status"] = "in_progress"
        dependent["status"] = "in_progress"
        candidate = project_os.Model(model.decisions, model.roadmap, epics)
        with self.assertRaises(project_os.GovernanceError):
            project_os.validate(candidate)

    def test_cross_epic_dependency_still_requires_done(self) -> None:
        # The ready_for_review relaxation is scoped to same-epic story chains. A dependency
        # on a story in a *different* epic must still be "done" - that epic is independently
        # closed and released (ADR-0014), not batch-reviewed alongside the dependent epic.
        # Two same-phase planned epics, neither touched at the epic-status level, so only the
        # story-dependency branch under test can produce the error.
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        upstream_epic = next(e for e in epics if e["id"] == "E02")
        downstream_epic = next(e for e in epics if e["id"] == "E03")
        self.assertEqual(upstream_epic["phase"], downstream_epic["phase"])
        upstream = upstream_epic["stories"][0]
        downstream = downstream_epic["stories"][0]
        downstream["dependencies"] = [upstream["id"]]
        upstream["status"] = "ready_for_review"
        downstream["status"] = "in_progress"
        candidate = project_os.Model(model.decisions, model.roadmap, epics)
        with self.assertRaises(project_os.GovernanceError):
            project_os.validate(candidate)

    def test_ready_for_review_requires_committed_executor_evidence(self) -> None:
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        # An epic with no committed evidence at all, so neither the story-level nor the
        # epic-level batch lookup can satisfy the gate. Story-level evidence lives under
        # project/evidence/<story-id>/, so check every story's own subdirectory too, not
        # just the flat "<epic-id>-*.md" batch-file pattern.
        evidence_root = project_os.PROJECT / "evidence"

        def has_evidence(epic: dict[str, object]) -> bool:
            if list(evidence_root.glob(f"{epic['id']}-*.md")):
                return True
            return any(list((evidence_root / story["id"]).glob("*.md")) for story in epic["stories"])

        epic = next(e for e in epics if not has_evidence(e))
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
            filler = base / "E00-FILLER.md"
            filler.write_text("E00-S01 " + "y" * project_os.MIN_EVIDENCE_BYTES, encoding="utf-8")
            good = base / "E00-GOOD.md"
            good.write_text(
                "# E00-S01\n\n## Outcome\nPASS\n\n## Verification\ntests\n\n## Residual risks\nnone\n" + "y" * project_os.MIN_EVIDENCE_BYTES,
                encoding="utf-8",
            )
            self.assertFalse(project_os.evidence_is_substantive(empty, "E00-S01"))
            self.assertFalse(project_os.evidence_is_substantive(unrelated, "E00-S01"))
            self.assertFalse(project_os.evidence_is_substantive(base / "missing.md", "E00-S01"))
            # Size plus a story id is filler, not evidence: it says nothing about outcome.
            self.assertFalse(project_os.evidence_is_substantive(filler, "E00-S01"))
            self.assertTrue(project_os.evidence_is_substantive(good, "E00-S01"))
            self.assertTrue(project_os.evidence_states_residual_risk(good))
            self.assertFalse(project_os.evidence_states_residual_risk(filler))

    def test_a_cr4_story_cannot_close_over_a_failing_safety_verdict(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            failing = base / "SAFETY_VERDICT.md"
            failing.write_text("# Safety Verdict - E00-S01\n\n## Verdict\n\n`FAIL`\n", encoding="utf-8")
            passing = base / "SAFETY_VERDICT_OK.md"
            passing.write_text("# Safety Verdict - E00-S01\n\n## Verdict\n\n`PASS`\n", encoding="utf-8")
            mixed = base / "SAFETY_VERDICT_MIXED.md"
            mixed.write_text("## Verdict\n\n`PASS`\n\n## Owner decision\n\n`REJECT`\n", encoding="utf-8")
            self.assertFalse(project_os.safety_verdict_passes(failing))
            self.assertTrue(project_os.safety_verdict_passes(passing))
            # A pass that is overridden by a rejection elsewhere in the file is not a pass.
            self.assertFalse(project_os.safety_verdict_passes(mixed))

    def test_ready_for_review_does_not_require_a_safety_verdict(self) -> None:
        # The Safety Verdict is the reviewer's output; demanding it at handoff would force
        # the executor to sign off on its own CR4 work.
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        target = next(s for epic in epics for s in epic["stories"] if s["change_risk"] == "CR4")
        target["status"] = "ready_for_review"

        # Reopening a story reopens everything downstream of it. Build the state a real
        # reopening would produce, transitively, rather than an inconsistent one the
        # validator would reject for reasons unrelated to what this test asserts.
        def reopen_dependents() -> bool:
            done_ids = {epic["id"] for epic in epics if epic["status"] == "done"}
            done_ids |= {s["id"] for epic in epics for s in epic["stories"] if s["status"] == "done"}
            changed = False
            for epic in epics:
                if epic["status"] != "planned" and not set(epic["dependencies"]) <= done_ids:
                    epic["status"] = "planned"
                    changed = True
                if epic["status"] == "done" and any(s["status"] != "done" for s in epic["stories"]):
                    epic["status"] = "in_progress"
                    changed = True
                for story in epic["stories"]:
                    if story["status"] != "planned" and story is not target and not set(story["dependencies"]) <= done_ids:
                        story["status"] = "planned"
                        changed = True
            return changed

        while reopen_dependents():
            pass

        ready_for_review = project_os.Model(model.decisions, model.roadmap, epics)
        project_os.validate(ready_for_review)

    def test_an_epic_cannot_be_done_while_a_story_is_open(self) -> None:
        model = project_os.load_model()
        epics = copy.deepcopy(model.epics)
        closed = next(epic for epic in epics if epic["status"] == "done")
        closed["stories"][0]["status"] = "in_progress"
        with self.assertRaises(project_os.GovernanceError):
            project_os.validate(project_os.Model(model.decisions, model.roadmap, epics))

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

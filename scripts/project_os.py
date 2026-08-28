#!/usr/bin/env python3
"""cancellAI Engineering OS control-plane validator and doc generator.

This script intentionally uses only the Python standard library so the project
control plane stays usable during the Python -> Rust migration.

Commands:
  python3 scripts/project_os.py check
  python3 scripts/project_os.py generate
  python3 scripts/project_os.py status
  python3 scripts/project_os.py next
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
PROJECT = ROOT / "project"
DOCS = ROOT / "docs"

VALID_DECISION_STATUS = {"proposed", "accepted", "superseded", "deprecated"}
VALID_EPIC_STATUS = {"planned", "ready", "in_progress", "ready_for_review", "verification", "blocked", "done", "cancelled"}
VALID_STORY_STATUS = VALID_EPIC_STATUS
VALID_RISKS = {"CR0", "CR1", "CR2", "CR3", "CR4"}
# ADR-0014: review is per epic, at most twice, not story by story. A same-epic dependency is
# therefore satisfied once the predecessor has reached ready_for_review, not "done" - the whole
# epic is verified and closed together. Cross-epic and epic-level dependencies still require "done".
SAME_EPIC_DEPENDENCY_SATISFIED_STATUSES = {"ready_for_review", "verification", "done"}
DECISION_ID_RE = re.compile(r"^PD-\d{3}$")
EPIC_ID_RE = re.compile(r"^E\d{2}$")
STORY_ID_RE = re.compile(r"^E\d{2}-S\d{2}$")
SAFETY_ID_RE = re.compile(r"^###\s+(SI-\d{3})\b", re.MULTILINE)
# An evidence file must actually say something about the story it is offered for. Size alone
# is filler-shaped, so the gate also requires the sections the evidence template defines:
# what the outcome was, how it was verified, and what risk remains.
MIN_EVIDENCE_BYTES = 400
# An evidence file has to say what happened and how that was established. Both groups must
# appear; "residual risk" is strongly expected but is a warning, because a genuine PASS may
# have none and forcing the phrase would only teach people to paste it.
EVIDENCE_OUTCOME_TERMS = ("verdict", "outcome")
EVIDENCE_METHOD_TERMS = ("verification", "evidence", "test")
EVIDENCE_RESIDUAL_TERMS = ("residual", "known risk")
# A Safety Verdict file satisfying the CR4 gate must actually record a passing verdict. A
# committed FAIL is evidence that the story is not finished, not evidence that it is.
FAILING_VERDICT_RE = re.compile(r"^\s*`?(FAIL|REJECT)`?\s*$", re.MULTILINE | re.IGNORECASE)
PASSING_VERDICT_RE = re.compile(r"^\s*`?(PASS|PASS_WITH_RESIDUALS)`?\s*$", re.MULTILINE | re.IGNORECASE)


class GovernanceError(RuntimeError):
    pass


@dataclass(frozen=True)
class Model:
    decisions: dict[str, Any]
    roadmap: dict[str, Any]
    epics: list[dict[str, Any]]

    @property
    def stories(self) -> list[dict[str, Any]]:
        return [story for epic in self.epics for story in epic["stories"]]


def load_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise GovernanceError(f"missing required file: {path.relative_to(ROOT)}") from exc
    except json.JSONDecodeError as exc:
        raise GovernanceError(f"invalid JSON in {path.relative_to(ROOT)}: {exc}") from exc
    if not isinstance(data, dict):
        raise GovernanceError(f"expected JSON object in {path.relative_to(ROOT)}")
    return data


def load_model() -> Model:
    decisions = load_json(PROJECT / "decisions.json")
    roadmap = load_json(PROJECT / "roadmap.json")
    epic_paths = roadmap.get("epic_files")
    if not isinstance(epic_paths, list) or not epic_paths:
        raise GovernanceError("project/roadmap.json must contain non-empty epic_files")
    epics: list[dict[str, Any]] = []
    for rel in epic_paths:
        if not isinstance(rel, str):
            raise GovernanceError("roadmap epic_files entries must be strings")
        epics.append(load_json(ROOT / rel))
    return Model(decisions=decisions, roadmap=roadmap, epics=epics)


def safety_invariant_ids() -> set[str]:
    path = DOCS / "security" / "SAFETY_INVARIANTS.md"
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise GovernanceError("missing docs/security/SAFETY_INVARIANTS.md") from exc
    return set(SAFETY_ID_RE.findall(text))


def evidence_is_substantive(path: Path, story_id: str) -> bool:
    """An evidence file counts only if it follows the template and names the story.

    Without this the gate is satisfied by any Markdown filename, which makes the handoff
    requirement ceremonial. Requiring the template's sections is what turns "a file exists"
    into "someone recorded an outcome, how it was verified, and what risk remains".
    """
    try:
        if not path.is_file() or path.stat().st_size < MIN_EVIDENCE_BYTES:
            return False
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return False
    if story_id not in text:
        return False
    lowered = text.lower()
    return any(term in lowered for term in EVIDENCE_OUTCOME_TERMS) and any(term in lowered for term in EVIDENCE_METHOD_TERMS)


def safety_verdict_passes(path: Path) -> bool:
    """Whether a Safety Verdict records a pass rather than merely existing.

    Checking only for a file named "verdict" lets a rejected story be marked done while the
    rejection sits next to it in the repository.
    """
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return False
    return bool(PASSING_VERDICT_RE.search(text)) and not FAILING_VERDICT_RE.search(text)


def evidence_states_residual_risk(path: Path) -> bool:
    try:
        lowered = path.read_text(encoding="utf-8", errors="replace").lower()
    except OSError:
        return False
    return any(term in lowered for term in EVIDENCE_RESIDUAL_TERMS)


def assert_acyclic(graph: dict[str, list[str]]) -> None:
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node: str, trail: list[str]) -> None:
        if node in visited:
            return
        if node in visiting:
            try:
                start = trail.index(node)
                cycle = [*trail[start:], node]
            except ValueError:
                cycle = [*trail, node]
            raise GovernanceError("dependency cycle: " + " -> ".join(cycle))
        visiting.add(node)
        for dep in graph.get(node, []):
            visit(dep, [*trail, node])
        visiting.remove(node)
        visited.add(node)

    for node in graph:
        visit(node, [])


def story_by_id(model: Model, story_id: str) -> dict[str, Any]:
    for story in model.stories:
        if story["id"] == story_id:
            return story
    raise GovernanceError(f"unknown story id: {story_id}")


def require_string(obj: dict[str, Any], key: str, where: str) -> str:
    value = obj.get(key)
    if not isinstance(value, str) or not value.strip():
        raise GovernanceError(f"{where}: {key} must be a non-empty string")
    return value


def require_string_list(obj: dict[str, Any], key: str, where: str, *, allow_empty: bool = True) -> list[str]:
    value = obj.get(key)
    if not isinstance(value, list) or (not allow_empty and not value):
        qualifier = "non-empty " if not allow_empty else ""
        raise GovernanceError(f"{where}: {key} must be a {qualifier}list")
    if not all(isinstance(item, str) and item.strip() for item in value):
        raise GovernanceError(f"{where}: {key} must contain only non-empty strings")
    return value


def validate(model: Model) -> list[str]:
    warnings: list[str] = []
    if model.decisions.get("schema_version") != 1 or model.decisions.get("project") != "cancellAI":
        raise GovernanceError("project/decisions.json must use schema_version=1 and project=cancellAI")
    if model.roadmap.get("schema_version") != 1 or model.roadmap.get("project") != "cancellAI":
        raise GovernanceError("project/roadmap.json must use schema_version=1 and project=cancellAI")
    known_safety = safety_invariant_ids()
    if not known_safety:
        raise GovernanceError("no Safety Invariants found")

    decisions = model.decisions.get("decisions")
    if not isinstance(decisions, list) or not decisions:
        raise GovernanceError("project/decisions.json: decisions must be a non-empty list")
    decision_ids: set[str] = set()
    for item in decisions:
        if not isinstance(item, dict):
            raise GovernanceError("decision entries must be objects")
        did = require_string(item, "id", "decision")
        if not DECISION_ID_RE.fullmatch(did):
            raise GovernanceError(f"invalid decision id: {did}")
        if did in decision_ids:
            raise GovernanceError(f"duplicate decision id: {did}")
        decision_ids.add(did)
        status = require_string(item, "status", did)
        if status not in VALID_DECISION_STATUS:
            raise GovernanceError(f"{did}: invalid status {status}")
        require_string(item, "title", did)
        require_string(item, "decision", did)
        require_string(item, "rationale", did)
        require_string_list(item, "implications", did, allow_empty=False)

    phases = model.roadmap.get("phases")
    if not isinstance(phases, list) or not phases:
        raise GovernanceError("roadmap phases must be a non-empty list")
    phase_ids: set[str] = set()
    phase_epics: list[str] = []
    for phase in phases:
        if not isinstance(phase, dict):
            raise GovernanceError("roadmap phase entries must be objects")
        pid = require_string(phase, "id", "phase")
        if pid in phase_ids:
            raise GovernanceError(f"duplicate phase id: {pid}")
        phase_ids.add(pid)
        require_string(phase, "name", pid)
        require_string(phase, "goal", pid)
        require_string_list(phase, "exit_criteria", pid, allow_empty=False)
        phase_epics.extend(require_string_list(phase, "epics", pid, allow_empty=False))

    epic_ids: set[str] = set()
    story_ids: set[str] = set()
    for epic in model.epics:
        if epic.get("schema_version") != 1:
            raise GovernanceError("epic files must use schema_version=1")
        eid = require_string(epic, "id", "epic")
        if not EPIC_ID_RE.fullmatch(eid):
            raise GovernanceError(f"invalid epic id: {eid}")
        if eid in epic_ids:
            raise GovernanceError(f"duplicate epic id: {eid}")
        epic_ids.add(eid)
        phase = require_string(epic, "phase", eid)
        if phase not in phase_ids:
            raise GovernanceError(f"{eid}: unknown phase {phase}")
        status = require_string(epic, "status", eid)
        if status not in VALID_EPIC_STATUS:
            raise GovernanceError(f"{eid}: invalid status {status}")
        require_string(epic, "title", eid)
        require_string(epic, "objective", eid)
        require_string_list(epic, "dependencies", eid)
        stories = epic.get("stories")
        if not isinstance(stories, list) or not stories:
            raise GovernanceError(f"{eid}: stories must be a non-empty list")
        for story in stories:
            if not isinstance(story, dict):
                raise GovernanceError(f"{eid}: story entries must be objects")
            sid = require_string(story, "id", eid)
            if not STORY_ID_RE.fullmatch(sid):
                raise GovernanceError(f"invalid story id: {sid}")
            if not sid.startswith(f"{eid}-S"):
                raise GovernanceError(f"{sid}: story id must be prefixed by {eid}-S")
            if sid in story_ids:
                raise GovernanceError(f"duplicate story id: {sid}")
            story_ids.add(sid)
            require_string(story, "title", sid)
            require_string(story, "outcome", sid)
            status = require_string(story, "status", sid)
            if status not in VALID_STORY_STATUS:
                raise GovernanceError(f"{sid}: invalid status {status}")
            risk = require_string(story, "change_risk", sid)
            if risk not in VALID_RISKS:
                raise GovernanceError(f"{sid}: invalid change_risk {risk}")
            require_string_list(story, "dependencies", sid)
            acceptance = require_string_list(story, "acceptance_criteria", sid, allow_empty=False)
            verification = require_string_list(story, "verification", sid, allow_empty=False)
            safety = require_string_list(story, "safety_obligations", sid)
            documentation = require_string_list(story, "documentation_impact", sid, allow_empty=False)
            unknown_safety = set(safety) - known_safety
            if unknown_safety:
                raise GovernanceError(f"{sid}: unknown safety obligation(s) {sorted(unknown_safety)}")
            for rel in documentation:
                if not (ROOT / rel).exists():
                    raise GovernanceError(f"{sid}: documentation impact target does not exist: {rel}")
            if len(acceptance) < 2:
                warnings.append(f"{sid}: fewer than 2 acceptance criteria")
            if risk == "CR4" and not safety:
                raise GovernanceError(f"{sid}: CR4 stories require explicit safety_obligations")
            if risk in {"CR3", "CR4"} and not verification:
                raise GovernanceError(f"{sid}: {risk} stories require verification")

    if set(phase_epics) != epic_ids:
        missing = epic_ids - set(phase_epics)
        unknown = set(phase_epics) - epic_ids
        raise GovernanceError(f"phase/epic mismatch: missing={sorted(missing)} unknown={sorted(unknown)}")
    if len(phase_epics) != len(set(phase_epics)):
        raise GovernanceError("an epic is listed in more than one roadmap phase")

    all_refs = epic_ids | story_ids
    phase_order = {phase["id"]: index for index, phase in enumerate(phases)}
    epic_phase = {epic["id"]: epic["phase"] for epic in model.epics}
    story_epic = {story["id"]: epic["id"] for epic in model.epics for story in epic["stories"]}
    epic_graph: dict[str, list[str]] = {}
    story_graph: dict[str, list[str]] = {}
    for epic in model.epics:
        eid = epic["id"]
        epic_graph[eid] = list(epic["dependencies"])
        for dep in epic["dependencies"]:
            if dep not in epic_ids:
                raise GovernanceError(f"{eid}: unknown epic dependency {dep}")
            if phase_order[epic_phase[dep]] > phase_order[epic["phase"]]:
                raise GovernanceError(f"{eid}: dependency {dep} belongs to a later roadmap phase")
        for story in epic["stories"]:
            story_graph[story["id"]] = list(story["dependencies"])
            for dep in story["dependencies"]:
                if dep not in all_refs:
                    raise GovernanceError(f"{story['id']}: unknown dependency {dep}")
                dependency_epic = dep if dep in epic_ids else story_epic[dep]
                if phase_order[epic_phase[dependency_epic]] > phase_order[epic["phase"]]:
                    raise GovernanceError(f"{story['id']}: dependency {dep} belongs to a later roadmap phase")
    assert_acyclic(epic_graph)
    # Story dependencies may reference an epic as a coarse prerequisite. Only
    # story-to-story edges participate in this graph.
    assert_acyclic({sid: [dep for dep in deps if dep in story_ids] for sid, deps in story_graph.items()})

    current = model.roadmap.get("current_phase")
    if current not in phase_ids:
        raise GovernanceError(f"roadmap current_phase is invalid: {current}")

    epic_status = {epic["id"]: epic["status"] for epic in model.epics}
    story_status = {story["id"]: story["status"] for story in model.stories}
    dependency_gated_statuses = {"ready", "in_progress", "ready_for_review", "verification", "done"}
    for epic in model.epics:
        if epic["status"] in dependency_gated_statuses:
            unfinished = [dep for dep in epic["dependencies"] if epic_status[dep] != "done"]
            if unfinished:
                raise GovernanceError(f"{epic['id']}: status {epic['status']} but epic dependencies are not done: {unfinished}")
        if epic["status"] == "done":
            # An epic closed over an unfinished story is a lie the roadmap would repeat.
            open_stories = [story["id"] for story in epic["stories"] if story["status"] not in {"done", "cancelled"}]
            if open_stories:
                raise GovernanceError(f"{epic['id']}: cannot be done while stories are open: {open_stories}")
        for story in epic["stories"]:
            if story["status"] in dependency_gated_statuses:
                unfinished_story_deps: list[str] = []
                for dep in story["dependencies"]:
                    if dep in story_ids and story_epic[dep] == epic["id"]:
                        # Review is per epic (ADR-0014): a same-epic predecessor only needs to
                        # have reached ready_for_review, since the whole epic is verified and
                        # closed together. A cross-epic/epic-level dependency still needs the
                        # independent close that "done" represents.
                        if story_status[dep] not in SAME_EPIC_DEPENDENCY_SATISFIED_STATUSES:
                            unfinished_story_deps.append(dep)
                    else:
                        dep_status = epic_status[dep] if dep in epic_status else story_status[dep]
                        if dep_status != "done":
                            unfinished_story_deps.append(dep)
                if unfinished_story_deps:
                    raise GovernanceError(f"{story['id']}: status {story['status']} but dependencies are not satisfied: {unfinished_story_deps}")
            if story["status"] in {"ready_for_review", "done"}:
                evidence_root = PROJECT / "evidence"
                candidates = list(evidence_root.glob(f"{story['id']}*.md"))
                story_evidence_dir = evidence_root / story["id"]
                if story_evidence_dir.is_dir():
                    candidates.extend(story_evidence_dir.glob("*.md"))
                if story["status"] == "ready_for_review":
                    # A review handoff may share one executor packet for a batch of stories.
                    # It is a candidate alongside story-level files rather than a fallback
                    # behind them: a story-level file from an earlier round is history, and
                    # history must not be able to satisfy the current handoff.
                    candidates.extend(evidence_root.glob(f"{epic['id']}-*.md"))
                evidence = [item for item in candidates if evidence_is_substantive(item, story["id"])]
                if not evidence:
                    raise GovernanceError(
                        f"{story['id']}: status {story['status']} requires committed evidence under project/evidence/ that "
                        f"names {story['id']}, is at least {MIN_EVIDENCE_BYTES} bytes, and states an outcome and how it "
                        "was established"
                    )
                if not any(evidence_states_residual_risk(item) for item in evidence):
                    warnings.append(f"{story['id']}: no evidence file states residual risk")
                # The Safety Verdict is the verifier's output, so it is required to close a
                # CR4 story, never to hand one over for review.
                verdicts = [item for item in evidence if "safety" in item.name.lower() or "verdict" in item.name.lower()]
                if story["status"] == "done" and story["change_risk"] == "CR4":
                    if not verdicts:
                        raise GovernanceError(f"{story['id']}: completed CR4 story requires an owner-visible Safety Verdict evidence file")
                    if not any(safety_verdict_passes(item) for item in verdicts):
                        raise GovernanceError(
                            f"{story['id']}: cannot be done - no committed Safety Verdict records PASS or "
                            "PASS_WITH_RESIDUALS, and at least one records FAIL/REJECT"
                        )

    return warnings


def decision_markdown(model: Model) -> str:
    lines = [
        "# Product Decision Register",
        "",
        "<!-- Generated by scripts/project_os.py from project/decisions.json. Do not edit by hand. -->",
        "",
        (
            "Accepted product decisions are constitutional inputs to architecture and roadmap work. "
            "Supersede them through a new ADR/decision record rather than silently editing their meaning."
        ),
        "",
    ]
    for item in model.decisions["decisions"]:
        lines.extend(
            [
                f"## {item['id']} - {item['title']}",
                "",
                f"**Status:** {item['status']}",
                "",
                item["decision"],
                "",
                f"**Rationale.** {item['rationale']}",
                "",
                "**Implications**",
                "",
            ]
        )
        lines.extend(f"- {x}" for x in item["implications"])
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def roadmap_markdown(model: Model) -> str:
    epics = {e["id"]: e for e in model.epics}
    lines = [
        "# Roadmap",
        "",
        "<!-- Generated by scripts/project_os.py from project/roadmap.json and project/epics/*.json. Do not edit by hand. -->",
        "",
        f"**North star:** {model.roadmap['north_star']}",
        "",
        f"**Current phase:** `{model.roadmap['current_phase']}`",
        "",
        (
            "The roadmap is capability- and evidence-gated, not date-driven. A later phase may be researched in parallel, "
            "but it cannot acquire release authority before its dependency gates are satisfied."
        ),
        "",
    ]
    for phase in model.roadmap["phases"]:
        lines.extend([f"## {phase['id']} - {phase['name']}", "", phase["goal"], "", "**Exit criteria**", ""])
        lines.extend(f"- {x}" for x in phase["exit_criteria"])
        lines.extend(["", "**Epics**", ""])
        for eid in phase["epics"]:
            e = epics[eid]
            lines.append(f"- **{eid} - {e['title']}** (`{e['status']}`): {e['objective']}")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def backlog_markdown(model: Model) -> str:
    lines = [
        "# Engineering Backlog",
        "",
        "<!-- Generated by scripts/project_os.py from project/epics/*.json. Do not edit by hand. -->",
        "",
        (
            "Every story is a work contract. Implementation may refine internals, but changing outcome, acceptance criteria, "
            "safety obligations, or risk level requires an explicit spec/ADR update before code changes are accepted."
        ),
        "",
    ]
    for epic in model.epics:
        deps = ", ".join(epic["dependencies"]) or "none"
        lines.extend(
            [
                f"## {epic['id']} - {epic['title']}",
                "",
                f"**Phase:** `{epic['phase']}` | **Status:** `{epic['status']}` | **Epic dependencies:** {deps}",
                "",
                epic["objective"],
                "",
            ]
        )
        for s in epic["stories"]:
            deps = ", ".join(s["dependencies"]) or "none"
            safety = ", ".join(s["safety_obligations"]) or "none"
            lines.extend(
                [
                    f"### {s['id']} - {s['title']}",
                    "",
                    f"**Status:** `{s['status']}` | **Change Risk:** `{s['change_risk']}` "
                    f"| **Dependencies:** {deps} | **Safety obligations:** {safety}",
                    "",
                    f"**Outcome.** {s['outcome']}",
                    "",
                    "**Acceptance criteria**",
                    "",
                ]
            )
            lines.extend(f"- {x}" for x in s["acceptance_criteria"])
            lines.extend(["", "**Verification**", ""])
            lines.extend(f"- {x}" for x in s["verification"])
            lines.extend(["", "**Documentation impact**", ""])
            lines.extend(f"- `{x}`" for x in s["documentation_impact"])
            lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def project_status_markdown(model: Model) -> str:
    stories = model.stories
    counts: dict[str, int] = {}
    risks: dict[str, int] = {}
    for s in stories:
        counts[s["status"]] = counts.get(s["status"], 0) + 1
        risks[s["change_risk"]] = risks.get(s["change_risk"], 0) + 1
    ready = [s for s in stories if s["status"] == "ready"]
    in_review = [s for s in stories if s["status"] == "ready_for_review"]
    lines = [
        "# Project Status",
        "",
        "<!-- Generated by scripts/project_os.py. Do not edit by hand. -->",
        "",
        f"Current phase: **{model.roadmap['current_phase']}**",
        "",
        f"Epics: **{len(model.epics)}** | Stories: **{len(stories)}**",
        "",
        "## Story status",
        "",
    ]
    for key in sorted(counts):
        lines.append(f"- `{key}`: {counts[key]}")
    lines.extend(["", "## Change risk distribution", ""])
    for key in sorted(risks):
        lines.append(f"- `{key}`: {risks[key]}")
    lines.extend(["", "## Explicitly ready work", ""])
    if ready:
        for s in ready:
            lines.append(f"- **{s['id']}** - {s['title']} ({s['change_risk']})")
    else:
        lines.append("- none")
    lines.extend(["", "## Awaiting independent review", ""])
    if in_review:
        for s in in_review:
            lines.append(f"- **{s['id']}** - {s['title']} ({s['change_risk']})")
    else:
        lines.append("- none")
    lines.append("")
    return "\n".join(lines)


def generated_outputs(model: Model) -> dict[Path, str]:
    return {
        DOCS / "DECISION_REGISTER.md": decision_markdown(model),
        DOCS / "ROADMAP.md": roadmap_markdown(model),
        DOCS / "BACKLOG.md": backlog_markdown(model),
        PROJECT / "generated" / "PROJECT_STATUS.md": project_status_markdown(model),
    }


def write_generated(model: Model) -> None:
    for path, content in generated_outputs(model).items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        print(f"wrote {path.relative_to(ROOT)}")


def check_generated(model: Model) -> None:
    drift: list[str] = []
    for path, content in generated_outputs(model).items():
        current = path.read_text(encoding="utf-8") if path.exists() else ""
        if current != content:
            drift.append(str(path.relative_to(ROOT)))
    if drift:
        raise GovernanceError("generated documentation drift: " + ", ".join(drift) + "; run `python3 scripts/project_os.py generate`")


def print_status(model: Model) -> None:
    stories = model.stories
    print(f"cancellAI Engineering OS | phase={model.roadmap['current_phase']} | epics={len(model.epics)} | stories={len(stories)}")
    by_status: dict[str, int] = {}
    for story in stories:
        by_status[story["status"]] = by_status.get(story["status"], 0) + 1
    print(" ".join(f"{k}={by_status[k]}" for k in sorted(by_status)))


def print_review(model: Model) -> None:
    waiting = [s for s in model.stories if s["status"] == "ready_for_review"]
    if not waiting:
        print("No stories are awaiting independent review.")
        return
    print("Awaiting independent review:")
    for story in waiting:
        print(f"{story['id']} [{story['change_risk']}] {story['title']}")
        print(f"  brief: python3 scripts/project_os.py brief {story['id']} --role verifier")


def print_next(model: Model) -> None:
    ready = [s for s in model.stories if s["status"] == "ready"]
    if not ready:
        print("No stories are explicitly marked ready.")
        return
    for story in ready:
        print(f"{story['id']} [{story['change_risk']}] {story['title']}")
        print(f"  {story['outcome']}")


def print_brief(model: Model, story_id: str, role: str) -> None:
    story = story_by_id(model, story_id)
    invariant_text = (DOCS / "security" / "SAFETY_INVARIANTS.md").read_text(encoding="utf-8")
    blocks: dict[str, str] = {}
    matches = list(re.finditer(r"^###\s+(SI-\d{3})\b.*$", invariant_text, flags=re.MULTILINE))
    for index, match in enumerate(matches):
        end = matches[index + 1].start() if index + 1 < len(matches) else len(invariant_text)
        blocks[match.group(1)] = invariant_text[match.start() : end].strip()

    print(f"# {role.capitalize()} Brief - {story['id']} - {story['title']}")
    print()
    print(f"Status: {story['status']} | Change Risk: {story['change_risk']}")
    print(f"Outcome: {story['outcome']}")
    print(f"Dependencies: {', '.join(story['dependencies']) or 'none'}")
    print()
    print("## Acceptance Criteria")
    for item in story["acceptance_criteria"]:
        print(f"- {item}")
    print()
    print("## Verification Contract")
    for item in story["verification"]:
        print(f"- {item}")
    print()
    print("## Safety Obligations")
    if story["safety_obligations"]:
        for sid in story["safety_obligations"]:
            print()
            print(blocks.get(sid, sid))
    else:
        print("- none")
    print()
    print("## Documentation Impact")
    for item in story["documentation_impact"]:
        print(f"- {item}")
    print()
    if role == "executor":
        print("## Role")
        print(
            "Implement the smallest coherent change satisfying this contract. Define falsification-oriented tests before "
            "implementation and produce an evidence packet. Read AGENTS.md and docs/development/AGENT_PROTOCOL.md before coding."
        )
    else:
        print("## Role")
        print(
            "Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor "
            "reasoning. For CR4, issue an owner-visible Safety Verdict."
        )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate and render the cancellAI Engineering OS control plane.")
    subparsers = parser.add_subparsers(dest="command", required=True)
    for command in ("check", "generate", "status", "next", "review"):
        subparsers.add_parser(command)
    brief = subparsers.add_parser("brief", help="Render a self-contained executor/verifier work brief for one story.")
    brief.add_argument("story_id")
    brief.add_argument("--role", choices=["executor", "verifier"], default="executor")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        model = load_model()
        warnings = validate(model)
        if args.command == "generate":
            write_generated(model)
        elif args.command == "check":
            check_generated(model)
            print(f"governance OK: {len(model.decisions['decisions'])} decisions, {len(model.epics)} epics, {len(model.stories)} stories")
        elif args.command == "status":
            print_status(model)
        elif args.command == "next":
            print_next(model)
        elif args.command == "review":
            print_review(model)
        else:
            print_brief(model, args.story_id, args.role)
        for warning in warnings:
            print(f"WARNING: {warning}", file=sys.stderr)
        return 0
    except GovernanceError as exc:
        print(f"GOVERNANCE ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

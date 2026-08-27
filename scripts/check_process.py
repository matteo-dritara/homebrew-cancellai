#!/usr/bin/env python3
"""Enforce the parts of the engineering process that live in files rather than in code.

`project_os.py` validates the control plane and `check_docs.py` validates the documentation
graph. This script covers what neither of them can see: whether decisions, ADRs, evidence
and commit messages actually follow the conventions the repository documents.

Everything here is enforceable by a machine on purpose. A rule that only exists in prose is
a rule that a tired human or an eager agent will skip, and the whole point of the cEOS is
that the repository - not memory - is the contract.

Commands:
  python3 scripts/check_process.py check
  python3 scripts/check_process.py commit-msg <path-to-message-file>
  python3 scripts/check_process.py commits <git-range>
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ADRS = ROOT / "docs" / "adrs"
EVIDENCE = ROOT / "project" / "evidence"

ADR_FILENAME_RE = re.compile(r"^(\d{4})-[a-z0-9][a-z0-9-]*\.md$")
ADR_STATUS_RE = re.compile(r"^- Status:\s*(.+?)\s*$", re.MULTILINE)
ADR_LINK_RE = re.compile(r"\((\d{4}-[a-z0-9-]+\.md)\)")
DECISION_REF_RE = re.compile(r"\bPD-\d{3}\b")
STORY_REF_RE = re.compile(r"\bE\d{2}(?:-S\d{2})?\b")

VALID_ADR_STATUSES = ("Accepted", "Proposed", "Rejected", "Superseded")

# Conventional Commits, restricted to the types AGENTS.md lists.
COMMIT_TYPES = ("feat", "fix", "docs", "chore", "test", "refactor", "style", "ci", "perf", "build", "revert")
COMMIT_SUBJECT_RE = re.compile(rf"^({'|'.join(COMMIT_TYPES)})(\([a-z0-9][a-z0-9.\-/]*\))?!?: .+$")
MAX_SUBJECT_LENGTH = 100

# A Safety Verdict must carry the two sections that record decisions, and must show that
# invariants were actually considered. Requiring a specific heading for the analysis would
# police formatting; requiring an invariant reference polices substance.
SAFETY_VERDICT_SECTIONS = ("## verdict", "## owner decision")
SAFETY_INVARIANT_RE = re.compile(r"\bSI-\d{3}\b")
GENERATED_FILES = (
    "docs/DECISION_REGISTER.md",
    "docs/ROADMAP.md",
    "docs/BACKLOG.md",
    "docs/CLI.md",
    "project/generated/PROJECT_STATUS.md",
)


class ProcessError(RuntimeError):
    pass


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def known_ids() -> tuple[set[str], set[str]]:
    """Decision ids and epic/story ids, read from the control plane itself."""
    decisions = json.loads(read(ROOT / "project" / "decisions.json"))
    decision_ids = {item["id"] for item in decisions["decisions"]}
    roadmap = json.loads(read(ROOT / "project" / "roadmap.json"))
    work_ids: set[str] = set()
    for rel in roadmap["epic_files"]:
        epic = json.loads(read(ROOT / rel))
        work_ids.add(epic["id"])
        work_ids.update(story["id"] for story in epic["stories"])
    return decision_ids, work_ids


def check_adrs(errors: list[str], decision_ids: set[str], work_ids: set[str]) -> None:
    numbers: dict[str, Path] = {}
    for path in sorted(ADRS.glob("*.md")):
        rel = path.relative_to(ROOT)
        match = ADR_FILENAME_RE.match(path.name)
        if not match:
            errors.append(f"{rel}: ADR filename must be NNNN-kebab-case-title.md")
            continue
        number = match.group(1)
        if number in numbers:
            errors.append(f"{rel}: duplicate ADR number {number} (also {numbers[number].relative_to(ROOT)})")
        numbers[number] = path

        text = read(path)
        status_match = ADR_STATUS_RE.search(text)
        if not status_match:
            errors.append(f"{rel}: missing '- Status:' line")
            continue
        status = status_match.group(1)
        if not status.lower().startswith(tuple(item.lower() for item in VALID_ADR_STATUSES)):
            errors.append(f"{rel}: status {status!r} must start with one of {list(VALID_ADR_STATUSES)}")
        if status.startswith("Superseded"):
            # An accepted ADR is never deleted; a superseded one must say what replaced it.
            forward = ADR_LINK_RE.search(status)
            if not forward:
                errors.append(f"{rel}: superseded ADR must link forward to the ADR that replaced it")
            elif not (ADRS / forward.group(1)).exists():
                errors.append(f"{rel}: forward link {forward.group(1)} does not exist")
        for ref in DECISION_REF_RE.findall(text):
            if ref not in decision_ids:
                errors.append(f"{rel}: references unknown decision {ref}")
        for ref in STORY_REF_RE.findall(text):
            if ref not in work_ids:
                errors.append(f"{rel}: references unknown work item {ref}")

    if numbers:
        expected = {f"{n:04d}" for n in range(1, len(numbers) + 1)}
        if set(numbers) != expected:
            errors.append(f"ADR numbering must be contiguous from 0001: missing {sorted(expected - set(numbers))}")


def check_superseded_decisions(errors: list[str]) -> None:
    decisions = json.loads(read(ROOT / "project" / "decisions.json"))["decisions"]
    ids = {item["id"] for item in decisions}
    for item in decisions:
        if item["status"] != "superseded":
            continue
        referenced = {ref for ref in DECISION_REF_RE.findall(item["decision"]) if ref != item["id"]}
        if not referenced:
            errors.append(f"project/decisions.json: {item['id']} is superseded but names no replacement decision")
        for ref in referenced - ids:
            errors.append(f"project/decisions.json: {item['id']} names unknown replacement {ref}")


def check_evidence(errors: list[str], work_ids: set[str]) -> None:
    for path in sorted(EVIDENCE.rglob("*.md")):
        rel = path.relative_to(ROOT)
        if path.name == "README.md":
            continue
        text = read(path)
        # Evidence is addressed by work item, not by filename convention alone: a packet
        # nobody can trace back to a story cannot be used to close one.
        referenced = {ref for ref in STORY_REF_RE.findall(f"{path.parent.name} {path.name} {text}") if ref in work_ids}
        if not referenced:
            errors.append(f"{rel}: evidence must name at least one existing epic or story id")
        if "verdict" in path.name.lower() and "safety" in path.name.lower():
            lowered = text.lower()
            missing = [section for section in SAFETY_VERDICT_SECTIONS if section not in lowered]
            if missing:
                errors.append(f"{rel}: Safety Verdict is missing required section(s) {missing}")
            if not SAFETY_INVARIANT_RE.search(text):
                errors.append(f"{rel}: Safety Verdict must reference at least one Safety Invariant (SI-xxx)")


def check_generated_banners(errors: list[str]) -> None:
    for rel in GENERATED_FILES:
        path = ROOT / rel
        if not path.exists():
            errors.append(f"{rel}: generated file is missing")
            continue
        head = read(path)[:600].lower()
        if "generated" not in head or "do not edit" not in head:
            errors.append(f"{rel}: generated file must carry a visible 'do not edit by hand' banner")


def validate_commit_subject(subject: str) -> list[str]:
    problems: list[str] = []
    if subject.startswith(("Merge ", "Revert ", "fixup!", "squash!")):
        return problems
    if not COMMIT_SUBJECT_RE.match(subject):
        problems.append(f"subject must be a Conventional Commit ({'|'.join(COMMIT_TYPES)}): {subject!r}")
    if len(subject) > MAX_SUBJECT_LENGTH:
        problems.append(f"subject is {len(subject)} characters; keep it under {MAX_SUBJECT_LENGTH}")
    if subject.endswith("."):
        problems.append("subject must not end with a period")
    return problems


def check_process() -> None:
    errors: list[str] = []
    decision_ids, work_ids = known_ids()
    check_adrs(errors, decision_ids, work_ids)
    check_superseded_decisions(errors)
    check_evidence(errors, work_ids)
    check_generated_banners(errors)
    if errors:
        raise ProcessError("\n".join(errors))


def check_commit_message(path: Path) -> None:
    lines = read(path).splitlines()
    body = [line for line in lines if not line.startswith("#")]
    if not body or not body[0].strip():
        raise ProcessError("commit message is empty")
    problems = validate_commit_subject(body[0].strip())
    if len(body) > 1 and body[1].strip():
        problems.append("leave a blank line between the subject and the body")
    if problems:
        raise ProcessError("\n".join(problems))


def check_commit_range(rev_range: str) -> None:
    git = shutil.which("git")
    if not git:
        raise ProcessError("git is not available on PATH")
    result = subprocess.run(  # noqa: S603
        [git, "log", "--format=%H%x00%s", rev_range],
        capture_output=True,
        text=True,
        check=False,
        cwd=ROOT,
    )
    if result.returncode != 0:
        raise ProcessError(f"git log failed for {rev_range!r}: {result.stderr.strip()}")
    problems: list[str] = []
    count = 0
    for line in result.stdout.splitlines():
        if not line.strip():
            continue
        sha, _, subject = line.partition("\x00")
        count += 1
        problems.extend(f"{sha[:8]}: {problem}" for problem in validate_commit_subject(subject))
    if problems:
        raise ProcessError("\n".join(problems))
    print(f"commit messages OK: {count} commit(s) in {rev_range}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate cancellAI engineering process conventions.")
    sub = parser.add_subparsers(dest="command")
    sub.add_parser("check")
    message = sub.add_parser("commit-msg")
    message.add_argument("path")
    commits = sub.add_parser("commits")
    commits.add_argument("range")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    command = args.command or "check"
    try:
        if command == "commit-msg":
            check_commit_message(Path(args.path))
            return 0
        if command == "commits":
            check_commit_range(args.range)
            return 0
        check_process()
        print("process OK: ADR lifecycle, decision supersession, evidence, and generated banners are consistent")
        return 0
    except (ProcessError, OSError, KeyError, ValueError) as exc:
        print(f"PROCESS ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

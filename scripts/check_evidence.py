#!/usr/bin/env python3
"""The evidence ledger is checked against the story it discharges (E25-S03).

`scripts/project_os.py` refuses a `ready_for_review` story with no evidence, and
`scripts/check_process.py` requires an evidence file to name a real work item. Neither compares
the packet against the contract it claims to discharge, which means the ledger - one of the six
owner-visible artifacts cEOS promises to keep stable - was prose nothing validated.

The consequence was measurable before this checker existed. Fifteen story packets past
`ready_for_review` did not carry a row per acceptance criterion, including all five E20 stories,
two of them CR4. Nothing reported it; the methodology review found it by counting. And worse than
a missing row is a wrong one: the E24 review found two rows claiming `PASS` on the strength of
tests that could not have detected the defects present, both sitting in `ready_for_review` with
every gate green.

Four rules:

1. a story past `ready_for_review` has a packet, and the packet carries a row per acceptance
   criterion - a criterion with no row was not discharged, it was omitted;
2. a CR3 or CR4 packet declares residual risk. "None" at those levels is itself a finding: a
   change that can mutate or that holds authority and has no residual risk has not been examined
   for one;
3. a CR4 story at `done` has a Safety Verdict recording a pass - that is the reviewer's output and
   the story cannot close without it;
4. every `scripts/*.py` command a packet claims to have run names a script that exists.

Pre-convention packets are listed explicitly in `baseline`, printed on every run, rather than
exempted silently - the same treatment `project/risk_floors.json` gives a classification that
predates its floor, and for the same reason: an exemption nobody sees is a decision nobody makes.

Stdlib-only, like every other governance checker here.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
PROJECT = ROOT / "project"
EVIDENCE = PROJECT / "evidence"
PACKET = "EVIDENCE.md"

ACCEPTED_STATUSES = {"ready_for_review", "verification", "done"}
RESIDUAL_REQUIRED_AT = {"CR3", "CR4"}

AC_ROW = re.compile(r"^\|\s*AC\s*(\d+)", re.MULTILINE | re.IGNORECASE)
RESIDUAL_SECTION = re.compile(r"##\s*Residual risks?\s*\n(.*?)(?=\n##|\Z)", re.DOTALL | re.IGNORECASE)
SCRIPT_COMMAND = re.compile(r"python3\s+(scripts/[\w./-]+\.py)")
FENCED = re.compile(r"^```.*?^```", re.MULTILINE | re.DOTALL)


def prose_only(text: str) -> str:
    """The packet with fenced blocks removed.

    A packet whose only acceptance-criteria rows sat inside a fenced example was counted as fully
    covered. A template is not evidence.
    """
    return FENCED.sub("", text)


# Packets written before the convention existed. Listed one by one rather than as a rule, so the
# list can only shrink: a new story cannot fall into it without someone adding a line here.
BASELINE = {
    "E00-S01": "predates the evidence-packet template (E00 was the trust-floor remediation)",
    "E00-S02": "predates the evidence-packet template",
    "E00-S03": "predates the evidence-packet template",
    "E00-S04": "predates the evidence-packet template",
    "E00-S05": "predates the evidence-packet template",
    "E00-S06": "predates the evidence-packet template",
    "E00-S07": "packet exists but predates the per-criterion row convention",
    "E00-S08": "predates the evidence-packet template",
    "E00-S09": "predates the evidence-packet template",
    "E07-S05": "packet exists but records its evidence in prose rather than per-criterion rows",
    "E20-S01": "packet exists but records its evidence in prose rather than per-criterion rows",
    "E20-S02": "packet exists but records its evidence in prose rather than per-criterion rows",
    "E20-S03": "packet exists but records its evidence in prose rather than per-criterion rows",
    "E20-S04": "packet exists but records its evidence in prose rather than per-criterion rows",
    "E20-S05": "packet exists but records its evidence in prose rather than per-criterion rows",
}


class EvidenceError(RuntimeError):
    pass


def load_stories() -> dict[str, dict[str, Any]]:
    roadmap = json.loads((PROJECT / "roadmap.json").read_text(encoding="utf-8"))
    stories: dict[str, dict[str, Any]] = {}
    for relative in roadmap["epic_files"]:
        for story in json.loads((ROOT / relative).read_text(encoding="utf-8"))["stories"]:
            stories[story["id"]] = story
    return stories


def residual_body(text: str) -> str:
    """The residual-risk section's content, or the empty string.

    A section whose first item is "none" counts as empty: at CR3 and above the honest answer is
    almost never none, and writing it is how the section stops being read.
    """
    match = RESIDUAL_SECTION.search(prose_only(text))
    if not match:
        return ""
    body = match.group(1).strip()
    first = body.lower().lstrip("-*~ ").strip()
    return "" if first.startswith("none") else body


def safety_verdict_exists(story_id: str) -> bool:
    directory = EVIDENCE / story_id
    if not directory.is_dir():
        return False
    return any("verdict" in path.name.lower() for path in directory.glob("*.md"))


def check_story(story: dict[str, Any], root: Path) -> list[str]:
    """Problems with one story's packet, ignoring the baseline."""
    story_id = story["id"]
    risk = story["change_risk"]
    packet = root / "project" / "evidence" / story_id / PACKET
    if not packet.is_file():
        return [f"{story_id}: status {story['status']} but no evidence packet at project/evidence/{story_id}/{PACKET}"]

    text = packet.read_text(encoding="utf-8")
    problems: list[str] = []

    # The *set* of criterion numbers, not the count: AC5-AC9 satisfied a three-criterion story
    # under a count comparison, and rows inside a fenced example counted as coverage.
    found = set(AC_ROW.findall(prose_only(text)))
    expected = {str(number) for number in range(1, len(story["acceptance_criteria"]) + 1)}
    missing = sorted(expected - found, key=int)
    if missing:
        problems.append(f"{story_id}: no evidence row for AC{', AC'.join(missing)} - a criterion with no row was not discharged, it was omitted")

    if risk in RESIDUAL_REQUIRED_AT and not residual_body(text):
        problems.append(
            f"{story_id}: {risk} packet declares no residual risk - at this level an empty residual "
            "section is itself a finding, because a change that can mutate or that holds authority "
            "has not been examined for one"
        )

    if risk == "CR4" and story["status"] == "done" and not safety_verdict_exists(story_id):
        problems.append(f"{story_id}: CR4 story is done with no Safety Verdict recorded - that verdict is the reviewer's output")

    for script in sorted(set(SCRIPT_COMMAND.findall(text))):
        if not (root / script).is_file():
            problems.append(f"{story_id}: claims to have run {script}, which does not exist")

    return problems


def evaluate(stories: dict[str, dict[str, Any]], root: Path) -> tuple[list[str], list[str], int]:
    errors: list[str] = []
    warnings: list[str] = []
    considered = 0
    for story_id, story in sorted(stories.items()):
        if story["status"] not in ACCEPTED_STATUSES:
            continue
        considered += 1
        problems = check_story(story, root)
        if not problems:
            continue
        if story_id in BASELINE:
            warnings.extend(f"{problem} [baseline: {BASELINE[story_id]}]" for problem in problems)
        else:
            errors.extend(problems)
    return errors, warnings, considered


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Check every evidence packet against the story contract it discharges.")
    parser.add_argument("command", nargs="?", default="check", choices=["check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    build_parser().parse_args(argv)
    try:
        errors, warnings, considered = evaluate(load_stories(), ROOT)
    except (EvidenceError, OSError, ValueError, KeyError) as exc:
        print(f"EVIDENCE ERROR: {exc}", file=sys.stderr)
        return 2

    for warning in warnings:
        print(f"warning: {warning}")
    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    if errors:
        print(f"\nEVIDENCE ERROR: {len(errors)} packets do not discharge their story contract", file=sys.stderr)
        return 2
    unresolved = len({w.split(":")[0] for w in warnings})
    print(f"evidence OK: {considered} packets checked against their contracts ({unresolved} pre-convention, recorded)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

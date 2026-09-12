#!/usr/bin/env python3
"""Acceptance criteria are written in a constrained syntax (E25-S08).

Acceptance criteria here are free English. Many are excellent, none is classified, and nothing
connects a criterion to the check that discharges it. EARS (Mavin et al., RE'09, Rolls-Royce)
constrains a requirement to five patterns:

| Pattern | Keyword | Shape |
| --- | --- | --- |
| Ubiquitous | (none) | the system shall `<response>` |
| State-driven | **While** | While `<state>`, the system shall `<response>` |
| Event-driven | **When** | When `<trigger>`, the system shall `<response>` |
| Optional | **Where** | Where `<feature>`, the system shall `<response>` |
| Unwanted behaviour | **If / then** | If `<condition>`, then the system shall `<response>` |

The value here is not tidiness. It is the **unwanted-behaviour** pattern, which forces failure
triggers to be written as first-class requirements rather than left as "error handling". For a
tool whose defining risk is deleting the wrong thing, the ratio of `If/Then` criteria to
happy-path criteria measures something real: whether the requirement set describes the *feature*
or the *hazard*.

So the rule this checker enforces is deliberately not "every criterion parses". A criterion that
resists the syntax is usually a criterion with an unstated precondition, and that is worth
reporting - but making it an error would trade a real property for a formatting one. What it
errors on is the substantive claim:

**a CR2, CR3 or CR4 story with no unwanted-behaviour criterion has described only the happy path.**

At those levels the change decides classification, can mutate, or holds authority. A requirement set that never says what
must happen when something is wrong has not specified the part that matters.

Stdlib-only, like every other governance checker here.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
PROJECT = ROOT / "project"

# EARS' unwanted-behaviour shape is a *leading* condition: "If <cond>, then the system shall ...".
# An earlier version matched `if` anywhere, so "the user is notified if verbose" counted as a
# hazard requirement - an independent review demonstrated it. The condition must open the clause,
# or be a "when X fails/cannot" form, which is the same requirement wearing an event keyword.
UNWANTED = re.compile(
    r"(^\s*(if|unless)\b)|(\b(if|unless)\b[^,.]{0,80},)|(\bwhen\b[^,.]{0,60}\b(cannot|fails?|is missing|is unreadable|does not|is not)\b)",
    re.IGNORECASE,
)
STATE = re.compile(r"\b(while|during|as long as|whenever)\b", re.IGNORECASE)
EVENT = re.compile(r"\b(when|on |upon|after|before)\b", re.IGNORECASE)
OPTIONAL = re.compile(r"\b(where|for a |for any |in the .* case)\b", re.IGNORECASE)

ENFORCED_AT = {"CR2", "CR3", "CR4"}
# Stories whose contracts were written before this rule. Listed one by one so the list can only
# shrink, exactly as `check_evidence.py` and `risk_floors.json` do it.
BASELINE_FILE = PROJECT / "ears_baseline.json"


class EarsError(RuntimeError):
    pass


def classify(criterion: str) -> str:
    """The EARS pattern a criterion most closely matches.

    Deliberately a classifier over prose rather than a parser: these criteria were not written in
    EARS, and a parser would report that every one of them is malformed, which is true and useless.
    What is wanted is the *distribution* - and above all whether any criterion describes unwanted
    behaviour at all.
    """
    text = criterion.strip()
    if UNWANTED.search(text):
        return "unwanted"
    if STATE.search(text):
        return "state-driven"
    if EVENT.search(text):
        return "event-driven"
    if OPTIONAL.search(text):
        return "optional"
    return "ubiquitous"


def load_stories() -> dict[str, dict[str, Any]]:
    roadmap = json.loads((PROJECT / "roadmap.json").read_text(encoding="utf-8"))
    stories: dict[str, dict[str, Any]] = {}
    for relative in roadmap["epic_files"]:
        epic = json.loads((ROOT / relative).read_text(encoding="utf-8"))
        for story in epic["stories"]:
            stories[story["id"]] = {**story, "epic": epic["id"]}
    return stories


def load_baseline() -> dict[str, str]:
    if not BASELINE_FILE.is_file():
        return {}
    data: dict[str, str] = json.loads(BASELINE_FILE.read_text(encoding="utf-8")).get("stories", {})
    return data


def profile(story: dict[str, Any]) -> Counter[str]:
    return Counter(classify(criterion) for criterion in story["acceptance_criteria"])


def evaluate(stories: dict[str, dict[str, Any]], baseline: dict[str, str]) -> tuple[list[str], list[str], Counter[str]]:
    errors: list[str] = []
    warnings: list[str] = []
    totals: Counter[str] = Counter()
    for story_id, story in sorted(stories.items()):
        if story["status"] == "cancelled":
            continue
        counts = profile(story)
        totals.update(counts)
        if story["change_risk"] not in ENFORCED_AT:
            continue
        if counts["unwanted"]:
            continue
        message = (
            f"{story_id} ({story['change_risk']}): no acceptance criterion describes unwanted behaviour. "
            "At this level the change can mutate or holds authority, and a requirement set that never "
            "says what must happen when something is wrong has not specified the part that matters"
        )
        if story_id in baseline:
            warnings.append(f"{message} [baseline: {baseline[story_id]}]")
        else:
            errors.append(message)
    return errors, warnings, totals


def ratio_by_epic(stories: dict[str, dict[str, Any]]) -> list[tuple[str, int, int]]:
    by_epic: dict[str, Counter[str]] = {}
    for story in stories.values():
        if story["status"] == "cancelled":
            continue
        by_epic.setdefault(story["epic"], Counter()).update(profile(story))
    rows = []
    for epic, counts in sorted(by_epic.items()):
        rows.append((epic, counts["unwanted"], sum(counts.values())))
    return rows


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Classify acceptance criteria and require CR2+ stories to specify unwanted behaviour.")
    parser.add_argument("command", nargs="?", default="check", choices=["check", "report", "ratio"])
    return parser


def main(argv: list[str] | None = None) -> int:
    command = build_parser().parse_args(argv).command
    try:
        stories = load_stories()
        errors, warnings, totals = evaluate(stories, load_baseline())
    except (EarsError, OSError, ValueError, KeyError) as exc:
        print(f"EARS ERROR: {exc}", file=sys.stderr)
        return 2

    if command == "ratio":
        # Data, not prose: `scripts/process_metrics.py` reads this rather than importing this
        # module, because two files in mypy's own file list importing each other confuses module
        # resolution, and a shared number is not a reason to couple two checkers.
        print(json.dumps([{"epic": e, "unwanted": u, "total": n} for e, u, n in ratio_by_epic(stories)]))
        return 0

    if command == "report":
        print("Acceptance criteria by EARS pattern\n")
        for pattern, count in totals.most_common():
            print(f"  {pattern:14} {count}")
        print("\nUnwanted-behaviour ratio per epic (the number that measures whether the")
        print("requirements describe the hazard or the feature):\n")
        for epic, unwanted, total in ratio_by_epic(stories):
            share = f"{100 * unwanted / total:.0f}%" if total else "-"
            print(f"  {epic}  {unwanted:>3} of {total:>3}  {share}")
        return 0

    for warning in warnings:
        print(f"warning: {warning}")
    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    if errors:
        return 2
    unwanted = totals["unwanted"]
    total = sum(totals.values())
    print(f"EARS OK: {total} acceptance criteria classified, {unwanted} describe unwanted behaviour ({100 * unwanted / total:.0f}%)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

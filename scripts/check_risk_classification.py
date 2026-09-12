#!/usr/bin/env python3
"""The Change Risk Level stops being self-assessment (E25-S02).

`change_risk` is validated in exactly one other place in this repository, and the validation is
that the string is one of five permitted values. Every gate the system has is downstream of that
field: writing `CR1` where `CR4` was correct removes, in one edit that passes every check, the
adversarial tests, the fault injection, the independent verification, the Safety Verdict and the
release-evidence obligation. No safety standard lets the implementing party assign its own
criticality level - DO-178C takes it from the system safety assessment, ISO 26262 subjects the
hazard analysis itself to an independent confirmation review, and NPR 7150.2 classifies twice and
treats a disagreement as an escalation event.

This is the mechanical half of the answer: a **floor** derived from the paths a change touches.
`project/risk_floors.json` names the authority surfaces - the code that decides what is
*permitted*, never the code that decides what the user *asked for* - and a change reaching one of
them may not declare a level below its floor.

Two design decisions worth stating, because both are places a weaker implementation would go
wrong:

**Attribution is refused when it would be a guess.** A story's paths are derived from commits, and
this repository batches several stories into one commit routinely - 58 of its commits name more
than one story. Attributing every file in such a commit to every story it mentions produces
confident nonsense: it flagged 38 stories, most of them for files they never touched. Only a commit
naming exactly one story attributes, and the coverage that buys is reported rather than hidden.

**Pre-existing violations are a visible baseline, not a silent exemption.** Five stories were
already below their floor when it was introduced. They are listed in `baseline` and printed as a
warning on every run, so the owner decides rather than the list quietly becoming permanent. A new
violation fails.

Stdlib-only, like every other governance checker here.
"""

from __future__ import annotations

import argparse
import collections
import fnmatch
import json
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
PROJECT = ROOT / "project"
FLOORS_FILE = PROJECT / "risk_floors.json"

LEVELS = ("CR0", "CR1", "CR2", "CR3", "CR4")
STORY_ID = re.compile(r"E\d{2}-S\d{2}")
# A commit subject routinely writes several stories as `E25-S04/S05/S10`. Matching STORY_ID alone
# finds one id there, so the commit reads as unambiguous and every file in it is attributed to the
# first story - which is exactly the confident wrong attribution this checker refuses to make. This
# was found by the checker itself, on its own commit.
STORY_SHORTHAND = re.compile(r"E(\d{2})-S(\d{2})((?:/S\d{2})+)")
# A level at or above this requires a second, independent classification before the story closes.
INDEPENDENT_CLASSIFICATION_REQUIRED_AT = "CR4"


class RiskClassificationError(RuntimeError):
    pass


def rank(level: str) -> int:
    return LEVELS.index(level) if level in LEVELS else -1


@dataclass(frozen=True)
class Floor:
    level: str
    pattern: str
    path: str
    reason: str


def load_floors() -> dict[str, Any]:
    data: dict[str, Any] = json.loads(FLOORS_FILE.read_text(encoding="utf-8"))
    for entry in data.get("floors", []):
        if entry.get("level") not in LEVELS:
            raise RiskClassificationError(f"floor {entry.get('pattern')!r} has invalid level {entry.get('level')!r}")
        if not entry.get("reason"):
            raise RiskClassificationError(
                f"floor {entry.get('pattern')!r} has no reason; a floor nobody can argue with is a floor nobody will respect"
            )
    return data


def normalise(path: str) -> str:
    """A repository-relative path, whatever form it arrived in.

    `./x`, `x/../x` and an absolute path inside the repository are the same file, and a floor that
    matched only one spelling would be a barrier that depends on how a name happens to be typed -
    the thing this repository's own README refuses for protected provider names.
    """
    candidate = Path(path)
    if candidate.is_absolute():
        try:
            return candidate.resolve().relative_to(ROOT.resolve()).as_posix()
        except ValueError:
            return candidate.as_posix()
    import posixpath

    return posixpath.normpath(path).lstrip("./") if path.startswith("./") else posixpath.normpath(path)


def floor_for(paths: list[str], floors: list[dict[str, Any]]) -> Floor | None:
    """The highest floor any path triggers.

    Deliberately the highest rather than the most specific: a change touching both a README and
    the safety kernel is governed by the kernel, and a most-specific rule could be defeated by
    adding a narrow low-level pattern next to a broad high-level one.
    """
    best: Floor | None = None
    for raw in paths:
        path = normalise(raw)
        for entry in floors:
            if fnmatch.fnmatch(path, entry["pattern"]):
                candidate = Floor(entry["level"], entry["pattern"], path, entry["reason"])
                if best is None or rank(candidate.level) > rank(best.level):
                    best = candidate
    return best


def git(*args: str) -> str:
    binary = shutil.which("git")
    if binary is None:
        return ""
    try:
        result = subprocess.run(  # noqa: S603
            [binary, *args], cwd=ROOT, capture_output=True, text=True, timeout=60, check=False
        )
    except OSError:
        return ""
    return result.stdout if result.returncode == 0 else ""


def working_tree_paths() -> list[str]:
    """Everything the working tree changes relative to HEAD, tracked and untracked."""
    tracked = git("diff", "--name-only", "HEAD").splitlines()
    staged = git("diff", "--name-only", "--cached").splitlines()
    untracked = git("ls-files", "--others", "--exclude-standard").splitlines()
    return sorted({path for path in (*tracked, *staged, *untracked) if path})


def attributable_paths() -> tuple[dict[str, set[str]], int]:
    """Story -> files, from commits naming exactly one story, plus the count that were ambiguous.

    The ambiguous count is returned rather than dropped: a checker whose coverage is a third of
    the backlog must say so, or a clean run reads as "nothing is below its floor" when it means
    "most stories could not be checked".
    """
    raw = git("log", "--format=%x02%H%x01%s%x01%b%x01", "--name-only")
    attributed: dict[str, set[str]] = collections.defaultdict(set)
    ambiguous = 0
    for block in raw.split("\x02")[1:]:
        fields = block.split("\x01")
        if len(fields) < 4:
            continue
        message = f"{fields[1]} {fields[2]}"
        files = {line.strip() for line in fields[3].splitlines() if line.strip() and "/" in line}
        found = story_ids(message)
        if len(found) == 1:
            attributed[found.pop()] |= files
        elif len(found) > 1:
            ambiguous += 1
    return dict(attributed), ambiguous


def story_ids(message: str) -> set[str]:
    """Every story a commit message names, expanding `E25-S04/S05/S10` shorthand."""
    found = set(STORY_ID.findall(message))
    for epic, first, rest in STORY_SHORTHAND.findall(message):
        found.add(f"E{epic}-S{first}")
        found.update(f"E{epic}-{part}" for part in rest.strip("/").split("/") if part)
    return found


def load_stories() -> dict[str, dict[str, Any]]:
    roadmap = json.loads((PROJECT / "roadmap.json").read_text(encoding="utf-8"))
    stories: dict[str, dict[str, Any]] = {}
    for relative in roadmap["epic_files"]:
        for story in json.loads((ROOT / relative).read_text(encoding="utf-8"))["stories"]:
            stories[story["id"]] = story
    return stories


def evaluate(stories: dict[str, dict[str, Any]], attributed: dict[str, set[str]], data: dict[str, Any]) -> tuple[list[str], list[str]]:
    """Errors and warnings. A baseline entry demotes an error to a warning; an override removes it."""
    floors = data.get("floors", [])
    baseline = {entry["story"]: entry for entry in data.get("baseline", [])}
    overrides = {entry["story"]: entry for entry in data.get("overrides", [])}
    errors: list[str] = []
    warnings: list[str] = []

    for story_id, paths in sorted(attributed.items()):
        story = stories.get(story_id)
        if story is None:
            continue
        floor = floor_for(sorted(paths), floors)
        if floor is None:
            continue
        declared = story["change_risk"]
        if rank(declared) >= rank(floor.level):
            continue
        message = f"{story_id}: declared {declared} but {floor.path} sets a floor of {floor.level} - {floor.reason}"
        if story_id in overrides:
            override = overrides[story_id]
            if not override.get("reason"):
                errors.append(f"{story_id}: override records no reason; 'it is a small change' is not a reason")
            else:
                warnings.append(f"{message} [override: {override['reason']}]")
        elif story_id in baseline and rank(floor.level) <= rank(baseline[story_id].get("floor", "CR4")):
            warnings.append(f"{message} [baseline, owner decision pending: {baseline[story_id].get('note', '')}]")
        elif story_id in baseline:
            # A baselined story that later reaches a *higher* surface is a new violation, not the
            # recorded one. Keying the exemption on the story alone made it permanent and made the
            # printed note stale - an independent review demonstrated exactly that.
            errors.append(
                f"{message} - this story is in the baseline at floor "
                f"{baseline[story_id].get('floor')}, which does not cover a floor of {floor.level}"
            )
        else:
            errors.append(message)

    classifications = {entry["story"]: entry for entry in data.get("independent_classifications", [])}
    for story_id, story in sorted(stories.items()):
        if story.get("change_risk") != INDEPENDENT_CLASSIFICATION_REQUIRED_AT:
            continue
        if story.get("status") in {"ready_for_review", "verification", "done"} and story_id not in classifications:
            warnings.append(
                f"{story_id}: {INDEPENDENT_CLASSIFICATION_REQUIRED_AT} story past ready_for_review with no independent "
                "classification - the floor is the only thing checking this level"
            )

    for story_id, entry in sorted(classifications.items()):
        story = stories.get(story_id)
        if story is None:
            errors.append(f"independent classification names unknown story {story_id}")
            continue
        if entry.get("level") not in LEVELS:
            errors.append(f"{story_id}: independent classification has invalid level {entry.get('level')!r}")
        if not entry.get("classifier"):
            errors.append(f"{story_id}: independent classification records no classifier; an unattributed second opinion is not one")

    return errors, warnings


def disagreement_summary(stories: dict[str, dict[str, Any]], data: dict[str, Any]) -> str:
    """How often the second classification differs from the declared one.

    A rate of exactly zero over many stories is evidence that the second classification is not
    independent, so the mechanism is able to detect its own failure. That is the whole point of
    reporting it rather than merely recording the classifications.
    """
    entries = data.get("independent_classifications", [])
    if not entries:
        return (
            "independent classifications recorded: 0\n"
            f"  No story has been classified twice. Until one is, the {INDEPENDENT_CLASSIFICATION_REQUIRED_AT} gate\n"
            "  rests on the floor alone, which catches a wrong level only where a floor exists."
        )
    disagreements = [entry for entry in entries if stories.get(entry["story"], {}).get("change_risk") != entry.get("level")]
    rate = 100 * len(disagreements) / len(entries)
    lines = [f"independent classifications recorded: {len(entries)}; disagreements: {len(disagreements)} ({rate:.0f}%)"]
    for entry in disagreements:
        declared = stories.get(entry["story"], {}).get("change_risk")
        lines.append(f"  {entry['story']}: declared {declared}, independently classified {entry['level']} by {entry['classifier']}")
    if len(entries) >= 10 and not disagreements:
        lines.append("  A disagreement rate of zero over ten or more stories is evidence that the second")
        lines.append("  classification is not independent. Check who is producing it and how.")
    return "\n".join(lines)


def cmd_floor(paths: list[str] | None) -> int:
    data = load_floors()
    targets = paths or working_tree_paths()
    if not targets:
        print("no changed paths; nothing to classify")
        return 0
    floor = floor_for(targets, data["floors"])
    print(f"paths considered: {len(targets)}")
    if floor is None:
        print("risk floor:       none - no authority surface is touched")
        print("                  the level is the executor's judgement, and nothing here constrains it")
        return 0
    print(f"risk floor:       {floor.level}")
    print(f"                  triggered by {floor.path}")
    print(f"                  because: {floor.reason}")
    return 0


def cmd_check() -> int:
    data = load_floors()
    stories = load_stories()
    attributed, ambiguous = attributable_paths()
    errors, warnings = evaluate(stories, attributed, data)

    for warning in warnings:
        print(f"warning: {warning}")
    for error in errors:
        print(f"error: {error}", file=sys.stderr)

    checkable = sum(1 for story_id in attributed if story_id in stories)
    print(f"\nrisk classification: {checkable} of {len(stories)} stories had unambiguously attributable paths")
    print(f"  {ambiguous} commits named more than one story and were not attributed - attribution is refused")
    print("  rather than guessed, so this coverage is a floor on what was checked, not a claim about the rest.")
    print(disagreement_summary(stories, data))
    if errors:
        print(f"\nRISK CLASSIFICATION ERROR: {len(errors)} stories declare a level below their floor", file=sys.stderr)
        return 2
    print(f"\nrisk classification OK: no new story is below its floor ({len(warnings)} recorded, owner decision pending)")
    return 0


def cmd_commit_msg(message_path: Path) -> int:
    """Gate the change being committed, which is the only moment the floor can still influence it.

    `check` reads history: by the time it can see a story's paths, the change is already made. This
    runs from the `commit-msg` hook, where the staged diff and the story ids are both available -
    the floor stops being an audit and becomes a constraint.
    """
    data = load_floors()
    stories = load_stories()
    message = message_path.read_text(encoding="utf-8")
    named = story_ids(message)
    staged = [path for path in git("diff", "--name-only", "--cached").splitlines() if path]
    floor = floor_for(staged, data["floors"])
    if floor is None or not named:
        return 0

    overrides = {entry["story"]: entry for entry in data.get("overrides", [])}
    for story_id in sorted(named):
        story = stories.get(story_id)
        if story is None or story_id in overrides:
            continue
        if rank(story["change_risk"]) < rank(floor.level):
            print(
                f"REFUSED: {story_id} declares {story['change_risk']}, but this commit touches "
                f"{floor.path}, which sets a floor of {floor.level}.\n\n{floor.reason}\n\n"
                "Raise the story's change_risk, or record an override in project/risk_floors.json "
                "naming the specific reason the floor does not apply. 'It is a small change' is not "
                "such a reason.",
                file=sys.stderr,
            )
            return 1
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Check declared Change Risk Levels against the floor their paths set.")
    sub = parser.add_subparsers(dest="command")
    floor = sub.add_parser("floor", help="the risk floor for a change (default: the working tree)")
    floor.add_argument("paths", nargs="*", help="paths to classify instead of the working tree")
    sub.add_parser("check", help="validate declared levels against their floors")
    commit = sub.add_parser("commit-msg", help="gate the staged change against the floor its paths set")
    commit.add_argument("message", type=Path)
    parser.set_defaults(command="check")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "floor":
            return cmd_floor(list(args.paths) or None)
        if args.command == "commit-msg":
            return cmd_commit_msg(args.message)
        return cmd_check()
    except (RiskClassificationError, OSError, ValueError, KeyError) as exc:
        print(f"RISK CLASSIFICATION ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Measure the engineering process the way the engineering process measures the code (E25-S01).

cEOS can say, at any moment, whether a gate passed. It cannot say whether the gates *work* -
whether review finds defects, whether the second round is worth running, whether the evidence
ledger is intact, whether an epic reviewed by a different agent fares differently from one
reviewed by the same agent that wrote it. A process that has never caught a defect and a process
with no defects to catch produce identical evidence; everything here exists to separate those two
cases, and every number is computed from artifacts this repository already commits.

The four measurements, and why each one:

* **Review yield per round.** Capers Jones's defect-removal work puts pre-test inspection at the
  top of the efficiency table; the only way to know whether *this* inspection is anywhere near
  that is to count what each round found. A round-2 yield near zero means the ceiling is generous;
  a round-2 yield near round-1's means the ceiling cuts while the review is still productive, and
  the fixed ceiling of two (ADR-0014) is then a budget rather than a stopping rule.

* **Lincoln-Petersen residual estimate.** Where an epic has two rounds, `N = n1*n2/m` estimates the
  defect population from the overlap, and `N - (n1+n2-m)` estimates what is left. The estimate is
  a known *under*-estimate and the literature says so; the diagnostic value is in the overlap, not
  in N. Near-total overlap between two reviewers means they are not independent samples - they
  found the same things because they are, in effect, one reviewer. That is the measurement that
  tells you whether "independent verifier" is a description or a label.

* **First-pass rejection rate, split by reviewer independence.** The rate at which `ready_for_review`
  work - work whose executor ran every gate green - is rejected on first review is the gap between
  "the gates passed" and "the work is right". Splitting it by whether the reviewer was a different
  agent or the same one is the only local evidence available about self-preference bias.

* **Evidence-ledger integrity.** How many story packets carry a row per acceptance criterion. A
  convention that decays silently is worse than no convention, because it is still cited.

Stdlib-only, like every other governance checker here. Read-only: it computes, it never writes
project state.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from collections import Counter
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
PROJECT = ROOT / "project"
EVIDENCE = PROJECT / "evidence"
OUTPUT = PROJECT / "generated" / "PROCESS_METRICS.md"

# The second column must be an actual verdict token. An earlier version accepted any
# capitalised word, so a review record whose second column held the Change Risk Level was read
# as three PASS verdicts - a measurement tool reporting a confident wrong number, which is the
# one failure mode it may never have.
VERDICTS = ("PASS_WITH_RESIDUALS", "PASS", "FAIL")
VERDICT_ROW = re.compile(r"^\|\s*(E\d{2}-S\d{2})\s*\|\s*\**(" + "|".join(VERDICTS) + r")\**\s*\|", re.MULTILINE)
AC_ROW = re.compile(r"^\|\s*AC\s*(\d+)", re.MULTILINE | re.IGNORECASE)
# `E22-VERIFIER-REVIEW-ROUND2.md` is an epic round; `E07-S07-VERIFIER-REVIEW.md` is a
# story-scoped record from before ADR-0014 made review epic-scoped. Counting the second as an
# epic round would invent rounds that never happened and corrupt every number below it.
REVIEW_FILE = re.compile(r"^(E\d{2})(?P<story>-S\d{2})?-(?:VERIFIER|SELF)-REVIEW(?:-ROUND(?P<round>\d+))?\.md$")
SELF_REVIEW = re.compile(r"SELF-REVIEW")
REJECTING_VERDICTS = {"FAIL"}
ACCEPTED_STATUSES = {"ready_for_review", "verification", "done"}


@dataclass
class Round:
    number: int
    path: Path
    independent: bool
    verdicts: dict[str, str] = field(default_factory=dict)

    @property
    def rejected(self) -> set[str]:
        return {story for story, verdict in self.verdicts.items() if verdict in REJECTING_VERDICTS}


def load_stories() -> dict[str, dict[str, Any]]:
    roadmap = json.loads((PROJECT / "roadmap.json").read_text(encoding="utf-8"))
    stories: dict[str, dict[str, Any]] = {}
    for relative in roadmap["epic_files"]:
        epic = json.loads((ROOT / relative).read_text(encoding="utf-8"))
        for story in epic["stories"]:
            stories[story["id"]] = {**story, "epic": epic["id"]}
    return stories


def load_rounds() -> dict[str, list[Round]]:
    """Review records per epic, ordered by round.

    A record is `independent` when it was produced by the reviewer the protocol names, and not
    when the executor reviewed its own epic. The filename carries that distinction, because a
    self-review is committed under a different name precisely so it cannot be mistaken for one.
    """
    rounds: dict[str, list[Round]] = {}
    for path in sorted(EVIDENCE.glob("*REVIEW*.md")):
        match = REVIEW_FILE.match(path.name)
        if not match:
            continue
        if match.group("story"):
            continue
        record = Round(
            number=int(match.group("round") or 1),
            path=path,
            independent=not SELF_REVIEW.search(path.name),
            verdicts=dict(VERDICT_ROW.findall(path.read_text(encoding="utf-8"))),
        )
        rounds.setdefault(match.group(1), []).append(record)
    for records in rounds.values():
        records.sort(key=lambda record: record.number)
    return rounds


def lincoln_petersen(first: set[str], second: set[str]) -> tuple[float, float] | None:
    """Estimated defect population and residual, or None when the overlap is empty.

    An empty overlap makes the estimator undefined, and that is itself the finding: two reviewers
    who share nothing have not bounded the population, they have shown it is larger than either
    of them saw.
    """
    overlap = len(first & second)
    if overlap == 0:
        return None
    population = len(first) * len(second) / overlap
    return population, population - len(first | second)


def evidence_coverage(stories: dict[str, dict[str, Any]]) -> tuple[int, int, list[str]]:
    """Story packets with a row per acceptance criterion, out of those that need one."""
    covered = 0
    considered = 0
    gaps: list[str] = []
    for story_id, story in sorted(stories.items()):
        if story["status"] not in ACCEPTED_STATUSES:
            continue
        considered += 1
        packet = EVIDENCE / story_id / "EVIDENCE.md"
        rows = len(set(AC_ROW.findall(packet.read_text(encoding="utf-8")))) if packet.is_file() else 0
        if rows >= len(story["acceptance_criteria"]):
            covered += 1
        else:
            gaps.append(f"{story_id} ({story['change_risk']}): {rows} rows for {len(story['acceptance_criteria'])} criteria")
    return covered, considered, gaps


def git_subjects(limit: int) -> list[str]:
    """Commit subjects, or nothing.

    Resolved absolutely and soft-failing, like `scripts/check_docs.py`'s git call: the rework
    proxy is a refinement, and a source export with no history or no `git` binary must still get
    every other number rather than an error.
    """
    git = shutil.which("git")
    if git is None:
        return []
    try:
        result = subprocess.run(  # noqa: S603
            [git, "log", f"-{limit}", "--format=%s"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
    except OSError:
        return []
    return result.stdout.splitlines() if result.returncode == 0 else []


def rework_ratio(limit: int = 300) -> tuple[int, int]:
    """`fix:` commits against `feat:` commits.

    A proxy for DORA's rework rate, and an honest one only if read as a proxy: many `fix:` commits
    here repair a finding from review, which is the process working rather than failing. The number
    is worth having because its *trend* is informative even when its level is not.
    """
    subjects = git_subjects(limit)
    fixes = sum(1 for subject in subjects if subject.startswith(("fix:", "fix(")))
    feats = sum(1 for subject in subjects if subject.startswith(("feat:", "feat(")))
    return fixes, feats


def render(stories: dict[str, dict[str, Any]], rounds: dict[str, list[Round]]) -> str:
    lines = [
        "# Process Metrics",
        "",
        "<!-- Generated by scripts/process_metrics.py from project/evidence/*, project/epics/*.json",
        "     and git history. Do not edit by hand. -->",
        "",
        "Every number here is computed from artifacts this repository already commits. None of it",
        "is a target: a gate count that becomes a target stops measuring anything (Goodhart), and",
        "these are diagnostics for the owner, not scores for the executor.",
        "",
        "## Review yield per round",
        "",
        "A round that rejects nothing has either nothing to reject or is not looking. The two are",
        "distinguished by the next round, which is why the ceiling matters.",
        "",
        "| Epic | Round | Reviewer | Stories judged | Rejected | Yield |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for epic in sorted(rounds):
        for record in rounds[epic]:
            reviewer = "independent" if record.independent else "**self**"
            if not record.verdicts:
                lines.append(f"| {epic} | {record.number} | {reviewer} | - | - | **not machine-readable** |")
                continue
            judged = len(record.verdicts)
            rejected = len(record.rejected)
            lines.append(f"| {epic} | {record.number} | {reviewer} | {judged} | {rejected} | {100 * rejected / judged:.0f}% |")

    independent_judged = sum(len(r.verdicts) for rs in rounds.values() for r in rs if r.independent and r.number == 1 and r.verdicts)
    independent_rejected = sum(len(r.rejected) for rs in rounds.values() for r in rs if r.independent and r.number == 1 and r.verdicts)
    self_judged = sum(len(r.verdicts) for rs in rounds.values() for r in rs if not r.independent and r.number == 1 and r.verdicts)
    self_rejected = sum(len(r.rejected) for rs in rounds.values() for r in rs if not r.independent and r.number == 1 and r.verdicts)

    lines += [
        "",
        "## First-pass rejection rate",
        "",
        "The gap between *every gate passed* and *the work is right*. Each rejected story below had",
        "already been declared `ready_for_review` by an executor who ran the full gate set green.",
        "",
        f"- **Independent reviewer:** {independent_rejected} of {independent_judged} round-1 verdicts were `FAIL`"
        f"{f' - **{100 * independent_rejected / independent_judged:.0f}%**' if independent_judged else ''}.",
        f"- **Self-review (same agent as executor):** {self_rejected} of {self_judged} round-1 verdicts were `FAIL`"
        f"{f' - **{100 * self_rejected / self_judged:.0f}%**' if self_judged else ''}.",
        "",
        "Read this split with care, and do not read a conclusion into it. The self-review sample is",
        "tiny, its composition differs from the independent one, and the published work on",
        "self-preference bias in model-as-judge evaluation reports a wide range of effects rather",
        "than a single number - so this table cannot confirm that bias and cannot rule it out. What",
        "it does establish is that the first-pass rejection rate is high under *either* reviewer:",
        "work that passed every gate is rejected on review often enough that the gates cannot be",
        "read as evidence of correctness. That conclusion does not depend on the split.",
        "",
        "## Residual-defect estimate where two rounds exist",
        "",
        "Lincoln-Petersen over the two rounds' rejection sets. The estimate is a known under-estimate;",
        "the diagnostic is the **overlap**. Near-total overlap means the two rounds were not",
        "independent samples, and the population estimate collapses to what one round already found.",
        "",
        "| Epic | Round 1 found | Round 2 found | Overlap | Estimated population | Estimated residual |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    two_round = [(epic, records) for epic, records in sorted(rounds.items()) if len([record for record in records if record.verdicts]) >= 2]
    if not two_round:
        lines.append("| - | - | - | - | - | - |")
    for epic, records in two_round:
        parsed = [record for record in records if record.verdicts]
        first, second = parsed[0].rejected, parsed[1].rejected
        estimate = lincoln_petersen(first, second)
        if estimate is None:
            lines.append(
                f"| {epic} | {len(first)} | {len(second)} | 0 | undefined - disjoint findings | at least {len(first | second)} were present |"
            )
        else:
            population, residual = estimate
            lines.append(f"| {epic} | {len(first)} | {len(second)} | {len(first & second)} | {population:.1f} | {max(residual, 0.0):.1f} |")

    covered, considered, gaps = evidence_coverage(stories)
    risks = Counter(story["change_risk"] for story in stories.values())
    fixes, feats = rework_ratio()

    lines += [
        "",
        "## Evidence-ledger integrity",
        "",
        f"- **{covered} of {considered}** story packets past `ready_for_review` carry a row per acceptance criterion.",
    ]
    if gaps:
        lines.append("- Packets that do not:")
        lines += [f"  - {gap}" for gap in gaps]
    else:
        lines.append("- No gaps.")

    lines += [
        "",
        "## Change Risk distribution",
        "",
        "| CR0 | CR1 | CR2 | CR3 | CR4 |",
        "| --- | --- | --- | --- | --- |",
        f"| {risks['CR0']} | {risks['CR1']} | {risks['CR2']} | {risks['CR3']} | {risks['CR4']} |",
        "",
        f"{risks['CR3'] + risks['CR4']} of {sum(risks.values())} stories are CR3 or CR4, the levels whose gates require",
        "independent verification. That share is what makes the reviewer-independence question above",
        "load-bearing rather than academic.",
        "",
        "## Rework proxy",
        "",
        f"- Last 300 commits: **{feats}** `feat:`, **{fixes}** `fix:`.",
        "- Read as a trend, not a level: a `fix:` that repairs a review finding is the process working.",
        "  A rising ratio against a flat review yield is the shape to worry about.",
        "",
    ]
    # One trailing newline, no blank line before it: pre-commit's end-of-file-fixer would
    # otherwise strip it and leave the committed report permanently at odds with its generator.
    return "\n".join(lines).rstrip("\n") + "\n"


def display(path: Path) -> str:
    """A path for a human, without assuming it sits inside the repository."""
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Measure the cancellAI engineering process from its own artifacts.")
    parser.add_argument("command", nargs="?", default="report", choices=["report", "generate", "check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    command = build_parser().parse_args(argv).command
    try:
        stories = load_stories()
        rounds = load_rounds()
        report = render(stories, rounds)
    except (OSError, ValueError, KeyError) as exc:
        print(f"PROCESS METRICS ERROR: {exc}", file=sys.stderr)
        return 2

    if command == "report":
        print(report)
        return 0
    if command == "generate":
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text(report, encoding="utf-8")
        print(f"wrote {display(OUTPUT)}")
        return 0

    current = OUTPUT.read_text(encoding="utf-8") if OUTPUT.is_file() else ""
    if current != report:
        print(
            f"PROCESS METRICS ERROR: {display(OUTPUT)} is stale; run `python3 scripts/process_metrics.py generate`",
            file=sys.stderr,
        )
        return 2
    print("process metrics OK: the generated report matches the committed evidence and history")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

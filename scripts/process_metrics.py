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
FENCED = re.compile(r"^```.*?^```", re.MULTILINE | re.DOTALL)


def prose_only(text: str) -> str:
    """The document with fenced blocks removed.

    A review record that shows the verdict-table *format* in a fenced example had its example rows
    counted as real verdicts, which inflated the judged count and deflated the yield. A template is
    not a finding.
    """
    return FENCED.sub("", text)


VERDICT_ROW = re.compile(r"^\|\s*(E\d{2}-S\d{2})\s*\|\s*\**(" + "|".join(VERDICTS) + r")\**\s*\|", re.MULTILINE)
AC_ROW = re.compile(r"^\|\s*AC\s*(\d+)", re.MULTILINE | re.IGNORECASE)
# `E22-VERIFIER-REVIEW-ROUND2.md` is an epic round; `E07-S07-VERIFIER-REVIEW.md` is a
# story-scoped record from before ADR-0014 made review epic-scoped. Counting the second as an
# epic round would invent rounds that never happened and corrupt every number below it.
REVIEW_FILE = re.compile(r"^(E\d{2})(?P<story>-S\d{2})?-(?:VERIFIER|SELF)-REVIEW(?:-ROUND(?P<round>\d+))?\.md$", re.IGNORECASE)
SELF_REVIEW = re.compile(r"SELF-REVIEW", re.IGNORECASE)
REJECTING_VERDICTS = {"FAIL"}
ACCEPTED_STATUSES = {"ready_for_review", "verification", "done"}
# Review records under `project/evidence/` whose filename this tool cannot classify. Reported, not
# dropped: a record that vanishes is worse than one that fails to parse, because nothing says so.
UNCLASSIFIABLE: list[str] = []


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


def parse_verdicts(text: str) -> dict[str, str]:
    """Story -> verdict, or nothing at all when the record is ambiguous.

    A story appearing twice with different verdicts used to take the last one silently, so a record
    whose summary said FAIL and whose detail table said PASS read as a clean round. Ambiguity is
    reported as unparseable rather than resolved by position.
    """
    rows = VERDICT_ROW.findall(prose_only(text))
    verdicts: dict[str, str] = {}
    for story, verdict in rows:
        if story in verdicts and verdicts[story] != verdict:
            return {}
        verdicts[story] = verdict
    return verdicts


def load_rounds() -> dict[str, list[Round]]:
    """Review records per epic, ordered by round.

    A record is `independent` when it was produced by the reviewer the protocol names, and not
    when the executor reviewed its own epic. The filename carries that distinction, because a
    self-review is committed under a different name precisely so it cannot be mistaken for one.
    """
    rounds: dict[str, list[Round]] = {}
    unclassifiable: list[str] = []
    for path in sorted(EVIDENCE.rglob("*REVIEW*.md")):
        match = REVIEW_FILE.match(path.name)
        if not match:
            # Dropped silently, this hid a review record entirely - including a self-review named
            # with different capitalisation, which is precisely the case the independence split
            # depends on getting right.
            unclassifiable.append(str(path.relative_to(ROOT)))
            continue
        if match.group("story"):
            continue
        record = Round(
            number=int(match.group("round") or 1),
            path=path,
            independent=not SELF_REVIEW.search(path.name),
            verdicts=parse_verdicts(path.read_text(encoding="utf-8")),
        )
        rounds.setdefault(match.group(1), []).append(record)
    for records in rounds.values():
        records.sort(key=lambda record: record.number)
    UNCLASSIFIABLE.clear()
    UNCLASSIFIABLE.extend(sorted(unclassifiable))
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
        found = set(AC_ROW.findall(prose_only(packet.read_text(encoding="utf-8")))) if packet.is_file() else set()
        expected = {str(n) for n in range(1, len(story["acceptance_criteria"]) + 1)}
        rows = len(found & expected)
        if expected <= found:
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


DOC_REFERENCE = re.compile(r"(?<![\w/.-])((?:docs|project)/[\w./-]*\.md)")
# Documents addressed by something other than a review: GitHub consumes these by location, and
# evidence packets are addressed by work-item id. Listing them as unread would be noise.
READERSHIP_EXEMPT = ("project/evidence/", "project/generated/", "docs/adrs/", ".github/")


def readership() -> tuple[list[str], int]:
    """Documents no committed review record has ever named, and how many were named.

    Haddon-Cave's list of what a safety case degenerates into ends at decorative shelf-ware, and
    the only way to know is to record what a review actually opened. This is the measurement, not
    a purge: an unread document may still be the right document, and the output is a list for the
    owner rather than a deletion.
    """
    named: set[str] = set()
    for path in sorted(EVIDENCE.rglob("*REVIEW*.md")):
        named.update(DOC_REFERENCE.findall(path.read_text(encoding="utf-8")))
    everything = {p.relative_to(ROOT).as_posix() for p in ROOT.rglob("*.md") if "docs/" in p.as_posix() or "project/" in p.as_posix()}
    candidates = {doc for doc in everything if not doc.startswith(READERSHIP_EXEMPT) and (doc.startswith("docs/") or doc.startswith("project/"))}
    return sorted(candidates - named), len(named & candidates)


def risk_summary() -> str:
    """The independent-classification disagreement rate, which E25-S02's AC3 says is reported here.

    A rate of exactly zero over many stories is evidence that the second classification is not
    independent, so the number is only useful next to its denominator.
    """
    floors = PROJECT / "risk_floors.json"
    if not floors.is_file():
        return "- No risk-floor configuration; classification is unchecked."
    data = json.loads(floors.read_text(encoding="utf-8"))
    entries = data.get("independent_classifications", [])
    baseline = data.get("baseline", [])
    if not entries:
        return (
            f"- Independent classifications recorded: **0**. {len(baseline)} stories are recorded below their "
            "floor, owner decision pending.\n"
            "- With no second classification the level is still self-assessed everywhere a floor does not reach."
        )
    stories = load_stories()
    disagreements = [e for e in entries if stories.get(e["story"], {}).get("change_risk") != e.get("level")]
    rate = 100 * len(disagreements) / len(entries)
    tail = (
        " A rate of zero over this many stories is evidence the second opinion is not independent."
        if (len(entries) >= 10 and not disagreements)
        else ""
    )
    return f"- Independent classifications recorded: **{len(entries)}**; disagreements: **{len(disagreements)}** ({rate:.0f}%).{tail}"


def ears_summary() -> str:
    """The unwanted-behaviour ratio, which E25-S08's AC3 says is reported here.

    For a tool whose defining risk is deleting the wrong thing, this is the number that says
    whether the requirements describe the feature or the hazard.
    """
    try:
        result = subprocess.run(  # noqa: S603
            [sys.executable, str(ROOT / "scripts" / "check_ears.py"), "ratio"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        rows = [(row["epic"], row["unwanted"], row["total"]) for row in json.loads(result.stdout)]
    except (OSError, ValueError, KeyError):
        return "- The EARS classifier could not be run, so the requirements shape was not measured."
    if not rows:
        return "- No acceptance criteria to classify."
    unwanted = sum(row[1] for row in rows)
    total = sum(row[2] for row in rows)
    worst = sorted((row for row in rows if row[2]), key=lambda row: row[1] / row[2])[:5]
    lines = [
        f"- Acceptance criteria describing **unwanted behaviour**: **{unwanted} of {total}** ({100 * unwanted / total:.0f}%).",
        "- Epics whose requirements describe the feature and not the hazard:",
    ]
    lines += [f"  - `{epic}` - {unwanted_n} of {total_n}" for epic, unwanted_n, total_n in worst]
    return "\n".join(lines)


def _readership_lines() -> list[str]:
    unread, read = readership()
    lines = [
        f"- Documents a committed review record has named: **{read}**. Never named: **{len(unread)}**.",
        "- Never named is not the same as never read, and an unread document may still be the right",
        "  document. This is a list for the owner once a phase, not a purge, and accepted ADRs are",
        "  undeletable regardless of what it says.",
    ]
    if unread:
        lines += ["", "<details><summary>Never named in a review record</summary>", ""]
        lines += [f"- `{doc}`" for doc in unread]
        lines += ["", "</details>"]
    return lines


def render(stories: dict[str, dict[str, Any]], rounds: dict[str, list[Round]]) -> str:
    """The committed report.

    Deliberately a pure function of committed artifacts. The rework proxy is computed from git
    history and therefore lives in `report` only: a generated file that describes the commit log
    is stale the instant it is committed, because committing it is a commit. A drift check over a
    self-referential artifact can never pass, so the artifact must not be self-referential.
    """
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
        f"- **Round-1 records excluded as unreadable:** {len([r for rs in rounds.values() for r in rs if r.number == 1 and not r.verdicts])}."
        " Their verdicts are in neither denominator above, so both rates are computed over the"
        " subset of records that use a machine-readable verdict table.",
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
        "| Epic | Rounds compared | First found | Second found | Overlap | Estimated population | Estimated residual |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    two_round = [(epic, records) for epic, records in sorted(rounds.items()) if len([record for record in records if record.verdicts]) >= 2]
    if not two_round:
        lines.append("| - | - | - | - | - | - | - |")
    for epic, records in two_round:
        parsed = [record for record in records if record.verdicts]
        first, second = parsed[0].rejected, parsed[1].rejected
        estimate = lincoln_petersen(first, second)
        if estimate is None:
            lines.append(
                f"| {epic} | {parsed[0].number} and {parsed[1].number} | {len(first)} | {len(second)} | 0 "
                f"| undefined - disjoint findings | at least {len(first | second)} were present |"
            )
        else:
            population, residual = estimate
            lines.append(f"| {epic} | {len(first)} | {len(second)} | {len(first & second)} | {population:.1f} | {max(residual, 0.0):.1f} |")

    covered, considered, gaps = evidence_coverage(stories)
    risks = Counter(story["change_risk"] for story in stories.values())

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
        "## Risk classification",
        "",
        risk_summary(),
        "",
        "## Requirements shape",
        "",
        ears_summary(),
        "",
        "## Documentation readership",
        "",
        *_readership_lines(),
        "",
        "## Review records this tool could not classify",
        "",
        *([f"- `{path}`" for path in UNCLASSIFIABLE] or ["- none"]),
        "",
        "## Rework proxy",
        "",
        "Computed from git history by `python3 scripts/process_metrics.py report`, and deliberately",
        "not committed here: a generated file that describes the commit log is stale the instant it",
        "is committed, because committing it is a commit. Its drift check could then never pass.",
        "Run the command when you want the number.",
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
        fixes, feats = rework_ratio()
        history = (
            f"\n## Rework proxy (history-derived, not committed)\n\n"
            f"- Last 300 commits: **{feats}** `feat:`, **{fixes}** `fix:`.\n"
            "- Read as a trend, not a level: a `fix:` that repairs a review finding is the process\n"
            "  working. A rising ratio against a flat review yield is the shape to worry about.\n"
        )
        print(report + history)
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

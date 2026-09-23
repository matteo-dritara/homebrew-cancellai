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

# ADR-0014 / PD-022: an epic gets at most two independent review rounds. An unbounded loop
# hides defects behind a status that never changes; findings surviving round two become new
# backlog work items instead. E00 predates the rule and ran three rounds - it is the reason
# the rule exists, so it is recorded as an explicit exception rather than quietly exempted.
MAX_REVIEW_ROUNDS = 3
REVIEW_ROUND_EXCEPTIONS = {
    # Originally recorded as "ran three rounds before ADR-0014 bounded them" - E22-S06's own
    # fix surfaced a fourth, previously-uncounted record (`E00-S03-VERIFIER-REVIEW.md`, a
    # story-scoped round the old epic-only regex missed the same way it missed E07's). All
    # four predate ADR-0014, which is why this stays an exception rather than an error.
    "E00": "4 records once story-scoped reviews are counted (E00-S03, plus the three "
    "epic-level rounds) - all four predate ADR-0014 bounding review to two rounds",
    # E22-S06: this story's own fix - counting story-scoped records against their epic for
    # the first time - surfaces three pre-existing E07 records this check previously missed
    # (E07-S07 round 1, E07-S07 round 2, E07-S09 round 1), on top of the epic-level round
    # that closed E07. All three predate the fix and the epic's own closure; the owner-
    # authorized combined verify+fix+close round (E07-VERIFIER-REVIEW.md's "Process
    # exception" note) superseded them rather than being a fourth round chosen deliberately.
    "E07": "4 records once story-scoped reviews are counted (E07-S07 round 1, E07-S07 round "
    "2, E07-S09 round 1, plus the epic-level closing round) - the first three predate the "
    "epic-level round that owner-authorized combined verify+fix+close and superseded them; "
    "see E07-VERIFIER-REVIEW.md's Process exception note",
    # Round 1 and round 2 each found a genuine CR4 defect (crash-recovery, then a
    # non-collision-resistant digest); round 3 - the ADR-0025 cost ceiling - found a further
    # genuine defect in round 3's own crash-recovery repair (pending-sidecar recovery trusted
    # filename presence over identity), rather than a residual an owner could accept as-is.
    # The owner explicitly authorized a fourth round after that specific finding, in the same
    # conversation directing the round-4 identity-witness repair - recorded here rather than
    # silently exceeding the ceiling.
    "E12": "beyond the 3-round ceiling: rounds 3, 4 and 5 each found a genuine, distinct CR4 "
    "correctness hazard in three successive attempts at one automated crash-recovery scanner "
    "(recover_pending_moves) - filename-only proof, then an unrelated operation's finalized "
    "witness accepted as proof, then a finalization-ordering hazard within one operation's own "
    "group. The owner explicitly authorized each round to close what the previous one found, "
    "then, after round 5, decided three defects in one mechanism was itself a signal and "
    "removed that mechanism outright rather than authorize a fourth repair attempt - round 6 "
    "reviews that scope reduction, not a further patch. See "
    "E12-VERIFIER-REVIEW-ROUND3.md/-ROUND4.md/-ROUND5.md and the E12-S01/E12-S02/E12-S03 "
    "evidence packets' own 'round N' sections",
    # Round 4 reviews E13-S04's repaired ownership boundary and new carrier E13-S06, not the
    # already-closed E13-S02/S03/S05; see E13-VERIFIER-REVIEW-ROUND3.md and -ROUND4.md.
    "E13": "round 4 is new/repaired E13-S04+E13-S06 scope after round 3's marker-mimicry "
    "finding, not a repeated attempt at the already-closed E13-S02/S03/S05 stories; see "
    "E13-VERIFIER-REVIEW-ROUND3.md and E13-VERIFIER-REVIEW-ROUND4.md",
    # E14-S04 alone hit three successive design failures (round 1: no consumer; round 2: a
    # skippable opt-in function; round 3/ADR-0034: a mandatory but caller-discardable ceiling
    # field) - the owner explicitly authorized continuing past the 3-round ceiling for a fourth,
    # structurally different design (ADR-0035: the raw observation itself, not a derived
    # ceiling, is the authority input) rather than accepting round 3's finding as final. E14-S01/
    # S02/S03 closed in round 1/2 and are not part of this count's repeated-attempt story.
    "E14": "round 4 reviews E14-S04's ADR-0035 redesign (raw observation as the authority "
    "input) after round 3 found ADR-0034's mandatory-but-discardable ceiling field "
    "insufficient - the owner authorized this specific fourth attempt rather than accepting "
    "round 3 as final; see E14-S04-VERIFIER-REVIEW-ROUND3.md and ADR-0035. Round 5 reviews "
    "ADR-0036, an owner-authorized structurally different design after round 4 declined a "
    "fifth patch on ADR-0035's field shape; round 5 itself found and the executor repaired a "
    "forgeable-identity-binding defect within that same design before a second, same-round "
    "verification pass - see E14-S04-VERIFIER-REVIEW-ROUND5.md and ADR-0036. That second pass "
    "found a distinct TOCTOU defect (identity and markers read via two separate path-based "
    "syscalls); the owner was consulted directly and authorized both the repair "
    "(cancellai-sealedfs handle-bound metadata/listing) and one further, sixth review round "
    "- see E14-S04-VERIFIER-REVIEW-ROUND5-PASS2.md. Round 6 confirmed the TOCTOU/symlink "
    "repair sound but found list_child_names could not distinguish a real readdir() failure "
    "from end-of-directory, returning a truncated-but-Ok listing; repaired with POSIX errno "
    "discrimination and submitted for a seventh review round - see "
    "E14-S04-VERIFIER-REVIEW-ROUND6.md. Round 7 confirmed that errno repair correct but found "
    "its regression test never reached readdir() at all (it failed earlier, at try_clone); "
    "repaired with a test-only hook matching this crate's own established pattern and "
    "submitted for an eighth review round - see E14-S04-VERIFIER-REVIEW-ROUND7.md.",
    # E16: three epic rounds plus E16-S07's owner-authorized standalone CR4 review. Round 3
    # failed E16-S08 on a fail-open Git probe the round-2 verifier had added as a repair; the
    # owner capped independent review at two rounds per story and directed a repair to round
    # 3's exact prescription instead of a fourth round.
    "E16": "4 records once E16-S07's owner-authorized standalone review is counted; round 3 "
    "failed E16-S08 and the owner, having capped review at two rounds per story, accepted a "
    "repair to round 3's exact required repair with each of its counterexamples pinned as a "
    "regression test in place of a fourth round - see project/evidence/E16-S08/CEILING_DECISION.md",
    # E17: rounds 1-3 plus E17-S08's standalone story-scoped review. Rounds 2 and 3 are the two
    # independent reviews of E17-S07 the owner allowed per story; both failed it.
    "E17": "4 records once E17-S08's standalone story review is counted; rounds 2 and 3 are the "
    "owner's two-review limit for E17-S07, both FAIL - see E17-VERIFIER-REVIEW-ROUND3.md and "
    "project/evidence/E17-S07/EVIDENCE.md for what happened after the limit. On 2026-09-23 the "
    "owner, asked directly, authorized exactly one further round (round 4) for E17-S07, because "
    "a CR4 story cannot close without an independent Safety Verdict. Round 4 failed on two "
    "narrow deduplication defects, both repaired in f4eefe5; the owner then authorized exactly "
    "one further round (round 5) to verify those repairs",
    # E06-S06..S13 are new cutover stories added 2026-09-23 under ADR-0039, long after E06's
    # original rounds 1 and 2 (2026-09-01/02) reviewed E06-S01..S03.
    "E06": "round 3 is the first independent pass over E06-S07..S13, the cutover stories added "
    "under ADR-0039 on 2026-09-23 with the owner's direction to close the cutover, and the second "
    "for E06-S06; rounds 1 and 2 reviewed different stories (E06-S01..S03) - not a repeated "
    "attempt at the same work. See E06-VERIFIER-REVIEW-ROUND3.md. Round 4 re-reviews only the "
    "three stories round 3 failed (E06-S08, E06-S10, E06-S13) - their second pass, within the "
    "owner's two-review limit per story. Round 5 reviews E06-S13 alone: it failed rounds 3 and 4 and, "
    "as a CR4 story, cannot close without an independent verdict; the owner authorized exactly "
    "this one extra round on 2026-09-23. E06-S08 (CR1) instead closes on "
    "project/evidence/E06-S08/CEILING_DECISION.md. Round 6 is the first independent pass over "
    "E06-S04, the canonical switch, whose gate stories closed in rounds 3-5",
}
# Matches both an epic-scoped record (`E07-VERIFIER-REVIEW.md`) and a story-scoped one
# (`E07-S07-VERIFIER-REVIEW.md`), counting both against the *epic's* ceiling (captured
# group 1). Before E22-S06 this only matched the epic-scoped form, so a story-scoped review
# never counted at all: E07 read as one round in this check while four were actually run
# (`E07-VERIFIER-REVIEW.md` plus three `E07-S0N-VERIFIER-REVIEW*.md` records).
VERIFIER_REVIEW_RE = re.compile(r"^(E\d{2})(?:-S\d{2})?-VERIFIER-REVIEW.*\.md$")
GENERATED_FILES = (
    "project/generated/PROCESS_METRICS.md",
    "project/generated/GATE_SENSITIVITY.md",
    "docs/DECISION_REGISTER.md",
    "docs/ROADMAP.md",
    "docs/BACKLOG.md",
    "docs/CLI.md",
    "docs/PLATFORMS.md",
    "project/generated/PROJECT_STATUS.md",
)

# E01-S06: the Python reference freeze is a standing rule, not prose an agent can miss. The
# marker is a heading, checked case-insensitively like check_generated_banners checks for
# "generated"/"do not edit" - simple text presence, not parsed structure.
REFERENCE_FREEZE_MARKER = "python reference freeze"


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


def check_review_rounds(errors: list[str], warnings: list[str]) -> None:
    rounds: dict[str, list[str]] = {}
    for path in sorted(EVIDENCE.glob("*.md")):
        match = VERIFIER_REVIEW_RE.match(path.name)
        if match:
            rounds.setdefault(match.group(1), []).append(path.name)
    for epic_id, records in sorted(rounds.items()):
        if len(records) <= MAX_REVIEW_ROUNDS:
            continue
        reason = REVIEW_ROUND_EXCEPTIONS.get(epic_id)
        message = (
            f"{epic_id}: {len(records)} independent review rounds committed, above the cost ceiling "
            f"of {MAX_REVIEW_ROUNDS} (ADR-0025, amending ADR-0014): {records}"
        )
        if reason:
            warnings.append(f"{message} - recorded exception: {reason}")
        else:
            errors.append(message)


def check_generated_banners(errors: list[str]) -> None:
    for rel in GENERATED_FILES:
        path = ROOT / rel
        if not path.exists():
            errors.append(f"{rel}: generated file is missing")
            continue
        head = read(path)[:600].lower()
        if "generated" not in head or "do not edit" not in head:
            errors.append(f"{rel}: generated file must carry a visible 'do not edit by hand' banner")


def check_reference_freeze_marker(errors: list[str]) -> None:
    agents_path = ROOT / "AGENTS.md"
    if REFERENCE_FREEZE_MARKER not in read(agents_path).lower():
        errors.append(f"{agents_path.relative_to(ROOT)}: missing the {REFERENCE_FREEZE_MARKER!r} marker (E01-S06)")

    migration_path = ROOT / "docs" / "development" / "MIGRATION_PYTHON_RUST.md"
    migration_text = read(migration_path).lower()
    if "rollback" not in migration_text:
        errors.append(f"{migration_path.relative_to(ROOT)}: missing a documented rollback strategy (E01-S06 AC)")
    if "gate" not in migration_text:
        errors.append(f"{migration_path.relative_to(ROOT)}: missing a documented migration gate (E01-S06 AC)")


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


def check_process() -> list[str]:
    errors: list[str] = []
    warnings: list[str] = []
    decision_ids, work_ids = known_ids()
    check_adrs(errors, decision_ids, work_ids)
    check_superseded_decisions(errors)
    check_evidence(errors, work_ids)
    check_review_rounds(errors, warnings)
    check_generated_banners(errors)
    check_reference_freeze_marker(errors)
    if errors:
        raise ProcessError("\n".join(errors))
    return warnings


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
        warnings = check_process()
        print("process OK: ADR lifecycle, decision supersession, evidence, review rounds, and generated banners are consistent")
        for warning in warnings:
            print(f"WARNING: {warning}", file=sys.stderr)
        return 0
    except (ProcessError, OSError, KeyError, ValueError) as exc:
        print(f"PROCESS ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

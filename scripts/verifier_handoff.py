#!/usr/bin/env python3
"""The executor/verifier handoff is a mechanism, not a manual step (E28-S02).

Executor/verifier separation is the method this repository is built on and it was the only part of
it with no artifact. `project_os.py brief <ID> --role verifier` renders the verifier's input;
nothing carried it anywhere. A human copied it into the other agent and copied the verdict back,
which is how E16, E17, E09/E10, E27-S01 and E27-S06 were reviewed. Two things the ledger could not
see follow from that: whether the verifier was given the brief the gate rendered or a paraphrase of
it, and whether the verdict committed is the verdict the verifier produced.

A rendered brief is now a file with a checksum over its own body, and a verdict names the checksum
it answers. `check` refuses a verdict naming a brief that does not exist or whose checksum does not
match.

**What must not change is who may write what**, and automating a handoff between two roles is the
most direct way to collapse them. So the mechanism is built around a refusal rather than a
convenience: a brief records who rendered it, a verdict records who produced it, and a verdict
whose author is the party that rendered the brief is refused outright. That is the mechanical form
of `AGENT_PROTOCOL.md`'s rule that an executor's work ends at `ready_for_review` and it does not
write its own Safety Verdict - a rule that until now was enforced by the executor choosing to obey
it.

Three further properties, each of which a more convenient design would have lost:

* **The verifier being unavailable is not a fallback.** There is no path here that produces a
  verdict without one. A story with a rendered brief and no verdict stays where it is.
* **A CR4 Safety Verdict stays attributable.** The verdict names its author, and an unattributed
  verdict is not expressible - the field is required, not defaulted.
* **The mechanism is optional.** A handoff performed by a human remains valid, because the method
  is the separation and not the automation of it. A story with no brief is not failed; it is
  reported as having used the manual route.

Stdlib-only, like every other governance checker here.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
PROJECT = ROOT / "project"
EVIDENCE = PROJECT / "evidence"
BRIEF = "VERIFIER_BRIEF.md"

HEADER_END = "<!-- end handoff header -->"
CHECKSUM_LINE = re.compile(r"^Brief-Checksum:\s*([0-9a-f]{64})\s*$", re.MULTILINE)
RENDERED_BY = re.compile(r"^Rendered-by:\s*(.+?)\s*$", re.MULTILINE)
VERIFIER_LINE = re.compile(r"^Verifier:\s*(.+?)\s*$", re.MULTILINE)
STORY_ID = re.compile(r"^E\d{2}-S\d{2}$")
EPIC_SCOPE = re.compile(r"^Review-Scope:\s*epic\s*$", re.MULTILINE | re.IGNORECASE)
EPIC_CHECKSUM = re.compile(
    r"^\|\s*(E\d{2}-S\d{2})\s*\|[^\n]*?Brief-Checksum:\s*([0-9a-f]{64})\s*\|?\s*$",
    re.MULTILINE,
)

# A verdict lives in one of these next to the story's packet. Both names are already conventions in
# project/evidence/; this reads them rather than introducing a third.
VERDICT_GLOBS = ("*VERIFIER-REVIEW*.md", "*SAFETY_VERDICT*.md", "*VERDICT*.md")


class HandoffError(Exception):
    """A condition that stops the handoff from being checkable at all."""


def digest(body: str) -> str:
    """Checksum over the brief's body, excluding its own header.

    The header carries the checksum, so hashing the whole file would be self-referential. Line
    endings are normalised so a checkout on another platform does not read as tampering.
    """
    return hashlib.sha256(body.replace("\r\n", "\n").strip().encode("utf-8")).hexdigest()


def body_of(text: str) -> str:
    _, _, body = text.partition(HEADER_END)
    return body if body else text


def render_brief(story_id: str) -> str:
    """The verifier brief exactly as `project_os.py brief` renders it.

    Shelling out rather than importing keeps one renderer: a second in-process path would drift
    from the command the protocol tells a human to run, and then the checksum would attest to a
    document nobody was given.
    """
    result = subprocess.run(  # noqa: S603 - fixed argument list, interpreter from sys.executable
        [sys.executable, str(ROOT / "scripts" / "project_os.py"), "brief", story_id, "--role", "verifier"],
        capture_output=True,
        text=True,
        check=False,
        cwd=ROOT,
    )
    if result.returncode != 0:
        raise HandoffError(f"could not render a verifier brief for {story_id}: {result.stderr.strip() or result.stdout.strip()}")
    return result.stdout


def brief_path(story_id: str) -> Path:
    return EVIDENCE / story_id / BRIEF


def write_brief(story_id: str, rendered_by: str, today: dt.date) -> Path:
    if not STORY_ID.match(story_id):
        raise HandoffError(f"{story_id!r} is not a story id")
    body = render_brief(story_id)
    path = brief_path(story_id)
    path.parent.mkdir(parents=True, exist_ok=True)
    header = (
        f"<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this\n"
        f"header; a verdict answering this brief repeats it, so the ledger can tell whether the\n"
        f"verifier was given this document or a paraphrase of it. -->\n\n"
        f"Story: {story_id}\n"
        f"Rendered-by: {rendered_by}\n"
        f"Rendered-on: {today.isoformat()}\n"
        f"Brief-Checksum: {digest(body)}\n\n"
        f"{HEADER_END}\n"
    )
    path.write_text(header + body, encoding="utf-8")
    return path


def read_brief(story_id: str) -> dict[str, Any] | None:
    path = brief_path(story_id)
    if not path.is_file():
        return None
    text = path.read_text(encoding="utf-8")
    declared = CHECKSUM_LINE.search(text)
    rendered_by = RENDERED_BY.search(text)
    return {
        "path": path,
        "declared": declared.group(1) if declared else None,
        "actual": digest(body_of(text)),
        "rendered_by": rendered_by.group(1) if rendered_by else None,
    }


def verdicts_for(story_id: str) -> list[Path]:
    directory = EVIDENCE / story_id
    found: list[Path] = []
    if directory.is_dir():
        for pattern in VERDICT_GLOBS:
            found.extend(directory.glob(pattern))
    # Epic-level reviews sit beside the story directories and name the story in the filename.
    for pattern in VERDICT_GLOBS:
        found.extend(p for p in EVIDENCE.glob(pattern) if p.name.startswith(story_id))
    # An epic-scoped round is one record for every story it judges. Its table carries each
    # brief checksum, so the record must be checked for every named story rather than becoming
    # invisible to the handoff gate merely because its filename begins with the epic id.
    for pattern in VERDICT_GLOBS:
        for path in EVIDENCE.glob(pattern):
            text = path.read_text(encoding="utf-8")
            if EPIC_SCOPE.search(text) and any(row_story == story_id for row_story, _ in EPIC_CHECKSUM.findall(text)):
                found.append(path)
    return sorted(set(found))


def checksum_for_story(text: str, story_id: str) -> str | None:
    """The checksum a verdict declares for one story, including the epic-table form."""
    for row_story, checksum in EPIC_CHECKSUM.findall(text):
        if row_story == story_id:
            return str(checksum)
    claimed = CHECKSUM_LINE.search(text)
    return str(claimed.group(1)) if claimed else None


def check_story(story_id: str) -> list[str]:
    """Problems with one story's handoff. A story with no brief used the manual route and is fine."""
    problems: list[str] = []
    brief = read_brief(story_id)

    if brief is not None:
        if brief["declared"] is None:
            problems.append(f"{story_id}: {BRIEF} carries no Brief-Checksum, so nothing can be paired to it")
        elif brief["declared"] != brief["actual"]:
            problems.append(
                f"{story_id}: {BRIEF} declares checksum {brief['declared'][:12]} but its body hashes to "
                f"{brief['actual'][:12]} - the brief was edited after it was rendered"
            )
        if not brief["rendered_by"]:
            problems.append(f"{story_id}: {BRIEF} records no Rendered-by, so the separation it exists to prove cannot be checked")

    for verdict in verdicts_for(story_id):
        text = verdict.read_text(encoding="utf-8")
        claimed = checksum_for_story(text, story_id)
        author = VERIFIER_LINE.search(text)
        name = verdict.relative_to(ROOT)
        if claimed is None:
            if brief is not None:
                problems.append(
                    f"{name}: {BRIEF} exists for {story_id}, but this verdict carries no Brief-Checksum. "
                    "A manual handoff remains valid only when no rendered brief exists."
                )
            continue  # A verdict that claims no brief is a manual handoff; it is not failed here.
        if brief is None:
            problems.append(f"{name}: names a brief checksum, but {story_id} has no {BRIEF} to answer")
            continue
        if claimed != brief["actual"]:
            problems.append(
                f"{name}: answers checksum {claimed[:12]}, but {story_id}'s brief hashes to "
                f"{brief['actual'][:12]} - this verdict answers a document that is not the committed brief"
            )
        if author is None:
            problems.append(f"{name}: records no Verifier, and an unattributed verdict is not expressible")
        elif brief["rendered_by"] and author.group(1).strip().lower() == brief["rendered_by"].strip().lower():
            problems.append(
                f"{name}: Verifier is {author.group(1)!r}, which is the party that rendered the brief. "
                "An executor's work ends at ready_for_review and it does not write its own verdict - "
                "automating the handoff must not collapse the two roles it exists to keep apart"
            )
    return problems


def all_story_ids() -> list[str]:
    ids: list[str] = []
    for path in sorted((PROJECT / "epics").glob("*.json")):
        epic = json.loads(path.read_text(encoding="utf-8"))
        ids.extend(story["id"] for story in epic.get("stories", []))
    return ids


def cmd_brief(story_id: str, rendered_by: str) -> int:
    path = write_brief(story_id, rendered_by, dt.date.today())
    brief = read_brief(story_id)
    assert brief is not None  # noqa: S101 - just written
    print(f"wrote {path.relative_to(ROOT)}")
    print(f"  Brief-Checksum: {brief['actual']}")
    print(f"  Rendered-by:    {rendered_by}")
    print()
    print("Give this file to the verifier. Its verdict must repeat the checksum above as a")
    print("`Brief-Checksum:` line and name itself on a `Verifier:` line - and that name may not be")
    print("the one above, because the party that rendered the brief does not judge it.")
    return 0


def cmd_check() -> int:
    problems: list[str] = []
    briefed = 0
    for story_id in all_story_ids():
        if brief_path(story_id).is_file():
            briefed += 1
        problems.extend(check_story(story_id))
    print(f"verifier handoff: {briefed} stories carry a rendered brief; the rest used the manual route, which stays valid")
    for problem in problems:
        print(f"error: {problem}", file=sys.stderr)
    if problems:
        print(f"\nVERIFIER HANDOFF ERROR: {len(problems)} problem(s)", file=sys.stderr)
        return 1
    print("verifier handoff OK: every verdict that names a brief answers the brief that was committed")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Render and check the executor/verifier handoff.")
    sub = parser.add_subparsers(dest="command")
    brief = sub.add_parser("brief", help="render a checksummed verifier brief for a story")
    brief.add_argument("story")
    brief.add_argument("--rendered-by", required=True, help="the party rendering it; it may not also be the verifier")
    sub.add_parser("check", help="verify that every verdict answers the brief that was committed")
    parser.set_defaults(command="check")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "brief":
            return cmd_brief(args.story, args.rendered_by)
        return cmd_check()
    except (HandoffError, OSError, ValueError, KeyError) as exc:
        print(f"VERIFIER HANDOFF ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

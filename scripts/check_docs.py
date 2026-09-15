#!/usr/bin/env python3
"""Validate repository-local Markdown links and documentation invariants.

The checker deliberately uses only the standard library so documentation
integrity remains available before and after the Python -> Rust migration.
External URLs are not fetched here; CI/network checks may validate those
separately.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parent.parent
MARKDOWN_LINK = re.compile(r"(?<!!)\[[^\]]*\]\(([^)]+)\)")
INVARIANT_HEADING = re.compile(r"^###\s+(SI-\d{3})\b")

# A document nobody links to is a document nobody reads, and stale unread documentation is
# worse than none. Every Markdown file must be reachable from the documentation graph -
# either linked directly, or living in a directory that is linked as a whole.
#
# Two categories are exempt, both because they are addressed by something other than a
# link: files GitHub consumes by location, and evidence records addressed by work-item id
# (scripts/check_process.py enforces that they name a real story).
UNLINKED_BY_DESIGN = (
    ".github/ISSUE_TEMPLATE",
    ".github/PULL_REQUEST_TEMPLATE.md",
    "project/evidence",
)


class DocsError(RuntimeError):
    pass


@dataclass(frozen=True)
class LocalLink:
    source: Path
    line: int
    raw_target: str
    target: Path


# Tool caches are not documentation.
IGNORED_DIRECTORIES = {".git", ".pytest_cache", ".mypy_cache", ".ruff_cache", "__pycache__", ".venv", "node_modules"}


def gitignored(paths: list[Path]) -> set[Path]:
    """Which of `paths` Git ignores, via `git check-ignore --stdin`. A local, un-tracked file
    that happens to sit under the repository root - this machine's own `.claude/` skill/state
    cache is the concrete case that motivated this - is not part of the versioned documentation
    graph; a fresh clone or CI checkout never has it, so this checker must not treat it as a
    document nobody links to. Soft-fails to "nothing is ignored" if `git` is unavailable or the
    call otherwise fails, rather than raising: this is a filtering refinement, not something a
    missing `git` binary should be able to break the whole check over.
    """
    git = shutil.which("git")
    if git is None or not paths:
        return set()
    try:
        result = subprocess.run(  # noqa: S603
            [git, "check-ignore", "--stdin"],
            cwd=ROOT,
            input="\n".join(str(path) for path in paths),
            capture_output=True,
            text=True,
            timeout=10,
        )
    except OSError:
        return set()
    if result.returncode not in (0, 1):
        return set()
    return {Path(line) for line in result.stdout.splitlines() if line}


def markdown_files() -> list[Path]:
    candidates = [path for path in ROOT.rglob("*.md") if not IGNORED_DIRECTORIES.intersection(path.parts)]
    ignored = gitignored(candidates)
    return sorted(path for path in candidates if path not in ignored)


def _link_destination(raw: str) -> str:
    value = raw.strip()
    if value.startswith("<") and ">" in value:
        value = value[1 : value.index(">")]
    elif " " in value:
        # Markdown permits an optional title after the URL. Repository-local
        # paths containing spaces should be percent-encoded, which avoids
        # ambiguous parsing here.
        value = value.split(" ", 1)[0]
    return unquote(value)


def local_links(path: Path) -> list[LocalLink]:
    links: list[LocalLink] = []
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        for match in MARKDOWN_LINK.finditer(line):
            destination = _link_destination(match.group(1))
            if not destination or destination.startswith(("http://", "https://", "mailto:", "#")):
                continue
            without_anchor = destination.split("#", 1)[0]
            if not without_anchor:
                continue
            target = (path.parent / without_anchor).resolve()
            links.append(LocalLink(source=path, line=line_no, raw_target=destination, target=target))
    return links


def safety_invariant_ids() -> set[str]:
    path = ROOT / "docs" / "security" / "SAFETY_INVARIANTS.md"
    ids: set[str] = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        match = INVARIANT_HEADING.match(line)
        if match:
            ids.add(match.group(1))
    return ids


# The reference class these documents lean on hardest, and the one class this gate did not read:
# 424 distinct work-item ids across the non-generated documentation. A renamed or removed story
# leaves a citation pointing at nothing and nothing says so (E31-S01).
WORK_ITEM_REF = re.compile(r"\bE\d{2}(?:-S\d{2})?\b")
RETIRED_FILE = ROOT / "project" / "retired_work_items.json"
# Produced from the control plane, so they cannot disagree with it. Checking them would be
# checking the generator against itself.
GENERATED_DOCS = frozenset({"docs/DECISION_REGISTER.md", "docs/ROADMAP.md", "docs/BACKLOG.md", "project/generated/PROJECT_STATUS.md"})


def work_item_ids() -> set[str]:
    """Every epic and story id the control plane currently defines."""
    ids: set[str] = set()
    for path in sorted((ROOT / "project" / "epics").glob("*.json")):
        epic = json.loads(path.read_text(encoding="utf-8"))
        ids.add(epic["id"])
        ids.update(story["id"] for story in epic["stories"])
    return ids


def retired_work_items() -> list[dict[str, object]]:
    """Identifiers that existed and no longer do, so history is not mistaken for a typo."""
    if not RETIRED_FILE.exists():
        return []
    data = json.loads(RETIRED_FILE.read_text(encoding="utf-8"))
    if not isinstance(data, dict) or data.get("schema_version") != 1:
        raise DocsError("project/retired_work_items.json: expected schema_version 1")
    entries = data.get("retired")
    if not isinstance(entries, list):
        raise DocsError("project/retired_work_items.json: retired must be a list")
    if not all(isinstance(entry, dict) for entry in entries):
        raise DocsError("project/retired_work_items.json: every retirement must be an object")
    return entries


def work_item_reference_errors(files: list[Path]) -> list[str]:
    """Work-item citations in prose that name nothing.

    Deliberately checks existence and not truth. The defect that motivated this - RELEASE_GATES.md
    naming a dependency that had closed - would not be caught here, because the id existed; E30-S01
    closed that one by moving the claim into a field a gate reads. A citation of a *cancelled* item
    passes, because cancelling a story does not unwrite the seven documents that explain why.
    """
    known = work_item_ids()
    retired_entries = retired_work_items()
    errors: list[str] = []
    retired: set[str] = set()

    for index, entry in enumerate(retired_entries, start=1):
        where = f"project/retired_work_items.json: retirement {index}"
        identifier = entry.get("id")
        if not isinstance(identifier, str) or not WORK_ITEM_REF.fullmatch(identifier):
            errors.append(f"{where}: id must be one work-item identifier")
            continue
        if identifier in retired:
            errors.append(f"{where}: {identifier} is recorded more than once; a retirement is recorded once")
            continue
        retired.add(identifier)
        if identifier in known:
            errors.append(f"{where}: {identifier} still resolves and is not retired")
        became = entry.get("became")
        if not isinstance(became, str) or not became:
            errors.append(f"{where}: {identifier} records no successor")
        elif became not in known:
            errors.append(
                f"project/retired_work_items.json: {identifier} is recorded as having become {became!r}, "
                "which does not resolve either - a retirement record that rots is the defect this file exists to prevent"
            )
        recorded = entry.get("recorded")
        if not isinstance(recorded, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}", recorded):
            errors.append(f"{where}: {identifier} records no valid date")
        reason = entry.get("reason")
        if not isinstance(reason, str) or not reason.strip():
            errors.append(f"{where}: {identifier} records no reason")

    for path in files:
        relative = path.relative_to(ROOT).as_posix()
        if relative in GENERATED_DOCS:
            continue
        for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            for identifier in sorted(set(WORK_ITEM_REF.findall(line))):
                if identifier in known or identifier in retired:
                    continue
                errors.append(
                    f"{relative}:{line_no}: cites work item {identifier}, which no epic or story defines. "
                    "Fix the citation, or record it in project/retired_work_items.json with what it became"
                )
    return errors


def is_exempt(path: Path) -> bool:
    relative = path.relative_to(ROOT).as_posix()
    return any(relative == item or relative.startswith(f"{item}/") for item in UNLINKED_BY_DESIGN)


def validate_docs() -> None:
    errors: list[str] = []
    files = markdown_files()
    linked: set[Path] = set()
    for path in files:
        for link in local_links(path):
            linked.add(link.target)
            if not link.target.exists():
                source = link.source.relative_to(ROOT)
                errors.append(f"{source}:{link.line}: missing local link target {link.raw_target!r}")

    for path in files:
        if path == ROOT / "README.md" or is_exempt(path):
            continue
        if path in linked or path.parent in linked:
            continue
        errors.append(f"{path.relative_to(ROOT)}: not reachable from any other document; link it or the reader will never find it")

    errors.extend(work_item_reference_errors(files))

    invariant_ids = safety_invariant_ids()
    if not invariant_ids:
        errors.append("docs/security/SAFETY_INVARIANTS.md contains no SI-xxx headings")
    expected = {f"SI-{n:03d}" for n in range(1, 32)}
    if invariant_ids != expected:
        missing = sorted(expected - invariant_ids)
        unexpected = sorted(invariant_ids - expected)
        errors.append(f"safety invariant ID set drift: missing={missing} unexpected={unexpected}")

    if errors:
        raise DocsError("\n".join(errors))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate cancellAI documentation integrity.")
    parser.add_argument("command", nargs="?", default="check", choices=["check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    build_parser().parse_args(argv)
    try:
        validate_docs()
        print(f"docs OK: {len(markdown_files())} Markdown files; local links and safety IDs are consistent")
        return 0
    except (DocsError, OSError, UnicodeError) as exc:
        print(f"DOCS ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

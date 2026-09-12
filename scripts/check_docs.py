#!/usr/bin/env python3
"""Validate repository-local Markdown links and documentation invariants.

The checker deliberately uses only the standard library so documentation
integrity remains available before and after the Python -> Rust migration.
External URLs are not fetched here; CI/network checks may validate those
separately.
"""

from __future__ import annotations

import argparse
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

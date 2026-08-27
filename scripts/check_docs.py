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
import sys
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parent.parent
MARKDOWN_LINK = re.compile(r"(?<!!)\[[^\]]*\]\(([^)]+)\)")
INVARIANT_HEADING = re.compile(r"^###\s+(SI-\d{3})\b")


class DocsError(RuntimeError):
    pass


@dataclass(frozen=True)
class LocalLink:
    source: Path
    line: int
    raw_target: str
    target: Path


def markdown_files() -> list[Path]:
    return sorted(path for path in ROOT.rglob("*.md") if ".git" not in path.parts)


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


def validate_docs() -> None:
    errors: list[str] = []
    files = markdown_files()
    for path in files:
        for link in local_links(path):
            if not link.target.exists():
                source = link.source.relative_to(ROOT)
                errors.append(f"{source}:{link.line}: missing local link target {link.raw_target!r}")

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

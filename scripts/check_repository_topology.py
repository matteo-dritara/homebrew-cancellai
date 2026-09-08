#!/usr/bin/env python3
"""The canonical source repository is unambiguously identified (E17-S06, AC2).

Three independent sources of truth name the canonical repository today:
`Formula/cancellai.rb`'s `homepage`, `scripts/release.py`'s `REPO` constant, and
`docs/RELEASING.md`'s "Repository topology transition" section ("The current remote is
`OWNER/NAME`"). A drift between any two of them is exactly the kind of silent inconsistency
`docs/RELEASING.md`'s migration runbook (E17-S06) depends on not existing - the runbook assumes
these three agree, so this script makes that an enforced fact rather than an unchecked one.

Stdlib-only, like every other governance checker in this repository.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FORMULA = ROOT / "Formula" / "cancellai.rb"
RELEASE_PY = ROOT / "scripts" / "release.py"
RELEASING_MD = ROOT / "docs" / "RELEASING.md"

REPO_RE = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?/[A-Za-z0-9._-]+$")
FORMULA_HOMEPAGE_RE = re.compile(r'^\s*homepage\s+"https://github\.com/([^"]+)"\s*$', re.MULTILINE)
RELEASE_PY_REPO_RE = re.compile(r'^REPO\s*=\s*"([^"]+)"\s*$', re.MULTILINE)
RELEASING_MD_REMOTE_RE = re.compile(r"The current remote is `([^`]+)`")


class TopologyError(RuntimeError):
    pass


def read(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as exc:
        raise TopologyError(f"cannot read {path}: {exc}") from exc


def _extract(pattern: re.Pattern[str], text: str, where: str, what: str) -> str:
    match = pattern.search(text)
    if not match:
        raise TopologyError(f"{where}: could not find {what}")
    repo = match.group(1)
    if not REPO_RE.fullmatch(repo):
        raise TopologyError(f"{where}: {what} {repo!r} does not look like an OWNER/NAME repository")
    return repo


def check_texts(formula_text: str, release_py_text: str, releasing_md_text: str) -> list[str]:
    """The comparison itself, over already-read text - the part worth testing directly against
    synthetic variations without touching the real files."""
    errors: list[str] = []

    formula_repo = _extract(FORMULA_HOMEPAGE_RE, formula_text, "Formula/cancellai.rb", "homepage repository")
    release_py_repo = _extract(RELEASE_PY_REPO_RE, release_py_text, "scripts/release.py", "REPO constant")
    releasing_md_repo = _extract(RELEASING_MD_REMOTE_RE, releasing_md_text, "docs/RELEASING.md", "'current remote' claim")

    named = {
        "Formula/cancellai.rb homepage": formula_repo,
        "scripts/release.py REPO": release_py_repo,
        "docs/RELEASING.md 'current remote'": releasing_md_repo,
    }
    distinct = set(named.values())
    if len(distinct) > 1:
        detail = ", ".join(f"{label}={repo!r}" for label, repo in named.items())
        errors.append(
            f"canonical source repository is ambiguous - these disagree: {detail}. E17-S06 AC2 requires all three to name the same repository."
        )
    return errors


def check() -> list[str]:
    return check_texts(read(FORMULA), read(RELEASE_PY), read(RELEASING_MD))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Check the canonical source repository is unambiguously identified.")
    parser.add_argument("command", nargs="?", default="check", choices=["check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    build_parser().parse_args(argv)
    try:
        errors = check()
    except TopologyError as exc:
        print(f"REPOSITORY TOPOLOGY ERROR: {exc}", file=sys.stderr)
        return 2
    if errors:
        print("REPOSITORY TOPOLOGY ERROR:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 2
    print("repository topology OK: Formula, scripts/release.py, and docs/RELEASING.md agree on the canonical repository")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

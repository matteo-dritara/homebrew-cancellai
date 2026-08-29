#!/usr/bin/env python3
"""Static check: no crate but cancellai-platform's mutation.rs deletes anything directly.

E03-S05 (SI-019: "all filesystem/vendor mutations route through the safety executor",
C-07 "one safety kernel") requires that provider adapters, UI crates, and every other
production source file in this workspace be structurally unable to call a filesystem
removal primitive directly. This is the one part of that requirement checkable purely from
source text, the same way scripts/check_rust_workspace.py checks the one dependency-direction
rule expressible purely from Cargo.toml.

Test code is exempt: it routinely creates and tears down its own temporary directories, and
that is not the mutation path this invariant is about. This codebase's own convention is one
`#[cfg(test)] mod tests { ... }` block at the end of each file (visible throughout
rust/crates/*/src/), so only the text before the first `#[cfg(test)]` in a file is scanned as
production code. Integration test files under a crate's `tests/` directory are not scanned at
all - `src/**/*.rs` never reaches them.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUST_CRATES_DIR = ROOT / "rust" / "crates"
ALLOWED_FILE = RUST_CRATES_DIR / "cancellai-platform" / "src" / "mutation.rs"

BANNED_RE = re.compile(r"\bstd::fs::(remove_file|remove_dir_all|remove_dir)\b")
TEST_MODULE_MARKER = "#[cfg(test)]"


class MutationBoundaryError(RuntimeError):
    pass


def source_files() -> list[Path]:
    if not RUST_CRATES_DIR.is_dir():
        raise MutationBoundaryError(f"{RUST_CRATES_DIR.relative_to(ROOT)} does not exist")
    return sorted(RUST_CRATES_DIR.glob("*/src/**/*.rs"))


def production_text(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    marker = text.find(TEST_MODULE_MARKER)
    return text if marker == -1 else text[:marker]


def is_comment_line(text: str, match_start: int) -> bool:
    """Whether the line containing `match_start` is a `//`/`///`/`//!` comment line.

    This codebase uses only line comments (no `/* */` blocks), and a doc comment is exactly
    where a match's own name legitimately needs to appear in prose - explaining the boundary
    this script enforces, as several of the files it scans do. A comment can't perform a
    mutation, so it is not a violation.
    """
    line_start = text.rfind("\n", 0, match_start) + 1
    return text[line_start:match_start].lstrip().startswith("//")


def validate() -> list[str]:
    errors: list[str] = []
    for path in source_files():
        if path.resolve() == ALLOWED_FILE.resolve():
            continue
        text = production_text(path)
        for match in BANNED_RE.finditer(text):
            if is_comment_line(text, match.start()):
                continue
            line = text.count("\n", 0, match.start()) + 1
            rel = path.relative_to(ROOT)
            errors.append(
                f"{rel}:{line}: direct filesystem mutation outside the safety executor: "
                f"{match.group(0)} (SI-019) - only "
                f"{ALLOWED_FILE.relative_to(ROOT)} may call this"
            )
    return errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Enforce the E03-S05 mutation boundary (SI-019).")
    parser.add_argument("command", nargs="?", default="check", choices=["check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    build_parser().parse_args(argv)
    try:
        errors = validate()
    except (MutationBoundaryError, OSError, UnicodeError) as exc:
        print(f"MUTATION BOUNDARY ERROR: {exc}", file=sys.stderr)
        return 2
    if errors:
        print("MUTATION BOUNDARY ERROR:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 2
    print(f"mutation boundary OK: {len(source_files())} Rust source files scanned; only {ALLOWED_FILE.relative_to(ROOT)} deletes anything")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

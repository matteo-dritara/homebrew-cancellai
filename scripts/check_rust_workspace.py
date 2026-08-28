#!/usr/bin/env python3
"""Validate the Rust workspace skeleton against docs/architecture/TARGET.md (E02-S01).

Stdlib-only, like the other governance checkers - Cargo.toml is parsed with a small
line-oriented reader for just the fields this needs (`name = "..."` and `[dependencies]`
entries), the same convention scripts/release.py already uses for pyproject.toml/the
Homebrew formula, rather than adding a TOML-parsing dependency.

Checks:

- every crate TARGET.md lists exists under rust/crates/, and vice versa (no drift between
  the documented and the actual workspace);
- the crate dependency graph has no cycles (Cargo itself would refuse this, but a clear
  message here is cheaper than reading a Cargo error);
- the one dependency-direction rule expressible purely from Cargo.toml - cancellai-model and
  cancellai-safety may never depend on a provider adapter or UI/Guardian crate - actually
  holds, not merely intended.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUST_CRATES_DIR = ROOT / "rust" / "crates"
TARGET_DOC = ROOT / "docs" / "architecture" / "TARGET.md"

CRATE_LIST_BLOCK_RE = re.compile(r"## Target Rust workspace\n.*?```text\ncrates/\n(.*?)```", re.DOTALL)
CRATE_LIST_ENTRY_RE = re.compile(r"^\s*(cancellai-[a-z0-9-]+)/", re.MULTILINE)
PACKAGE_NAME_RE = re.compile(r'^name\s*=\s*"([^"]+)"', re.MULTILINE)
DEPENDENCY_TABLE_RE = re.compile(r"^\[dependencies\]\s*$(.*?)(?=^\[|\Z)", re.MULTILINE | re.DOTALL)
DEPENDENCY_NAME_RE = re.compile(r"^(cancellai-[a-z0-9-]+)\s*=", re.MULTILINE)

# Forbidden dependency direction (docs/architecture/TARGET.md): model/safety may not depend
# on UI or provider implementation crates. This is the one rule in that list expressible
# purely from the Cargo.toml dependency graph; the others (provider adapters may not bypass
# the safety executor, UI may not access raw provider roots, network/knowledge may not
# mutate) describe runtime behavior no crate has yet and cannot be checked statically here.
ISOLATED_CRATES = {"cancellai-model", "cancellai-safety"}


class RustWorkspaceError(RuntimeError):
    pass


def _display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def documented_crates() -> list[str]:
    text = TARGET_DOC.read_text(encoding="utf-8")
    block = CRATE_LIST_BLOCK_RE.search(text)
    if not block:
        raise RustWorkspaceError(f"{_display_path(TARGET_DOC)}: could not find the 'crates/' block under '## Target Rust workspace'")
    return CRATE_LIST_ENTRY_RE.findall(block.group(1))


def crate_dependencies(cargo_toml: Path) -> set[str]:
    text = cargo_toml.read_text(encoding="utf-8")
    match = DEPENDENCY_TABLE_RE.search(text)
    if not match:
        return set()
    return set(DEPENDENCY_NAME_RE.findall(match.group(1)))


def build_graph() -> dict[str, set[str]]:
    graph: dict[str, set[str]] = {}
    for cargo_toml in sorted(RUST_CRATES_DIR.glob("*/Cargo.toml")):
        text = cargo_toml.read_text(encoding="utf-8")
        name_match = PACKAGE_NAME_RE.search(text)
        if not name_match:
            raise RustWorkspaceError(f"{_display_path(cargo_toml)}: no [package] name found")
        graph[name_match.group(1)] = crate_dependencies(cargo_toml)
    return graph


def find_cycle(graph: dict[str, set[str]]) -> list[str] | None:
    WHITE, GRAY, BLACK = 0, 1, 2
    color = dict.fromkeys(graph, WHITE)
    stack: list[str] = []

    def visit(node: str) -> list[str] | None:
        color[node] = GRAY
        stack.append(node)
        for neighbor in sorted(graph.get(node, ())):
            if neighbor not in graph:
                continue
            if color[neighbor] == GRAY:
                cycle_start = stack.index(neighbor)
                return [*stack[cycle_start:], neighbor]
            if color[neighbor] == WHITE:
                found = visit(neighbor)
                if found:
                    return found
        stack.pop()
        color[node] = BLACK
        return None

    for node in sorted(graph):
        if color[node] == WHITE:
            found = visit(node)
            if found:
                return found
    return None


def validate() -> list[str]:
    errors: list[str] = []
    if not RUST_CRATES_DIR.is_dir():
        raise RustWorkspaceError(f"{_display_path(RUST_CRATES_DIR)} does not exist")

    documented = documented_crates()
    if not documented:
        raise RustWorkspaceError(f"{_display_path(TARGET_DOC)}: crate list block is empty")

    on_disk = sorted(p.parent.name for p in RUST_CRATES_DIR.glob("*/Cargo.toml"))
    documented_sorted = sorted(documented)
    if documented_sorted != on_disk:
        missing_on_disk = sorted(set(documented) - set(on_disk))
        undocumented = sorted(set(on_disk) - set(documented))
        if missing_on_disk:
            errors.append(f"documented in {_display_path(TARGET_DOC)} but missing under {_display_path(RUST_CRATES_DIR)}: {missing_on_disk}")
        if undocumented:
            errors.append(f"present under {_display_path(RUST_CRATES_DIR)} but not documented in {_display_path(TARGET_DOC)}: {undocumented}")

    graph = build_graph()

    for directory_name in on_disk:
        cargo_toml = RUST_CRATES_DIR / directory_name / "Cargo.toml"
        text = cargo_toml.read_text(encoding="utf-8")
        name_match = PACKAGE_NAME_RE.search(text)
        package_name = name_match.group(1) if name_match else None
        if package_name != directory_name:
            errors.append(f"{_display_path(cargo_toml)}: package name {package_name!r} does not match its directory {directory_name!r}")

    cycle = find_cycle(graph)
    if cycle:
        errors.append(f"dependency cycle: {' -> '.join(cycle)}")

    for isolated in sorted(ISOLATED_CRATES & set(graph)):
        forbidden = {dep for dep in graph[isolated] if dep not in ISOLATED_CRATES}
        if forbidden:
            errors.append(
                f"{isolated}: forbidden dependency on {sorted(forbidden)} - model/safety may not "
                "depend on provider/UI/store crates (docs/architecture/TARGET.md)"
            )

    return errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate the cancellAI Rust workspace skeleton against TARGET.md.")
    parser.add_argument("command", nargs="?", default="check", choices=["check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    build_parser().parse_args(argv)
    try:
        errors = validate()
    except RustWorkspaceError as exc:
        print(f"RUST WORKSPACE ERROR: {exc}", file=sys.stderr)
        return 2
    if errors:
        print("RUST WORKSPACE ERROR:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 2
    print(f"rust workspace OK: {len(documented_crates())} crates match TARGET.md, acyclic, model/safety isolated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

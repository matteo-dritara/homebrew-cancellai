#!/usr/bin/env python3
"""Validate the synthetic provider-layout fixture corpus (E01-S02).

Stdlib-only, like the other governance checkers, so it stays usable before and after the
Python -> Rust migration. Checks:

- every manifest entry has a matching recipe in tests/fixtures/recipes.py;
- the required fixture categories are all represented;
- every fixture builds without error;
- built fixture trees contain no obvious real path/secret/email pattern.

The pattern scan is a best-effort guard, not a guarantee - see tests/fixtures/README.md.
"""

from __future__ import annotations

import argparse
import contextlib
import importlib.util
import json
import re
import sys
import tempfile
from pathlib import Path
from types import ModuleType
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
FIXTURES_DIR = ROOT / "tests" / "fixtures"
MANIFEST_PATH = FIXTURES_DIR / "manifest.json"
RECIPES_PATH = FIXTURES_DIR / "recipes.py"

REQUIRED_CATEGORIES = {
    "normal_session",
    "subagent_tree",
    "active_data",
    "protected_state",
    "partial_tree",
    "symlink",
    "layout_drift",
}
VALID_TOOLS = {"claude", "codex"}

FORBIDDEN_PATTERNS = [
    re.compile(r"/Users/[^/\s]+"),
    re.compile(r"/home/[^/\s]+"),
    re.compile(r"C:\\Users\\"),
    re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"),
    re.compile(r"sk-[A-Za-z0-9]{20,}"),
    re.compile(r"gh[pousr]_[A-Za-z0-9]{20,}"),
    re.compile(r"AKIA[0-9A-Z]{16}"),
    re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
]


class FixturesError(RuntimeError):
    pass


def load_recipes() -> ModuleType:
    spec = importlib.util.spec_from_file_location("fixture_recipes", RECIPES_PATH)
    if spec is None or spec.loader is None:
        raise FixturesError(f"cannot load recipes module from {RECIPES_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_manifest() -> dict[str, Any]:
    try:
        raw = MANIFEST_PATH.read_text(encoding="utf-8")
    except OSError as exc:
        raise FixturesError(f"cannot read {MANIFEST_PATH}: {exc}") from exc
    try:
        data: dict[str, Any] = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise FixturesError(f"{MANIFEST_PATH}: invalid JSON: {exc}") from exc
    return data


def _scan_text(text: str, where: str, errors: list[str]) -> None:
    for pattern in FORBIDDEN_PATTERNS:
        if pattern.search(text):
            errors.append(f"{where}: matches forbidden pattern {pattern.pattern!r}")


def _restore_permissions(base: Path) -> None:
    for path in base.rglob("*"):
        with contextlib.suppress(OSError):
            path.chmod(0o755)


def _scan_tree(base: Path, fixture_id: str, errors: list[str]) -> None:
    for path in sorted(base.rglob("*")):
        relative = path.relative_to(base)
        _scan_text(str(relative), f"{fixture_id}:{relative} (path)", errors)
        if path.is_symlink() or path.is_dir():
            continue
        try:
            content = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        _scan_text(content, f"{fixture_id}:{relative} (content)", errors)


def validate() -> list[str]:
    errors: list[str] = []
    data = load_manifest()
    if data.get("schema_version") != 1:
        errors.append(f"{MANIFEST_PATH}: unsupported or missing schema_version")
    fixtures = data.get("fixtures")
    if not isinstance(fixtures, list) or not fixtures:
        raise FixturesError(f"{MANIFEST_PATH}: 'fixtures' must be a non-empty list")

    recipes = load_recipes()
    seen_ids: set[str] = set()
    seen_categories: set[str] = set()

    for entry in fixtures:
        fixture_id = entry.get("id")
        tool = entry.get("tool")
        category = entry.get("category")
        description = entry.get("description")

        if not isinstance(fixture_id, str) or not fixture_id:
            errors.append(f"fixture entry missing a string 'id': {entry!r}")
            continue
        if fixture_id in seen_ids:
            errors.append(f"{fixture_id}: duplicate fixture id")
        seen_ids.add(fixture_id)

        if tool not in VALID_TOOLS:
            errors.append(f"{fixture_id}: invalid tool {tool!r}, expected one of {sorted(VALID_TOOLS)}")
        if category not in REQUIRED_CATEGORIES:
            errors.append(f"{fixture_id}: invalid category {category!r}, expected one of {sorted(REQUIRED_CATEGORIES)}")
        else:
            seen_categories.add(category)
        if not isinstance(description, str) or not description:
            errors.append(f"{fixture_id}: missing a description")

        if fixture_id not in recipes.FIXTURES:
            errors.append(f"{fixture_id}: no matching recipe in {RECIPES_PATH.relative_to(ROOT)}")
            continue

        with tempfile.TemporaryDirectory(prefix="cancellai-fixture-") as tmp:
            base = Path(tmp)
            provider_root = base / "provider-home"
            provider_root.mkdir()
            try:
                recipes.build(fixture_id, provider_root)
            except Exception as exc:  # report every recipe failure, don't abort the run
                errors.append(f"{fixture_id}: recipe raised {exc!r}")
                continue
            _scan_tree(base, fixture_id, errors)
            _restore_permissions(base)

    orphaned_recipes = set(recipes.FIXTURES) - seen_ids
    if orphaned_recipes:
        errors.append(f"recipe(s) with no manifest entry: {sorted(orphaned_recipes)}")

    missing_categories = REQUIRED_CATEGORIES - seen_categories
    if missing_categories:
        noun = "category" if len(missing_categories) == 1 else "categories"
        errors.append(f"no fixture covers required {noun}: {sorted(missing_categories)}")

    return errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate the cancellAI synthetic fixture corpus.")
    parser.add_argument("command", nargs="?", default="check", choices=["check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    build_parser().parse_args(argv)
    try:
        errors = validate()
    except FixturesError as exc:
        print(f"FIXTURES ERROR: {exc}", file=sys.stderr)
        return 2
    if errors:
        print("FIXTURES ERROR:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 2
    data = load_manifest()
    print(f"fixtures OK: {len(data['fixtures'])} fixtures cover all required categories")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

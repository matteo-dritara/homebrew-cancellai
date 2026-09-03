#!/usr/bin/env python3
"""Validate the synthetic provider-layout fixture corpus (E01-S02).

Stdlib-only, like the other governance checkers, so it stays usable before and after the
Python -> Rust migration. Checks:

- every manifest entry has a matching recipe in tests/fixtures/recipes.py;
- the required fixture categories are all represented;
- a category covered for one reference provider is covered for the other, or the asymmetry is
  declared in the manifest with a reason (E21-S02);
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
VALID_ASYMMETRY_KINDS = {"structural", "tracked_gap"}

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


def _check_category_symmetry(fixtures: list[dict[str, Any]], declared: Any, errors: list[str]) -> None:
    """A category covered for one reference provider must be covered for the other.

    The 2026-09-03 target-engine review (CR-TE-03) found the corpus carried `partial_tree` for
    Claude and not for Codex, and the differential parity gate was therefore structurally unable
    to observe an incomplete Codex scan - while the engine was deleting on one. The gate is only
    ever worth its corpus, so the corpus itself needs a rule.

    An asymmetry is allowed, but only when the manifest declares it and says why. `structural`
    means the category cannot exist for that provider (Codex has a rollout parent/child graph,
    Claude has none). `tracked_gap` means it could and does not yet - which stays visible in the
    manifest instead of being indistinguishable from a deliberate decision.
    """
    covered: dict[str, set[str]] = {}
    for entry in fixtures:
        category, tool = entry.get("category"), entry.get("tool")
        if isinstance(category, str) and tool in VALID_TOOLS:
            covered.setdefault(category, set()).add(tool)

    if declared is None:
        declared = []
    if not isinstance(declared, list):
        errors.append("manifest 'category_asymmetry' must be a list of declarations")
        return

    allowed: dict[tuple[str, str], dict[str, Any]] = {}
    for item in declared:
        if not isinstance(item, dict):
            errors.append(f"category_asymmetry entry must be an object: {item!r}")
            continue
        category, absent_for = item.get("category"), item.get("absent_for")
        kind, reason = item.get("kind"), item.get("reason")
        if not isinstance(category, str) or category not in REQUIRED_CATEGORIES:
            errors.append(f"category_asymmetry: invalid category {category!r}")
            continue
        if absent_for not in VALID_TOOLS:
            errors.append(f"category_asymmetry[{category}]: 'absent_for' must be one of {sorted(VALID_TOOLS)}")
            continue
        if kind not in VALID_ASYMMETRY_KINDS:
            errors.append(f"category_asymmetry[{category}]: 'kind' must be one of {sorted(VALID_ASYMMETRY_KINDS)}")
        # A reason short enough to be a label explains nothing; the point of the declaration is
        # that a reader can tell a deliberate decision from an unexamined hole.
        if not isinstance(reason, str) or len(reason.strip()) < 40:
            errors.append(f"category_asymmetry[{category}]: 'reason' must be a real explanation, not a label")
        allowed[(category, absent_for)] = item

    for category, tools in sorted(covered.items()):
        for tool in sorted(VALID_TOOLS - tools):
            if (category, tool) not in allowed:
                errors.append(
                    f"category {category!r} is covered for {sorted(tools)} but not for {tool!r}, and the "
                    f"manifest does not declare the asymmetry. Add a fixture, or declare it in "
                    f"'category_asymmetry' with a reason (E21-S02 / CR-TE-03)."
                )

    for category, tool in sorted(allowed):
        if tool in covered.get(category, set()):
            errors.append(
                f"category_asymmetry declares {category!r} absent for {tool!r}, but a fixture now covers it. Remove the stale declaration."
            )


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

    _check_category_symmetry(fixtures, data.get("category_asymmetry"), errors)

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

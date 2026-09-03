#!/usr/bin/env python3
"""Characterize Python v1's actual behavior on the E01-S02 fixture corpus (E01-S04).

For each fixture in tests/fixtures/manifest.json, this records what cancellai.py's
build_plan/coverage_payload actually produce and a human classification of that behavior:
NORMATIVE, INTENTIONAL_DIVERGENCE, LEGACY_ONLY, or KNOWN_DEFECT (see
docs/development/VERIFICATION_STRATEGY.md - Python reference contract). The classification
is a reviewed judgment call recorded in CLASSIFICATIONS below, not something derived from the
output automatically - a defect does not un-classify itself.

`generate` writes tests/fixtures/characterization/<fixture-id>.characterization.json.
`check` (default) regenerates in memory and diffs against the committed files, so the suite
is reproducible on a clean checkout per the story's verification contract.

Stdlib-only, like the other governance checkers.
"""

from __future__ import annotations

import argparse
import contextlib
import importlib.util
import json
import sys
import tempfile
from pathlib import Path
from types import ModuleType
from typing import Any
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
CHARACTERIZATION_DIR = ROOT / "tests" / "fixtures" / "characterization"


def _load_module(name: str, path: Path) -> ModuleType:
    """Load a sibling script/module by file location, avoiding a `scripts.` package import.

    A real `from scripts import check_fixtures` would make mypy see check_fixtures.py under
    two module names (bare and `scripts.check_fixtures`) whenever both files are checked in
    the same run - the same reason tests/test_cancellai.py loads cancellai.py this way rather
    than importing it directly.
    """
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ImportError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


check_fixtures = _load_module("cancellai_check_fixtures", ROOT / "scripts" / "check_fixtures.py")
recipes = check_fixtures.load_recipes()
cancellai = recipes.cancellai

VALID_CLASSIFICATIONS = {"NORMATIVE", "INTENTIONAL_DIVERGENCE", "LEGACY_ONLY", "KNOWN_DEFECT"}

# Reviewed judgment per fixture: (classification, rationale). Every entry here must be
# justified by what the reference documents (AS_IS.md, SAFETY_INVARIANTS.md) already commit
# to, not merely by what the code happens to do today.
CLASSIFICATIONS: dict[str, tuple[str, str]] = {
    "claude-normal-session": (
        "NORMATIVE",
        "No eligible action; a healthy no-op is normative behavior any engine must reproduce.",
    ),
    "codex-normal-session": (
        "NORMATIVE",
        "Same, for Codex: nothing inside the retention window is ever eligible.",
    ),
    "codex-subagent-tree": (
        "NORMATIVE",
        "The whole tree is selected together via the subagent graph - the documented, tested "
        "contract in AS_IS.md's 'Codex subagent graph' section, not incidental behavior.",
    ),
    "claude-active-data": (
        "NORMATIVE",
        "A session touched moments ago stays inside any real cutoff and is never selected; freshness alone must never become eligibility.",
    ),
    "claude-protected-state": (
        "NORMATIVE",
        "Every CLAUDE_PROTECTED_NAMES entry is refused even at 400 days old under --aggressive "
        "- the E00-S01 barrier - and must hold identically in the Rust target.",
    ),
    "codex-protected-state": (
        "NORMATIVE",
        "Same barrier, Codex side: every CODEX_PROTECTED_NAMES entry is refused regardless of age.",
    ),
    "claude-partial-tree": (
        "NORMATIVE",
        "The locked subtree marks the claude scan incomplete and withholds destructive "
        "authority for the whole tool (SI-008) - the documented E00-S05 behavior.",
    ),
    "codex-partial-tree": (
        "NORMATIVE",
        "The locked session directory marks the codex scan incomplete and withholds destructive "
        "authority for the whole tool (SI-008/SI-009) - the same E00-S05 behavior claude-partial-tree "
        "pins on the Claude side. Added by E21-S02 because the corpus had no Codex partial-scan "
        "fixture at all, which is why the differential gate could not observe the target engine "
        "deleting here (docs/audits/2026-09-03-CODE_REVIEW.md, CR-TE-01/CR-TE-03).",
    ),
    "claude-partial-project": (
        "NORMATIVE",
        "An unreadable *project* directory is recorded by discover_claude_sessions' own "
        "project_dir.iterdir() error branch and withholds the whole tool - a different branch from "
        "claude-partial-tree's companion payload directory, which is the only one E06-S02 repaired "
        "on the Rust side. Both must be pinned or repairing one leaves the other untested.",
    ),
    "codex-symlink-escape": (
        "NORMATIVE",
        "The injected symlink is not named like a rollout file, so discovery never surfaces it "
        "as a candidate at all; nothing outside the root is ever touched or sized.",
    ),
    "claude-symlink-protected-name": (
        "NORMATIVE",
        "A case-variant symlink of a protected name is still recognized as protected (the "
        "E00-S01 Unicode/case fix) and excluded from any candidate set.",
    ),
    "codex-layout-drift": (
        "NORMATIVE",
        "The unrecognized plugin_cache_v2/ entry is reported as coverage state 'unknown' and "
        "never appears as a plan action (the E00-S08 coverage vocabulary).",
    ),
}


class CharacterizationError(RuntimeError):
    pass


def _display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def _redact_dict(mapping: dict[str, Any], root_str: str) -> dict[str, Any]:
    return {key: _redact_paths(item, root_str) for key, item in mapping.items()}


def _redact_paths(value: Any, root_str: str) -> Any:
    """Replace every occurrence of the fixture's (fresh, random) temp path with a placeholder.

    scan.unreadable entries embed the full absolute path of whatever could not be read
    (`f"{path}: {exc.strerror}"`), so a plain field-level drop is not enough - the volatile
    prefix has to be scrubbed out of string content, recursively, wherever it appears.
    """
    if isinstance(value, str):
        return value.replace(root_str, "<PROVIDER_ROOT>")
    if isinstance(value, list):
        return [_redact_paths(item, root_str) for item in value]
    if isinstance(value, dict):
        return {key: _redact_paths(item, root_str) for key, item in value.items()}
    return value


def _normalize_plan_summary(summary: dict[str, Any], provider_root: Path) -> dict[str, Any]:
    """Drop/redact the fields that vary between otherwise-identical runs.

    `cutoff` is derived from wall-clock time; `roots.*.path` and any path embedded in
    `scan.unreadable`/`notes` are absolute paths inside a fresh temp directory every run.
    Everything else in plan_summary_dict (counts, bytes, notes text, withheld tools, scan
    completeness, root confidence/markers) is deterministic given fixed fixture recipes and
    fixed days/keep_latest/aggressive parameters.
    """
    normalized = dict(summary)
    normalized.pop("cutoff", None)
    roots = normalized.get("roots")
    if isinstance(roots, dict):
        normalized["roots"] = {tool: {k: v for k, v in root.items() if k != "path"} for tool, root in roots.items()}
    return _redact_dict(normalized, str(provider_root))


def characterize_one(fixture_id: str, tool: str) -> dict[str, Any]:
    if fixture_id not in CLASSIFICATIONS:
        raise CharacterizationError(f"{fixture_id}: no entry in CLASSIFICATIONS - every fixture must be reviewed and classified")
    classification, rationale = CLASSIFICATIONS[fixture_id]
    if classification not in VALID_CLASSIFICATIONS:
        raise CharacterizationError(f"{fixture_id}: invalid classification {classification!r}")

    with tempfile.TemporaryDirectory(prefix="cancellai-characterize-") as tmp:
        base = Path(tmp)
        provider_root = base / "provider-home"
        provider_root.mkdir()
        recipes.build(fixture_id, provider_root)

        empty_other = base / "unused-home"
        homes = {"codex": empty_other, "claude": empty_other}
        homes[tool] = provider_root

        # build_plan's destructive-authority withholding (ADR-0013) only ever applies to the
        # provider's own *default* directory - a "custom" root is always inspection-only,
        # regardless of confidence. Every fixture lives in a fresh temp directory, so without
        # this patch every fixture would show the same "withheld: custom root" outcome and
        # the more specific behavior (protected-name barrier, subagent selection, incomplete
        # scan) this corpus exists to characterize would never surface. Mirrors
        # tests/test_cancellai.py's use_as_default_roots() for the same reason.
        with mock.patch.object(cancellai, "default_home", side_effect=lambda t: homes[t]):
            plan = cancellai.build_plan(
                days=30,
                keep_latest=0,
                tools={tool},
                codex_home=homes["codex"],
                claude_home=homes["claude"],
                codex_backend="filesystem",
                aggressive=True,
                for_mutation=True,
            )
        plan_summary = _normalize_plan_summary(cancellai.plan_summary_dict(plan), provider_root)

        entries = cancellai.root_entry_sizes(provider_root)
        coverage = cancellai.coverage_payload(entries, tool)

        # Some fixtures (claude-partial-tree) lock a directory to 0o000; restore permissions
        # before the TemporaryDirectory context tries to remove the tree, or cleanup fails.
        for path in provider_root.rglob("*"):
            with contextlib.suppress(OSError):
                path.chmod(0o755)

    return {
        "fixture_id": fixture_id,
        "tool": tool,
        "classification": classification,
        "classification_rationale": rationale,
        "days": 30,
        "keep_latest": 0,
        "aggressive": True,
        "plan_summary": plan_summary,
        "coverage": coverage,
    }


def characterize_all() -> dict[str, dict[str, Any]]:
    manifest = check_fixtures.load_manifest()
    records: dict[str, dict[str, Any]] = {}
    for entry in manifest["fixtures"]:
        records[entry["id"]] = characterize_one(entry["id"], entry["tool"])
    return records


def write_records(records: dict[str, dict[str, Any]]) -> None:
    CHARACTERIZATION_DIR.mkdir(parents=True, exist_ok=True)
    for fixture_id, record in records.items():
        path = CHARACTERIZATION_DIR / f"{fixture_id}.characterization.json"
        path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")


def check() -> list[str]:
    errors: list[str] = []
    records = characterize_all()

    existing = sorted(p.name for p in CHARACTERIZATION_DIR.glob("*.characterization.json")) if CHARACTERIZATION_DIR.is_dir() else []
    expected_names = sorted(f"{fixture_id}.characterization.json" for fixture_id in records)
    if existing != expected_names:
        errors.append(
            f"committed characterization files do not match the fixture corpus: have {existing}, expected {expected_names}. "
            "Run 'python3 scripts/characterize.py generate'."
        )

    for fixture_id, record in records.items():
        path = CHARACTERIZATION_DIR / f"{fixture_id}.characterization.json"
        if not path.is_file():
            continue
        try:
            committed = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            errors.append(f"{_display_path(path)}: cannot read/parse: {exc}")
            continue
        if committed != record:
            errors.append(
                f"{_display_path(path)}: committed characterization does not match a fresh run - "
                "Python's behavior on this fixture changed, or the file is stale. Review and, if the new "
                "behavior is correct, run 'python3 scripts/characterize.py generate'."
            )

    return errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Characterize cancellai.py's behavior on the fixture corpus.")
    parser.add_argument("command", nargs="?", default="check", choices=["check", "generate"])
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.command == "generate":
        write_records(characterize_all())
        print(f"characterization written: {len(list(CHARACTERIZATION_DIR.glob('*.characterization.json')))} files")
        return 0

    try:
        errors = check()
    except CharacterizationError as exc:
        print(f"CHARACTERIZATION ERROR: {exc}", file=sys.stderr)
        return 2
    if errors:
        print("CHARACTERIZATION ERROR:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 2
    print(
        f"characterization OK: {len(list(CHARACTERIZATION_DIR.glob('*.characterization.json')))} fixtures match their committed characterization"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

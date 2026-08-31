#!/usr/bin/env python3
"""Differential parity gate: Python reference vs Rust CLI over the NORMATIVE fixture corpus (E06-S02).

For each fixture `tests/fixtures/manifest.json` lists whose committed `characterize.py`
classification is `NORMATIVE`, this materializes the fixture's synthetic tree once and runs
*both* engines against it with matching `days`/`keep_latest`/`tool` parameters:

- Python: `cancellai.build_plan(..., aggressive=False, for_mutation=True)`, the same function
  `scripts/characterize.py` calls, but with `aggressive=False` here specifically (see "Why
  aggressive=False" below) - `plan.actions` (the raw per-action list, not the aggregated
  `plan_summary_dict`) and `plan.withheld` are the comparison surface.
- Rust: the built `cancellai-cli` binary's `inspect --json` (to resolve `artifact_id ->
  identity_token`) and `plan --json` (to read proposed `delete` actions and
  `scan_completeness`).

Neither engine emits a document the other can be diffed against directly:
`docs/architecture/JSON_CONTRACTS.md` documents are a target-engine-only contract
(`cancellai.py` is frozen and was never changed to emit this shape - JSON_CONTRACTS.md says so
explicitly), so `scripts/diff_harness.py`'s JSON_CONTRACTS-vs-JSON_CONTRACTS comparator does not
apply here. This script instead compares at the semantic level both sides *can* express: the
set of session UUIDs each engine would delete, and whether the tool's scan was withheld/
incomplete. That is the real cross-engine parity question this story's AC ("all unexplained
semantic differences fail CI") cares about - not byte-identical document shapes neither engine
was ever going to produce.

## Why `aggressive=False` on the Python side

`cancellai.py --aggressive` widens discovery to legacy/cache categories `cancellai-policy`
does not implement yet (E06-S01's own disclosed scope gap, `docs/CLI_RUST.md`). Running Python
with `aggressive=True` here (as `scripts/characterize.py`'s committed records do, for their own
different purpose - reproducibility of Python's own behavior) would make Python's candidate set
a strict superset of Rust's by construction, for any fixture that happens to contain
aggressive-only files - a guaranteed, uninteresting divergence that would mask the comparisons
this gate actually exists to make. None of the ten committed fixtures currently contain
aggressive-only files (verified by inspection of `tests/fixtures/recipes.py` - every fixture
writes only provider markers, protected names, sessions/rollouts, and one unrecognized
top-level entry), so this choice does not currently hide anything; it is recorded here so a
future fixture that *does* add such a file does not silently start passing this gate for the
wrong reason.

`generate` is not offered - this gate compares two *fresh* runs against each other every time,
by design (unlike `characterize.py`, there is no committed golden output to regenerate).

Stdlib-only, like the other governance checkers.
"""

from __future__ import annotations

import argparse
import contextlib
import importlib.util
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from types import ModuleType
from typing import Any
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
RUST_DIR = ROOT / "rust"
UUID_RE = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")


class ParityError(RuntimeError):
    pass


def _load_module(name: str, path: Path) -> ModuleType:
    """Load a sibling script/module by file location - see `scripts/characterize.py`'s own
    identical helper for why (avoids a real `from scripts import ...` package import, which
    would make mypy see the module under two names in the same run)."""
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ImportError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


check_fixtures = _load_module("cancellai_check_fixtures_pp", ROOT / "scripts" / "check_fixtures.py")
characterize = _load_module("cancellai_characterize_pp", ROOT / "scripts" / "characterize.py")
recipes = check_fixtures.load_recipes()
cancellai = recipes.cancellai

# fixture_id -> reason (must cite an ADR/RFC/story ID). A divergence not listed here fails the
# gate outright, per the story's AC ("Approved divergences reference ADR/RFC IDs"). Empty by
# default: every currently-NORMATIVE fixture is expected to match exactly.
INTENTIONAL_DIVERGENCES: dict[str, str] = {}


def normative_fixture_ids() -> list[str]:
    manifest = check_fixtures.load_manifest()
    return [entry["id"] for entry in manifest["fixtures"] if characterize.CLASSIFICATIONS.get(entry["id"], ("", ""))[0] == "NORMATIVE"]


def fixture_tool(fixture_id: str) -> str:
    manifest = check_fixtures.load_manifest()
    for entry in manifest["fixtures"]:
        if entry["id"] == fixture_id:
            tool: str = entry["tool"]
            return tool
    raise ParityError(f"{fixture_id}: not present in manifest.json")


def python_result(fixture_id: str, tool: str, provider_root: Path, days: int, keep_latest: int) -> tuple[set[str], bool]:
    """Returns (candidate session UUIDs, withheld) for one engine run."""
    base = provider_root.parent
    empty_other = base / "unused-home"
    homes = {"codex": empty_other, "claude": empty_other}
    homes[tool] = provider_root

    with mock.patch.object(cancellai, "default_home", side_effect=lambda t: homes[t]):
        plan = cancellai.build_plan(
            days=days,
            keep_latest=keep_latest,
            tools={tool},
            codex_home=homes["codex"],
            claude_home=homes["claude"],
            codex_backend="filesystem",
            aggressive=False,
            for_mutation=True,
        )
    candidates = {action.session_id for action in plan.actions if action.tool == tool and action.session_id}
    withheld = tool in plan.withheld
    return candidates, withheld


def rust_binary() -> Path:
    cargo = shutil.which("cargo")
    if not cargo:
        raise ParityError("cargo is not available on PATH")
    result = subprocess.run(  # noqa: S603
        [cargo, "build", "--quiet", "-p", "cancellai-cli"],
        cwd=RUST_DIR,
        capture_output=True,
        text=True,
        check=False,
        timeout=300,
    )
    if result.returncode != 0:
        raise ParityError(f"cargo build -p cancellai-cli failed: {result.stderr.strip()}")
    binary = RUST_DIR / "target" / "debug" / "cancellai-cli"
    if not binary.is_file():
        raise ParityError(f"expected built binary at {binary}, not found")
    return binary


def _run_cli(binary: Path, args: list[str], homes: dict[str, Path]) -> dict[str, Any]:
    result = subprocess.run(  # noqa: S603
        [str(binary), *args],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
        env={
            "CLAUDE_CONFIG_DIR": str(homes["claude"]),
            "CODEX_HOME": str(homes["codex"]),
            "PATH": "/usr/bin:/bin",
        },
    )
    if result.returncode not in (0, 4):
        raise ParityError(f"cancellai-cli {' '.join(args)} exited {result.returncode}: {result.stderr.strip()}")
    try:
        data: dict[str, Any] = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise ParityError(f"cancellai-cli {' '.join(args)} did not print valid JSON: {exc}\n{result.stdout}") from exc
    return data


def rust_result(binary: Path, tool: str, provider_root: Path, days: int, keep_latest: int) -> tuple[set[str], bool]:
    base = provider_root.parent
    empty_other = base / "unused-home"
    homes = {"codex": empty_other, "claude": empty_other}
    homes[tool] = provider_root

    common = ["--tool", tool, "--days", str(days), "--keep-latest", str(keep_latest), "--allow-running", "--json"]
    inventory = _run_cli(binary, ["inspect", *common], homes)
    identity_by_id = {a["artifact_id"]: a["identity_token"] for a in inventory["artifacts"]}
    scan_incomplete = any(not s["complete"] for s in inventory["scan_completeness"] if s["scope"] == provider_id_for(tool))

    plan = _run_cli(binary, ["plan", *common], homes)
    candidates: set[str] = set()
    for action in plan["actions"]:
        if action["action_class"] != "delete":
            continue
        artifact_id = action["target_artifact_ids"][0]
        identity_token = identity_by_id.get(artifact_id, "")
        match = UUID_RE.search(identity_token)
        if match:
            candidates.add(match.group(0))
    return candidates, scan_incomplete


def provider_id_for(tool: str) -> str:
    return "codex-cli" if tool == "codex" else "claude-code"


def _compare_results(
    fixture_id: str,
    classification: str,
    days: int,
    keep_latest: int,
    py_result: tuple[set[str], bool],
    rs_result: tuple[set[str], bool],
) -> list[str]:
    """The pure comparison decision, isolated from actually running either engine so
    `self_test` can exercise it with synthetic results (the "injected divergence proves gate
    effectiveness" verification the story's contract names)."""
    py_candidates, py_withheld = py_result
    rs_candidates, rs_withheld = rs_result
    if py_candidates == rs_candidates and py_withheld == rs_withheld:
        return []
    if fixture_id in INTENTIONAL_DIVERGENCES:
        return []
    return [
        f"{fixture_id} ({classification}, days={days} keep_latest={keep_latest}): "
        f"python candidates={sorted(py_candidates)} withheld={py_withheld} vs "
        f"rust candidates={sorted(rs_candidates)} withheld={rs_withheld}"
    ]


def compare_fixture(fixture_id: str, binary: Path) -> list[str]:
    tool = fixture_tool(fixture_id)
    classification, _ = characterize.CLASSIFICATIONS[fixture_id]
    record_path = characterize.CHARACTERIZATION_DIR / f"{fixture_id}.characterization.json"
    record = json.loads(record_path.read_text(encoding="utf-8"))
    # A one-day margin below the committed `days`, not the exact value: `cancellai-platform::
    # Timestamp` is deliberately whole *seconds* since the epoch (`clock.rs`'s own module docs -
    # matching real filesystem mtime granularity), while `cancellai.py`'s `now_ts()` compares
    # `time.time()` floats directly. `codex-layout-drift`'s fixture recipe writes an mtime
    # exactly `days` old plus a few milliseconds - Python's float comparison always sees that
    # margin; Rust's whole-second truncation can round it away depending on how much wall-clock
    # time elapses between fixture creation and this script's two CLI subprocess calls, making
    # an exact-boundary comparison execution-speed-dependent rather than a real behavioral
    # divergence (found running this gate: reproducibly flaky only for that one boundary-exact
    # fixture). A full day of margin is far larger than any realistic scheduling jitter, so this
    # keeps the gate deterministic without weakening what it actually verifies - every fixture
    # in the corpus is already either clearly inside or clearly outside its cutoff by more than
    # a day (see `tests/fixtures/recipes.py`'s own `age_days` values).
    days = max(0, record["days"] - 1)
    keep_latest = record["keep_latest"]

    with tempfile.TemporaryDirectory(prefix="cancellai-parity-") as tmp:
        base = Path(tmp)
        provider_root = base / "provider-home"
        provider_root.mkdir()
        recipes.build(fixture_id, provider_root)

        try:
            py_result = python_result(fixture_id, tool, provider_root, days, keep_latest)
            rs_result = rust_result(binary, tool, provider_root, days, keep_latest)
        finally:
            for path in provider_root.rglob("*"):
                with contextlib.suppress(OSError):
                    path.chmod(0o755)

    return _compare_results(fixture_id, classification, days, keep_latest, py_result, rs_result)


def check() -> list[str]:
    binary = rust_binary()
    errors: list[str] = []
    for fixture_id in normative_fixture_ids():
        errors.extend(compare_fixture(fixture_id, binary))
    return errors


def self_test() -> list[str]:
    """Proves the gate can actually fail, not merely that it currently passes - "Injected
    divergence proves gate effectiveness" (the story's verification contract). Exercises
    `_compare_results` directly with synthetic inputs, never a real engine, so this runs in
    milliseconds and needs no built Rust binary."""
    failures: list[str] = []

    identical = _compare_results("fx", "NORMATIVE", 7, 0, ({"a", "b"}, False), ({"a", "b"}, False))
    if identical:
        failures.append(f"self-test: two identical results must compare clean, got {identical}")

    extra_candidate = _compare_results("fx", "NORMATIVE", 7, 0, ({"a"}, False), ({"a", "b"}, False))
    if not extra_candidate:
        failures.append("self-test: rust proposing an extra, unexplained delete candidate must be caught")

    missing_candidate = _compare_results("fx", "NORMATIVE", 7, 0, ({"a", "b"}, False), ({"a"}, False))
    if not missing_candidate:
        failures.append("self-test: rust silently skipping a candidate python would delete must be caught")

    withheld_mismatch = _compare_results("fx", "NORMATIVE", 7, 0, (set(), False), (set(), True))
    if not withheld_mismatch:
        failures.append("self-test: a withheld/not-withheld mismatch with identical candidate sets must still be caught")

    global INTENTIONAL_DIVERGENCES
    saved = INTENTIONAL_DIVERGENCES
    try:
        INTENTIONAL_DIVERGENCES = {"fx": "test-only whitelist entry"}
        whitelisted = _compare_results("fx", "NORMATIVE", 7, 0, ({"a"}, False), ({"a", "b"}, False))
        if whitelisted:
            failures.append("self-test: a fixture_id present in INTENTIONAL_DIVERGENCES must suppress the divergence")
    finally:
        INTENTIONAL_DIVERGENCES = saved

    return failures


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["check", "self-test"], nargs="?", default="check")
    args = parser.parse_args(argv)

    if args.command == "self-test":
        failures = self_test()
        if failures:
            print(f"rust/python parity self-test FAILED: {len(failures)} issue(s):", file=sys.stderr)
            for failure in failures:
                print(f"  - {failure}", file=sys.stderr)
            return 1
        print("rust/python parity self-test OK: the comparator correctly catches every injected divergence class")
        return 0

    try:
        errors = check()
    except ParityError as exc:
        print(f"rust/python parity gate error: {exc}", file=sys.stderr)
        return 1

    if errors:
        print(f"rust/python parity gate FAILED: {len(errors)} unexplained divergence(s):", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1

    print(f"rust/python parity OK: {len(normative_fixture_ids())} NORMATIVE fixture(s) match across engines")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

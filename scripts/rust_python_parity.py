#!/usr/bin/env python3
"""Differential parity gate: Python reference vs Rust CLI over the NORMATIVE fixture corpus (E06-S02).

For each fixture `tests/fixtures/manifest.json` lists whose committed `characterize.py`
classification is `NORMATIVE`, this materializes the fixture's synthetic tree once and runs
*both* engines against it, under *two* independent root-origin scenarios (see "Two root-origin
scenarios" below), with matching `days`/`keep_latest`/`tool` parameters:

- Python: `cancellai.build_plan(..., aggressive=False, for_mutation=True)`, the same function
  `scripts/characterize.py` calls, but with `aggressive=False` here specifically (see "Why
  aggressive=False" below).
- Rust: the built `cancellai-cli` binary's `inspect --json` (to resolve `artifact_id ->
  identity_token`) and `plan --json` (to read proposed `delete` actions, `scan_completeness`,
  and `provider_roots`).

Neither engine emits a document the other can be diffed against directly:
`docs/architecture/JSON_CONTRACTS.md` documents are a target-engine-only contract
(`cancellai.py` is frozen and was never changed to emit this shape - JSON_CONTRACTS.md says so
explicitly), so `scripts/diff_harness.py`'s JSON_CONTRACTS-vs-JSON_CONTRACTS comparator does not
apply here. This script instead compares at the semantic level both sides *can* express - see
`semantic_projection` for the full field list this gate actually checks (E06 verifier review
round 1: an earlier version compared only the delete-candidate UUID set plus one withheld
boolean, which cannot express a root-authority/confidence divergence at all).

## Two root-origin scenarios

`compare_fixture` runs every NORMATIVE fixture through two scenarios, not one:

- `default`: the fixture root is made to look like the provider's own OS-default directory to
  *both* engines (Python via `mock.patch.object(cancellai, "default_home", ...)`, Rust via a
  real `$HOME/.claude`/`$HOME/.codex` directory name and no `CLAUDE_CONFIG_DIR`/`CODEX_HOME`
  override - a Python-only patch cannot make a compiled Rust binary agree, so this is the one
  piece of scenario setup the two engines cannot share verbatim). This is the corpus' original,
  and still primary, comparison.
- `custom`: the fixture root is left exactly where it is, addressed through
  `CLAUDE_CONFIG_DIR`/`CODEX_HOME` (Rust) or an explicit, unmocked `claude_home`/`codex_home`
  kwarg (Python) - `default_home` resolves to something else entirely on both sides, so both
  engines see a genuinely non-default root. E06 verifier review round 1's finding was exactly
  that this gate, always simulating "default" on both sides by construction, could never
  surface a root-authority divergence (a stale session under a real `CLAUDE_CONFIG_DIR` custom
  root was deleted by the Rust CLI and withheld by the Python reference) - this scenario is
  that missing coverage, not a new fixture.

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

## Approved divergences must cite a real, accepted ADR/RFC

`INTENTIONAL_DIVERGENCES` maps a fixture id to a free-text reason - but a divergence is only
actually suppressed when `_citation_is_accepted_adr_or_rfc` can find a real, `Status: Accepted`
ADR document (`docs/adrs/NNNN-*.md`) or RFC whose id is cited in that text (the story's own AC:
"Approved divergences reference ADR/RFC IDs"). E06 verifier review round 1's finding was that
an earlier version accepted *any* string here, including uncited free text - `self_test` proves
both that a real citation suppresses and that an uncited one does not.

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
ADR_DIR = ROOT / "docs" / "adrs"
UUID_RE = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
ADR_CITATION_RE = re.compile(r"\bADR-(\d{4})\b")
RFC_CITATION_RE = re.compile(r"\bRFC-(\d{4})\b")


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

# fixture_id -> reason (must cite an ADR/RFC id, e.g. "ADR-0013", whose document this repository
# actually has in `docs/adrs/` with `Status: Accepted`). A divergence not listed here, or listed
# with a reason that does not cite a real accepted ADR/RFC, fails the gate outright, per the
# story's AC ("Approved divergences reference ADR/RFC IDs"). Empty by default: every currently-
# NORMATIVE fixture, in both root-origin scenarios, is expected to match exactly.
INTENTIONAL_DIVERGENCES: dict[str, str] = {}


def _citation_is_accepted_adr_or_rfc(reason: str) -> bool:
    """A divergence reason only actually suppresses a mismatch when it names a real, accepted
    ADR (a document this repository has, whose first ~15 lines contain `Status: Accepted`) or an
    RFC by the same convention. Free text - however plausible-sounding - is never enough (E06
    verifier review round 1's exact finding: `{"fx": "uncited free text"}` suppressed a real
    divergence)."""
    for match in ADR_CITATION_RE.finditer(reason):
        hits = list(ADR_DIR.glob(f"{match.group(1)}-*.md"))
        if hits and _adr_is_accepted(hits[0]):
            return True
    for match in RFC_CITATION_RE.finditer(reason):
        rfc_dir = ROOT / "docs" / "rfcs"
        hits = list(rfc_dir.glob(f"{match.group(1)}-*.md")) if rfc_dir.is_dir() else []
        if hits and _adr_is_accepted(hits[0]):
            return True
    return False


def _adr_is_accepted(path: Path) -> bool:
    head = "\n".join(path.read_text(encoding="utf-8").splitlines()[:15])
    return bool(re.search(r"^-?\s*Status:\s*Accepted\s*$", head, re.MULTILINE | re.IGNORECASE))


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


def provider_id_for(tool: str) -> str:
    return "codex-cli" if tool == "codex" else "claude-code"


def semantic_projection(
    *,
    candidates: set[str],
    withheld: bool,
    root_origin: str,
    root_confidence: str,
    mutation_eligible: bool,
    scan_complete: bool,
) -> dict[str, Any]:
    """The canonical cross-engine comparison surface (E06 verifier review round 1's required
    repair: "compare a canonical semantic projection ... including root authority, scan
    completeness ... not only deletion UUIDs and one Boolean"). `candidates` is still a UUID
    set, not a full identity token - the two engines' `identity_token` formats differ
    syntactically (a provider-relative path vs. `claude:projects/...`) for the same underlying
    session, so the session UUID remains the real matching key; everything else here is new."""
    return {
        "candidates": frozenset(candidates),
        "withheld": withheld,
        "root_origin": root_origin,
        "root_confidence": root_confidence,
        "mutation_eligible": mutation_eligible,
        "scan_complete": scan_complete,
    }


def python_result(tool: str, provider_root: Path, other_root: Path, days: int, keep_latest: int, *, simulate_default: bool) -> dict[str, Any]:
    homes = {"codex": other_root, "claude": other_root}
    homes[tool] = provider_root

    def build() -> Any:
        return cancellai.build_plan(
            days=days,
            keep_latest=keep_latest,
            tools={tool},
            codex_home=homes["codex"],
            claude_home=homes["claude"],
            codex_backend="filesystem",
            aggressive=False,
            for_mutation=True,
        )

    if simulate_default:
        with mock.patch.object(cancellai, "default_home", side_effect=lambda t: homes[t]):
            plan = build()
    else:
        # Deliberately unmocked: `default_home` resolves to the real machine's actual home,
        # which `provider_root` (a fresh temp directory) can never coincidentally equal - see
        # this module's own "Two root-origin scenarios" docs.
        plan = build()

    candidates = {action.session_id for action in plan.actions if action.tool == tool and action.session_id}
    withheld = tool in plan.withheld
    authority = plan.root_authority[tool]
    scan_complete = all(scan.complete for scan in plan.scans if scan.scope == tool)
    return semantic_projection(
        candidates=candidates,
        withheld=withheld,
        root_origin=authority.origin,
        root_confidence=authority.confidence,
        mutation_eligible=authority.destructive_allowed(),
        scan_complete=scan_complete,
    )


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


def _run_cli(binary: Path, args: list[str], env: dict[str, str]) -> dict[str, Any]:
    result = subprocess.run(  # noqa: S603
        [str(binary), *args],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
        env={**env, "PATH": "/usr/bin:/bin"},
    )
    if result.returncode not in (0, 4):
        raise ParityError(f"cancellai-cli {' '.join(args)} exited {result.returncode}: {result.stderr.strip()}")
    try:
        data: dict[str, Any] = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise ParityError(f"cancellai-cli {' '.join(args)} did not print valid JSON: {exc}\n{result.stdout}") from exc
    return data


def _rust_env(tool: str, provider_root: Path, unused_home: Path, *, simulate_default: bool) -> dict[str, str]:
    """Builds the environment a real `cancellai-cli` process needs to see `provider_root` as
    either the OS-default root (`simulate_default=True`: `HOME` points at `provider_root`'s
    parent, and `provider_root` is literally named `.claude`/`.codex` - see `compare_fixture`,
    which arranges that naming) or a genuinely custom one (`simulate_default=False`:
    `CLAUDE_CONFIG_DIR`/`CODEX_HOME` point at `provider_root` directly, with `HOME` pointed
    somewhere unrelated so the two can never coincidentally match)."""
    env_var = "CLAUDE_CONFIG_DIR" if tool == "claude" else "CODEX_HOME"
    if simulate_default:
        return {"HOME": str(provider_root.parent)}
    return {"HOME": str(unused_home), env_var: str(provider_root)}


def rust_result(
    binary: Path, tool: str, provider_root: Path, unused_home: Path, days: int, keep_latest: int, *, simulate_default: bool
) -> dict[str, Any]:
    env = _rust_env(tool, provider_root, unused_home, simulate_default=simulate_default)
    common = ["--tool", tool, "--days", str(days), "--keep-latest", str(keep_latest), "--allow-running", "--json"]
    inventory = _run_cli(binary, ["inspect", *common], env)
    identity_by_id = {a["artifact_id"]: a["identity_token"] for a in inventory["artifacts"]}
    pid = provider_id_for(tool)
    scan_complete = all(s["complete"] for s in inventory["scan_completeness"] if s["scope"] == pid)
    root_doc = next(r for r in inventory["provider_roots"] if r["provider_id"] == pid)

    plan = _run_cli(binary, ["plan", *common], env)
    candidates: set[str] = set()
    for action in plan["actions"]:
        if action["action_class"] != "delete":
            continue
        artifact_id = action["target_artifact_ids"][0]
        identity_token = identity_by_id.get(artifact_id, "")
        match = UUID_RE.search(identity_token)
        if match:
            candidates.add(match.group(0))
    # Mirrors `cancellai.py::build_plan`'s own `plan.withheld` construction exactly: destructive
    # work for this tool was withheld either because the root is not mutation-eligible (not the
    # default root) or because the scan was incomplete - never inferred from action reason text.
    withheld = (not root_doc["mutation_eligible"]) or (not scan_complete)
    return semantic_projection(
        candidates=candidates,
        withheld=withheld,
        root_origin=root_doc["origin"],
        root_confidence=root_doc["confidence"],
        mutation_eligible=root_doc["mutation_eligible"],
        scan_complete=scan_complete,
    )


def _compare_results(
    fixture_id: str,
    classification: str,
    days: int,
    keep_latest: int,
    scenario: str,
    py_result: dict[str, Any],
    rs_result: dict[str, Any],
) -> list[str]:
    """The pure comparison decision, isolated from actually running either engine so
    `self_test` can exercise it with synthetic results (the "injected divergence proves gate
    effectiveness" verification the story's contract names). Reports every field that diverged,
    not just "something differs" - each is independently meaningful (SI-002/SI-008/SI-009)."""
    diffs = [key for key in py_result if py_result[key] != rs_result[key]]
    if not diffs:
        return []
    if fixture_id in INTENTIONAL_DIVERGENCES and _citation_is_accepted_adr_or_rfc(INTENTIONAL_DIVERGENCES[fixture_id]):
        return []
    detail = "; ".join(f"{key}: python={py_result[key]!r} vs rust={rs_result[key]!r}" for key in diffs)
    return [f"{fixture_id} [{scenario}] ({classification}, days={days} keep_latest={keep_latest}): {detail}"]


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

    errors: list[str] = []
    for scenario, simulate_default in (("default", True), ("custom", False)):
        with tempfile.TemporaryDirectory(prefix="cancellai-parity-") as tmp:
            base = Path(tmp)
            if simulate_default:
                # Literally named `.claude`/`.codex` so the *real* Rust default-root resolution
                # (`$HOME/.claude`) finds it without any `CLAUDE_CONFIG_DIR` override - a Python-
                # side `mock.patch` cannot make a separate compiled binary agree.
                home = base / "home"
                provider_root = home / (".claude" if tool == "claude" else ".codex")
            else:
                provider_root = base / "custom-root"
            provider_root.mkdir(parents=True)
            recipes.build(fixture_id, provider_root)
            unused_home = base / "unused-home"

            try:
                py_result = python_result(tool, provider_root, unused_home, days, keep_latest, simulate_default=simulate_default)
                rs_result = rust_result(binary, tool, provider_root, unused_home, days, keep_latest, simulate_default=simulate_default)
            finally:
                for path in provider_root.rglob("*"):
                    with contextlib.suppress(OSError):
                        path.chmod(0o755)

        errors.extend(_compare_results(fixture_id, classification, days, keep_latest, scenario, py_result, rs_result))
    return errors


def check() -> list[str]:
    binary = rust_binary()
    errors: list[str] = []
    for fixture_id in normative_fixture_ids():
        errors.extend(compare_fixture(fixture_id, binary))
    return errors


def self_test() -> list[str]:
    """Proves the gate can actually fail, not merely that it currently passes - "Injected
    divergence proves gate effectiveness" (the story's verification contract). Exercises
    `_compare_results`/`_citation_is_accepted_adr_or_rfc` directly with synthetic inputs, never
    a real engine, so this runs in milliseconds and needs no built Rust binary."""
    failures: list[str] = []

    def proj(
        candidates: set[str],
        withheld: bool,
        *,
        origin: str = "default",
        confidence: str = "default",
        eligible: bool | None = None,
        scan_complete: bool = True,
    ) -> dict[str, Any]:
        return semantic_projection(
            candidates=candidates,
            withheld=withheld,
            root_origin=origin,
            root_confidence=confidence,
            mutation_eligible=eligible if eligible is not None else origin == "default",
            scan_complete=scan_complete,
        )

    identical = _compare_results("fx", "NORMATIVE", 7, 0, "default", proj({"a", "b"}, False), proj({"a", "b"}, False))
    if identical:
        failures.append(f"self-test: two identical results must compare clean, got {identical}")

    extra_candidate = _compare_results("fx", "NORMATIVE", 7, 0, "default", proj({"a"}, False), proj({"a", "b"}, False))
    if not extra_candidate:
        failures.append("self-test: rust proposing an extra, unexplained delete candidate must be caught")

    missing_candidate = _compare_results("fx", "NORMATIVE", 7, 0, "default", proj({"a", "b"}, False), proj({"a"}, False))
    if not missing_candidate:
        failures.append("self-test: rust silently skipping a candidate python would delete must be caught")

    withheld_mismatch = _compare_results("fx", "NORMATIVE", 7, 0, "default", proj(set(), False), proj(set(), True))
    if not withheld_mismatch:
        failures.append("self-test: a withheld/not-withheld mismatch with identical candidate sets must still be caught")

    # E06 verifier review round 1's exact reproduction: identical candidate sets, but Rust
    # reports the root as `default` when Python (correctly) sees it as `custom` - the previous
    # comparator (candidates + one bool only) could not express this at all.
    root_origin_mismatch = _compare_results(
        "fx",
        "NORMATIVE",
        7,
        0,
        "custom",
        proj({"a"}, True, origin="custom", confidence="low", eligible=False),
        proj({"a"}, True, origin="default", confidence="default", eligible=True),
    )
    if not root_origin_mismatch:
        failures.append("self-test: a root_origin/mutation_eligible mismatch must be caught even with identical candidates and withheld")

    scan_complete_mismatch = _compare_results(
        "fx",
        "NORMATIVE",
        7,
        0,
        "default",
        proj(set(), True, scan_complete=False),
        proj(set(), True, scan_complete=True),
    )
    if not scan_complete_mismatch:
        failures.append("self-test: a scan_complete mismatch must be caught")

    global INTENTIONAL_DIVERGENCES
    saved = INTENTIONAL_DIVERGENCES
    try:
        INTENTIONAL_DIVERGENCES = {"fx": "uncited free text, not a real ADR/RFC id"}
        uncited = _compare_results("fx", "NORMATIVE", 7, 0, "default", proj({"a"}, False), proj({"a", "b"}, False))
        if not uncited:
            failures.append(
                "self-test: an uncited free-text divergence entry must NOT suppress a real mismatch "
                "(E06 verifier review round 1's exact finding)"
            )

        INTENTIONAL_DIVERGENCES = {"fx": "fabricated citation ADR-9999 does not exist in docs/adrs/"}
        fabricated = _compare_results("fx", "NORMATIVE", 7, 0, "default", proj({"a"}, False), proj({"a", "b"}, False))
        if not fabricated:
            failures.append("self-test: a citation to a non-existent ADR id must NOT suppress a real mismatch")

        real_adrs = sorted(ADR_DIR.glob("*-*.md"))
        if not real_adrs:
            failures.append("self-test: no ADR documents found under docs/adrs/ to validate the citation check against")
        else:
            real_id = real_adrs[-1].name.split("-", 1)[0]
            if not _adr_is_accepted(real_adrs[-1]):
                failures.append(f"self-test: expected the newest ADR ({real_adrs[-1].name}) to be Status: Accepted")
            INTENTIONAL_DIVERGENCES = {"fx": f"see ADR-{real_id} for the accepted rationale"}
            whitelisted = _compare_results("fx", "NORMATIVE", 7, 0, "default", proj({"a"}, False), proj({"a", "b"}, False))
            if whitelisted:
                failures.append("self-test: a fixture_id whose reason cites a real, accepted ADR must suppress the divergence")
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

    print(f"rust/python parity OK: {len(normative_fixture_ids())} NORMATIVE fixture(s) match across engines, in both root-origin scenarios")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

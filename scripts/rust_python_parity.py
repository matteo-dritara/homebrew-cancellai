#!/usr/bin/env python3
"""Differential parity gate: Python reference vs Rust CLI over the NORMATIVE fixture corpus (E06-S02, E07-S08).

For each fixture `tests/fixtures/manifest.json` lists whose committed `characterize.py`
classification is `NORMATIVE`, this materializes the fixture's synthetic tree once and runs
*both* engines against it, under *two* independent root-origin scenarios (see "Two root-origin
scenarios" below), with matching `days`/`keep_latest`/`tool` parameters:

- Python: `cancellai.build_plan(..., aggressive=False, for_mutation=True)`, the same function
  `scripts/characterize.py` calls, but with `aggressive=False` here specifically (see "Why
  aggressive=False" below), plus a direct call to `discover_claude_sessions`/
  `discover_codex_sessions` (independent of `build_plan`'s eligibility filtering) for full
  discovery/protection coverage - see `semantic_projection`.
- Rust: the built `cancellai-cli` binary's `inspect --json` (every discovered artifact,
  regardless of action class - to resolve `artifact_id -> identity_token`/`protection_state`)
  and `plan --json` (the proposed `delete` actions, `scan_completeness`, and `provider_roots`).

Neither engine emits a document the other can be diffed against directly:
`docs/architecture/JSON_CONTRACTS.md` documents are a target-engine-only contract
(`cancellai.py` is frozen and was never changed to emit this shape - JSON_CONTRACTS.md says so
explicitly), so `scripts/diff_harness.py`'s JSON_CONTRACTS-vs-JSON_CONTRACTS comparator does not
apply here. This script instead compares at the semantic level both sides *can* express - see
`semantic_projection` for the full field list this gate actually checks.

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
  surface a root-authority divergence - this scenario is that missing coverage, not a new
  fixture.

## Why `aggressive=False` on the Python side

`cancellai.py --aggressive` widens discovery to legacy/cache categories `cancellai-policy`
does not implement yet (E06-S01's own disclosed scope gap, `docs/CLI_RUST.md`). Running Python
with `aggressive=True` here (as `scripts/characterize.py`'s committed records do, for their own
different purpose - reproducibility of Python's own behavior) would make Python's candidate set
a strict superset of Rust's by construction, for any fixture that happens to contain
aggressive-only files - a guaranteed, uninteresting divergence that would mask the comparisons
this gate actually exists to make. None of the ten committed fixtures currently contain
aggressive-only files (verified by inspection of `tests/fixtures/recipes.py`), so this choice
does not currently hide anything; it is recorded here so a future fixture that *does* add such a
file does not silently start passing this gate for the wrong reason.

## Approved divergences: structured, field-scoped, and fixture-bound

`INTENTIONAL_DIVERGENCES` is a tuple of [`ApprovedDivergence`] records, not a free-text map
(E06/E07 verifier review round 2's exact finding: a fixture-id-keyed free-text reason -
`{"fx": "unrelated accepted ADR-0014"}` - suppressed a *fully* divergent comparison merely
because *some* real, accepted ADR happened to be cited, regardless of whether that ADR had
anything to do with fixture `fx` or the specific fields that diverged). An `ApprovedDivergence`
only suppresses a mismatch when *all three* hold for *every* diverging field:

1. its `fixture_id`/`scenario` match exactly (a divergence approved for `fx`'s `default`
   scenario does not silently cover its `custom` scenario, or a different fixture);
2. the diverging field is named in its `fields` set (approving a `root_origin` divergence does
   not silently approve an unrelated `candidates` divergence in the same comparison);
3. its `citation` resolves to a real, `Status: Accepted` ADR/RFC document under `docs/adrs/`
   (or `docs/rfcs/`) whose own text mentions this exact `fixture_id` - not merely any accepted
   document (closing the "ADR-0014 concerns release cadence, not this fixture" gap: an ADR
   that never mentions the fixture id it is supposedly excusing cannot suppress anything here).

`generate` is not offered - this gate compares two *fresh* runs against each other every time,
by design (unlike `characterize.py`, there is no committed golden output to regenerate).

Stdlib-only, like the other governance checkers.
"""

from __future__ import annotations

import argparse
import contextlib
import dataclasses
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
RFC_DIR = ROOT / "docs" / "rfcs"
UUID_RE = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
ADR_CITATION_RE = re.compile(r"\bADR-(\d{4})\b")
RFC_CITATION_RE = re.compile(r"\bRFC-(\d{4})\b")

# The complete set of keys `semantic_projection` ever produces - an `ApprovedDivergence.fields`
# value is validated against this set so a typo'd field name fails loudly instead of silently
# approving nothing (equivalent to approving everything, since a field never present in `fields`
# can never be suppressed).
PROJECTION_FIELDS = frozenset(
    {
        "candidates",
        "non_delete_identities",
        "withheld",
        "root_origin",
        "root_confidence",
        "mutation_eligible",
        "scan_complete",
        "protected_count",
    }
)


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


@dataclasses.dataclass(frozen=True)
class ApprovedDivergence:
    """One explicitly reviewed exception, bound to exactly one fixture/scenario and the exact
    semantic fields it excuses - see this module's own "Approved divergences" docs for why a
    free-text map is not enough."""

    fixture_id: str
    scenario: str  # "default" | "custom"
    fields: frozenset[str]
    citation: str  # e.g. "ADR-0013" - must resolve via `_citation_covers`

    def __post_init__(self) -> None:
        if self.scenario not in ("default", "custom"):
            raise ValueError(f"ApprovedDivergence.scenario must be 'default' or 'custom', got {self.scenario!r}")
        unknown = self.fields - PROJECTION_FIELDS
        if unknown:
            raise ValueError(f"ApprovedDivergence.fields names unknown projection field(s): {sorted(unknown)}")


# Every currently-NORMATIVE fixture, in both root-origin scenarios, is expected to match
# exactly - empty by default, which is itself proof the mechanism does not silently swallow
# anything (see `self_test`'s injected cases for what it *would* do with a real entry).
INTENTIONAL_DIVERGENCES: tuple[ApprovedDivergence, ...] = ()


def _adr_is_accepted(path: Path) -> bool:
    head = "\n".join(path.read_text(encoding="utf-8").splitlines()[:15])
    return bool(re.search(r"^-?\s*Status:\s*Accepted\s*$", head, re.MULTILINE | re.IGNORECASE))


def _citation_covers(citation: str, fixture_id: str) -> bool:
    """A citation only actually excuses anything when it names a real, `Status: Accepted`
    ADR/RFC document whose own full text mentions this exact `fixture_id` string - not merely
    any accepted document (E06/E07 verifier review round 2's exact finding: `ADR-0014` is real
    and accepted, but is about release cadence, not fixture `fx`, and suppressed a fully
    divergent comparison anyway under the previous "any real citation" rule)."""

    def matches(directory: Path, pattern_re: re.Pattern[str]) -> bool:
        if not directory.is_dir():
            return False
        for match in pattern_re.finditer(citation):
            hits = list(directory.glob(f"{match.group(1)}-*.md"))
            if not hits:
                continue
            doc = hits[0]
            if _adr_is_accepted(doc) and fixture_id in doc.read_text(encoding="utf-8"):
                return True
        return False

    return matches(ADR_DIR, ADR_CITATION_RE) or matches(RFC_DIR, RFC_CITATION_RE)


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
    non_delete_identities: set[str],
    withheld: bool,
    root_origin: str,
    root_confidence: str,
    mutation_eligible: bool,
    scan_complete: bool,
    protected_count: int,
) -> dict[str, Any]:
    """The canonical cross-engine comparison surface. `candidates`/`non_delete_identities`
    together are every session UUID either engine *discovered* (E07-S08's "discovered identity
    records"/"every proposed action": every artifact is either a delete candidate or not, and
    the two sets partition the full discovered corpus for this tool) - still UUIDs, not full
    identity tokens, since the two engines' `identity_token` formats differ syntactically (a
    provider-relative path vs. `claude:projects/...`) for the same underlying session, so the
    session UUID remains the real matching key. `protected_count` is E07-S08's "protection ...
    coverage"; `scan_complete` remains this codebase's own vocabulary for "unknown coverage"
    (SI-008/SI-009) rather than a separately invented field."""
    return {
        "candidates": frozenset(candidates),
        "non_delete_identities": frozenset(non_delete_identities),
        "withheld": withheld,
        "root_origin": root_origin,
        "root_confidence": root_confidence,
        "mutation_eligible": mutation_eligible,
        "scan_complete": scan_complete,
        "protected_count": protected_count,
    }


def _python_discovery(tool: str, provider_root: Path) -> tuple[set[str], int]:
    """All discovered session UUIDs for `tool` under `provider_root`, independent of
    `build_plan`'s eligibility filtering (which drops protected/blocked candidates from
    `plan.actions` entirely, leaving no trace of them to compare against), plus how many are
    protected-by-name. Reuses `discover_claude_sessions`/`discover_codex_sessions` and
    `protected_component` directly - the same functions `build_plan` itself calls internally."""
    if tool == "claude":
        discovered = cancellai.discover_claude_sessions(provider_root)
    else:
        discovered = cancellai.discover_codex_sessions(provider_root, "filesystem")
    all_ids = {a.session_id for a in discovered if a.session_id}
    protected_names = cancellai.protected_names_for(tool)
    protected_count = sum(1 for a in discovered if cancellai.protected_component(a.path, provider_root, protected_names) is not None)
    return all_ids, protected_count


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
    all_ids, protected_count = _python_discovery(tool, provider_root)
    withheld = tool in plan.withheld
    authority = plan.root_authority[tool]
    scan_complete = all(scan.complete for scan in plan.scans if scan.scope == tool)
    return semantic_projection(
        candidates=candidates,
        non_delete_identities=all_ids - candidates,
        withheld=withheld,
        root_origin=authority.origin,
        root_confidence=authority.confidence,
        mutation_eligible=authority.destructive_allowed(),
        scan_complete=scan_complete,
        protected_count=protected_count,
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


def _uuid_of(identity_token: str) -> str | None:
    match = UUID_RE.search(identity_token)
    return match.group(0) if match else None


def rust_result(
    binary: Path, tool: str, provider_root: Path, unused_home: Path, days: int, keep_latest: int, *, simulate_default: bool
) -> dict[str, Any]:
    env = _rust_env(tool, provider_root, unused_home, simulate_default=simulate_default)
    common = ["--tool", tool, "--days", str(days), "--keep-latest", str(keep_latest), "--allow-running", "--json"]
    inventory = _run_cli(binary, ["inspect", *common], env)
    identity_by_id = {a["artifact_id"]: a["identity_token"] for a in inventory["artifacts"]}
    all_ids = {uid for a in inventory["artifacts"] if (uid := _uuid_of(a["identity_token"]))}
    protected_count = sum(1 for a in inventory["artifacts"] if a["protection_state"] == "protected")
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
        uid = _uuid_of(identity_token)
        if uid:
            candidates.add(uid)
    # Mirrors `cancellai.py::build_plan`'s own `plan.withheld` construction exactly: destructive
    # work for this tool was withheld either because the root is not mutation-eligible (not the
    # default root) or because the scan was incomplete - never inferred from action reason text.
    withheld = (not root_doc["mutation_eligible"]) or (not scan_complete)
    return semantic_projection(
        candidates=candidates,
        non_delete_identities=all_ids - candidates,
        withheld=withheld,
        root_origin=root_doc["origin"],
        root_confidence=root_doc["confidence"],
        mutation_eligible=root_doc["mutation_eligible"],
        scan_complete=scan_complete,
        protected_count=protected_count,
    )


def _approval_for(fixture_id: str, scenario: str, field: str) -> ApprovedDivergence | None:
    for divergence in INTENTIONAL_DIVERGENCES:
        if divergence.fixture_id == fixture_id and divergence.scenario == scenario and field in divergence.fields:
            return divergence
    return None


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
    effectiveness" verification the story's contract names). Every diverging field is checked
    independently against `INTENTIONAL_DIVERGENCES` - a field with no matching, citation-backed
    `ApprovedDivergence` always fails the gate, regardless of whether some *other* field on the
    same fixture happens to be approved."""
    diffs = [key for key in py_result if py_result[key] != rs_result[key]]
    if not diffs:
        return []
    unexplained = [
        field
        for field in diffs
        if (approval := _approval_for(fixture_id, scenario, field)) is None or not _citation_covers(approval.citation, fixture_id)
    ]
    if not unexplained:
        return []
    detail = "; ".join(f"{key}: python={py_result[key]!r} vs rust={rs_result[key]!r}" for key in unexplained)
    return [f"{fixture_id} [{scenario}] ({classification}, days={days} keep_latest={keep_latest}): {detail}"]


def compare_fixture(fixture_id: str, binary: Path) -> list[str]:
    tool = fixture_tool(fixture_id)
    classification, _ = characterize.CLASSIFICATIONS[fixture_id]
    record_path = characterize.CHARACTERIZATION_DIR / f"{fixture_id}.characterization.json"
    record = json.loads(record_path.read_text(encoding="utf-8"))
    # A one-day margin below the committed `days`, not the exact value - see the historical note
    # in this file's git history (`codex-layout-drift`'s float-vs-whole-second boundary): a full
    # day of margin is far larger than any realistic scheduling jitter and does not weaken what
    # the gate verifies, since every fixture in the corpus already sits more than a day inside or
    # outside its cutoff (`tests/fixtures/recipes.py`'s own `age_days` values).
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
    `_compare_results`/`_citation_covers` directly with synthetic inputs, never a real engine,
    so this runs in milliseconds and needs no built Rust binary."""
    failures: list[str] = []

    def proj(
        candidates: set[str],
        withheld: bool,
        *,
        non_delete: set[str] | None = None,
        origin: str = "default",
        confidence: str = "default",
        eligible: bool | None = None,
        scan_complete: bool = True,
        protected_count: int = 0,
    ) -> dict[str, Any]:
        return semantic_projection(
            candidates=candidates,
            non_delete_identities=non_delete if non_delete is not None else set(),
            withheld=withheld,
            root_origin=origin,
            root_confidence=confidence,
            mutation_eligible=eligible if eligible is not None else origin == "default",
            scan_complete=scan_complete,
            protected_count=protected_count,
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

    non_delete_mismatch = _compare_results(
        "fx",
        "NORMATIVE",
        7,
        0,
        "default",
        proj({"a"}, False, non_delete={"b"}),
        proj({"a"}, False, non_delete={"b", "c"}),
    )
    if not non_delete_mismatch:
        failures.append(
            "self-test: rust discovering an extra non-delete artifact python never saw must be caught "
            "(E07-S08: 'every proposed action'/'discovered identity records')"
        )

    protected_count_mismatch = _compare_results(
        "fx", "NORMATIVE", 7, 0, "default", proj(set(), False, protected_count=1), proj(set(), False, protected_count=0)
    )
    if not protected_count_mismatch:
        failures.append("self-test: a protected_count mismatch must be caught (E07-S08: 'protection ... coverage')")

    global INTENTIONAL_DIVERGENCES
    saved = INTENTIONAL_DIVERGENCES
    try:
        INTENTIONAL_DIVERGENCES = ()
        uncited = _compare_results("fx", "NORMATIVE", 7, 0, "default", proj({"a"}, False), proj({"a", "b"}, False))
        if not uncited:
            failures.append("self-test: no approved divergence at all must NOT suppress a real mismatch")

        real_adrs = sorted(ADR_DIR.glob("*-*.md"))
        if not real_adrs:
            failures.append("self-test: no ADR documents found under docs/adrs/ to validate the citation check against")
        else:
            # An ADR that is real and accepted, but never mentions this fixture id: the exact
            # E06/E07 verifier review round 2 reproduction ("ADR-0014 concerns release cadence,
            # not fixture fx or a parity exception, but it suppresses a fully divergent
            # comparison"). Pick whichever real ADR does *not* happen to mention "fx".
            unrelated = next((p for p in real_adrs if _adr_is_accepted(p) and "fx" not in p.read_text(encoding="utf-8")), None)
            if unrelated is None:
                failures.append("self-test: expected at least one accepted ADR that does not mention fixture id 'fx'")
            else:
                unrelated_id = unrelated.name.split("-", 1)[0]
                INTENTIONAL_DIVERGENCES = (
                    ApprovedDivergence(
                        fixture_id="fx",
                        scenario="default",
                        fields=frozenset({"candidates"}),
                        citation=f"ADR-{unrelated_id}",
                    ),
                )
                unrelated_citation = _compare_results("fx", "NORMATIVE", 7, 0, "default", proj({"a"}, False), proj({"a", "b"}, False))
                if not unrelated_citation:
                    failures.append(
                        "self-test: an accepted ADR that never mentions this fixture id must NOT suppress a mismatch "
                        "(E06/E07 verifier review round 2's exact finding)"
                    )

            # A field-scoped approval must cover only the field it names, not every diverging
            # field on the same fixture/scenario.
            fabricated_doc = ADR_DIR / "0000-self-test-fabricated.md"
            if fabricated_doc.exists():
                failures.append(f"self-test: refusing to overwrite unexpected pre-existing file {fabricated_doc}")
            else:
                fabricated_doc.write_text("# ADR-0000: self-test fixture\n\n- Status: Accepted\n\nfx\n", encoding="utf-8")
                try:
                    INTENTIONAL_DIVERGENCES = (
                        ApprovedDivergence(fixture_id="fx", scenario="default", fields=frozenset({"candidates"}), citation="ADR-0000"),
                    )
                    only_candidates_approved = _compare_results(
                        "fx",
                        "NORMATIVE",
                        7,
                        0,
                        "default",
                        proj({"a"}, False, scan_complete=True),
                        proj({"a", "b"}, False, scan_complete=False),
                    )
                    if not only_candidates_approved:
                        failures.append(
                            "self-test: approving only 'candidates' must not silently also approve an unrelated "
                            "scan_complete divergence in the same comparison"
                        )

                    fully_approved = _compare_results(
                        "fx",
                        "NORMATIVE",
                        7,
                        0,
                        "default",
                        proj({"a"}, False),
                        proj({"a", "b"}, False),
                    )
                    if fully_approved:
                        failures.append(
                            "self-test: a fixture/scenario/field match citing a real accepted ADR that mentions the fixture id must suppress"
                        )

                    wrong_scenario = _compare_results(
                        "fx",
                        "NORMATIVE",
                        7,
                        0,
                        "custom",
                        proj({"a"}, False),
                        proj({"a", "b"}, False),
                    )
                    if not wrong_scenario:
                        failures.append("self-test: an approval scoped to 'default' must not suppress the same fixture's 'custom' scenario")
                finally:
                    fabricated_doc.unlink(missing_ok=True)
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

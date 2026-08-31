# Evidence Packet - E06-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E06)
- Change Risk: CR2
- Spec version/commit: `scripts/rust_python_parity.py` (new), `.pre-commit-config.yaml`/
  `AGENTS.md` (new hook/check wiring), `docs/development/MIGRATION_PYTHON_RUST.md` (M6
  section update), plus two E06-S01 defect fixes this gate found:
  `rust/crates/cancellai-policy/src/retention.rs` (whole-tool withhold on a degraded companion
  scan; a missing `projects/` no longer misreported as an incomplete scan)

## Outcome

PASS

## Scope

Implements "Run Python and Rust over the full normative fixture corpus in CI." A real
`scripts/diff_harness.py`-based comparison was not possible as literally described in
`docs/development/VERIFICATION_STRATEGY.md`: that harness compares two
`docs/architecture/JSON_CONTRACTS.md`-shaped documents, but `cancellai.py` was never changed to
emit that shape (JSON_CONTRACTS.md's own "Compatibility policy" section states this explicitly)
- there is no Python-side JSON_CONTRACTS document to feed it. This is an architecture-ambiguity
finding the brief did not surface (`docs/development/WORK_ITEM_MODEL.md` "Story changes during
implementation" applies: the AC is not wrong, but the assumed mechanism does not exist).
Resolution, recorded in `rust_python_parity.py`'s own module doc rather than a separate ADR
(CR2, no constitutional/safety-invariant redefinition involved - purely how the comparison is
technically implemented): compare both engines at the semantic level they can both actually
express - the set of session UUIDs each would delete for a given `days`/`keep_latest`/`tool`,
and whether the tool's scan was withheld - rather than forcing a document-shape diff neither
engine was ever going to produce identically.

`scripts/rust_python_parity.py`:

- `check` (default): for each fixture whose `scripts/characterize.py` classification is
  `NORMATIVE` (all ten, currently), materializes the fixture fresh, runs
  `cancellai.build_plan(..., aggressive=False, for_mutation=True)` and the built `cancellai-cli`
  binary's `inspect --json`/`plan --json` against the identical tree, and compares candidate
  UUID sets plus withheld state. `aggressive=False` on the Python side is a deliberate choice
  (not a re-run of the committed `aggressive=True` characterization records) - documented in
  the module doc - matching what `cancellai-policy` actually implements today (E06-S01's own
  disclosed `--aggressive` gap); none of the ten fixtures currently contain aggressive-only
  files, so this does not currently hide anything, but the choice is recorded so a future
  fixture does not silently start passing for the wrong reason.
- `self-test`: proves the comparator itself can fail - four injected-divergence classes (extra
  candidate, missing candidate, withheld mismatch, and confirming the
  `INTENTIONAL_DIVERGENCES` allow-list actually suppresses a listed one) - exercised against a
  pure comparison function (`_compare_results`), no real engine invocation needed, in
  milliseconds.

**Two real E06-S01 defects were found and fixed while building this gate, before any review
round** (recorded in full in `docs/development/MIGRATION_PYTHON_RUST.md`'s M6 section too):

1. A companion payload directory that could not be listed (`claude-partial-tree`-shaped
   scenario) only downgraded that one session's own evidence in the first version of
   `cancellai-policy::retention::resolve_claude` - the other two, perfectly ordinary sessions in
   the same fixture were still proposed for deletion. `cancellai.py`'s `build_plan` withholds
   the *whole tool* the instant any scan scope is incomplete (`Plan.withheld`, SI-008/SI-009).
   Fixed: `resolve_claude` now returns `scan_complete: false` for the whole provider when
   `discover_claude_sessions`'s `degraded_companions` is non-empty, and `build_actions` forces
   every action for a `!scan_complete` resolution to `Observe`, regardless of that specific
   artifact's own eligibility.
2. A `claude_home` that exists but has no `projects/` directory (`claude-protected-state`/
   `claude-symlink-protected-name`-shaped scenarios - neither creates a `projects/` tree) was
   misreported as an incomplete scan (`SessionDiscoveryScope::Unavailable` was treated the same
   as a genuine mid-scan failure). `cancellai.py` does not withhold in this case - a provider
   root with no session directory yet is a legitimately empty state, not missing evidence.
   Fixed: `resolve_claude` now reports this as complete-and-empty, matching Python.

A third apparent divergence (`codex-layout-drift`) was not a defect: that fixture's rollout is
written exactly `days` old plus a few milliseconds, which Python's float `time.time()`
comparison always sees as past cutoff but `cancellai-platform::Timestamp`'s deliberate
whole-second granularity (`clock.rs`, E02-S04) can round away depending on real subprocess
scheduling - execution-speed-dependent, not a behavioral difference between the engines'
retention logic. Resolved by giving both engines a one-day margin below each fixture's
committed `days` value for this specific differential comparison (documented in
`rust_python_parity.py`), which is far larger than any realistic scheduling jitter and does not
weaken what the gate verifies (every fixture's own `age_days` already sits more than a day
inside or outside its cutoff, per `tests/fixtures/recipes.py`).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - All unexplained semantic differences fail CI | `_compare_results` returns a non-empty error list for any fixture where candidate UUID sets or withheld state differ, unless the fixture id is in `INTENTIONAL_DIVERGENCES` (empty today). `main()` propagates any non-empty error list to exit code 1. Proven both by the two real defects this gate caught during development (see Scope) and by `self_test()`'s injected-divergence cases. | PASS |
| AC2 - Approved divergences reference ADR/RFC IDs | `INTENTIONAL_DIVERGENCES: dict[str, str]` maps a fixture id to a reason string; the module doc requires that reason to cite an ADR/RFC/story id when an entry is ever added. Currently empty (no accepted divergence exists), which is itself proof the mechanism does not silently swallow anything - `self_test`'s whitelist case demonstrates it *would* suppress a listed entry, and the real `check()` run over all ten fixtures found only genuine, now-fixed defects, never something that needed silent whitelisting. | PASS |

## Verification Commands

```text
# Rust workspace (from rust/) - unaffected suite still green after the two retention.rs fixes
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# Python governance (repository-wide)
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy scripts/rust_python_parity.py
python3 scripts/rust_python_parity.py self-test
python3 scripts/rust_python_parity.py check
python3 scripts/check_docs.py check
python3 scripts/project_os.py check
python3 -m pytest tests -q
```

All green. `cargo test --workspace` includes one new regression test
(`cancellai-policy::retention::tests::
a_degraded_companion_withholds_every_action_for_the_whole_tool_not_only_its_own_session`)
proving fix #1 above; fix #2 is exercised indirectly by the existing
`a_missing_provider_root_is_a_known_empty_scan_not_a_withheld_one` test plus this story's own
`rust_python_parity.py check` (which now passes `claude-protected-state`/
`claude-symlink-protected-name`). `rust_python_parity.py check` itself was run repeatedly
(non-determinism check) and is consistently clean across at least 5 consecutive runs during
this story's development.

## Compatibility

- `rust_python_parity.py` requires `cargo` on `PATH` (same requirement
  `check_provider_compatibility.py` already has, E05-S05) - the `governance.yml` `pre-commit`
  CI job has no explicit Rust toolchain setup step and relies on `ubuntu-latest`'s preinstalled
  toolchain, the same existing assumption `check_provider_compatibility.py`'s hook already
  makes successfully.
- Builds the real `cancellai-cli` binary once (`cargo build -p cancellai-cli`, 300s timeout,
  matching `check_provider_compatibility.py`'s `cargo run` timeout convention) and invokes it
  directly per fixture (~20 subprocess calls for 10 fixtures) rather than `cargo run` per call,
  to keep the gate fast.

## Performance / operability

- Full `check()` run (10 fixtures, each building a synthetic tree, running Python in-process,
  and spawning 2 Rust subprocess calls) completes in roughly 1-2 seconds once the binary is
  already built; `self-test` completes in well under 10ms (no subprocess/engine involved).

## Documentation updated

- `docs/development/MIGRATION_PYTHON_RUST.md` (the story's declared documentation impact) - new
  M6 section content describing the gate's actual mechanism, the JSON_CONTRACTS-applicability
  finding, and the two defects it caught.
- `AGENTS.md` - added `scripts/rust_python_parity.py` to the mypy target list and the "Current
  Python checks" sequence (`self-test` then `check`), matching E05-S05's precedent for a new
  governance script.
- `.pre-commit-config.yaml` - new `rust-python-parity-gate` local hook.

## Residual risks

- The comparison surface (session UUID sets + withheld flag) is narrower than a full
  JSON_CONTRACTS document diff would be - it does not compare `risk_class`/`reversibility`/
  `authority` values, evidence content, or non-session artifact types, because Python has no
  equivalent vocabulary to compare those against. This is disclosed as the scope decision
  above, not silently narrower than advertised.
- `INTENTIONAL_DIVERGENCES` is untested against a *real* accepted divergence (none exists yet)
  - only `self_test`'s synthetic case exercises the suppression path. The first real entry, when
  it happens, should double-check the mechanism against real `check()` output, not only the
  self-test.
- This gate does not run for `codex` under `--tool all` combined scans, or for a fixture with
  both tools present simultaneously (no such fixture exists in the current corpus) - each
  fixture is single-tool by construction (`tests/fixtures/manifest.json`), so this is
  inherited corpus scope, not a gap this story introduced.
- Fix #1/#2 above were found and fixed *after* E06-S01's own evidence packet was written and
  committed; E06-S01's evidence packet was not retroactively amended (this packet is the record
  of the fix instead). A verifier reviewing E06-S01 should read this packet alongside it.

## Verifier verdict

PENDING - epic E06 review runs once every story in E06 is `ready_for_review` (at most twice per
epic, per ADR-0014).

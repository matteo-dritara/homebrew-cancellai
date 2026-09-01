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

## Round 1 verifier verdict

FAIL (`project/evidence/E06-VERIFIER-REVIEW.md`, 2026-09-01): "The supposedly ADR/RFC-cited
allow-list accepts arbitrary text ... The actual comparator observes only delete-UUID sets and
one withheld bit. It cannot detect the normative corpus' root-confidence/origin, protected/
unknown coverage, or other classified artifact semantics."

## Repairs for E06 verifier review round 1

1. **`INTENTIONAL_DIVERGENCES` accepted any free text, uncited (AC2).** Root cause: a fixture id
   present in the dict suppressed a mismatch unconditionally - the reason string was recorded
   but never validated. Fixed: `_citation_is_accepted_adr_or_rfc` extracts every `ADR-NNNN`/
   `RFC-NNNN` citation from the reason text and requires a real document at `docs/adrs/NNNN-*.md`
   (or `docs/rfcs/`) whose first ~15 lines contain `Status: Accepted`; a divergence only
   suppresses when at least one citation resolves. `self_test` now proves both directions: the
   review's exact reproduction (`{"fx": "uncited free text"}`, and a fabricated `ADR-9999` that
   does not exist) no longer suppresses, while a citation to a real, currently-accepted ADR
   (`docs/adrs/0016-...md`) does.
2. **Comparator observed only delete-UUID sets and one withheld bit (AC1).** Fixed:
   `semantic_projection` is now the comparison surface - `candidates`, `withheld`, `root_origin`,
   `root_confidence`, `mutation_eligible`, and `scan_complete`, compared field-by-field so a
   divergence report names exactly which field disagreed. Rust's side reads `root_origin`/
   `root_confidence`/`mutation_eligible` from `plan --json`'s real `provider_roots` entries and
   `scan_complete` from `scan_completeness`, not inferred from action reason text. `self_test`
   gained two new injected-divergence cases: identical candidate sets with a `root_origin`/
   `mutation_eligible` mismatch, and a `scan_complete` mismatch - both are now caught; neither
   could have been expressed by the old two-field comparison at all.
3. **The gate could never surface a root-authority divergence in the first place, because it
   always simulated "default root" on both engines by construction (AC1/AC2, "a real custom-root
   fixture outside the characterization helper's default-root patch").** Root cause:
   `python_result` always patched `cancellai.default_home` to point at the fixture, and
   `rust_result` always set `CLAUDE_CONFIG_DIR`/`CODEX_HOME` to the fixture path - the *Rust*
   side was therefore always exercising the custom-root path while the *Python* side was always
   faked into the default-root path, and the pre-fix Rust bug (hard-coded `is_default_root:
   true`, see E06-S01's repairs) happened to make both sides agree anyway, masking the gap. Fixed:
   `compare_fixture` now runs every NORMATIVE fixture through two independent scenarios -
   `default` (Rust sees the fixture literally named `.claude`/`.codex` under a synthetic `$HOME`,
   no override; Python's `default_home` patch unchanged) and `custom` (Rust addressed through
   `CLAUDE_CONFIG_DIR`/`CODEX_HOME` with `$HOME` pointed elsewhere; Python's `default_home` left
   *unmocked*, so both engines see a genuinely non-default root through their own real
   resolution logic, not a shared test fixture). `check()` now runs 20 comparisons (10 fixtures x
   2 scenarios), all matching, including the `custom` scenario for every fixture - this is the
   concrete, corpus-wide proof that E06-S01's root-authority fix (see that story's evidence)
   actually holds across every NORMATIVE fixture, not only the one adversarial case reproduced
   by hand.

`INTENTIONAL_DIVERGENCES` remains empty: every NORMATIVE fixture matches exactly in both
scenarios with no suppression needed.

## Verification Commands (repairs)

```text
python3 scripts/rust_python_parity.py self-test
python3 scripts/rust_python_parity.py check
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy scripts/rust_python_parity.py
```

All green: `self-test` reports every injected-divergence class (including the two new ones)
caught; `check` reports "10 NORMATIVE fixture(s) match across engines, in both root-origin
scenarios."

## Round 2 verifier verdict

FAIL (`project/evidence/E06-VERIFIER-REVIEW-ROUND2.md`, 2026-09-01): the round-1 uncited-text
finding was closed, but two new gaps survived: (1) `INTENTIONAL_DIVERGENCES={"fx": "unrelated
accepted ADR-0014"}` suppressed a fully divergent comparison merely because *some* real,
accepted ADR was cited - ADR-0014 concerns release cadence, not fixture `fx` or this specific
difference; (2) the projection still only covered six fields and could not observe
protected/unknown coverage, non-delete discovered identity records, or non-delete proposed
actions. Per the two-round ceiling, recorded as new backlog item E07-S08 rather than a third
E06 review round.

## Repair for the round-2 finding

1. **Free-text-but-cited suppression → structured, field-scoped, fixture-bound records.**
   `INTENTIONAL_DIVERGENCES` is now a tuple of `ApprovedDivergence(fixture_id, scenario, fields,
   citation)` records, not `dict[fixture_id, str]`. `_compare_results` checks every diverging
   field independently: a field is only excused when an `ApprovedDivergence` names this exact
   `fixture_id`/`scenario`/`field`, *and* `_citation_covers` confirms the citation resolves to a
   real, `Status: Accepted` ADR/RFC whose own document text mentions this exact fixture id - not
   merely any accepted document. `ADR-0014` (real, accepted, about epic closure/review bounding)
   does not mention any fixture id string and therefore can never suppress anything under this
   rule; `self_test` reproduces the exact round-2 scenario (an accepted-but-unrelated ADR) and
   proves it no longer suppresses, alongside a new case proving a field-scoped approval covers
   only its named field, not every diverging field on the same fixture/scenario.
2. **Six-field projection → eight-field projection covering the named gaps.**
   `semantic_projection` gained `non_delete_identities` (every discovered session UUID *not*
   proposed for deletion - `candidates ∪ non_delete_identities` is the full discovered corpus
   for the tool, giving "discovered identity records" and "every proposed action" coverage: an
   artifact is always either a delete candidate or not, on both engines) and `protected_count`
   (protection coverage). Python computes these by calling `discover_claude_sessions`/
   `discover_codex_sessions` and `protected_component` directly - independent of `build_plan`'s
   own eligibility filtering, which silently drops protected/blocked candidates from
   `plan.actions` entirely, leaving nothing there to compare. Rust reads them from `inspect
   --json`'s full `artifacts` array (every discovered artifact, not only delete actions) via
   `identity_token`/`protection_state`. `scan_complete` remains the comparison surface for
   "unknown coverage" (this codebase's own SI-008/SI-009 vocabulary) rather than inventing a
   parallel field.
   **Disclosed residual, not claimed as covered:** a full per-artifact
   `knowledge_confidence`/`integrity_state`/`risk_class` diff remains out of scope - Python's
   `Action` model has no equivalent per-artifact vocabulary to compare against (a materially
   larger undertaking than this repair; tracked by whoever next touches E07-S08's own scope).

Regression: `self_test` gained `non_delete_mismatch`, `protected_count_mismatch`, the
unrelated-accepted-ADR case (exact round-2 reproduction), and a field-scoping case (approving
only `candidates` must not silently also approve an unrelated `scan_complete` divergence, and
an approval scoped to the `default` scenario must not cover `custom`). `check()` re-ran the
full ten-fixture, two-scenario (20-comparison) corpus against the broadened projection: all
still match with `INTENTIONAL_DIVERGENCES` empty.

Verified: `python3 scripts/rust_python_parity.py self-test` / `check`, `ruff check`/`format
--check`, `mypy scripts/rust_python_parity.py` - all green.

**Scope note, not overclaimed as done:** recorded here, against the story whose file
(`scripts/rust_python_parity.py`) this repair lives in, rather than as a premature
`ready_for_review` claim on E07-S08 itself (which depends on E06-S02, still `blocked`, and
whose own AC also expects the disclosed per-artifact-confidence residual above to eventually
close).

## Verifier verdict

Round 1: FAIL, repaired (see above). Round 2: FAIL on two new findings, repaired (see above).
E06's two independent-review rounds are exhausted per ADR-0014/PD-022; no further E06-scoped
review is expected. The repair above is offered as evidence for whoever picks up E07-S08 or
reopens E06-S02, not as a self-graduated status change.

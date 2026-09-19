# Evidence Packet - E13-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E13 epic review
- Change Risk: CR3 (declared CR3 at planning time in `project/epics/E13.json`; no risk floor in
  `project/risk_floors.json` applies to `rust/crates/cancellai-store/*` - only kernel-ring paths
  and `cancellai-model`/`cancellai-policy`/`cancellai.py` carry a floor above the declared level,
  and `python3 scripts/check_risk_classification.py check` confirms no new story falls below its
  floor. CR3 stays declared, matching E13-S02/E13-S03's own precedent of the crate carrying no
  floor.)
- Spec version/commit: `docs/architecture/PERSISTENCE_MODEL.md` "Self-budget";
  `docs/security/SAFETY_INVARIANTS.md` SI-026 ("cancellAI reset/self-budget cannot target
  provider payload"); `scripts/check_mutation_boundary.py` (SI-019, the structural constraint
  this story's design had to fit inside)

## Outcome

PASS

## Scope

Implements self-budget enforcement and local-state reset for the three SQLite-backed layers
`cancellai-store` already holds (E13-S01 `CurrentStateStore`, E13-S02 `EventLedger`, E13-S03
`AnalyticalMemory`). New module `cancellai_store::budget` (`rust/crates/cancellai-store/src/
budget.rs`) - no new crate, no new dependency, no new `Connection`/schema/file of its own.

**Design decision - "when" lives in `budget.rs`, "how" lives with each layer's own data (asked
implicitly by the mutation-boundary constraint).** `scripts/check_mutation_boundary.py` (SI-019)
allows a raw filesystem delete (`std::fs::remove_file`/`remove_dir`/`remove_dir_all`) only from
`cancellai-platform/src/mutation.rs`, and the `SystemMutationExecutor`/`.mutate(...)` capability
only from that file and `cancellai-safety/src/mutation_executor.rs` - the one production
orchestration path that checks root/authority/reversibility/identity before ever calling it. Both
are the wrong fit for cancellAI's own internal state (an identity-bound provider-artifact mutation
path, not a "empty my own SQLite tables" operation) and are structurally forbidden to this crate
regardless. Every mutating method this story adds - `CurrentStateStore::reset`,
`EventLedger::compact_oldest_to_fit`/`reset`, `AnalyticalMemory::compact_if_over`/`reset` - is SQL
only, against the connection its own type already owns, in the module that already owns that
layer's schema and error type; `budget.rs` itself contains no SQL and touches no `Connection` at
all, so `check_mutation_boundary.py` has nothing to scan for in it. This discharges SI-026 by
construction, not convention: none of these methods accept a path parameter, so a provider root
cannot even be expressed as an argument, let alone reached.

**Design decision - `EventLedger::reset` uses `DROP TABLE`, not `DELETE FROM` (found while
implementing, not anticipated in the falsification plan).** `ledger_compactions`' `BEFORE DELETE`
trigger (E13-S02) refuses unconditionally, by design - "compaction summaries are immutable." A
reset that used `DELETE FROM ledger_compactions` would therefore always fail, defeating the whole
point of a reset (a caller who explicitly asked to wipe local state should not be left with a
non-empty audit trail because an earlier immutability guarantee, correctly, refuses a plain
delete). `EventLedger::reset` instead drops all three ledger tables and re-runs `MIGRATIONS`
inside one transaction - DDL, which the trigger (attached to a table `DROP TABLE` itself removes)
never intercepts - ending at the same fully-migrated `PRAGMA user_version` it started at, so the
same open connection stays immediately usable. `CurrentStateStore::reset`/`AnalyticalMemory::reset`
carry no such trigger and use a plain `DELETE FROM` per table (for `CurrentStateStore`, this is
exactly `rebuild(&[])` - full reuse of the existing, already-tested primitive).

**Design decision - Layer 1 has no compaction action (see `budget.rs`'s own module doc, "Why
Layer 1 has no compaction action here").** The story's instruction text and `docs/architecture/
PERSISTENCE_MODEL.md`'s own "Self-budget" list both name a current-state-DB budget, but
`docs/architecture/PERSISTENCE_MODEL.md` names Layer 3 sampling, not Layer 1 itself, as what
degrades under Layer 1 budget pressure ("safety-critical current facts may force analytical
sampling to degrade rather than exceed the budget") - and Layer 1's own content is entirely
determined by the last external `rebuild`, so this crate autonomously discarding rows to fit a
budget would be silent divergence from the last scan, not compaction. `check_current_state_budget`
therefore only observes `CurrentStateStore::row_count` against its limit; wiring the actual
cross-layer degradation this document names needs a live (Guardian) caller this workspace does not
have yet - flagged in Residual risks, not invented here.

**Design decision - no `reset --local-state` CLI flag, no new CLI dependency.** `cancellai-cli`
and `cancellai-guardian` both already declare `cancellai-store` as a dependency in their
`Cargo.toml` (predating this story, unrelated to it - `cancellai-guardian/src/main.rs` even
carries `use cancellai_store as _;`, an explicit "not wired yet" marker), but neither references
it from source (`grep -rn "cancellai_store::" rust/crates/cancellai-cli/src rust/crates/
cancellai-guardian/src` finds nothing beyond that marker). This story adds no CLI wiring - the
same "primitive delivered, orchestrator not yet built" precedent `CurrentStateStore`/`EventLedger`/
`AnalyticalMemory` already set at their own `ready_for_review` (their evidence packets' own
"Compatibility" sections). `docs/CLI.md` is machine-generated from the Python CLI
(`AGENTS.md`, "Generated project docs") and `cancellai.py` has no `reset`/`local-state`/`ephemeral`
concept at all (`grep -n "reset\|local-state\|ephemeral" cancellai.py` returns nothing - Layer
1/2/3 are Rust-only, introduced by E13) - so `docs/CLI.md` is correctly untouched by this story,
and `python3 scripts/gen_docs.py --check` confirms it stayed in sync throughout. The story's
declared documentation impact named `docs/CLI.md`; this is the recorded deviation from that list
and the reasoning above is why.

**Design decision - `reset_local_state` composes three independent `reset()` calls, sequentially,
not atomically.** `CurrentStateStore`, `EventLedger` and `AnalyticalMemory` each own an
independent file/`Connection`/`PRAGMA user_version` history (every one of their own module docs
already states this, and for the same reason). A single cross-file SQLite transaction spanning
all three is not something this crate's design offers, so `reset_local_state` calls each layer's
own `reset()` in sequence and returns a `ResetError` naming which layer failed. Each individual
`reset()` remains atomic and fails closed; only the composition across three files is
non-atomic - documented as a residual, not hidden.

**Public surface**: `BudgetLimits` (validated constructor, `DEFAULT`, one accessor per threshold);
`BudgetStatus`; `check_current_state_budget`; `enforce_ledger_budget`/`enforce_rollup_budget`;
`ResetError`; `reset_local_state`; plus, on the existing types,
`CurrentStateStore::{row_count, reset}`, `EventLedger::{event_id_bounds, compact_oldest_to_fit,
reset}`, `AnalyticalMemory::{raw_sample_count, compact_if_over, reset}`.

## Falsification plan (written before implementation)

Per `docs/development/AGENT_PROTOCOL.md`'s "plan verification before code" and the
`adversarial-cases` skill's eleven axes, worked for this change, and the story's own "self-budget
stress test, reset boundary tests" verification contract:

| Falsifier | Would prove the implementation wrong | Test |
| --- | --- | --- |
| Budget exactly at the limit must not compact | A store exactly at its configured threshold gets compacted anyway (over-eager enforcement) | `check_current_state_budget_reports_within_and_over` (exactly-at-limit case), `ledger::tests::compact_oldest_to_fit_does_not_compact_exactly_at_the_limit`, `budget::tests::enforce_ledger_budget_does_not_compact_exactly_at_the_limit`, `rollup::tests::compact_if_over_does_not_compact_exactly_at_the_limit`, `budget::tests::enforce_rollup_budget_does_not_compact_exactly_at_the_limit` |
| One unit over the limit must compact, exactly the excess | Growth continues silently past budget, or compaction removes more/less than the exact excess | `ledger::tests::compact_oldest_to_fit_compacts_exactly_the_excess_one_event_over_the_limit`, `budget::tests::enforce_ledger_budget_compacts_when_one_event_over_the_limit`, `rollup::tests::compact_if_over_runs_compact_when_one_sample_over_the_limit_and_promotes_aged_samples`, `budget::tests::enforce_rollup_budget_compacts_when_over_the_limit_and_samples_have_aged` |
| A compaction that fails partway must leave the store no worse than the original overrun | A refused range compaction partially deletes rows, or writes a wrong/partial summary | `ledger::tests::compact_oldest_to_fit_fails_closed_when_the_computed_prefix_has_a_gap` - seeds a non-prefix compaction to create a gap, then proves the subsequent `compact_oldest_to_fit` call is refused and leaves every remaining event and every existing compaction summary byte-for-byte unchanged |
| Reset called against a store must never touch a provider path | A reset primitive accepts, or can be made to act on, a path outside its own SQLite file | `tests::reset_never_touches_a_provider_path` (Layer 1), `ledger::tests::reset_never_touches_a_provider_path` (Layer 2), `rollup::tests::reset_never_touches_a_provider_path` (Layer 3), `budget::tests::reset_local_state_empties_all_three_layers_and_never_touches_a_provider_path` (composed) - each places a real "provider artifact" file next to the store's own database file and asserts `reset`/`reset_local_state` leaves it byte-for-byte untouched |
| Reset must be structurally unable to name a provider root at all | A future caller could pass a path/root into a reset function | By construction: `CurrentStateStore::reset`, `EventLedger::reset`, `AnalyticalMemory::reset` and `reset_local_state` take no path parameter of any kind - `scripts/check_mutation_boundary.py check` (static grep for `std::fs::remove_*`/`SystemMutationExecutor`/`.mutate(`) confirms zero occurrences in every new file this story adds |
| Reset concurrent with an ongoing write | A reset racing another writer corrupts data, hangs, or silently partially applies | `ledger::tests::reset_under_a_held_write_lock_fails_closed_without_corrupting_existing_data` - a second `Connection` holds `BEGIN IMMEDIATE` on the same file; `reset()` fails with an `Err` (never panics, never blocks), and once the lock is released the ledger's prior content is exactly unchanged |
| Reset leaves the schema (`PRAGMA user_version`) intact and reusable, not requiring an incoherent re-migration | After reset, the connection is left at a different/lower `user_version` than before, or needs a fresh `open()` to become usable again | `ledger::tests::reset_leaves_user_version_at_the_same_fully_migrated_value_and_the_ledger_reusable` - asserts `user_version` identical before/after, and that the same open connection accepts a new `append` immediately afterward with fresh, non-colliding event ids |
| Reset must empty every table, including the one an immutability trigger normally protects | `EventLedger::reset` leaves `ledger_compactions` non-empty because the trigger silently absorbs the delete attempt | `ledger::tests::reset_empties_every_table_including_compaction_summaries` - seeds a real compaction summary first, then asserts it is gone after `reset` |
| Self-budget stress test: growth never exceeds the configured limit across many writes | Repeated writes without ever exceeding budget once (should be impossible by design) eventually let the store grow unboundedly | `budget::tests::ledger_self_budget_stress_test_growth_never_exceeds_the_configured_limit` (500 append/enforce cycles, asserting the bound after every single append) and `budget::tests::rollup_self_budget_stress_test_growth_never_exceeds_the_configured_limit` (500 record/enforce cycles, same per-iteration assertion) |
| Ephemeral mode leaves zero persistent writes across many operations, not just one call | A single-call ephemeral test passes but a longer session leaks a file (e.g. lazily-created SQLite journal/WAL sidecar) | `budget::tests::ephemeral_layers_perform_zero_persistent_writes_across_many_operations` - 50 iterations of rebuild/enforce/append/enforce/record against `open_in_memory()` instances, plus `reset_local_state`, then asserts a real, dedicated temp directory holds exactly zero files |
| Budget threshold validation rejects a zero limit | `BudgetLimits::new` silently accepts a limit of 0, which every real write would then exceed | `budget::tests::budget_limits_rejects_a_zero_threshold_in_any_position` |
| Rollup budget over-limit but no sample old enough per policy (policy/budget interaction) | `compact_if_over` invents an eligibility rule not backed by `RetentionPolicy`, silently promoting a too-young sample just because the row count is over budget | `rollup::tests::compact_if_over_can_run_with_zero_promotions_when_no_sample_has_aged_yet` - proves compaction runs but the raw tier stays over budget until the policy actually ages a sample; documented as a real policy/budget interaction, not silently resolved |

Axes from the `adversarial-cases` skill not applicable here, with reason (same reasoning
E13-S01/S02/S03's own evidence packets already give for this crate): path/identity, links/mounts,
provider-layout drift, platform differences (this module touches no filesystem path but each
layer's own SQLite file, and reset's provider-path falsifiers above cover the identity axis
directly); policy/trust conflicts, protection bypass, root escape (this crate makes no authority/
mutation decision and adds no `cancellai-safety`/`cancellai-platform` dependency - `cancellai-store`'s
`Cargo.toml` is unchanged by this story); malformed/untrusted input (no new deserialization
surface - every new method takes only already-typed Rust values, never a string/path/blob a
caller could corrupt); performance/large datasets (the stress tests exercise hundreds of
enforce/write cycles; each `compact_oldest_to_fit`/`compact_if_over` call is bounded by the same
`O(n in the eligible rows)` cost `compact_range`/`compact` themselves already carry, not
`O(table size)`).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Budget overrun triggers compaction before growth continues." | `enforce_ledger_budget`/`enforce_rollup_budget` (thin wrappers over `EventLedger::compact_oldest_to_fit`/`AnalyticalMemory::compact_if_over`) compact exactly when a layer is strictly over its configured `BudgetLimits` threshold, never at exactly the limit, and are designed to be called immediately before the write that might push a layer over budget. `enforce_ledger_budget_does_not_compact_exactly_at_the_limit`/`enforce_ledger_budget_compacts_when_one_event_over_the_limit` and their rollup counterparts prove the exact boundary; `ledger_self_budget_stress_test_growth_never_exceeds_the_configured_limit`/`rollup_self_budget_stress_test_growth_never_exceeds_the_configured_limit` prove growth stays bounded (never more than the limit plus the one just-written row) across hundreds of writes, never silent unbounded growth. | PASS |
| AC2 - "`reset --local-state` cannot target provider roots." | `CurrentStateStore::reset`/`EventLedger::reset`/`AnalyticalMemory::reset`/`reset_local_state` take no path or root parameter at all - structurally unable to express a provider location, discharging SI-026 by construction. `reset_never_touches_a_provider_path` at each of the three layers plus the composed `reset_local_state_empties_all_three_layers_and_never_touches_a_provider_path` place a real adjacent "provider artifact" file and prove it is byte-for-byte untouched. `scripts/check_mutation_boundary.py check` confirms zero raw filesystem deletes and zero references to the safety-executor capability anywhere in this story's new code. | PASS |
| AC3 - "Ephemeral inspect performs no persistent writes." | Discharged by the three layers' pre-existing `open_in_memory` (SQLite `:memory:`) constructors - a `:memory:` connection cannot create a file, by construction. `ephemeral_layers_perform_zero_persistent_writes_across_many_operations` proves this holds across 50 iterations of rebuild/budget-enforcement/append/record plus a full `reset_local_state`, not merely for one call, by asserting a dedicated real directory stays at zero files throughout. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-026 "cancellAI reset/self-budget cannot target provider payload" | A caller passes, or a future change lets a caller pass, a provider path into a reset/compaction call | By construction (no path parameter anywhere in this story's new API) plus the four `reset_never_touches_a_provider_path`-family tests placing a real adjacent file and proving it untouched | PASS |
| SI-019 "all filesystem/vendor mutations route through the safety executor" (this story's own binding structural constraint, not itself a new obligation) | This story's reset/compaction code calls a raw filesystem delete primitive or the `SystemMutationExecutor`/`.mutate(...)` capability outside the two files SI-019 allows | `scripts/check_mutation_boundary.py check` - "mutation boundary OK: 85 Rust source files scanned; only `cancellai-platform/src/mutation.rs` deletes anything, only that file and `cancellai-safety/src/mutation_executor.rs` reference the capability that does" (this story's new files, `budget.rs` and the edits to `lib.rs`/`ledger.rs`/`rollup.rs`, are entirely SQL-only and contain neither pattern) | PASS |
| `ledger_compactions` immutability (E13-S02, "compaction summaries are immutable") is not silently defeated by `reset` | `EventLedger::reset` somehow leaves a stale, unrecorded compaction summary behind, or the reset itself corrupts (rather than replaces) the schema | `reset_empties_every_table_including_compaction_summaries` (seeds a real summary first, proves it is gone after reset) and `reset_leaves_user_version_at_the_same_fully_migrated_value_and_the_ledger_reusable` (schema is fully, coherently rebuilt, not left partial) | PASS |
| Compaction/reset atomicity under failure | A compaction that cannot complete (non-contiguous computed range) or a reset that cannot acquire a lock leaves the store worse off than before | `compact_oldest_to_fit_fails_closed_when_the_computed_prefix_has_a_gap` (compaction) and `reset_under_a_held_write_lock_fails_closed_without_corrupting_existing_data` (reset) - both assert the store's prior content is exactly unchanged after the failure | PASS |
| Fail-closed, not silent, on a policy/budget interaction (Layer 3) | `compact_if_over` invents a promotion rule not backed by `RetentionPolicy` to force itself under budget | `compact_if_over_can_run_with_zero_promotions_when_no_sample_has_aged_yet` - compaction runs (per AC1) but promotes nothing when `RetentionPolicy`'s own recent window has not been reached; the mismatch is surfaced (zero promotions, still over budget), never silently resolved by a rule this module invents | PASS |

## Verification Commands

```text
$ cd rust
$ cargo fmt --check                                                        clean
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   all suites "test result: ok", 0 failed (cancellai-store: 70 passed, up from 42 baseline - 28 new tests)
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
$ cd ..
$ python3 scripts/project_os.py check                                     governance OK: 24 decisions, 32 epics, 165 stories
$ python3 scripts/check_rust_workspace.py check                           rust workspace OK: 13 crates match TARGET.md, acyclic, model/safety isolated
$ python3 scripts/check_mutation_boundary.py check                        mutation boundary OK: 85 Rust source files scanned; only cancellai-platform/src/mutation.rs deletes anything, only that file and cancellai-safety/src/mutation_executor.rs reference the capability
$ python3 scripts/check_docs.py check                                     docs OK: 392 Markdown files; local links and safety IDs are consistent
$ python3 scripts/check_risk_classification.py check                      risk classification OK: no new story is below its floor (50 recorded, owner decision pending; E13-S04 not flagged)
$ python3 scripts/check_coverage.py report                                cancellai-store 92.44% (informational; outer-ring, not ratchet-gated)
$ python3 scripts/check_coverage.py check                                 coverage OK: 6 ratcheted crates at or above their recorded floor (cancellai-store not ratcheted)
$ python3 scripts/check_process.py check                                  process OK: ADR lifecycle, decision supersession, evidence, review rounds, and generated banners are consistent (pre-existing E00/E07/E12 round-ceiling warnings, unrelated to this story)
$ python3 scripts/check_evidence.py check                                 evidence OK: 146 packets checked against their contracts (plus this one once committed; 15 pre-convention, recorded, unrelated to this story)
$ python3 scripts/gate_sensitivity.py check                               gate sensitivity OK: 11 mutants, 11 killed, every gate clean on an unmutated tree
$ python3 scripts/check_ears.py check                                     EARS OK: 516 acceptance criteria classified, 66 describe unwanted behaviour (13%); pre-existing baseline warnings on other stories, none new to E13-S04 (AC2's "cannot target provider roots" is this story's own unwanted-behaviour criterion)
$ python3 scripts/safety_oracle.py check                                  safety oracle OK: protected-name checked against the engine's own decision; root-capability and retention checked as predicates only (2000 cases)
$ python3 scripts/verifier_handoff.py check                               verifier handoff OK: every verdict that names a brief answers the brief that was committed
$ python3 scripts/release.py check                                        release OK: v1.14.0 is consistent across source, packaging and formula
$ python3 scripts/release_manifest.py check                               release manifest OK: 1 golden document(s) match project/schemas/release_manifest.schema.json
$ python3 scripts/check_repository_topology.py check                      repository topology OK
$ python3 scripts/check_agent_skills.py check                             agent skills OK: 8 skills; frontmatter, names and every cited path resolve
$ python3 scripts/gen_docs.py --check                                     docs/CLI.md is up to date
$ python3 scripts/check_schemas.py check                                  schemas OK: 4 golden documents match docs/architecture/JSON_CONTRACTS.md
$ python3 scripts/check_fixtures.py check                                 fixtures OK: 13 fixtures cover all required categories
$ python3 scripts/diff_harness.py check                                   diff harness OK: self-test cases all behave as documented
$ python3 scripts/characterize.py check                                   characterization OK: 13 fixtures match their committed characterization
$ python3 scripts/rust_python_parity.py self-test                         rust/python parity self-test OK: the comparator correctly catches every injected divergence class
$ python3 scripts/rust_python_parity.py check                             rust/python parity OK: 13 NORMATIVE fixture(s) match across engines (unaffected - no cancellai.py change)
```

Cross-target clippy (`--target x86_64-pc-windows-gnu`/`x86_64-unknown-linux-gnu`) was not run:
`AGENTS.md` requires it specifically for changes touching `cancellai-platform` or moving the
lint surface workspace-wide, neither of which applies here (`cancellai-store` only, no
`cfg`-gated code added, no `unsafe`). Real CI still builds and lints natively on all three
platforms.

`mypy`/`pytest`/`ruff` were not re-run in this session for this change (no Python source file
changed - only `docs/`, `project/epics/E13.json`, `CHANGELOG.md`,
`rust/crates/cancellai-store/src/{lib.rs,ledger.rs,rollup.rs,budget.rs}`, and this evidence
packet); they are available locally and CI runs them regardless, matching E13-S02/E13-S03's own
precedent for the same situation.

## Compatibility

- `cancellai-store`'s `Cargo.toml` is unchanged - no new dependency.
- No existing public API (`CurrentStateStore::open`/`open_in_memory`/`rebuild`/`all`/`get`,
  `EventLedger::open`/`open_in_memory`/`append`/`read_all`/`compact_range`/`compactions`,
  `AnalyticalMemory::open`/`open_in_memory`/`record_sample`/`compact`/`raw_samples`/
  `hourly_rollups`/`daily_rollups`/`long_term_aggregates`) changed signature or behavior - this
  story only adds new methods (`row_count`, `reset`, `event_id_bounds`, `compact_oldest_to_fit`,
  `raw_sample_count`, `compact_if_over`) and one new module (`budget`).
- No CLI/TUI/Guardian surface consumes this yet (library-level capability only) - see "Scope"'s
  own design-decision note on why `cancellai-cli`'s and `cancellai-guardian`'s pre-existing,
  unused `cancellai-store` dependency does not change that assessment.

## Performance / operability

- `enforce_ledger_budget`/`enforce_rollup_budget`/`check_current_state_budget` each cost one
  `COUNT`/`MIN`/`MAX` query when under budget (no-op path), plus the underlying compaction's own
  already-documented `O(n in the eligible rows)` cost when over budget.
- `EventLedger::reset` costs three `DROP TABLE` statements plus re-running the fixed migration
  list (currently one migration) - `O(1)` in the number of prior rows, since dropping a table
  does not need to visit its rows individually.
- `CurrentStateStore::reset`/`AnalyticalMemory::reset` are `O(n)` `DELETE FROM` per table,
  matching `rebuild`/`compact`'s own precedent.

## Documentation updated

- `docs/architecture/PERSISTENCE_MODEL.md` - "Self-budget" now describes the real implementation:
  the `budget` module's when/how split, `BudgetLimits`, the two `enforce_*` wrappers, why Layer 1
  has no compaction action, and `reset_local_state`'s sequential (not cross-file-atomic) contract.
- `docs/CLI.md` - deliberately **not** edited. It is generated from the Python CLI
  (`AGENTS.md`, "Generated project docs"), `cancellai.py` has no `reset`/`local-state`/`ephemeral`
  concept, and no Rust CLI surface calls this story's code yet - see "Scope"'s own design-decision
  note. `python3 scripts/gen_docs.py --check` confirms it is unaffected and still up to date.
- `CHANGELOG.md` - `[Unreleased]` / `### Added`.

## Method defects

- **What happened**: the first `git commit` attempt for this story was refused by the `pre-commit` `skill-content-check` hook (`scripts/check_skill_content.py`), reporting "skillspector 2. is installed but this gate is pinned to 2.11.2" - a mismatch, but not the one it named; `skillspector --version` actually reports `2.11.2`, matching the pin exactly, but the installed binary embeds an ANSI colour escape sequence mid-digit-string even under non-interactive `subprocess.run(capture_output=True)` capture (`SkillSpector v2.\x1b[1;36m11.2\x1b[0m`, confirmed directly), which the gate's `VERSION_RE` character class does not tolerate, so it parsed only `2.` and reported a false version-drift; setting `TERM=dumb` in the invoking shell (skillspector honours it and stops emitting colour codes; `NO_COLOR=1` alone did not) let the gate parse the true, correctly-pinned version and pass, confirming this was a parsing/observation defect in the gate, not an actual toolchain drift, and no toolchain component was installed, updated, or removed to work around it. **Prevented by**: none exists; `VERSION_RE` does not strip ANSI escape sequences before matching, and nothing in `docs/development/AGENT_TOOLCHAIN.md` or the toolchain-review flow checks a pinned scanner's own colour-output behaviour against a non-interactive capture. **Disposition**: proposed 2026-09-16

## Residual risks

- **Superseded by round 2 independent verifier review and its own repair.** The original text of
  this entry ("Layer 1 has no compaction action, only an observation") was rejected as an accepted
  residual by `project/evidence/E13-VERIFIER-REVIEW-ROUND2.md`: an observation nothing consumed
  was not the compaction AC1 requires. The repair added `budget::enforce_current_state_pressure`
  (forces Layer 3's raw-sample tier down to a caller-supplied ceiling whenever Layer 1 crosses
  budget) and `budget::rebuild_within_current_state_budget` (wires that check into
  `CurrentStateStore::rebuild`, the only write path Layer 1 has, after a self-review found the
  first version of the repair left the enforcement function uncalled from any real write). What
  remains a genuine residual, not yet closed: no live caller in this workspace calls
  `rebuild_within_current_state_budget` instead of raw `rebuild` yet - that wiring is Guardian
  orchestration this workspace does not have, matching this crate's own "primitive, not
  orchestrator" precedent, and product-level threshold/ceiling values remain a caller's choice,
  not a ratified capacity policy.
- **`reset_local_state` is sequential across three files, not one atomic operation.** A failure
  partway (e.g. Layer 2's file is locked by another process while Layer 1's reset already
  committed) leaves Layer 1 empty and Layers 2/3 unchanged, not a coordinated all-or-nothing
  reset across all three files - `ResetError` names which layer failed, and retrying is always
  safe (each layer's own `reset()` is idempotent - resetting an already-empty layer is a no-op),
  but true cross-file atomicity is not provided. Documented in `budget.rs`'s own module doc
  rather than hidden; a coordinating mechanism (e.g. a marker file / two-phase protocol) is a
  separate, dedicated story's scope if ever needed.
- **No orchestrator/CLI wiring**, matching `CurrentStateStore`'s/`EventLedger`'s/
  `AnalyticalMemory`'s own state at their own `ready_for_review`. `enforce_ledger_budget`/
  `enforce_rollup_budget` are designed to be called immediately before the write that might
  exceed budget, but no caller in this workspace does so yet - `cancellai-cli`'s and
  `cancellai-guardian`'s pre-existing `cancellai-store` dependency is currently unused
  (`cancellai-guardian/src/main.rs`'s `use cancellai_store as _;` marker makes this explicit).
  Wiring real self-budget checks into a scan/Guardian loop, and a real `reset --local-state` CLI
  flag, is a later story's scope per `AGENTS.md`'s "work one story at a time."
- **`BudgetLimits`'s three thresholds are this story's own judgment call, not a ratified
  capacity-planning policy** - `DEFAULT` (50,000 / 10,000 / 10,000) is one reasonable choice
  named for symmetry with `RetentionPolicy::DEFAULT`'s own documented status, not a measured
  product decision. A caller supplying its own thresholds via `BudgetLimits::new` is expected to
  be the normal path once a real orchestrator exists.
- **No concurrent-access handling beyond fail-closed-on-lock-contention**, matching
  E13-S01/E13-S02/E13-S03's own recorded residual - `rusqlite`'s default behavior returns
  `SQLITE_BUSY` immediately (no configured `busy_timeout`) rather than retrying;
  `reset_under_a_held_write_lock_fails_closed_without_corrupting_existing_data` proves this fails
  safely, not that it retries or degrades gracefully under contention.
- **No fault-injection test for a mid-transaction failure inside `EventLedger::reset`'s
  drop/recreate sequence** beyond the lock-contention case already tested - atomicity rests on
  SQLite's own transaction guarantee (one `transaction()`/`commit()` wrapping the drop, every
  migration statement, and the `PRAGMA user_version` write), matching E13-S01/S02/S03's own
  precedent of relying on SQLite's own guarantee rather than a synthetic mid-transaction fault
  seam, which does not exist for `rusqlite::Transaction` in this crate.
- This packet is executor self-assessment. Independent review happens at epic scope, once every
  story in E13 is `ready_for_review`.

## Round 1 independent review repair (2026-09-17)

Codex's round 1 review (`project/evidence/E13-VERIFIER-REVIEW.md`) returned `FAIL`: with a
5-event ledger limit, six iterations of the documented "(enforce, append)" pattern left six raw
events, not five. `enforce_ledger_budget` called only *before* a write is a no-op once the
ledger is already exactly at the limit, and nothing re-checked after the append that followed
landed exactly on it - a one-write gap in the AC1 ("budget overrun triggers compaction before
growth continues") contract, reproducible on every write that lands on the limit.

Fixed by adding two new atomic admission primitives that compact **both** before and after their
own write, in one call a caller cannot split apart: `append_within_ledger_budget` (ledger) and
`record_sample_within_rollup_budget` (rollup). `EventLedger::compact_oldest_to_fit` has no
time-based eligibility gate, so the ledger's "after" compaction always succeeds and this
primitive never refuses a write. `AnalyticalMemory::compact_if_over` is time-gated, so
`record_sample_within_rollup_budget` can legitimately fail to free room (every held sample too
recent to promote); in that case it refuses the write with the new `SampleAdmissionError::
BudgetExceeded` rather than silently exceeding the budget - the other half of the required
repair ("refuse ... that write when safe compaction cannot meet the limit"). `enforce_ledger_
budget`/`enforce_rollup_budget` stay as the lower-level primitives these two compose; existing
callers of them directly are unaffected.

Regression tests: `append_within_ledger_budget_never_exceeds_the_limit_even_landing_exactly_on_
it` (Codex's exact six-iteration reproduction, now landing on 5) and `record_sample_within_
rollup_budget_refuses_when_every_held_sample_is_too_recent_to_promote`. Both stress tests
(`ledger_self_budget_stress_test_growth_never_exceeds_the_configured_limit`/`rollup_self_budget_
stress_test_growth_never_exceeds_the_configured_limit`) were rewritten to call the new admission
primitives and tighten their assertion from "never exceeds `limit + 1`" to "never exceeds
`limit`" - the literal "never observes a count above its limit" the verification contract asks
for, not the looser bound the original stress tests accepted.

The Layer 1 (current-state) remark in Codex's finding - "current-state overflow needs an
explicit bounded outcome rather than observation alone" - is not repaired in this change.
`check_current_state_budget` remains observation-only, per this module's own documented and
deliberate design ("Why Layer 1 has no compaction action here": `rebuild` is a full replace, not
an incremental write, and this crate discarding rows to fit a budget would silently diverge from
the last real scan with no recovery path). Turning that observation into an enforced,
non-discarding bounded outcome needs a product decision this story does not have the scope to
invent (what "degrade" means for Layer 1, and who decides it) - recorded as a residual risk below
with no story ID yet assigned, not fixed silently.

Verification after the repair: the same full Rust and Python gate set as this packet's original
run, re-executed and all passing; `cancellai-store` now has 95 tests (was 88).

## Self-review before round 2 (2026-09-17) - SELF-REVIEW, NOT INDEPENDENT

Per `docs/development/AGENT_PROTOCOL.md`'s "Self-review": this review was performed by the same
agent (Claude) that executed the round 1 repair above, so it is a self-review, not the
independent verification `AGENTS.md` assigns to Codex. It is recorded here because it found and
repaired a real defect in that repair; it does not close this CR3 story and is not a substitute
for the pending independent round 2.

**Defect found**: `record_sample_within_rollup_budget` (round 1's own repair, above) gated its
pre-write compaction on `enforce_rollup_budget`, which only compacts when the raw-sample tier is
*strictly over* `BudgetLimits::max_raw_samples` - never when it sits exactly at the limit. This
function's own steady state, once any admission has run at least once, sits at exactly the
limit (every prior successful admission left the tier at or under it), so in practice the
pre-write compaction call was a no-op on essentially every call once the tier reached capacity,
and the function refused the write outright - even when every held sample had already aged past
`RetentionPolicy`'s recent window and a real `compact` call would have promoted all of them and
freed room. Reproduced directly: a 1s recent window, three samples recorded at t=0/1/2, a
`max_raw_samples` limit of 3, and a fourth admission attempted at `now=1000` (every held sample
9+ orders of magnitude past its 1s eligibility window) returned `Err(BudgetExceeded { count: 3,
limit: 3 })` instead of compacting all three away and admitting the write. This contradicts the
function's own doc ("compacts first ... then checks whether room now exists") and AC1: a legitimate
write was refused even though safe compaction *could* meet the limit, which is exactly the case
round 1's own required repair ("refuse ... that write when safe compaction cannot meet the
limit") says must succeed, not refuse.

**Repair**: `record_sample_within_rollup_budget` now calls `AnalyticalMemory::compact` directly
and unconditionally before checking room, rather than going through `enforce_rollup_budget`'s
"strictly over" gate. `compact` is idempotent and a no-op when nothing is eligible, so this adds
no new failure mode - it only removes the case where a compaction that would have succeeded was
never attempted. `enforce_rollup_budget` itself is unchanged and still used elsewhere (its own
"exactly at the limit does not compact" contract is correct and tested for what it is - an
over-budget *observation* primitive, not an admission gate).

**Regression test added**: `record_sample_within_rollup_budget_compacts_and_admits_at_exactly_the_limit_when_every_held_sample_is_eligible`
(`rust/crates/cancellai-store/src/budget.rs`) - the exact reproduction above, asserting the
fourth write is admitted and the tier ends at 1 row (three aged samples promoted, one new sample
recorded), not refused. `cancellai-store` now has 96 tests (was 95).

**Verification re-run after this repair**: `cargo fmt --check`, `cargo clippy --workspace
--all-targets --all-features -- -D warnings`, `cargo test --workspace` (all suites `ok`,
`cancellai-store`: 96 passed), `cargo deny check` (advisories/bans/licenses/sources ok);
`python3 scripts/check_mutation_boundary.py check`, `python3 scripts/check_rust_workspace.py
check`, `python3 scripts/check_evidence.py check`, `python3 scripts/project_os.py check` - all
clean, no new finding.

**Class question** (per the epic-verifier method): is this the same class as round 1's ledger
gap, or a new one? Related but distinct - round 1's ledger gap was "nothing re-checks *after* a
write that lands exactly on the limit"; this one is "the *before* check never even attempts
compaction once steady state is reached, because it is gated on a stale 'over,' not 'at-or-over
for a pending write,' threshold." The ledger's admission primitive does not share this defect
(its "after" compaction has no time gate and always succeeds, so the before-check's gate on
"over" is harmless there - the ledger never needed the before-check to do anything at the
steady-state limit, only the after-check). The rollup tier's promotion is time-gated, which is
what makes the "before" check's own effectiveness depend on it actually running at exactly the
limit, not only when already over it - a rollup-specific consequence of round 1's shared "before
vs. after" repair shape, not a shared root cause across both primitives.

## Verifier verdict

Round 1 (Codex, 2026-09-17): FAIL - see the defect above. The ledger/rollup admission gap is
repaired in this packet; the Layer 1 observation-only design is an accepted residual, not a
repair, pending a product decision. Round 2 pending (self-review above found and repaired one
further defect in that repair ahead of round 2; it does not stand in for round 2).

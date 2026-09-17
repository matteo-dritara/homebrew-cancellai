# Evidence Packet - E13-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E13 epic review
- Change Risk: CR2 (declared CR2 at planning time in `project/epics/E13.json`; no risk floor in
  `project/risk_floors.json` applies to `rust/crates/cancellai-store/*` - only kernel-ring paths
  and `cancellai-model`/`cancellai-policy`/`cancellai.py` carry a floor above CR2, and
  `python3 scripts/check_risk_classification.py check` confirms no new story falls below its
  floor. CR2 stays declared, matching E13-S02's own precedent.)
- Spec version/commit: `docs/architecture/PERSISTENCE_MODEL.md` "Layer 3: Analytical Memory";
  `docs/architecture/GUARDIAN_MODEL.md` "Detection" (source for `MetricKind`'s initial variants);
  `docs/adrs/0019-dependency-rings-per-crate.md` (outer-ring `rusqlite`, `cancellai-store`
  already named "SQLite current-state store, event ledger, and analytical rollups" in its own
  `Cargo.toml` description before this story)

## Outcome

PASS

## Scope

Implements `cancellai_store::rollup::AnalyticalMemory`, Layer 3 of
`docs/architecture/PERSISTENCE_MODEL.md`. Lives in the same `cancellai-store` crate as E13-S01's
`CurrentStateStore` and E13-S02's `EventLedger`, as a new `rollup` module with its own
independent `Connection`/SQLite file/`PRAGMA user_version` history - no new dependency, no new
crate (`cancellai-store`'s `Cargo.toml` description already anticipated "analytical rollups").

**Design decision - source of raw samples (asked explicitly in the task brief).** Layer 3 owns
its own ingestion (`AnalyticalMemory::record_sample`) rather than deriving fine-grained
measurements from Layer 2's `EventLedger`. Rationale, recorded in `rollup.rs`'s own module doc:
the ledger records that a discrete, significant *event* happened (`EventKind` is a closed,
semantic set - `DISCOVERED`, `QUARANTINED`, ...); Layer 3 is a different kind of data, repeated
*numeric observations* of the same quantity over time (a footprint size, an artifact count)
whose individual readings are not independently significant and whose value is in the trend.
Deriving hourly/daily statistics by counting ledger events would conflate "an event happened"
with "a measurement was taken," and would make the ledger's own very different, audit-shaped
retention story (`compact_range`) responsible for Layer 3's continuously-sampled retention shape.
The two layers stay independently retained, matching `PERSISTENCE_MODEL.md` treating them as two
layers with two separate self-budgets. A future orchestrator may call both `EventLedger::append`
and `AnalyticalMemory::record_sample` for the same real-world observation; that is a caller-side
decision, not a reason to make one layer depend on the other's schema. No third write mechanism
was invented - ingestion is exactly one new, dedicated method on the new module, following the
same `open`/`open_in_memory`/one-method-per-concern shape both prior layers already use.

**Clock design.** Every time-dependent method (`record_sample`, `compact`) takes `now`/
`recorded_at` as an explicit `u64` parameter - the same seam `crate::ledger`'s own
`NewEvent::recorded_at`/`compact_range`'s `created_at` already use, and for the documented reason:
production code never calls `SystemTime::now()` in this crate's business logic. This module does
not depend on `cancellai_platform::clock::Clock` (which would pull an outer-ring crate into a
kernel-ring dependency for no capability a plain parameter does not already express - ADR-0019's
own "what `std` cannot express" test) - it follows `crate::ledger`'s existing precedent of a
plain, caller-supplied timestamp instead.

**Public surface**: `MetricKind` (closed, exhaustive: `ArtifactCount`, `ProviderFootprintBytes`,
`ReclaimableBytes`, `OrphanCount` - named for `docs/architecture/GUARDIAN_MODEL.md`'s own
"Detection" list without committing to its full future taxonomy); `SampleScope`
(`provider_id`/`category`, closed/allowlisted, mirroring `EventMetadata`); `NewSample`/`Sample`;
`RetentionPolicy` (validated constructor, `DEFAULT` = 1 day / 7 days / 90 days);
`AnalyticalMemory::open`/`open_in_memory`/`record_sample`/`compact`/`raw_samples`/
`hourly_rollups`/`daily_rollups`/`long_term_aggregates`; `Rollup`, `LongTermAggregate`,
`CompactionReport`.

## Falsification plan (written before implementation)

Per `docs/development/AGENT_PROTOCOL.md`'s "Plan verification before code" and the
`adversarial-cases` skill's eleven axes, worked for this change, and the story's own
"Time-travel compaction tests" verification contract:

| Falsifier | Would prove the implementation wrong | Test |
| --- | --- | --- |
| Sample exactly at the recent/medium boundary | A sample whose age equals `recent_window_secs` exactly is retained in `raw_samples` one more cycle (off-by-one), or a promoted hourly rollup that should stay in the medium window cascades away by the same off-by-one | `a_sample_exactly_at_the_recent_medium_boundary_is_promoted`, `a_sample_still_inside_the_recent_window_is_not_promoted` (one second short of the boundary) |
| Rollup aggregating zero samples (empty window) | `compact` over an empty database fabricates a row, or errors | `compact_with_no_samples_is_a_no_op` |
| Rollup idempotent if run twice on the same interval | A second `compact` call over unchanged data doubles a count/sum because it re-aggregates rows its first call already promoted | `compacting_twice_over_the_same_data_is_idempotent` - asserts the second call's `CompactionReport` is empty and the resulting `long_term_aggregates` row is byte-for-byte identical before/after |
| Future or backdated timestamp (clock skew) corrupts order | A sample recorded with an old timestamp but inserted after a newer one lands in the wrong bucket, or overwrites/merges with the wrong bucket, because promotion grouped by insertion order rather than the sample's own time | `a_late_arriving_backdated_sample_lands_in_its_historical_bucket_not_the_current_one` - inserts a "future-looking" sample first, a backdated one second, asserts each lands in its own distinct, correctly-valued hour bucket |
| Retention deletes a raw sample already promoted, losing the aggregate | After promotion, the raw row is gone but the destination aggregate never received its contribution (or received it twice) | `raw_samples_in_the_same_hour_aggregate_into_one_bounded_hourly_rollup` - asserts `raw_samples()` is empty and the hourly aggregate's count/sum/min/max exactly reflect what was deleted |
| Aggregate remains contentless even if the caller tries to pass sensitive data | A caller can smuggle a path/transcript/arbitrary string into a metric name or otherwise defeat the closed schema | `a_metric_name_cannot_carry_arbitrary_content_because_the_type_does_not_admit_one` (a real path string rejected by `MetricKind::from_key`, structurally, not by convention); `rollup_tables_have_only_the_allowlisted_columns` pins all four tables' actual `PRAGMA table_info` column sets |
| Advancing time by more than one window in a single call skips an intermediate window | A `now` that has crossed all three boundaries since the last compaction leaves a sample stranded mid-cascade (e.g. still in `hourly_rollups` instead of reaching `long_term_aggregates`) because only one promotion stage ran | `jumping_past_every_window_in_one_compact_call_still_reaches_long_term_without_skipping` - asserts all three `CompactionReport` counts are non-zero in one call and the sample lands correctly in `long_term_aggregates` |

Additional cases found while planning, not explicitly named by the brief but adjacent to the
above axes:

| Falsifier | Test |
| --- | --- |
| An unscoped sample (`provider_id`/`category` both absent) creates a new row per sample instead of aggregating, because SQL `UNIQUE` treats every `NULL` as distinct | `an_unscoped_sample_with_no_provider_or_category_still_aggregates_correctly` - this module does its own `IS`-based lookup rather than relying on a `UNIQUE` index, specifically to avoid that trap |
| Distinct scopes (different `provider_id`) silently merge into one row | `distinct_scopes_never_merge_into_each_other` |
| A long-term aggregate is overwritten instead of extended by a later batch | `long_term_aggregates_keep_growing_bounded_as_more_daily_rollups_are_promoted_into_them` |
| An invalid `RetentionPolicy` (windows not non-decreasing) is silently accepted | `retention_policy_rejects_windows_that_are_not_non_decreasing` |
| Corrupted stored `metric` panics instead of erroring | `malformed_stored_metric_is_reported_as_an_error_not_a_panic` |
| Crash recovery across a real process restart loses a promoted tier | `reopen_after_close_preserves_every_tier` |

Axes from the `adversarial-cases` skill not applicable here, with reason (same reasoning
E13-S02's own evidence packet already gives for this crate): path/identity, links/mounts,
provider-layout drift, platform differences (this module touches no filesystem path but its own
SQLite file); policy/trust conflicts, protection bypass, root escape (this crate makes no
authority/mutation decision - `cancellai-store`'s dependency surface remains `cancellai-model`/
`rusqlite`/`serde_json` only, unchanged by this story - no `cancellai-safety`/`cancellai-platform`
dependency to misuse); performance/large datasets (no caller yet exists at scale; each promotion
stage is `O(n)` in the rows currently eligible for that stage, not the whole table).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Raw samples expire according to policy." | `RetentionPolicy` (validated, non-hard-coded windows) plus `AnalyticalMemory::compact`, the one explicit primitive that ages a raw sample out of `raw_samples` once its age reaches `recent_window_secs`. `a_sample_exactly_at_the_recent_medium_boundary_is_promoted`, `a_sample_still_inside_the_recent_window_is_not_promoted`, `raw_samples_in_the_same_hour_aggregate_into_one_bounded_hourly_rollup` (asserts the raw row is gone after promotion) prove expiry happens exactly at the policy boundary, not earlier or later. | PASS |
| AC2 - "Aggregates support growth baseline without retaining sensitive content." | `hourly_rollups`/`daily_rollups`/`long_term_aggregates` carry only `metric` (closed `MetricKind` enum), `provider_id`/`category` (closed allowlist mirroring `EventMetadata`), and numeric statistics (`sample_count`/`sum_value`/`min_value`/`max_value` plus bucket/first/last timestamps) - `rollup_tables_have_only_the_allowlisted_columns` pins the actual schema; `a_metric_name_cannot_carry_arbitrary_content_because_the_type_does_not_admit_one` proves a caller cannot smuggle content through the one open-ended-looking field. `long_term_aggregates_keep_growing_bounded_as_more_daily_rollups_are_promoted_into_them` proves the long-term tier extends one row per `(metric, scope)` rather than growing unboundedly with every compaction. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| PERSISTENCE_MODEL.md Layer 3 "time-series rollups rather than permanent raw samples" | A raw sample never expires, or an aggregate silently grows per-sample instead of staying bounded | `raw_samples_in_the_same_hour_aggregate_into_one_bounded_hourly_rollup`, `jumping_past_every_window_in_one_compact_call_still_reaches_long_term_without_skipping`, `long_term_aggregates_keep_growing_bounded_as_more_daily_rollups_are_promoted_into_them` | PASS |
| PERSISTENCE_MODEL.md "Event payloads are contentless by default" (this story's own AC2 restates the same principle for Layer 3) | Widening the schema to a free-form/content field, or a metric name carrying arbitrary content | `rollup_tables_have_only_the_allowlisted_columns`, `a_metric_name_cannot_carry_arbitrary_content_because_the_type_does_not_admit_one`. Residual: `SampleScope`'s `provider_id`/`category` `String` fields cannot themselves be content-scanned - the same residual class E13-S02's evidence packet already records for `EventMetadata`'s `reason_code`. See Residual risks. | PASS (structural), residual noted |
| SI-024 "cached/local DB state is never destructive truth" | By construction: this module exposes no mutation-decision API - `record_sample`/`compact`/the four read methods only record and return numeric statistics; nothing here is, or could be, consulted as a mutation precondition. `cancellai-store`'s dependency surface (`cancellai-model`/`rusqlite`/`serde_json`) is unchanged by this story - no `cancellai-safety`/`cancellai-platform` dependency to misuse. | Cargo.toml inspection (unchanged); same argument E13-S01/E13-S02's own evidence packets already make | PASS |
| Fail-closed on corrupted content | A stored `metric` column is corrupted directly | `malformed_stored_metric_is_reported_as_an_error_not_a_panic` - `raw_samples()` returns `Err`, never panics | PASS |
| Crash/failure atomicity | Each `compact` call runs its three promotion stages in a single transaction | By construction (`AnalyticalMemory::compact`'s own body: one `self.conn.transaction()`, three stages, one `commit()`); not independently exercised by an injected-mid-transaction-failure test in this story (no fault-injection seam exists for `rusqlite::Transaction` in this crate, matching E13-S01/E13-S02's own precedent of relying on SQLite's own transaction guarantee rather than simulating a torn write) | PASS (by construction) |
| Crash recovery across process restart | Closing and reopening the database after a compaction | `reopen_after_close_preserves_every_tier` | PASS |
| Clock skew / non-monotonic recorded_at | A backdated sample inserted after a newer one | `a_late_arriving_backdated_sample_lands_in_its_historical_bucket_not_the_current_one` | PASS |

## Verification Commands

```text
$ cd rust
$ cargo fmt --check                                                        clean
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   all suites "test result: ok", 0 failed (cancellai-store: 42 passed, up from 26 - 16 new rollup tests)
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
$ cd ..
$ python3 scripts/project_os.py check                                     governance OK: 24 decisions, 32 epics, 165 stories
$ python3 scripts/check_rust_workspace.py check                           rust workspace OK: 13 crates match TARGET.md, acyclic, model/safety isolated
$ python3 scripts/check_mutation_boundary.py check                        mutation boundary OK (unaffected: cancellai-store deletes/moves nothing a provider owns)
$ python3 scripts/check_docs.py check                                     docs OK: 391 Markdown files; local links and safety IDs are consistent
$ python3 scripts/check_risk_classification.py check                      risk classification OK: no new story is below its floor (50 recorded, owner decision pending; E13-S03 not flagged)
$ python3 scripts/check_coverage.py report                                cancellai-store 89.70% (informational; outer-ring, not ratchet-gated)
$ python3 scripts/check_coverage.py check                                 coverage OK: 6 ratcheted crates at or above their recorded floor (cancellai-store not ratcheted)
$ python3 scripts/check_schemas.py check                                  schemas OK: 4 golden documents match docs/architecture/JSON_CONTRACTS.md
$ python3 scripts/check_fixtures.py check                                 fixtures OK: 13 fixtures cover all required categories
$ python3 scripts/diff_harness.py check                                   diff harness OK: self-test cases all behave as documented
$ python3 scripts/rust_python_parity.py self-test                         rust/python parity self-test OK: the comparator correctly catches every injected divergence class
$ python3 scripts/rust_python_parity.py check                             rust/python parity OK: 13 NORMATIVE fixture(s) match across engines (unaffected - no cancellai.py change, no Python-side equivalent of this rollup layer)
$ python3 scripts/characterize.py check                                   characterization OK: 13 fixtures match their committed characterization
$ python3 scripts/check_process.py check                                  process OK: ADR lifecycle, decision supersession, evidence, review rounds, and generated banners are consistent (pre-existing E00/E07/E12 round-ceiling warnings, unrelated to this story)
$ python3 scripts/check_evidence.py check                                 evidence OK: 145 packets checked (plus this one once committed); pre-existing baseline warnings unrelated to this story
$ python3 scripts/gate_sensitivity.py check                               gate sensitivity OK: 11 mutants, 11 killed
$ python3 scripts/check_ears.py check                                     EARS OK: 516 acceptance criteria classified; pre-existing baseline warnings on other stories, none new to E13-S03
$ python3 scripts/safety_oracle.py check                                  safety oracle OK
$ python3 scripts/verifier_handoff.py check                               verifier handoff OK
$ python3 scripts/release.py check                                        release OK: v1.14.0 is consistent across source, packaging and formula
$ python3 scripts/release_manifest.py check                               release manifest OK: 1 golden document(s) match project/schemas/release_manifest.schema.json
$ python3 scripts/check_repository_topology.py check                      repository topology OK
$ python3 scripts/check_agent_skills.py check                             agent skills OK: 8 skills; frontmatter, names and every cited path resolve
$ python3 scripts/gen_docs.py --check                                     docs/CLI.md is up to date
$ python3 -m pytest tests -q                                              665 passed, 551 subtests passed (Python reference suite unaffected - no cancellai.py change)
$ python3 -m ruff check .                                                 All checks passed!
$ python3 -m ruff format --check .                                        451 files already formatted
```

Cross-target clippy (`--target x86_64-pc-windows-gnu`/`x86_64-unknown-linux-gnu`) was not run:
`AGENTS.md` requires it specifically for changes touching `cancellai-platform` or moving the
lint surface workspace-wide, neither of which applies here (`cancellai-store` only, no
`cfg`-gated code added). Real CI still builds and lints natively on all three platforms.

`mypy` was not re-run against the full file list in this session for this change (no Python
source file changed - only `docs/`, `project/epics/E13.json`, `CHANGELOG.md`,
`rust/crates/cancellai-store/src/{lib.rs,rollup.rs}`, and this evidence packet); it is available
locally and CI runs it regardless, matching E13-S02's own precedent for the same situation.

## Compatibility

- `cancellai-store`'s `Cargo.toml` is unchanged - no new dependency (unlike E13-S02, which added
  `sha2`; this story needed no digest/hash capability).
- No existing public API (`CurrentStateStore`, `ledger::EventLedger`, or anything else) changed;
  `rollup` is a wholly new module and does not touch either prior layer's schema, file, or
  connection.
- No CLI/TUI/orchestrator surface consumes this yet (library-level capability only) - the same
  "primitive provided, orchestrator not yet built" precedent `CurrentStateStore` and
  `EventLedger` already set in this workspace.

## Performance / operability

- `record_sample` is one `INSERT` per call.
- `compact` runs its three promotion stages inside one transaction; each stage is `O(n)` in the
  rows currently eligible for that stage (one `SELECT` to group them, one upsert per distinct
  `(metric, scope, bucket)` group, one bounded `DELETE`), not in the whole table's size.
- `raw_samples`/`hourly_rollups`/`daily_rollups`/`long_term_aggregates` are full-table scans,
  matching `CurrentStateStore::all`/`EventLedger::read_all`'s own precedent - no caller yet needs
  a filtered read.

## Documentation updated

- `docs/architecture/PERSISTENCE_MODEL.md` - "Layer 3: Analytical Memory" now describes the real
  implementation: the independent-ingestion decision, the `RetentionPolicy`/`MetricKind`/
  `SampleScope` shapes, and the cascading-compaction contract.
- `CHANGELOG.md` - `[Unreleased]` / `### Added`.

## Method defects

- none

## Residual risks

- **`SampleScope`'s `String` fields cannot themselves be content-scanned.** The schema is closed
  (no free-form/blob field - `rollup_tables_have_only_the_allowlisted_columns` pins this, and
  `MetricKind` structurally refuses arbitrary content), but nothing stops a caller from putting a
  full path or excerpt into `provider_id`/`category`. Exactly the same residual class E13-S02's
  evidence packet already records for `EventMetadata`'s `reason_code`; not a new gap this story
  introduces. No story ID assigned; a future content-linter over call sites, if ever needed, is a
  separate, dedicated story.
- **`MetricKind`'s initial four variants are this story's own judgment call, not a ratified
  taxonomy.** They are named for `docs/architecture/GUARDIAN_MODEL.md`'s "Detection" list
  (free-disk capacity, provider/project budgets, session-count explosion, orphan-state growth)
  without attempting to enumerate Guardian's full future metric set, which is that document's own
  scope. Adding a metric later is a small, visible, reviewable diff to the enum (matching
  `EventKind`'s own precedent) - flagged here so a reviewer knows the set was deliberately kept
  minimal, not exhaustively designed.
- **No concurrent-access handling**, matching E13-S01/E13-S02's own recorded residual -
  `rusqlite`'s default behavior returns `SQLITE_BUSY` rather than retrying; no caller exists yet
  (single-process, single-connection use only).
- **No physical space reclamation beyond `DELETE`.** Promotion deletes the source rows (bounding
  each tier's logical size), but does not `VACUUM` the SQLite file - freed pages are reused by
  SQLite for future writes but the file does not shrink. Same acceptable-for-now position E13-S02
  already recorded; an actual reclamation/VACUUM policy is a self-budget-enforcer story's scope
  (E13-S04, which this story does not touch).
- **No orchestrator wiring.** No caller in this workspace records a sample or calls `compact` yet
  - this story delivers the primitive, matching `CurrentStateStore`'s and `EventLedger`'s own
  state at their own `ready_for_review`. Wiring real Guardian detection signals into
  `record_sample`, and scheduling `compact` calls, is out of this story's scope per `AGENTS.md`'s
  "work one story at a time" - flagged here, not fixed silently, should it belong to a later E13
  story, a Guardian story, or a new one.
- **No fault-injection test for a mid-transaction failure inside `compact`.** Atomicity across the
  three promotion stages rests on SQLite's own transaction guarantee (one `transaction()`/
  `commit()` wrapping all three stages) rather than on an independently exercised "kill partway
  through" test, matching E13-S01/E13-S02's own precedent in this crate (no fault-injection seam
  exists for `rusqlite::Transaction` here). Not a new gap this story introduces.
- This packet is executor self-assessment. Independent review happens at epic scope, once every
  story in E13 is `ready_for_review`.

## Round 1 independent review repair (2026-09-17)

Codex's round 1 review (`project/evidence/E13-VERIFIER-REVIEW.md`) returned `FAIL`: with
`RetentionPolicy::new(10, 20, 30)`, a sample recorded at `t=0`, and `compact(policy, now=5)`, the
raw-sample count went to `0` even though the sample's true age (5s) had not reached the 10s
recent window. `now.saturating_sub(recent_window_secs)` collapsed to a cutoff of `0` whenever
`now < recent_window_secs`, and the promotion query's `recorded_at <= cutoff` then matched the
`t=0` sample - `saturating_sub` silently turned "the window has not elapsed yet" into "everything
recorded at or before time zero is eligible," which is not the same claim.

Fixed in all three promotion stages (`promote_raw_to_hourly`/`promote_hourly_to_daily`/
`promote_daily_to_long_term`) by replacing `saturating_sub` with `checked_sub`: when `now` has
not yet reached the window, the function returns `Ok(0)` (nothing eligible) instead of computing
a cutoff at all. Regression test: `a_sample_is_not_promoted_when_now_has_not_yet_reached_the_
recent_window`, reproducing Codex's exact numbers.

Verification after the repair: the same full Rust gate set as this packet's original run,
re-executed and all passing; `cancellai-store` now has 95 tests (was 88), including the
regression above. No other change in this story's scope.

## Verifier verdict

Round 1 (Codex, 2026-09-17): FAIL - see the defect above, repaired in this packet. Round 2
pending.

# Evidence Packet - E13-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E13 epic review
- Change Risk: CR2 (declared CR2 at planning time in `project/epics/E13.json`; no risk floor in
  `project/risk_floors.json` applies to `rust/crates/cancellai-store/*` - only kernel-ring
  paths and `cancellai-model`/`cancellai-policy`/`cancellai.py` carry a floor above CR2 - and
  `python3 scripts/check_risk_classification.py check` confirms no new story falls below its
  floor. CR2 stays declared, unlike E13-S01's own reclassification to CR3.)
- Spec version/commit: `docs/architecture/PERSISTENCE_MODEL.md` "Layer 2: Operational Event
  Ledger"; `docs/adrs/0019-dependency-rings-per-crate.md` (outer-ring `rusqlite`/`sha2`,
  cancellai-store already named for "E13 a SQLite current-state store and event ledger")

## Outcome

PASS

## Scope

Implements `cancellai_store::ledger::EventLedger`, the append-only operational event ledger
Layer 2 of `docs/architecture/PERSISTENCE_MODEL.md` describes. Lives in the same
`cancellai-store` crate as E13-S01's `CurrentStateStore` (ADR-0019 names them as one story pair)
as a new `ledger` module with its own independent `Connection`/SQLite file/`PRAGMA
user_version` history - a deliberate choice over a new crate, since both are outer-ring
bundled-`rusqlite` state with no shared schema or shared caller today.

Public surface: `EventLedger::open`/`open_in_memory`, `append`, `read_all`, `compact_range`,
`compactions`. `EventKind` names the eleven kinds `PERSISTENCE_MODEL.md` lists;
`EventKind::is_mutation` names the six (`PLAN_CREATED`, `ACTION_BLOCKED`, `QUARANTINED`,
`RESTORED`, `ARCHIVED`, `PURGED`) this story's contract treats as representing an action.
`EventMetadata` is a closed field set (`artifact_id`, `provider_id`, `category`, `policy_id`,
`reason_code`); `MutationReference` (`plan_id` + `Vec<EvidenceId>`) is required for mutation-
class events. `CompactionSummary` carries the exact event count, a per-kind breakdown, and a
SHA-256 digest over a canonical, ordered encoding of the summarized events.

`plan_id` is a plain `String`, matching `docs/architecture/JSON_CONTRACTS.md`'s own untyped
`plan_id` field in the plan/explanation/result documents - no `PlanId` newtype exists anywhere
in `cancellai-model` yet (only `ActionId` does), and inventing one for this story alone would
touch the kernel-ring `cancellai-model` crate (CR3 floor) for a type no other story yet needs.
Adding it is a natural, separately-reviewable follow-up once a real `SealedPlan` field needs it.

`sha2` (0.11.0, `default-features = false`) is a new dependency of `cancellai-store`
(`rust/crates/cancellai-store/Cargo.toml`) - ADR-0019 outer-ring criteria: already used by two
kernel-ring crates (`cancellai-platform`, `cancellai-safety`) for the same class of integrity
digest (ADR-0030), already on the `rust/deny.toml` allow-list, and it makes no authority/
identity/mutation decision here - it only hashes an already-written audit record for tamper
evidence. Replaces nothing (`cancellai-store` had no prior hashing capability).

## Falsification plan (written before implementation)

Per `docs/development/AGENT_PROTOCOL.md`'s "Plan verification before code" and the
`adversarial-cases` skill's eleven axes, worked for this change:

| Falsifier | Would prove the implementation wrong | Test |
| --- | --- | --- |
| Append out of order | `read_all` returns events in an order other than append order (e.g. sorted by caller-supplied `recorded_at`) | `read_all_returns_events_in_append_order_not_in_recorded_at_order` (deliberately out-of-order timestamps) |
| Crash mid-write leaves the ledger incoherent | A failure partway through `append`/`compact_range` leaves a partial row, a partial delete, or a summary with no matching prior deletion | `compact_range_is_atomic_leaving_events_and_compactions_untouched_on_failure`, `append_rejects_a_mutation_event_with_no_mutation_reference_at_all` (asserts no partial row after a rejected append) |
| Mutate/delete a committed event | Any path - public or raw SQL - can alter or remove an already-committed `ledger_events` row outside `compact_range` | `raw_update_against_a_committed_event_is_rejected_by_the_database_itself`, `raw_delete_against_a_committed_event_is_rejected_outside_compaction`, `raw_update_against_a_compaction_summary_is_rejected_by_the_database_itself` - attempted directly against the connection, not merely absent as a Rust method |
| Event with artifact content in the payload | `EventMetadata`/the stored schema accepts a free-form/content-typed field | `ledger_events_schema_has_only_the_allowlisted_columns` pins the actual `PRAGMA table_info` column set against the closed allowlist |
| Mutation event with no plan_id/evidence_ids | `append` accepts a `QUARANTINED`/`RESTORED`/`ARCHIVED`/`PURGED`/`ACTION_BLOCKED`/`PLAN_CREATED` event with an absent, empty-`plan_id`, or empty-`evidence_ids` reference | `append_rejects_a_mutation_event_with_no_mutation_reference_at_all`, `append_rejects_a_mutation_event_with_an_empty_plan_id`, `append_rejects_a_mutation_event_with_no_evidence_ids` |
| Compaction loses not-yet-summarized events | `compact_range` succeeds over a range with a gap (part already compacted, part never appended), silently covering fewer events than the summary claims | `compact_range_rejects_a_range_with_a_gap_from_a_prior_compaction`, `compact_range_rejects_an_inverted_range` |
| Compaction changes audit/aggregate semantics | After compaction, a per-kind count (e.g. "how many QUARANTINED") becomes unrecoverable, or the digest cannot detect a re-attributed summary | `compact_range_replaces_the_events_with_one_summary_and_they_are_gone_from_read_all` asserts `kind_counts`/`digest_hex` shape; digest is over every field of every summarized event |
| Crash recovery across a real process restart | Reopening the database after a close loses events, reuses an event id, or forgets a compaction | `append_after_reopen_preserves_prior_events_and_never_reuses_an_event_id`, `compaction_survives_a_reopen_and_stays_the_only_way_events_disappeared` |
| Corrupted stored content | A hand-corrupted `evidence_ids` column panics instead of erroring | `malformed_stored_evidence_ids_is_reported_as_an_error_not_a_panic` |

Axes from the `adversarial-cases` skill not applicable here, with reason: path/identity,
links/mounts, provider-layout drift, platform differences (this crate touches no filesystem
path but its own SQLite file, same as `CurrentStateStore`); policy/trust conflicts, protection
bypass, root escape (this crate makes no authority/mutation decision - it only records that one
happened elsewhere, per SI-024's "cached/local DB state is never destructive truth" and this
crate's own zero dependency on `cancellai-safety`/`cancellai-platform`); performance/large
datasets (no story yet calls this at scale; `compact_range`'s own contiguous-range check is
O(n) in the range size, not the whole ledger).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Events are immutable after commit except compaction into signed/hashed summary records as specified." | `ledger_events`/`ledger_compactions` carry `BEFORE UPDATE`/`BEFORE DELETE` triggers that `RAISE(ABORT, ...)` unconditionally for update and (for `ledger_events`) for delete unless `compact_range`'s own gate is set inside its own transaction. `raw_update_against_a_committed_event_is_rejected_by_the_database_itself`, `raw_delete_against_a_committed_event_is_rejected_outside_compaction`, `raw_update_against_a_compaction_summary_is_rejected_by_the_database_itself` attempt exactly that bypass directly against the connection (not merely relying on the absence of a public Rust method) and assert the database refuses it. `compact_range_replaces_the_events_with_one_summary_and_they_are_gone_from_read_all` proves the one legitimate replacement path: a signed (SHA-256 digest), exact-count, per-kind-summarized record stands in place of the removed range. | PASS |
| AC2 - "Every mutation references plan/evidence IDs." | `EventKind::is_mutation` names the six action-representing kinds; `append` returns `Err` and writes nothing for any of them missing a non-empty `plan_id`/`evidence_ids`, proven by `append_rejects_a_mutation_event_with_no_mutation_reference_at_all`, `append_rejects_a_mutation_event_with_an_empty_plan_id`, `append_rejects_a_mutation_event_with_no_evidence_ids`. `append_then_read_all_round_trips_every_field` proves a valid mutation reference round-trips exactly. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| PERSISTENCE_MODEL.md "Event payloads are contentless by default" | Widening the schema to a free-form/content field | `ledger_events_schema_has_only_the_allowlisted_columns` pins the exact `ledger_events` column set; `EventMetadata`'s closed field list has no content-typed field. Residual: a caller can still misuse an allowlisted `String` field (e.g. `reason_code`) to carry something it should not - this crate cannot detect misuse of a field it does not itself populate (same class of residual `cancellai_model::Action`/`Precondition` already carry as inert data). See Residual risks. | PASS (structural), residual noted |
| SI-024 "cached/local DB state is never destructive truth" | By construction: this crate exposes no mutation-decision API - `append`/`read_all`/`compact_range`/`compactions` only record and return data a caller already produced/decided elsewhere; nothing here is, or could be, consulted as a mutation precondition. Not exercised by a counterexample test because there is no code path to attack - `cancellai-store`'s dependency surface is `cancellai-model`/`rusqlite`/`serde_json`/`sha2` only, no `cancellai-safety`/`cancellai-platform` dependency to misuse. | Cargo.toml inspection; same argument E13-S01's own evidence packet already makes for `CurrentStateStore` | PASS |
| Fail-closed on corrupted content | A stored `evidence_ids` column is corrupted directly (bypassing the immutability gate, as a hand edit or an unrelated tool would) | `malformed_stored_evidence_ids_is_reported_as_an_error_not_a_panic` - `read_all` returns `Err`, never panics | PASS |
| Crash/failure atomicity | A `compact_range` call whose range check fails partway (gap from a prior compaction, or an inverted range) | `compact_range_is_atomic_leaving_events_and_compactions_untouched_on_failure`, `compact_range_rejects_a_range_with_a_gap_from_a_prior_compaction`, `compact_range_rejects_an_inverted_range` - the transaction rolls back completely; no partial delete, no orphan summary | PASS |
| Crash recovery across process restart | Closing and reopening the database after events/compactions were written | `append_after_reopen_preserves_prior_events_and_never_reuses_an_event_id`, `compaction_survives_a_reopen_and_stays_the_only_way_events_disappeared` | PASS |

## Verification Commands

```text
$ cd rust
$ cargo fmt --check                                                        clean (after `cargo fmt` applied once during development)
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   all suites "test result: ok", 0 failed (cancellai-store: 26 passed, up from 9 - 17 new ledger tests)
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
$ cd ..
$ python3 scripts/project_os.py check                                     governance OK: 24 decisions, 32 epics, 165 stories
$ python3 scripts/check_rust_workspace.py check                           rust workspace OK: 13 crates match TARGET.md, acyclic, model/safety isolated
$ python3 scripts/check_mutation_boundary.py check                        mutation boundary OK (unaffected: cancellai-store deletes/moves nothing a provider owns)
$ python3 scripts/check_schemas.py check                                  schemas OK: 4 golden documents match docs/architecture/JSON_CONTRACTS.md
$ python3 scripts/check_fixtures.py check                                 fixtures OK: 13 fixtures cover all required categories
$ python3 scripts/check_docs.py check                                     docs OK: 390 Markdown files; local links and safety IDs are consistent
$ python3 scripts/check_risk_classification.py check                      risk classification OK: no new story is below its floor (E13-S02 not flagged)
$ python3 scripts/check_coverage.py report                                cancellai-store 92.18% (informational; outer-ring, not ratchet-gated)
$ python3 scripts/check_coverage.py check                                 coverage OK: 6 ratcheted crates at or above their recorded floor (cancellai-store not ratcheted)
$ python3 scripts/rust_python_parity.py self-test                         rust/python parity self-test OK: the comparator correctly catches every injected divergence class
$ python3 scripts/rust_python_parity.py check                             rust/python parity OK: 13 NORMATIVE fixture(s) match across engines (unaffected - no cancellai.py change, no Python-side equivalent of this ledger)
$ python3 scripts/diff_harness.py check                                   diff harness OK: self-test cases all behave as documented
$ python3 scripts/check_agent_skills.py check                             agent skills OK: 8 skills; frontmatter, names and every cited path resolve
$ python3 scripts/check_evidence.py check                                 evidence OK: 144 packets checked (plus this one once committed); pre-existing baseline warnings unrelated to this story
$ python3 scripts/gate_sensitivity.py check                               gate sensitivity OK: 11 mutants, 11 killed
$ python3 scripts/check_ears.py check                                     EARS OK: 516 acceptance criteria classified; E13-S02 carries the same pre-existing "no unwanted-behaviour AC" baseline warning every CR2 story written before E25-S08 carries (not new to this story)
$ python3 scripts/safety_oracle.py check                                  safety oracle OK
$ python3 scripts/verifier_handoff.py check                               verifier handoff OK
$ python3 -m pytest tests -q                                              658 passed (Python reference suite unaffected - no cancellai.py change)
$ python3 -m ruff check .                                                 All checks passed!
$ python3 -m ruff format --check .                                        450 files already formatted
```

Cross-target clippy (`--target x86_64-pc-windows-gnu`/`x86_64-unknown-linux-gnu`) was not run:
`AGENTS.md` requires it specifically for changes touching `cancellai-platform` or moving the
lint surface workspace-wide, neither of which applies here (`cancellai-store` only, no
`cfg`-gated code added). Real CI still builds and lints natively on all three platforms.

`mypy` was not re-run against the full file list in this session for this change (no Python
source file changed - only `docs/`, `project/epics/E13.json`, `CHANGELOG.md`, and this evidence
packet); it is available locally (`mypy 2.3.1`) and CI runs it regardless.

## Compatibility

- `cancellai-store`'s `Cargo.toml` gains one new dependency, `sha2` (see Scope above) -
  additive, no existing dependency version changed.
- No existing public API (`CurrentStateStore` or anything else) changed; `ledger` is a wholly
  new module and does not touch `CurrentStateStore`'s schema, file, or connection.
- No CLI/TUI/orchestrator surface consumes this yet (library-level capability only) - the same
  "primitive provided, orchestrator not yet built" precedent `cancellai_platform::mutation::
  verify_archive_integrity` and E13-S01's own `CurrentStateStore` already set in this workspace.

## Performance / operability

- `append` is one `INSERT` per call inside its own transaction (one fsync-equivalent commit).
- `compact_range` is `O(n)` in the size of the requested range (one `COUNT(*)`, one row scan
  for hashing/counting, one `DELETE`, one `INSERT`), not in the whole ledger's size.
- `read_all`/`compactions` are full-table scans, matching `CurrentStateStore::all`'s own
  precedent - no story yet needs a filtered read.

## Documentation updated

- `docs/architecture/PERSISTENCE_MODEL.md` - "Layer 2: Operational Event Ledger" now describes
  the real implementation: the trigger-enforced immutability mechanism, the mutation-reference
  requirement, the contentless metadata allowlist, and the compaction contract.
- `CHANGELOG.md` - `[Unreleased]` / `### Added`.

## Method defects

- none

## Residual risks

- **`EventMetadata`'s `String` fields cannot themselves be content-scanned.** The schema is
  closed (no free-form/blob field - `ledger_events_schema_has_only_the_allowlisted_columns`
  pins this), but nothing stops a caller from putting a full path or excerpt into e.g.
  `reason_code`. Same residual class `cancellai_model::Action`/`Precondition` already accept as
  inert data whose correctness is the caller's responsibility, not this type's. Not a new gap
  this story introduces - flagged here because Layer 2's own contentless requirement makes it
  worth naming explicitly. No story ID assigned; a future content-linter over ledger call sites,
  if ever needed, is a separate, dedicated story.
- **No `PlanId` newtype.** `plan_id` is a plain `String`, matching `JSON_CONTRACTS.md`'s own
  untyped field (see Scope above). A future story that adds a real `PlanId` to
  `cancellai-model` (once `SealedPlan` itself carries one) should migrate this field then,
  rather than this story guessing its shape now.
- **No concurrent-access handling**, matching E13-S01's own recorded residual for
  `CurrentStateStore` - `rusqlite`'s default behavior returns `SQLITE_BUSY` rather than
  retrying; no caller exists yet (single-process, single-connection use only).
- **No physical space reclamation beyond `DELETE`.** `compact_range` deletes the compacted
  rows (bounding `ledger_events`' logical size, per `PERSISTENCE_MODEL.md`'s "Self-budget"
  intent), but does not `VACUUM` the SQLite file - freed pages are reused by SQLite for future
  writes but the file does not shrink. Acceptable for this story (no self-budget enforcer story
  exists yet - that is E13-S04's scope, which depends on E13-S02); E13-S04 is the right place
  to decide an actual reclamation/VACUUM policy.
- **No orchestrator wiring.** No caller in this workspace appends to the ledger yet - this
  story delivers the primitive, matching E13-S01's own precedent (`CurrentStateStore` had the
  same state at its own `ready_for_review`). Wiring real callers (quarantine/restore/archive/
  purge/plan/policy code paths) is out of this story's scope per the story contract and
  `AGENTS.md`'s "work one story at a time" - flagged here, not fixed silently, should it belong
  to a later E13 story or a new one.
- This packet is executor self-assessment. Independent review happens at epic scope, once every
  story in E13 is `ready_for_review`.

## Verifier verdict

pending

# Evidence Packet - E13-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E13 epic review
- Change Risk: CR3 (declared CR2 at planning time in `project/epics/E13.json`; raised at
  commit time when the risk-floor gate refused it - the change touches
  `rust/crates/cancellai-model/src/agent_artifact.rs` (adding `Deserialize` to `AgentArtifact`
  and the types it carries), the domain-model floor `project/risk_floors.json` sets for that
  path. Additive-only: no field, semantics, or existing `Serialize` output changes.
  `project/epics/E13.json` records the reclassification)
- Spec version/commit: `docs/architecture/PERSISTENCE_MODEL.md` "Layer 1: Current State";
  `docs/CONSTITUTION.md` C-10 "Reconstructible local state";
  `docs/security/SAFETY_INVARIANTS.md` SI-024 "Persistent cache is never destructive truth";
  `docs/adrs/0019-dependency-rings-per-crate.md` (outer-ring `rusqlite` decision, named for this
  story at planning time)

## Outcome

PASS

## Scope

Implements `cancellai-store::CurrentStateStore`, the first real content in what was previously
an empty skeleton crate (E02-S01). A bundled-SQLite (ADR-0019) database holds one row per
`cancellai_model::AgentArtifact`, keyed by `ArtifactId`, with the full row stored as its own JSON
wire format (`data` column) and `provider_id`/`activity_state` pulled out as indexed columns for
the fast-query purpose `docs/architecture/PERSISTENCE_MODEL.md`'s Layer 1 names. Three operations:
`open`/`open_in_memory` (creates and migrates the database), `rebuild` (replaces the table's
entire content in one transaction), `all`/`get` (read back). Schema versioning uses SQLite's own
`PRAGMA user_version`, one migration per schema step, each in its own transaction.

`cancellai-model`'s `AgentArtifact` and every type it transitively carries (`ArtifactId`,
`EvidenceId`, `RelationshipKind`, `ArtifactRelationship`, `ProjectRef`, `AttributionSource`,
`ProjectAttribution`, `ActivitySignal`, and the vocabulary enums `Reversibility`,
`KnowledgeConfidence`, `ActivityState`, `ProtectionState`, `IntegrityState`, `RiskClass`,
`ResidencyState`) gained `serde::Deserialize` alongside their existing `Serialize` - necessary
infrastructure for this story's own round-trip requirement (AC2), not a separate refactor;
`AuthorityLevel` already had both from E11-S01's own need. No other type in `cancellai-model`
was touched.

No query API beyond `all`/`get` is implemented - no story yet asks for one, and
`docs/architecture/PERSISTENCE_MODEL.md`'s own updated text records that a fully normalized
schema or richer query surface is a follow-up story's job once a real caller needs a query this
shape cannot answer efficiently.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Deleting the DB never deletes provider artifacts." | This crate's only filesystem interaction is the SQLite file at the caller-supplied path (`Cargo.toml` depends on `cancellai-model` and `rusqlite` only - no filesystem-mutation capability). `deleting_the_database_file_never_touches_a_provider_artifact` creates a real "provider artifact" file, populates a store referencing it, deletes the store's own database file, and asserts the provider file and its content are untouched. | PASS |
| AC2 - "Rebuild from filesystem/provider produces equivalent current-state semantics." | `rebuild_then_all_round_trips_every_field_exactly` round-trips a fully-populated `AgentArtifact` (every `Option`/`Vec` field non-empty) through `rebuild`/`all` and asserts equality. `rebuild_replaces_the_entire_previous_content_rather_than_accumulating` and `rebuild_to_empty_leaves_no_rows_behind` prove a rebuild is a full replacement, not an upsert/accumulation - the store's content after a rebuild depends only on what was just scanned. | PASS |
| AC3 - "Schema migrations are transactional." | `apply_migrations_leaves_user_version_unchanged_when_a_later_migration_fails` runs a synthetic two-migration set where the second is deliberately invalid SQL after a valid `CREATE TABLE`; asserts `user_version` stays at the last fully-committed migration and the failed migration's own table does not exist (the whole batch rolled back, not just the failing statement). `apply_migrations_is_a_no_op_when_every_migration_was_already_applied` and `open_creates_the_real_schema_and_is_idempotent_across_repeated_opens` prove re-opening an already-migrated database never re-attempts a completed migration. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| C-10 (reconstructible, never source of truth) | Deleting the current-state database | `deleting_the_database_file_never_touches_a_provider_artifact` | PASS |
| SI-024 (persistent cache never destructive truth) | By construction: this crate exposes no mutation-decision API at all - `open`/`open_in_memory`/`rebuild`/`all`/`get` only store and return data a caller already produced; nothing here is, or could be, consulted as a mutation precondition. Not exercised by a counterexample test because there is no code path to attack - the invariant holds by the crate's own dependency surface (`cancellai-model` only; no `cancellai-safety`/`cancellai-platform` dependency exists to misuse). | PASS |
| Fail-closed on corrupted content | A stored row's `data` column is corrupted (hand edit, partial write from an unrelated tool) | `malformed_stored_content_is_reported_as_an_error_not_a_panic` - `all()`/`get()` return `Err`, never panic, never silently substitute a default | PASS |

## Verification Commands

```text
$ cargo fmt --check                                                        clean
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   all suites "test result: ok", 0 failed (cancellai-store: 9 passed, cancellai-model: 93 passed)
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
$ python3 scripts/project_os.py check                                      governance OK
$ python3 scripts/check_rust_workspace.py check                            OK (14 crates, acyclic)
$ python3 scripts/check_mutation_boundary.py check                         OK (unaffected: cancellai-store deletes/moves nothing)
$ python3 scripts/rust_python_parity.py check                              OK (unaffected, no cancellai.py change)
$ python3 scripts/check_docs.py check                                      OK
$ python3 scripts/check_coverage.py report                                 cancellai-store 94.44% (informational; outer-ring, not ratchet-gated)
```

Cross-target clippy (`--target x86_64-pc-windows-gnu`/`x86_64-unknown-linux-gnu`) was not run:
AGENTS.md requires it specifically for changes touching `cancellai-platform` or moving the lint
surface workspace-wide, neither of which applies here (`cancellai-model`/`cancellai-store` only,
no new `cfg`-gated code). Real CI still builds and lints natively on all three platforms.

If a dev tool is unavailable locally: ruff/mypy were available and run clean in this session
(`ruff 0.16.5`, `mypy 2.3.1`); not applicable to this change regardless, since it touches no
Python source.

## Compatibility

- `cancellai-store`'s `Cargo.toml` gains its first real dependency, `rusqlite` (outer ring,
  ADR-0019, already named for this exact story). `default-features = false`, `features =
  ["bundled"]` - compiles SQLite's own C source rather than linking a host's system SQLite, so
  behavior is identical across macOS/Linux/Windows regardless of host package state.
- `cancellai-model`'s public types gain `Deserialize` - strictly additive; no existing
  `Serialize` output, field, or behavior changes. Every other crate depending on
  `cancellai-model` continues to compile and pass its own tests unchanged (`cargo test
  --workspace` above).
- No CLI/TUI/scan-pipeline surface consumes this yet (library-level capability only) - the same
  "primitive provided, orchestrator not yet built" precedent this workspace already uses for
  `cancellai_platform::mutation::verify_archive_integrity` before any purge caller existed.

## Performance / operability

- `rebuild` is `O(n)` in the number of artifacts, one `DELETE` plus one prepared `INSERT` per
  artifact, all inside a single transaction (one fsync-equivalent commit, not one per row).
- Each row's `data` column is the artifact's full JSON encoding; `provider_id`/`activity_state`
  are duplicated into their own indexed columns specifically so a caller can filter by either
  without deserializing every row's JSON.

## Documentation updated

- `docs/architecture/PERSISTENCE_MODEL.md` - "Layer 1: Current State" now describes the real
  implementation, the reconstruction contract, and the migration-transactionality guarantee.
- `CHANGELOG.md` - `[Unreleased]` / Added.

## Method defects

- none

## Residual risks

- **No concurrent-access handling.** `rusqlite`'s default behavior returns an immediate
  `SQLITE_BUSY` error rather than waiting if a second connection holds a write lock; this crate
  sets no `busy_timeout` and has no retry policy. Acceptable for now because no caller exists yet
  (single-process, single-connection use only) - a real orchestrator story is the right place to
  decide the actual concurrency model (a single long-lived writer, a busy-timeout-and-retry
  policy, or something else) rather than guessing one here.
- **No query API beyond whole-table `all`/single-row `get`.** No story yet needs anything more
  specific; `docs/architecture/PERSISTENCE_MODEL.md` records that a richer query surface (e.g.
  filter by `provider_id`/`activity_state`, the two columns already indexed for this) is a
  follow-up story's job.
- **Schema is one JSON blob column plus two indexed columns, not fully normalized.** Deliberate
  for this story's scope (see Scope above) - revisit once a real caller needs a query this shape
  cannot answer efficiently.
- Coverage: `cancellai-store` measured 94.44% (`scripts/check_coverage.py report`); not
  ratchet-gated (outer ring, not in `project/coverage_baseline.json`'s gated set), reported here
  for visibility only.
- This packet is executor self-assessment. Independent review happens at epic scope, once every
  story in E13 is `ready_for_review`.

## Verifier verdict

pending

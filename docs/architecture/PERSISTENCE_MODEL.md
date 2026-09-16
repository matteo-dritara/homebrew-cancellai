# Persistence and Lifecycle Storage

## Principle

cancellAI remembers enough to become safer and more useful, never enough to become the storage problem it exists to control.

## Layer 1: Current State

A local database, initially expected to be SQLite in the Rust architecture, indexes the latest known metadata required for fast queries:

- artifact identity and relationships;
- provider/project/session references;
- lifecycle axes;
- size/reclaim observations;
- evidence/confidence summaries;
- policy/effective-authority results;
- last scan completeness.

This database is a **reconstructible cache/index**, not the source of truth. Dropping it must not change provider state. `reset --local-state` deletes cancellAI state only.

E13-S01 implements the store itself: `cancellai_store::CurrentStateStore`, a bundled-SQLite
database (ADR-0019's outer-ring `rusqlite`, named for this story at planning time) holding one
row per `cancellai_model::AgentArtifact`, keyed by its `ArtifactId`. Each row's full content is
the artifact's own JSON wire format (`docs/architecture/JSON_CONTRACTS.md`), with `provider_id`
and `activity_state` pulled out as their own indexed columns for the fast queries this layer
exists for; a fully normalized schema is a follow-up story's job once a real caller needs a query
this shape cannot answer. `CurrentStateStore::rebuild` is the reconstruction primitive: it
replaces the table's entire content, in one transaction, with exactly the set of artifacts it is
given - the store's content after a rebuild depends only on what was just scanned, never on what
it held before (including a fresh, empty database). Schema migrations use SQLite's own `PRAGMA
user_version` rather than a bookkeeping table, and each migration runs inside its own
transaction, so a migration that fails partway rolls back everything it had already done and
leaves `user_version` exactly where it started. This crate never touches a provider path - its
only filesystem interaction is the SQLite file itself - so deleting that file, exactly what
`reset --local-state` does, cannot delete a provider artifact by construction, not merely by
convention.

## Layer 2: Operational Event Ledger

Significant events are append-only logical records:

```text
DISCOVERED
CLASSIFIED
LIFECYCLE_CHANGED
POLICY_CHANGED
ANOMALY_DETECTED
PLAN_CREATED
ACTION_BLOCKED
QUARANTINED
RESTORED
ARCHIVED
PURGED
```

Mutation events reference the plan ID, evidence IDs, policy resolution, and observed result. Event payloads are contentless by default.

The ledger is not an excuse for infinite retention. Old events can be compacted into bounded summaries provided audit semantics and aggregate metrics remain defined.

E13-S02 implements the ledger itself: `cancellai_store::ledger::EventLedger`, a second,
independent bundled-SQLite database (same outer-ring `rusqlite` dependency Layer 1 uses, its
own `Connection`/file/`PRAGMA user_version` history) living alongside `CurrentStateStore` in
this crate rather than a new one - ADR-0019 already names "a SQLite current-state store and
event ledger" as one story pair. `EventLedger::append` is the only way a `ledger_events` row is
ever written; there is no public update or delete. Immutability is enforced at the SQLite layer
itself, not merely by the absence of a Rust method: the schema carries `BEFORE UPDATE`/`BEFORE
DELETE` triggers that `RAISE(ABORT, ...)` unconditionally for update, and for delete unless a
one-row gate (`ledger_control.compaction_in_progress`) is set - and only
`EventLedger::compact_range` ever sets that gate, inside the same transaction as the delete it
performs and the summary row it writes. `ledger_compactions` (the summary table) is immutable
the same way, unconditionally.

`EventKind::is_mutation` names the six kinds above that represent an action rather than a pure
observation - `PLAN_CREATED`, `ACTION_BLOCKED`, `QUARANTINED`, `RESTORED`, `ARCHIVED`, `PURGED`
- and `append` fails closed, writing nothing, for any event of one of those kinds whose
`MutationReference` (`plan_id` + at least one `EvidenceId`) is absent or empty. Every event
carries only a closed, allowlisted set of metadata (`artifact_id`, `provider_id`, `category`,
`policy_id`, `reason_code`) - there is no free-form map or content-typed field a caller could
use to smuggle a path, transcript, or artifact content into the ledger; the table's actual
column set is pinned by its own test against silent widening.

`EventLedger::compact_range` is the one explicit, never-silent compaction primitive: it
requires the requested `[from, to]` range to be exactly and contiguously present in
`ledger_events` (row count must equal `to - from + 1`), which is what stops a range that
overlaps an already-compacted window, or reaches past the newest appended event, from silently
summarizing fewer events than requested. On success it deletes exactly that range and writes one
`CompactionSummary` in its place, in the same transaction: the exact event count, a per-kind
breakdown (so an aggregate such as "how many `QUARANTINED` events happened in this window"
stays answerable once the raw rows are gone), and a SHA-256 digest over a canonical, ordered
encoding of every summarized event - a summary cannot be quietly re-attributed to a different
set of events without changing the digest. `read_all` returns events in append order (SQLite's
own `AUTOINCREMENT` rowid, which never reuses an id once assigned, including one a compaction
later retired), independent of each event's own caller-supplied `recorded_at` - a skewed or
malicious clock cannot reorder the audit trail.

## Layer 3: Analytical Memory

Guardian intelligence uses time-series rollups rather than permanent raw samples.

Indicative retention strategy:

- recent window: fine-grained samples;
- medium window: hourly rollups;
- long window: daily rollups;
- beyond long window: bounded statistics/tombstone aggregates.

Exact periods and budgets are product policy, not hard-coded architecture constants.

E13-S03 implements this layer: `cancellai_store::rollup::AnalyticalMemory`, a third, independent
bundled-SQLite database in this crate (its own `Connection`/file/`PRAGMA user_version` history,
alongside `CurrentStateStore` and `EventLedger`) that owns its own ingestion
(`AnalyticalMemory::record_sample`) rather than deriving from Layer 2's ledger events - the two
layers record different kinds of things (a discrete significant event versus a repeated numeric
reading) and stay independently retained under their own self-budgets. `RetentionPolicy` carries
the recent/medium/long window lengths named above as explicit, caller-supplied durations in
seconds - `RetentionPolicy::DEFAULT` (one day / one week / ninety days) is one reasonable choice,
not the only legal one, and `RetentionPolicy::new` rejects a policy whose windows are not
non-decreasing.

`MetricKind` is a closed, exhaustive enum naming the measurements this layer accepts
(`ArtifactCount`, `ProviderFootprintBytes`, `ReclaimableBytes`, `OrphanCount`, named for
`docs/architecture/GUARDIAN_MODEL.md`'s own "Detection" signals without committing to its full
future taxonomy) - a caller cannot smuggle a path, transcript, or other content through the
metric name, because the type does not admit one. `SampleScope` (`provider_id`, `category`) is
the same closed, allowlisted dimension shape Layer 2's `EventMetadata` already uses; a sample's
`value` is a plain number. `tests::rollup_tables_have_only_the_allowlisted_columns` pins the
actual schema of all four tables (`raw_samples`, `hourly_rollups`, `daily_rollups`,
`long_term_aggregates`) against this closed set.

`AnalyticalMemory::compact(policy, now)` is the one explicit primitive that ever moves a sample
or rollup out of its tier - there is no implicit background expiry. One call runs the full raw
-> hourly -> daily -> long-term cascade in a single transaction: it promotes every raw sample
whose age has reached the recent window into an hourly bucket (grouped by `metric`, `provider_id`,
`category`, and the hour it was recorded in - an inclusive boundary, so a sample exactly
`recent_window_secs` old is promoted, not retained one more cycle), then promotes every hourly
rollup old enough to leave the medium window into a daily bucket the same way, then promotes every
daily rollup old enough to leave the long window into one long-term aggregate per `(metric,
scope)` with no further time bucketing - `PERSISTENCE_MODEL.md`'s own "bounded statistics/
tombstone aggregates" beyond the long window. Because the cascade runs in that order within one
call, a `now` that has advanced past more than one window boundary since the last compaction does
not strand a sample in an intermediate tier: newly promoted hourly/daily rows are themselves
checked against the next cutoff before the transaction commits. Grouping keys on each row's own
recorded time (or the bucket it already carries), never on insertion order, so a backdated or
clock-skewed sample lands in its historically correct bucket instead of corrupting whichever
bucket happens to be "current." A second `compact` call over unchanged data finds nothing newly
eligible in any tier and is a no-op - promoting a row and deleting its source happen in the same
transaction, so nothing is ever double-counted by a later call.

## Self-budget

cancellAI enforces explicit budgets for:

- current-state DB;
- event ledger;
- analytical memory;
- logs;
- temporary release/scan artifacts.

When approaching its budget, cancellAI compacts/rotates its own data before collecting more optional history. Safety-critical current facts may force analytical sampling to degrade rather than exceed the budget.

E13-S04 implements self-budget enforcement and local-state reset for the three layers above that
this crate holds: `cancellai_store::budget`, a fourth module that adds no schema, file or
connection of its own - it is a thin policy layer over the compaction/reset primitives Layer 1/2/3
already expose (E13-S01/S02/S03), deciding *when* to act, never *how*. `BudgetLimits` carries one
explicit, caller-supplied threshold per layer (`max_current_state_rows`/`max_ledger_events`/
`max_raw_samples`) - "exact periods and budgets are product policy, not hard-coded architecture
constants" (this document's own words for Layer 3's retention windows, extended here to budgets);
`BudgetLimits::DEFAULT` is one reasonable choice, not the only legal one, matching
`RetentionPolicy::DEFAULT`'s own documented status.

`enforce_ledger_budget`/`enforce_rollup_budget` call `EventLedger::compact_oldest_to_fit`/
`AnalyticalMemory::compact_if_over` - each new, but each doing nothing except deciding whether to
invoke a compaction primitive that already existed (`compact_range`/`compact`) - before growth
continues: a store already exactly at its limit is left untouched, and a store one row over
compacts exactly the excess, never more, never a silent overrun. `EventLedger::compact_oldest_to_fit`
always targets the oldest contiguous prefix and fails closed, leaving the ledger completely
unchanged, if that computed range is not exactly and contiguously present - the same atomicity
`compact_range` itself already guarantees, so a self-budget enforcement call can never leave the
ledger worse off than the overrun it was trying to fix. `AnalyticalMemory::compact_if_over` still
only ages a sample out by `RetentionPolicy`'s own time windows; being over the row-count budget
does not, by itself, promote a sample that has not yet reached `recent_window_secs` - a budget
whose windows cannot keep the raw tier under its row limit at the caller's ingestion rate is a
policy/budget mismatch this surfaces (zero promotions despite running) rather than one it silently
resolves. `check_current_state_budget` only observes `CurrentStateStore::row_count` against its
limit - Layer 1 has no compaction action of its own, because its content is entirely determined by
the last external `rebuild`; this section's own "safety-critical current facts may force analytical
sampling to degrade" already names Layer 3 sampling, not Layer 1 itself, as what yields under
Layer 1 budget pressure, and wiring that cross-layer degradation decision needs a live (Guardian)
caller this workspace does not have yet.

`reset --local-state` (SI-026: "cancellAI reset/self-budget cannot target provider payload") is
`cancellai_store::budget::reset_local_state`, sequencing `CurrentStateStore::reset`,
`EventLedger::reset` and `AnalyticalMemory::reset` - one per layer, since each layer owns an
independent file/connection and cannot share one SQLite transaction. Every one of those three
methods takes no path and no caller-supplied target at all: the only thing any of them can act on
is the connection it already owns, which discharges SI-026 by construction, not by convention -
there is no parameter anywhere in this call chain a provider path could even be passed through.
Layer 1 and Layer 3 reset by a plain `DELETE FROM` per table inside one transaction (matching
`CurrentStateStore::rebuild`'s own pattern - `reset` is exactly `rebuild` given nothing). Layer 2 is
different: `ledger_compactions`' `BEFORE DELETE` trigger refuses unconditionally (compaction
summaries are immutable, by design - see "Layer 2" above), so a `DELETE FROM` can never empty it.
`EventLedger::reset` therefore uses `DROP TABLE`/re-migrate (DDL, which that trigger does not
intercept) in one transaction, ending at the same fully-migrated `PRAGMA user_version` it started
at, so the connection stays immediately usable with no re-open and no partial migration state.
`reset_local_state` calls the three layers' own `reset()` in sequence and is not itself atomic
across all three files - a failure partway leaves whichever layers already reset empty and the rest
unchanged, `ResetError` names which layer failed, and retrying is always safe because resetting an
already-empty layer is a no-op.

AC3 ("ephemeral inspect performs no persistent writes") is discharged by the three layers'
pre-existing `open_in_memory` constructors (E13-S01/S02/S03): a SQLite `:memory:` connection cannot
create a file, by construction, not merely by convention - this story adds the falsification test
proving that holds across many operations, rather than a new wrapper type with no caller yet to use
it.

No CLI/TUI/Guardian surface calls any of this yet - `cancellai-cli` and `cancellai-guardian` both
already depend on `cancellai-store` in their `Cargo.toml` (declared ahead of this story, unrelated
to it), but neither references it from source; orchestrating self-budget checks and wiring a real
`reset --local-state` flag is a later story's scope, matching `CurrentStateStore`'s and
`EventLedger`'s own state at their own `ready_for_review`.

## Quarantine store

Quarantine is logically separate from cancellAI metadata because it contains the user's original provider artifact. It is therefore governed by separate capacity and retention policy.

Rules:

- prefer same-volume atomic move;
- preserve enough identity/metadata for safe restore;
- never co-mingle quarantined payload contents into the metadata DB;
- surface quarantine footprint separately from "reclaimed from active provider" and "net free disk";
- quarantine expiry/purge remains a policy-controlled destructive event.

E12-S01 implements the move itself: an identity-confirmed, handle-relative rename (never a
copy) from the provider root to a second, explicitly-checked quarantine-store root
(`docs/architecture/PLATFORM_MODEL.md`'s boundary rules) - refusing rather than falling back to
a copy when the two roots are on different filesystems/volumes, or when the destination name
already exists. A contentless restore-metadata sidecar (original path, original identity, root
fingerprint - never artifact payload content) is written durably (`fsync`ed content and
directory entry) to a `.pending` name *before* the move is attempted, not after: the move itself
is then a plain rename, and finalizing the sidecar's name is a second, trivial rename with no new
content write. A failure before the move means nothing was attempted at all (E12-S01 round 1
independent verifier review found the write-after-move ordering an earlier version of this used
could leave a moved-but-unrecorded artifact on a synchronous write failure; round 2 found that
even round 1's own rollback repair could not survive a genuine crash in that same window, which
this write-before-move ordering closes rather than patches around).

**Automated recovery after a successful move but a failed or interrupted finalize is a disclosed,
deferred residual, not implemented.** Rounds 3, 4 and 5 of independent verifier review each found
a genuine correctness hazard in successive attempts at an automated recovery scanner for exactly
that window (round 3: a destination name existing was treated as proof a pending sidecar belonged
to it, letting a conflicting operation's leftovers overwrite a legitimate record; round 4: the fix
for that accepted an unrelated, older, already-completed operation's own finalized proof as if it
were a newer operation's; round 5: even a same-operation fix could finalize its proof before a
sibling sidecar's own finalize had succeeded, so a retry then discarded that sibling as an
orphan). Three real defects in the same mechanism was read as a signal, not bad luck, and the
owner's decision was to stop iterating on an automated scanner rather than ship a fourth attempt.
What still holds regardless: a sidecar whose finalize fails is left with its full, correct content
durably recorded under its own `.pending` name - never silently lost or silently wrong - so
completing it (renaming each `.pending` name to its real one) stays safe to do, today by a human
operator and, in a dedicated future story, by a properly and independently verified automated
tool. Unix only for now; Windows quarantine is a disclosed residual.

`rename_child_matching_unix_identity`'s move itself is a single, per-platform atomic no-replace
rename (`renameat2`/`RENAME_NOREPLACE` on Linux, `renameatx_np`/`RENAME_EXCL` on macOS), not a
separate destination-absence check followed by a plain rename - closing the window an attacker
(or, for `Restore`, the provider itself) could otherwise use to have their own concurrently
created object silently replaced (E12-S02 round-1 independent verifier review, SI-013).

## Archive

Archive is for artifacts the user wants to retain cheaply. Archive integrity must be verified before any source purge. Compression never changes risk class or authority ceiling by itself.

E12-S03 implements the move (the same identity-confirmed, no-clobber move E12-S01's quarantine
uses, into a second, cancellAI-controlled archive store) and an explicit format/version record,
but not real byte compression: that still needs its own dedicated, reviewed ADR
(`docs/adrs/0019-dependency-rings-per-crate.md`) if a future story picks it up. "Verifiable
archive integrity" is discharged by two signals captured at open time and compared again on
demand (`cancellai_platform::mutation::verify_archive_integrity`): the source's byte length, and
- since [ADR-0030](../adrs/0030-sha2-for-archive-integrity-in-cancellai-platform.md) - a real
SHA-256 digest. A truncation changes length; an equal-length in-place corruption - which a
length check alone accepted as intact, and which round 1's own non-cryptographic FNV-1a
fingerprint repair was itself judged insufficient to authorize a future purge against (E12-S03
round 1 and round 2 independent verifier review) - changes the digest instead. Archive shares
Quarantine's write-before-move/finalize/recovery protocol above for all three of its sidecars
(record, length, digest).

## Tombstones

After permanent purge, retain only an allowlisted metadata tombstone such as:

- opaque artifact ID;
- provider/category;
- size/reclaim observation;
- purge time;
- reason/policy ID;
- action result/evidence references.

No original path is required for long-term aggregate analytics unless an explicit privacy review approves it. Prompt/source/transcript content is prohibited.

## Ephemeral mode

Read-only inspection can run without persistent writes for CI, temporary hosts, troubleshooting, or privacy-sensitive use.

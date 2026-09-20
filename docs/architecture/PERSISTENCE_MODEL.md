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

E13-S05 implements incremental reuse (SI-024: "persistent cache is never destructive truth").
`CurrentStateStore::set_invalidation_key` attaches a small, primitive
`CacheInvalidationKey` (`identity_token`, `modified` mtime, an opaque `provider_fingerprint`,
an opaque `knowledge_version`, and a local `CacheCompleteness` echoing complete/partial/unknown)
to an already-`rebuild`-written row, stored as five nullable columns on the same
`agent_artifacts` row rather than a second table - a plain `DELETE FROM agent_artifacts` (every
`rebuild`, including `reset`'s empty one) already wipes these columns for every row, so a rebuilt
row can never inherit a stale invalidation key left over from a previous scan by construction,
not by a second cleanup step. `rebuild` itself is unchanged: its `INSERT` does not name the new
columns, so they default to `NULL` - "no invalidation key was ever attached to this row" -
which is also this mechanism's fail-safe default for a caller that crashes between `rebuild` and
populating keys, or that never calls `set_invalidation_key` at all.
`CurrentStateStore::cache_read_hint` compares a caller's *fresh* `CacheInvalidationKey` against
whatever is persisted and returns [`CacheReadHint`] - `ReuseForReading` or `Revalidate` -
deliberately not a `bool` and not named or shaped like an authorization. `ReuseForReading`
requires every axis to be **positively known and equal on both sides** - `identity_token` equal;
`modified` equal, so a backward-moved mtime is a change like any other, never treated as "no
change"; the same `provider_fingerprint` and `knowledge_version` - and **both** the persisted and
the fresh completeness to be `Complete`. `None` on either side of `modified`/`provider_fingerprint`/
`knowledge_version` is uncertainty about that axis, never a confirmed absence of change, so it
always forces `Revalidate` even when both sides are `None` (round 1 independent verifier review:
an earlier version of this comparison used plain `Option` equality, which let two unrelated rows
that both lacked, say, a provider fingerprint compare as "matching" on that axis - AC2's "or
completeness uncertainty invalidates the relevant cache scope" applies to every axis, not only
completeness itself). A row persisted under `Partial`/`Unknown` evidence is never
`ReuseForReading`, even against an identical fresh `Partial`/`Unknown` observation, so it can
never be treated as more reliable than it was when written. This crate does not depend on
`cancellai-inventory` for this: `CacheCompleteness` is a small, local echo of
`cancellai-inventory::completeness::ScopeCompleteness`'s complete/partial/unknown vocabulary at
the primitive level Layer 1's own dependency ring (`cancellai-model` only) admits, matching this
document's own framing of Layer 1 as a generic reconstructible cache/index rather than one
scanner's own output shape. `cancellai-store`'s `Cargo.toml` does not depend on
`cancellai-safety` at all, so nothing returned by this mechanism can reach the safety
executor's mutation-execution capability even by accident - a caller reading `ReuseForReading`
still must perform fresh, execution-time observation before any mutation decision;
`cancellai-store` supplies only this primitive, not the orchestration that decides when to
re-invoke a scan, which is a later story's scope (matching E13-S01 through E13-S04's own
"primitive delivered, no orchestrator yet" precedent).

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
`ledger_events` (row count must equal `to - from + 1`, computed with checked arithmetic so an
unrepresentable span - round 1 independent verifier review reproduced a panic at the `i64::MIN`/
`i64::MAX` extremes - is a rejection, never a panic), which is what stops a range that overlaps
an already-compacted window, or reaches past the newest appended event, from silently
summarizing fewer events than requested. On success it deletes exactly that range and writes one
`CompactionSummary` in its place, in the same transaction: the exact event count, a per-kind
breakdown (so an aggregate such as "how many `QUARANTINED` events happened in this window"
stays answerable once the raw rows are gone), and a SHA-256 digest over a **length-prefixed**
encoding of every summarized event - each field is hashed as its own byte length followed by its
bytes, not delimiter-joined, so a delimiter byte occurring inside one caller-supplied field can
never make two differently-shaped events hash identically (round 1 independent verifier review
reproduced exactly that collision against the prior delimiter-joined encoding, between a
`provider_id`/`category` pair that each carried one side of the same delimiter byte) - a summary
cannot be quietly re-attributed to a different
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
invoke a compaction primitive that already existed (`compact_range`/`compact`): a store already
exactly at its limit is left untouched, and a store one row over compacts exactly the excess,
never more, never a silent overrun. Calling either of these alone only *before* a write leaves a
gap round 1 independent verifier review reproduced: with a 5-event limit, six iterations of
(enforce, append) left six raw events, because a ledger already exactly at the limit makes the
next enforcement call a no-op, and nothing re-checks after the append that follows it lands
exactly on the limit. `append_within_ledger_budget`/`record_sample_within_rollup_budget` close
that gap as the actual AC1 ("budget overrun triggers compaction before growth continues")
primitive: each compacts *both* before and after its own write, in one call a caller cannot
split apart. `EventLedger::compact_oldest_to_fit` has no time-based eligibility gate - it removes
the oldest events unconditionally - so the "after" compaction always succeeds in bringing the
ledger back to the limit; `append_within_ledger_budget` therefore never refuses a write.
`AnalyticalMemory::compact_if_over` still only ages a sample out by `RetentionPolicy`'s own time
windows, so `record_sample_within_rollup_budget` cannot always free room this way: if every
currently-held raw sample is still too recent to promote, it refuses the write with
`SampleAdmissionError::BudgetExceeded` rather than silently exceeding the budget - "refuse ...
that write when safe compaction cannot meet the limit," the other half of round 1's required
repair. `enforce_ledger_budget`/`enforce_rollup_budget` themselves stay `pub` as the lower-level
primitive the admission calls compose, matching this module's own "owns *when*, never *how*"
doc; a policy/budget mismatch (a raw-sample tier whose windows cannot keep it under its row
limit at the caller's ingestion rate) now surfaces as an explicit refusal at the write that
would have exceeded it, rather than a silent overrun.

`check_current_state_budget` only observes `CurrentStateStore::row_count` against its
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

### cancellAI-owned local-state root (E13-S06)

E13-S04's own round-3 independent verifier review (`project/evidence/E13-VERIFIER-REVIEW-ROUND3.md`)
reproduced a gap in the AC2/SI-026 boundary above: `CurrentStateStore::open`, `EventLedger::open`
and `AnalyticalMemory::open` each took an arbitrary caller-supplied `Path`, and each accepted a
hand-crafted, provider-owned file that carried the exact schema, `user_version`, and a byte-for-byte
copy of the crate's own compiled-in identity marker - the marker is content inside a file the caller
supplies, so whoever supplies the file controls it, and a check against it can only ever refuse a
mimic that forgot the marker, never one that copied it. A per-install random secret was considered
and rejected for the identical structural reason (an attacker who fabricates both the secret and the
marker together controls both).

E13-S06 closes this not with a better check but by removing the caller-suppliable path entirely:
`cancellai_store::LocalStateRoot::resolve` is the crate's one public, reviewed way to establish
cancellAI's own local-state root (creating the directory if absent, canonicalizing it once), and
each layer's production `open()` now takes a `&LocalStateRoot` instead of a `Path`, deriving its own
database's location by joining a filename the crate alone fixes
(`current_state.sqlite3`/`event_ledger.sqlite3`/`analytical_memory.sqlite3`). A caller therefore
names *which directory* is cancellAI's own local-state root, but never *which file* within it a
layer opens - a provider-owned or marker-bearing mimic living anywhere else is unreachable from the
production entry point by construction, not because its content is inspected and rejected. Each
layer keeps a crate-private `open_at_path` alongside this for its own existing migration/marker/
reopen unit tests, unchanged. The compiled-in identity marker stays in place as a corruption/
migration sanity check on a file this crate already knows it owns; it is no longer the thing that
decides ownership.

**Disclosed residual:** `LocalStateRoot::resolve` canonicalizes the directory it is given once, but
does not re-verify that identity (device/inode) on every subsequent use the way
`cancellai-safety::ApprovedRoot::establish` does for provider roots. Reusing `ApprovedRoot` was
considered and rejected, because it would add a dependency from `cancellai-store` onto
`cancellai-safety`, contradicting this crate's own deliberate isolation stated above ("this crate
never touches a provider path", "nothing this mechanism returns can reach the safety executor's
mutation-execution capability even by accident"). A root directory replaced by a symlink between
resolution and a later `open()` is therefore not defended against by this story; the threat this
story closes is a mimicked file at a path a caller could otherwise have named, which is what round 3
actually reproduced.

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

E12-S04 implements the tombstone as a typed, narrower front door onto Layer 2's own
`EventKind::Purged` event rather than a fourth persisted layer: `cancellai_store::tombstone::
Tombstone` carries only opaque artifact ID and action result/evidence references (plan ID +
evidence IDs) - **deliberately narrower than the illustrative field list above.** Two independent
review rounds rejected wider field sets: round 1 found that restricting which *columns* exist does
not restrict what bytes a caller puts in them (every field round-tripped an arbitrary prompt,
source excerpt, or path unchanged); round 2 found that a syntactic content-safety validator over
those bytes does not close the gap either, because a short, ordinary phrase re-encoded with
hyphens (`do-not-purge-this`) is exactly as "identifier-shaped" as a real ID and is still
meaningful content - no further syntactic tightening fixes this, it is a structural limit. Rather
than a third review round patching the same kind of check, `provider_id`/`category`/`reason_code`/
`policy_id` were removed from `Tombstone` outright (the fields whose ordinary use is explanatory
prose, and not needed to identify what was purged), matching the response this document's own
`recover_pending_moves` history already gave a mechanism defeated three times in a row: narrow the
surface instead of re-attempting the same repair. `record_purge_tombstone` still validates
`artifact_id`/`plan_id`/`evidence_ids` against the same shape check (ASCII letters/digits joined
by up to four single hyphens, bounded per-segment and total length) before writing anything, but
this is now documented as a **disclosed residual, not a closed guarantee** (AC1, narrowed by
owner decision - [ADR-0033](../adrs/0033-purge-tombstone-content-safety-is-a-disclosed-residual.md)):
those three fields are still caller-supplied, and the shape check rejects an obvious
prompt/source/path without proving the absence of all short, ordinary phrases. A round-3
independent review additionally found that `EventLedger::append` being public let a caller
reconstruct the same content-smuggling channel directly, past `record_purge_tombstone`'s own
validation entirely; round 4 then found that closing only the content channel still left
`append` accepting a well-shaped `Purged` event with no `ActionClass`/`Reversibility` proof at
all (AC2/SI-020 - nothing distinguished a genuine irreversible deletion from a caller simply
choosing the label). Rather than teach the generic `append` a kind-specific pairing check it has
no columns to verify against, `EventLedger::append` now refuses `EventKind::Purged`
**unconditionally**: the only path to a written `Purged` row is the crate-private
`append_purged`, reachable exclusively from `record_purge_tombstone` after it has already checked
`ActionClass::Delete + Reversibility::Irreversible`, and which still applies the identical
content-shape check as defense in depth. Closing the remaining linkage-field residual needs a
real orchestrator that sources these values from already-trusted purge/evidence records instead
of an arbitrary public caller - that story's scope, not this one's (ADR-0033). `size/reclaim
observation` and
`provider/category`/`reason/policy`, the items this section's illustrative list names that the
current implementation does not carry, are disclosed residuals rather than a smaller schema:
widening an already schema-pinned table, or reintroducing those fields behind real closed
vocabularies or an orchestrator-verified source, is a larger, separately reviewable change this
story's acceptance criteria do not require. `AnalyticalMemory`'s `ReclaimableBytes` metric already
tracks reclaimable bytes as an aggregate time series (Layer 3, above).

`cancellai_store::tombstone::record_purge_tombstone` refuses - writing nothing - unless given
exactly `ActionClass::Delete` with `Reversibility::Irreversible`; every other combination
`cancellai-model`'s shared vocabulary admits is refused, including `Reversibility::
VendorConditional` (the vocabulary's own name for a vendor-native conditionally-reversible
outcome) paired with any action class (AC2). This restates, at the point the retained record is
written, the same coupling `cancellai_safety::authority::reversibility_allowed` already enforces
before the real OS call is attempted (SI-020) - as a strictly narrower predicate (it accepts only
the one combination that check's own `Delete` arm accepts), so the two can never disagree, and
never as a second, independent authorization decision: this function takes no `AuthorityLevel`
and decides nothing about whether a mutation may happen, only whether an already-decided outcome
is eligible to be labeled a purge tombstone. `cancellai-store` still does not depend on
`cancellai-safety` (E13-S06's own documented isolation) - the predicate is expressed locally
against the shared `cancellai-model` vocabulary. No orchestrator calls this from a real purge yet,
matching every prior E13 story's own "primitive delivered, no orchestrator yet" precedent; wiring
`mutation_executor::execute`'s `ActionClass::Delete` success path to a real tombstone write is a
later story's scope.

## Ephemeral mode

Read-only inspection can run without persistent writes for CI, temporary hosts, troubleshooting, or privacy-sensitive use.

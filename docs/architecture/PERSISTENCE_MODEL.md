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

## Layer 3: Analytical Memory

Guardian intelligence uses time-series rollups rather than permanent raw samples.

Indicative retention strategy:

- recent window: fine-grained samples;
- medium window: hourly rollups;
- long window: daily rollups;
- beyond long window: bounded statistics/tombstone aggregates.

Exact periods and budgets are product policy, not hard-coded architecture constants.

## Self-budget

cancellAI enforces explicit budgets for:

- current-state DB;
- event ledger;
- analytical memory;
- logs;
- temporary release/scan artifacts.

When approaching its budget, cancellAI compacts/rotates its own data before collecting more optional history. Safety-critical current facts may force analytical sampling to degrade rather than exceed the budget.

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

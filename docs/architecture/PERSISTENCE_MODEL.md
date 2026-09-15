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
fingerprint - never artifact payload content) is written atomically alongside the moved object.
Unix only for now; Windows quarantine is a disclosed residual.

## Archive

Archive is for artifacts the user wants to retain cheaply. Archive integrity must be verified before any source purge. Compression never changes risk class or authority ceiling by itself.

E12-S03 implements the move (the same identity-confirmed, no-clobber move E12-S01's quarantine
uses, into a second, cancellAI-controlled archive store) and an explicit format/version record,
but not real byte compression: that needs a kernel-ring dependency this workspace does not carry
yet (`docs/adrs/0019-dependency-rings-per-crate.md` requires a dedicated, reviewed ADR for one,
the same bar `libc`/`cancellai-sealedfs` cleared - not yet spent here). "Verifiable archive
integrity" is discharged for now by the cheapest real signal available without a new dependency:
the source's byte length, captured at open time and compared again on demand
(`cancellai_platform::mutation::verify_archive_integrity`) - a truncation or in-place corruption
changes length and is refused, though this is weaker than a cryptographic content hash and is
recorded as a disclosed residual, not overclaimed as more.

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

# Release Evidence - v1.15.0

## Source

- Tag: `v1.15.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-21
- Published: pending

## Included work

This release closes two epics whose changes accumulated together under `[Unreleased]` before
either was cut: E13 finished independent review while E12's own multi-round CR4 review was still
in progress, and both are cut into this one tag rather than an empty second release later.

- Epic: E12 - Quarantine, Archive, Restore, Purge
- Stories: E12-S01, E12-S02, E12-S03, E12-S04
- CR4 Safety Verdicts: `project/evidence/E12-S01/SAFETY_VERDICT.md`, `project/evidence/E12-S02/SAFETY_VERDICT.md`, `project/evidence/E12-S03/SAFETY_VERDICT.md`, `project/evidence/E12-S04/SAFETY_VERDICT.md`

- Epic: E13 - Local State, Event Ledger, Analytical Memory
- Stories: E13-S01, E13-S02, E13-S03, E13-S04, E13-S05, E13-S06
- CR4 Safety Verdicts: none

## Gates

Re-run at the tag by `.github/workflows/release.yml`; run locally before tagging:

```text
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py \
  scripts/check_docs.py scripts/check_workflows.py scripts/check_process.py scripts/release.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_process.py check
```

- G1 Functional: PASS
- G2 Safety: PASS
- G3 Compatibility: PASS
- G4 Operability: PASS

## Compatibility

- Platforms: macOS. Python 3.10 and 3.14 exercised in CI.
- Providers/capabilities: Codex CLI and Claude Code, layouts observed at release time.
  Unclassified entries are reported by `status --coverage` and never cleaned.
- State/schema migrations: none. The tool keeps no persistent state.

## Supply chain

- Checksums: the Homebrew formula records the SHA-256 of the tag archive, written by `scripts/release.py finalize`.
- SBOM: not produced at this stage. The shipped tool has no runtime dependencies; development tooling is pinned in `requirements-dev.txt`.
- Provenance/attestation: deferred to E17.
- Signature verification: deferred to E17.
- Release manifest: this file.

## Install smoke tests

- Homebrew: `brew audit --strict` and `brew style` run in CI on every change; `brew install`/`brew test` exercise the tagged archive.
- direct shell / PowerShell / Linux packages: not applicable at this stage.

## Performance

- Scan benchmarks: none formalised; deferred to E10.
- Self-budget: recorded scan errors are bounded, and root fingerprinting caps how much of an untrusted directory it will read.

## User-visible changes

### Added

- Defined the declarative policy document schema v1 (E11-S01, CR3 - raised from the declared
  CR2 by `project/risk_floors.json`'s domain-model floor,
  `docs/architecture/POLICY_MODEL.md` "Rust schema"): `cancellai-policy::schema::PolicyDocument`
  carries the global/machine/provider/project/artifact-type/session-pin scope hierarchy as
  versioned, `#[serde(deny_unknown_fields)]` data - the same pattern
  `cancellai-provider-api::manifest` and `cancellai-safety::knowledge_bundle` already use for
  their own documents. No field can express a shell command, script, or provider-native
  operation, and an absent scope deserializes as absent rather than a default authority. This
  story only parses and structurally validates a document; merging several scopes into one
  deterministic `EffectivePolicy` is E11-S02's constraint resolver.
- Implemented the policy constraint resolver (E11-S02, CR4, `docs/architecture/POLICY_MODEL.md`
  "Rust constraint resolver"): `cancellai-policy::resolver::resolve_effective_authority` walks
  the scope ladder most-specific-first (`ARTIFACT_TYPE -> PROJECT -> PROVIDER -> MACHINE ->
  GLOBAL`) to pick one requested authority, then folds it into
  `cancellai_safety::authority::AuthorityInputs::user_requested` and calls the already-verified
  `effective_authority`. The resolver computes no ceiling of its own - every existing safety
  constraint (artifact ceiling, confidence, lifecycle, provider trust, the constitutional safety
  floor) still binds the result exactly as it does for any other caller, discharging SI-025 by
  construction rather than by new logic. `SESSION`/pin is deliberately excluded from the ladder;
  pin/protect semantics are E11-S04's outcome.
- Added the policy explanation graph (E11-S03, CR3 - raised from the declared CR2 by the same
  crate-wide policy-resolution floor, `docs/PRODUCT.md` "policy engine" /
  `docs/architecture/POLICY_MODEL.md`): `cancellai-policy::explanation::explain_policy` reshapes
  the resolver's own output into one ordered, deterministic `PolicyExplanation` - every
  constraint `effective_authority` evaluated, in a fixed order, each flagged with whether it was
  among the one(s) that actually bound the final result, plus which policy scope supplied the
  original request. Invents no new authority logic; it is a read-only view over data E11-S02
  already produces.
- Implemented budgets, retention, and pinning (E11-S04, CR3, `docs/architecture/POLICY_MODEL.md`
  "Rust budgets, retention, and pinning"): `cancellai-policy::pinning::resolve_protection` maps
  an explicit session pin (`schema::PinEntry`) to `ProtectionState::Pinned`, feeding the same
  pre-existing lifecycle constraint every other protection fact already relies on - it never
  weakens an artifact already `Protected`. `cancellai-policy::budget` parses `retention`/
  `keep_latest`/`budget` scope fields (age-in-days text, a plain count, and byte-size text using
  `cancellai.py::format_bytes`'s own binary-1024 convention) via the same scope ladder the
  authority resolver uses, and `select_under_budget_pressure` chooses which artifacts to propose
  under storage pressure - restricted by construction to whatever `retention::build_actions`
  already marked eligible, so budget pressure can only narrow that set, never widen it.
- Re-added identity-confirmed quarantine as a real mutation (E12-S01, CR4,
  `docs/architecture/PERSISTENCE_MODEL.md` "Quarantine store",
  `docs/architecture/PLATFORM_MODEL.md` "Boundary rules"):
  `cancellai_platform::mutation::MutationOperation::Quarantine` mirrors `DeleteFile`'s
  identity-confirmed, handle-relative shape but ends in a cross-directory `renameat` (via a new
  `cancellai_sealedfs::rename_child_matching_unix_identity`) instead of an unlink, refusing
  rather than clobbering an existing destination name. `ApprovedRoot::prepare_destination` and
  `SealedPlan::seal_quarantine` add the destination a quarantine plan needs;
  `mutation_executor::execute` explicitly compares the source and destination roots' device
  identity before ever attempting a move (SI-018) - `renameat`'s own `EXDEV` is only the
  backstop. A contentless restore-metadata sidecar is written durably (fsynced content and
  directory entry) to a pending name *before* the move is attempted, then finalized by a second,
  trivial rename once the move succeeds - a failure before the move means nothing was attempted at
  all (E12-S01 round 1 and round 2 independent verifier review, SI-020). Automated recovery from a
  crash between a successful move and a failed/interrupted finalize is a disclosed, deferred
  residual: rounds 3-5 each found a genuine correctness hazard in successive attempts at an
  automated recovery scanner for that window, and the owner's decision was to stop iterating on
  one rather than ship a fourth attempt - the sidecar's content stays durably recorded under its
  own pending name regardless, safe for a human operator (or a future, independently verified
  tool) to complete. Unix-only for now; Windows quarantine refuses explicitly as a disclosed
  residual, matching `DeleteFile`'s own history before E20-S05.
- Added restore as the reverse of quarantine (E12-S02, CR4, `docs/security/THREAT_MODEL.md`
  "TM-14 Restore overwrites new provider state"): a new `ActionClass::Restore` sits at the same
  authority/reversibility floor as `Quarantine` - undoing a quarantine is no more dangerous than
  performing one. `cancellai_platform::mutation::MutationOperation::Restore` reuses the exact
  identity-confirmed, no-clobber move `Quarantine` already performs (both now share one
  `confirmed_move_inner`), just reversed in direction and without writing any sidecar - nothing
  of cancellAI's belongs at an artifact's original provider location.
  `SealedPlan::seal_restore`/`mutation_executor::execute` reuse the same explicit same-device
  boundary check (SI-018); a destination recreated after quarantine is refused
  (`SealError::DestinationAlreadyExists`), never silently overwritten, and a drifted artifact
  identity is refused by the same pre-existing `revalidate` (SI-013) every other action class
  already goes through. `ApprovedRoot::prepare_destination`'s return type is renamed
  `MoveDestination`, since it now serves both directions. The underlying move is a single,
  per-platform atomic no-replace rename (`renameat2`/`RENAME_NOREPLACE` on Linux,
  `renameatx_np`/`RENAME_EXCL` on macOS) rather than a separate destination-absence check
  followed by a plain rename, closing the window in which provider state created between those
  two syscalls could be silently replaced (E12-S02 round-1 independent verifier review, SI-013).
  Unix-only for now, matching `Quarantine`'s own residual.
- Wired archive as a real mutation, without byte compression (E12-S03, CR3,
  `docs/architecture/PERSISTENCE_MODEL.md` "Archive"): `ActionClass::Archive` (previously
  refused unconditionally) now builds a real
  `cancellai_platform::mutation::MutationOperation::Archive`, sharing the same
  identity-confirmed, no-clobber move `Quarantine`/`Restore` use. A real compressed archive
  format needs a kernel-ring dependency this workspace does not carry yet
  (`docs/adrs/0019-dependency-rings-per-crate.md` requires a dedicated, reviewed ADR for one) -
  deliberately out of scope here. What ships instead: an explicit format/version record
  (`SealedPlan::seal_archive`'s AC1), and `verify_archive_integrity`, which compares an archived
  artifact's current byte length *and* a SHA-256 digest against sidecars captured at archive
  time - length alone only catches truncation/extension; an equal-length in-place corruption
  verified successfully until round-1 repair added a content check (E12-S03 round-1 independent
  verifier review, SI-020), and round-1's own non-cryptographic FNV-1a fingerprint was itself
  judged insufficient in round 2 to authorize a future purge against, replaced in round 3 by a
  real digest under [ADR-0030](../../docs/adrs/0030-sha2-for-archive-integrity-in-cancellai-platform.md).
  Archive shares Quarantine's write-before-move/finalize/crash-recovery protocol for all three of
  its sidecars.
  `mutation_executor::execute` requires `Reversibility::Archivable` specifically
  for an archive plan (pre-existing gate, now actually reachable), which is what keeps
  "compression never changes semantic classification to disposable" true by construction.
  Unix-only for now, matching `Quarantine`/`Restore`'s own residual.
- Introduced the current-state SQLite store (E13-S01, CR2,
  `docs/architecture/PERSISTENCE_MODEL.md` "Layer 1: Current State"):
  `cancellai_store::CurrentStateStore` holds one row per `cancellai_model::AgentArtifact`,
  keyed by `ArtifactId`, using a bundled `rusqlite` (ADR-0019's outer-ring dependency, named
  for this story at planning time). `rebuild` replaces the table's entire content in one
  transaction with exactly the artifacts it is given, so the store's content after a rebuild
  depends only on what was just scanned, never on what it held before (C-10: reconstructible,
  never the source of truth). Schema migrations use SQLite's own `PRAGMA user_version`, each
  running inside its own transaction, so a migration failing partway rolls back everything it
  had already done rather than leaving a half-applied schema. This crate never touches a
  provider path, so deleting its database file - `reset --local-state`'s own contract - cannot
  delete a provider artifact by construction. `cancellai-model`'s `AgentArtifact` and every type
  it carries gained `Deserialize` (previously `Serialize`-only) so the store can read a row back
  out, matching the same wire format `docs/architecture/JSON_CONTRACTS.md` already defines.
- Introduced the append-only operational event ledger (E13-S02, CR2,
  `docs/architecture/PERSISTENCE_MODEL.md` "Layer 2: Operational Event Ledger"):
  `cancellai_store::ledger::EventLedger`, a second, independent bundled-SQLite database
  alongside `CurrentStateStore` in the same crate (ADR-0019 already names the store and the
  ledger as one story pair). `append` is the only way an event is ever written - there is no
  public update or delete - and immutability is enforced at the SQLite layer itself via triggers
  that reject any `UPDATE`/`DELETE` outside the one, gated transaction
  `compact_range` uses to replace a range of events with a signed/hashed `CompactionSummary`
  (SHA-256 digest, exact event count, per-kind breakdown), never silently. A mutation-class
  event (`PLAN_CREATED`, `ACTION_BLOCKED`, `QUARANTINED`, `RESTORED`, `ARCHIVED`, `PURGED`) is
  refused, writing nothing, unless it carries a non-empty `plan_id` and at least one
  `EvidenceId`. Every event's metadata is a closed, allowlisted set of fields
  (`artifact_id`/`provider_id`/`category`/`policy_id`/`reason_code`) - contentless by
  construction, per `docs/architecture/PERSISTENCE_MODEL.md`'s own constraint. `read_all`
  returns events in append order (SQLite's own never-reused `AUTOINCREMENT` id), independent of
  each event's caller-supplied `recorded_at`.
- Implemented analytical rollups and retention (E13-S03, CR2,
  `docs/architecture/PERSISTENCE_MODEL.md` "Layer 3: Analytical Memory"): a third, independent
  bundled-SQLite database, `cancellai_store::rollup::AnalyticalMemory`, alongside
  `CurrentStateStore` and `EventLedger` in the same crate. `record_sample` ingests fine-grained
  numeric measurements; `compact(policy, now)` is the one explicit primitive that ages them
  through the recent/medium/long windows `PERSISTENCE_MODEL.md` names - raw samples into hourly
  rollups, hourly into daily, daily into one bounded long-term aggregate per `(metric, scope)`
  beyond the long window - cascading through more than one window boundary in a single call
  rather than stranding data in an intermediate tier, and grouping every promotion by each
  sample's own recorded time (never insertion order), so a backdated or clock-skewed sample
  lands in its historically correct bucket. `RetentionPolicy` carries the three window lengths
  as explicit, caller-supplied durations (`DEFAULT`: one day / one week / ninety days) rather
  than hard-coded constants. `MetricKind` is a closed, exhaustive enum (`ArtifactCount`,
  `ProviderFootprintBytes`, `ReclaimableBytes`, `OrphanCount`) and `SampleScope`
  (`provider_id`/`category`) mirrors `EventMetadata`'s closed, allowlisted shape - an aggregate
  stays contentless by construction, never a path/transcript, because the types do not admit one.
- Enforced self-budgets and added local-state reset for the current-state store, event ledger and
  analytical memory (E13-S04, CR3, `docs/architecture/PERSISTENCE_MODEL.md` "Self-budget", SI-026
  "cancellAI reset/self-budget cannot target provider payload"): `cancellai_store::budget` is a
  thin policy layer over each layer's own compaction/reset primitives - `BudgetLimits` carries one
  explicit, caller-supplied threshold per layer (not a hard-coded constant), and
  `enforce_ledger_budget`/`enforce_rollup_budget` compact the oldest/aged-out data before growth
  continues once a layer is strictly over its limit, never at exactly the limit. `reset_local_state`
  sequences a new `reset()` on each of `CurrentStateStore`/`EventLedger`/`AnalyticalMemory`; every
  one takes no path and no caller-supplied target at all, so it cannot target a provider root by
  construction, not merely by convention. `EventLedger::reset` uses `DROP TABLE`/re-migrate rather
  than `DELETE FROM`, because `ledger_compactions`' immutability trigger refuses `DELETE`
  unconditionally by design - ending at the same fully-migrated `PRAGMA user_version` it started
  at. "Ephemeral inspect performs no persistent writes" is discharged by the three layers'
  pre-existing `open_in_memory` (`:memory:`) constructors, now with a falsification test proving
  zero files are created across many operations. No CLI/TUI/Guardian surface wires this yet -
  library-level primitives only, matching `CurrentStateStore`'s and `EventLedger`'s own state at
  their own `ready_for_review`.
- Implemented incremental inventory reuse (E13-S05, CR3, SI-024: "persistent cache is never
  destructive truth"): `CurrentStateStore::set_invalidation_key` attaches a small, primitive
  `CacheInvalidationKey` (identity, mtime, an opaque provider fingerprint, an opaque knowledge
  version, and a local complete/partial/unknown completeness) to an already-`rebuild`-written
  row, and `CurrentStateStore::cache_read_hint` compares a fresh key against it, returning
  `CacheReadHint::{ReuseForReading, Revalidate}` - deliberately not a `bool` and not shaped like
  an authorization. A row is `ReuseForReading` only when every axis matches exactly (a
  backward-moved mtime is a change like a forward one, never "no change") and both the persisted
  and the fresh completeness are `Complete`; a row persisted under `Partial`/`Unknown` evidence is
  never reusable, even against an identical fresh `Partial`/`Unknown` observation. `rebuild`
  itself is unchanged - the new columns default to `NULL` ("no key ever attached"), and every
  `rebuild` (including `reset`'s empty one) wipes them for every row, so a rebuilt row can never
  inherit a stale key. `cancellai-store` still does not depend on `cancellai-safety`, so nothing
  this mechanism returns can reach the safety executor's mutation-execution capability; fresh,
  execution-time observation remains mandatory before any mutation decision.
- Added remote target vocabulary (E18-S01, CR3 - raised from the declared CR2 by
  `project/risk_floors.json`'s domain-model floor, `docs/architecture/TARGET.md` "Remote target
  vocabulary"): `cancellai_model::remote_target` models an SSH/dev-container/CI-runner machine as
  an explicit `RemoteTarget` with its own `MachineId`, `RemoteCapabilities`, `RemoteTargetTrust`
  and `RemoteTargetConnection` - the first real occupant of `MachineId`, standalone and not yet
  wired onto `AgentArtifact`. `InventoryOrigin`'s two variants make "remote inventory never
  masquerades as local state" a shape guarantee (`Local` carries no `MachineId`, so no remote id
  can ever compare equal to it), and `state_is_current` treats anything but `Connected` as stale,
  never a time-window guess. `RemoteTargetTrust`/`RemoteCapabilities` deliberately derive
  `Serialize` only, not `Deserialize`, after this story's adversarial-cases pass found the
  alternative repeats E05 round 1's exact defect (a bare, deserializable trust value reaching an
  authority computation with no promotion gate) - a real execution grant is E18-S02's job.
- Added the local-agent remote execution boundary (E18-S02, CR4, SI-031, RFC-0001, ADR-0031,
  ADR-0032): `cancellai_safety::remote_execution` verifies a signed `RemoteExecutionRequest` from
  a remote controller (a distinct actor from E18-S01's `RemoteTarget`) and produces at most a
  `VerifiedRemoteIntent` carrying one `target`/`requested_action` pair - a semantic `ActionClass`,
  never an `AuthorityLevel` directly (ADR-0032, correcting an earlier version of this change that
  shipped the `AuthorityLevel`-over-the-wire shape RFC-0001 explicitly rejected). A caller feeds
  `minimum_authority_for(requested_action)` into `AuthorityInputs::user_requested` - the only
  value it supplies. Every other authority input and `effective_authority` itself are unchanged;
  a dedicated test proves a remote-originated request reaches an identical result to the same
  value supplied locally, and another proves `ActionClass`'s range can never resolve to
  `Recommend`/`Autopilot`, discharging AC1 ("remote control cannot bypass target safety
  invariants") by construction rather than convention. The envelope mirrors
  `knowledge_bundle::KnowledgeBundle` exactly (schema version, per-controller strictly-increasing
  sequence, expiry, SHA-256 digest, Ed25519 signature - no new dependency), verified against a new
  `TrustedRemoteControllers` policy whose ceiling bounds which `ActionClass`es a controller may
  request (via `minimum_authority_for`) - refused outright above it, never clamped. Verification
  also binds every check to the caller's own expected target, and the underlying stateless
  verifier is not `pub` - only `RemoteExecutionLog::verify_and_record` can ever produce a
  `VerifiedRemoteIntent`, closing a replay bypass and a target-confusion gap Codex's independent
  review found. `RemoteExecutionLog` tracks accepted sequences per controller as pure in-memory
  state (mirrors `knowledge_bundle::KnowledgeStore`, no `rusqlite`, keeping the kernel-ring bare
  per ADR-0019); it does not survive a process restart, and durable persistence across restart
  remains an open residual pending a future outer-ring caller. Audit-linking is discharged by the
  verification `Result` itself carrying every field an `EventLedger` entry needs; writing one is
  deferred to whichever outer-ring caller eventually wires a real transport (E18-S03 or later) -
  library-level primitives only, no CLI/TUI/Guardian surface wires this yet.
- Added purge tombstones (E12-S04, CR4, SI-020, `docs/architecture/PERSISTENCE_MODEL.md`
  "Tombstones"): `cancellai_store::tombstone::record_purge_tombstone` is a typed, narrower front
  door onto the event ledger's existing `EventKind::Purged` capability (E13-S02), adding no new
  schema, file or connection of its own. `Tombstone` carries only opaque artifact ID and
  plan/evidence references - two independent review rounds rejected a wider field set
  (`provider_id`/`category`/`reason_code`/`policy_id`), first for accepting arbitrary caller
  content unchecked, then for a syntactic content-safety check that still could not distinguish a
  real ID from a short ordinary phrase; those fields were removed outright rather than validated a
  third way. The three that remain are still validated as a short, hyphen-joined ASCII identifier
  before anything is written, refusing with nothing persisted otherwise. AC1 is narrowed by owner
  decision to state exactly this testable property rather than an unqualified guarantee no local
  primitive can prove - a disclosed residual, not a closed guarantee, full closure deferred to a
  future orchestrator that sources these values from already-trusted records instead of an
  arbitrary caller
  ([ADR-0033](../../docs/adrs/0033-purge-tombstone-content-safety-is-a-disclosed-residual.md)). A third
  review round then found `EventLedger::append` being public let a caller construct a `Purged`
  event directly with no `ActionClass`/`Reversibility` proof at all (AC2/SI-020); `append` now
  refuses `EventKind::Purged` unconditionally, and the crate-private `append_purged` - reachable
  only from `record_purge_tombstone` after it has checked `Delete + Irreversible` - is the sole
  path to a written `Purged` row. `record_purge_tombstone` refuses, writing nothing, unless given
  exactly `ActionClass::Delete`
  with `Reversibility::Irreversible` - every other combination `cancellai-model`'s vocabulary
  admits is rejected, including `Reversibility::VendorConditional` paired with any action class
  (AC2), a strictly narrower restatement of the same coupling
  `cancellai_safety::authority::reversibility_allowed` already enforces before the real OS call
  (SI-020), never a second, independent authorization decision - this function takes no
  `AuthorityLevel` and `cancellai-store` still does not depend on `cancellai-safety`. No
  orchestrator calls this from a real purge yet, matching E13's own "primitive delivered, no
  orchestrator yet" precedent.
- Added the Guardian pressure state model (E14-S01, CR2, SI-027,
  `docs/architecture/GUARDIAN_MODEL.md` "Pressure states"): `cancellai_guardian::pressure` is a
  pure, dependency-free `classify(inputs, previous) -> PressureState` over the five named signals
  (free space, self-budget usage, growth velocity, reclaimability, active workload), deterministic
  in both arguments and independently unit-tested with no live scanner. Per-boundary up/down
  thresholds give GREEN/YELLOW/ORANGE/RED hysteresis that holds even across a multi-level jump in
  one observation; a NaN or out-of-range axis resolves to its worst reading, never its safest.
  Imports no `AuthorityLevel`/`ActionClass`/`Reversibility` type, so AC2 ("pressure does not change
  authority by itself") holds by construction. No caller wires this to a live store yet.
- Added Guardian growth velocity and time-to-pressure forecasting (E14-S02, CR1,
  `docs/architecture/GUARDIAN_MODEL.md` "Forecasting"): `cancellai_guardian::forecast` fits an
  ordinary-least-squares trend over a caller-supplied time series, with at most one outlier
  trimmed by a median-absolute-deviation check so a single burst cannot manufacture or hide a
  trend. Every non-insufficient answer carries an explicit, ordered `Confidence`
  (`Low`/`Medium`/`High`) alongside its number (AC1); too few points, too short a span, or too
  poor a fit refuses a fit outright rather than returning a low-confidence guess, distinguishing
  "too little history" from "too noisy to trust" via `InsufficientDataReason` (AC2). A second,
  independent fit over the most recent half of the series must retain at least half the
  whole-series slope, or the fit is refused the same way - round 1 independent review found a
  burst followed by a sustained plateau otherwise fits a deceptively good whole-series line and
  reports the historical average as a false *current* growth rate. A declining or flat series
  floors growth velocity at zero and never yields a spurious exhaustion forecast. No caller wires
  this to a live store yet.
- Added Guardian behavioral baseline anomaly detection (E14-S03, CR2, SI-027,
  `docs/architecture/GUARDIAN_MODEL.md` "Baselines"): `cancellai_guardian::baseline::Baseline`
  holds a robust local model (median/median-absolute-deviation, not mean/standard-deviation) over
  a bounded window of numeric metadata observations, evicting the oldest reading before admitting
  a new one past its configured capacity so its memory never grows with the number of observations
  ever seen (AC1). `Baseline::assess` returns an `AnomalyAssessment` carrying the observed value,
  the baseline's own median and MAD, and the deviation in MAD units alongside an ordered
  `AnomalySeverity` (`Normal`/`Elevated`/`Anomalous`) - never a bare score (AC2) - and, like
  `pressure`/`forecast`, imports no authority/mutation type. Fewer than three observations refuses
  to assess rather than reading "no baseline yet" as normal; a perfectly flat baseline floors its
  MAD at a fraction of its own magnitude so it tolerates proportional noise instead of flagging
  every future wobble regardless of scale; NaN/infinite input is never admitted and always
  assesses as maximally anomalous. No caller wires this to a live store yet.
- Added Guardian structural anomaly detection (E14-S04, CR4 - raised from the declared CR2 by
  `project/risk_floors.json`'s safety-kernel floor once the fix below landed inside
  `rust/crates/cancellai-safety/src/authority.rs`, SI-004, `docs/architecture/GUARDIAN_MODEL.md`
  "Detection"): `cancellai_guardian::structural` names session-count explosion, giant-artifact,
  and orphan-growth detection as thin wrappers over `baseline::Baseline::assess`, and adds
  `assess_layout` for provider layout drift - a discrete comparison of an opaque
  `LayoutSignature` against a closed set of recognized signatures, returning
  `LayoutSupport::Recognized`/`Drifted` (`cancellai-model`'s existing vocabulary, no new crate
  dependency, and - after ADR-0035 - no authority ceiling at all: see below). A `provider_id` is
  threaded only into evidence text and never read by the comparison, so two calls differing only
  in provider name reach the identical verdict (SI-004: a recognized name cannot rescue a drifted
  layout). `LayoutDriftFinding`'s fields are private with `assess_layout` as its only production
  constructor (a `compile_fail` doctest proves external construction is impossible).
  `cancellai_safety::authority::AuthorityInputs::provider_layout` - a **mandatory** field
  (ADR-0034, ADR-0035) - carries the raw `known_signatures`/`observed` facts themselves, never a
  pre-computed ceiling; `base_constraints` derives whatever constraint they imply itself, the
  identical comparison `assess_layout` performs. `cancellai_guardian::capability_authority::
  authority_inputs_with_layout_observation` converts this crate's own `LayoutSignature` into
  `cancellai-safety`'s structurally identical, separate type of the same name and sets the field;
  it computes no ceiling either. Three independent review rounds found three successive ways an
  earlier shape failed to actually, unavoidably reduce authority: round 1, an unconsumed
  recommendation; round 2, a skippable opt-in `effective_authority_for_provider_capability`
  function (since deleted); round 3 (against ADR-0034's mandatory `Option<AuthorityLevel>`
  ceiling field), a caller-asserted conclusion disconnected from the facts it claimed to
  summarize, discardable while the real finding was still held. ADR-0035 removes the ceiling as
  an independent value entirely. An end-to-end test proves `assess_layout`'s own detection output
  and feeding the identical raw signatures into a real effective-authority computation agree: a
  destructive-capable input still ends at `Observe` under real drift. `structural.rs` itself
  still holds no reference to `cancellai-safety`, more literally than before (it can no longer
  produce an authority-typed value at all); ADR-0035 discloses, narrower than ADR-0034 but not
  closed, that no current caller constructs a real observation at all, as a residual for a future
  orchestrator story to close.

### Fixed

- Replaced the fixed `story-status-forged` gate-sensitivity anchor (E25-S16, CR2) with a dynamic
  mutation in the throwaway export. The E11-S01 anchor went stale when its supposedly future phase
  closed; the interim E19-S02 anchor had nine unmet dependencies, so it exercised dependency
  rejection rather than the required-evidence gate. The new mutation clears a selected planned
  story's dependencies before forging `done`, verifies the evidence-specific rejection, and fails
  loudly if no target exists.

### Security

- **`ApprovedRoot::prepare_destination` rejected `/`-separated names on Windows only through
  `MAIN_SEPARATOR`** (E12-S01, CR3), which is `\` there - a quarantine destination name like
  `"nested/name"` passed the bare-filename check and was silently joined into a nested path
  instead of being refused with `InvalidDestinationName`. Both `PathBuf::join` and the Windows
  filesystem APIs treat `/` as a separator regardless of `MAIN_SEPARATOR`, so the check now tests
  every character with `std::path::is_separator`, which is separator-complete per platform.
  Caught by the existing `prepare_destination_refuses_names_that_are_not_bare_filenames` test
  once it actually ran on `windows-latest` CI; the assertion was correct, the guarded code was
  not.
- **`CurrentStateStore`/`EventLedger`/`AnalyticalMemory::open` accepted a reset-capable handle
  over any file that mimicked this crate's own schema and compiled-in identity marker**
  (E13-S06, CR3, SI-026, closing E13-S04's round-3 independent verifier finding,
  `project/evidence/E13-VERIFIER-REVIEW-ROUND3.md`). The marker was content inside a
  caller-supplied file, so a hand-crafted provider-owned database that copied it - reproduced
  independently against all three layers - passed the check and had its row deleted by
  `reset()`. Each layer's production `open()` now takes a `&LocalStateRoot` instead of a `Path`,
  deriving its database's location by joining a filename the crate alone fixes; a mimicked file
  at any other location, marker included, is unreachable from the production entry point by
  construction, not because its content is inspected and rejected; the compiled-in marker
  remains only a corruption/migration sanity check. Existing migration/marker/reopen unit tests
  keep exercising a crate-private `open_at_path` directly, unchanged. A first attempt at
  `LocalStateRoot` itself still took an arbitrary caller-supplied directory as its only public
  constructor - round 4 independent review reproduced the identical provider-data-erasure
  outcome one call earlier, plus a fixed-filename symlink redirecting production `open()`
  outside an otherwise-legitimate root. `LocalStateRoot::resolve_platform_default()` (no path
  argument; computes `$CANCELLAI_HOME/state` or `$HOME/.cancellai/state` itself) is now the only
  public constructor, and `path_for` refuses a symlinked leaf.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

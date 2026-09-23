# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The Rust safety kernel can contain a safety incident by signed capability downgrade
  (`cancellai-safety::incident`, E17-S07). A containment arrives as the payload of a signed
  knowledge bundle, can cap a provider - optionally narrowed to versions, action classes and
  platforms - at Observe or Recommend and at nothing higher, and is recorded in a ledger that a
  replayed bundle, a knowledge-store rollback or a bundle's expiry cannot shrink; only a local
  lift removes one. An unreachable knowledge service leaves the installed kernel in charge.
  Each containment produces an incident evidence record of identifiers and provenance only. Not
  yet wired into the beta Rust CLI; that is cutover work (E06-S04).

### Fixed

- `scripts/check_provider_trust.py` accepted a provider trust promotion whose
  `fixture_references` named a fixture that did not exist: it checked that evidence was listed,
  not that it was there. A promotion above `Untrusted` now fails unless every fixture reference
  is a repository-relative path that exists, stays inside the repository after symbolic links
  are resolved, and is tracked by Git; when Git cannot answer, the check refuses (E16-S08).

## [1.17.4] - 2026-09-22

### Fixed

- The Rust engine's `cancellai-guardian` crate failed its real Windows smoke test on the v1.17.3
  tag: `status()` right after a successful `schtasks /Change ... /ENABLE` still failed to parse
  the task as enabled (`Unsupported { reason: "schtasks /XML output did not contain a
  recognizable Enabled field" }`), even though the very same `/Query /XML` parsed correctly
  right after `install` in the same run. That asymmetry - the only difference being a
  state-changing `/Change` call that had *just* run - points at `schtasks`' own task-cache
  lagging behind its own write, not at the XML shape two prior fixes already targeted (a UTF-16
  BOM decode, then a clippy-driven `as_chunks` change). `enable()` now polls `status()` for up to
  2 seconds to confirm the change before returning `Ok`, matching the macOS/Linux adapters' own
  enable contracts (AC1); the real smoke test accepts the resulting honest,
  distinguishable "registration could not be confirmed" outcome the same way the macOS one
  already does, instead of asserting a state `status` cannot yet corroborate.
  `parse_enabled_from_xml` was also made whitespace/attribute-tolerant as a defensive
  improvement, and `status()`'s `Unsupported` reason now includes a bounded snippet of the
  actual `/XML` output so any further occurrence is diagnosable from the failure itself.
  v1.17.3's tag stands as immutable history and is recorded as unpublished; this fix ships as the
  next version.

## [1.17.3] - 2026-09-22

### Fixed

- The Rust engine's `cancellai-guardian` crate failed real CI on the v1.17.2 tag: `cargo clippy`
  denied `chunks_exact` with a constant chunk size in `service.rs`'s `decode_command_output`
  (added by the v1.17.1 fix), a lint real CI's newer clippy (0.1.98) enforces. Switched to
  `slice::as_chunks::<2>()`, the idiom clippy itself suggests. v1.17.2's tag stands as immutable
  history and is recorded as unpublished; this fix ships as the next version.

## [1.17.2] - 2026-09-22

### Fixed

- The Rust engine's `cancellai-guardian` crate failed real cross-platform CI on the v1.17.1 tag
  in three independent ways, none reproducible locally: `KillSwitch::is_engaged` (Windows)
  trusted a bare `NotFound` read error as a confirmed-absent marker, but a path through a
  non-directory component also reports `NotFound` there, misreading a failed `engage()` as
  disengaged - it now also requires the marker's own parent to be confirmed a real directory
  before trusting `NotFound` at all. `schtasks /Query ... /XML` output (Windows) does not decode
  as UTF-8 - its own XML declares `encoding="UTF-16"` and `schtasks.exe` writes genuine UTF-16LE
  bytes with a byte-order mark, which `String::from_utf8_lossy` silently mangled into unmatched
  replacement characters - the shared command-output decoder now detects a UTF-16 BOM and
  decodes accordingly. `systemctl --user enable --now` (Linux) can fail with "Unit file ... does
  not exist" immediately after `install()` writes it, even with a reachable session bus and an
  explicit `daemon-reload` - `enable()` now retries up to three times, reloading fresh before
  each attempt, before reporting the failure honestly. v1.17.1's tag stands as immutable history
  and is recorded as unpublished; this fix ships as the next version.

## [1.17.1] - 2026-09-22

### Fixed

- The Rust engine's `cancellai-guardian` crate failed real Windows CI on the v1.17.0 tag: `cargo
  clippy --workspace --all-targets --all-features -- -D warnings` denied an unused `use
  std::path::PathBuf` in `service_windows.rs`, used only by that module's own `#[cfg(test)]`
  fixtures - on a real Windows build the module is included via `target_os = "windows"` alone
  (`lib.rs`'s `cfg(any(test, target_os = "windows"))`), so the plain library target compiles it
  with those fixtures absent and the import genuinely unused there, a platform-specific lint gap
  this workspace's own tooling documentation already names and no local host could reproduce
  without a Windows cross-compiler. Gated the import to `#[cfg(test)]` to match its real usage.
  v1.17.0's tag stands as immutable history and is recorded as unpublished; this fix ships as the
  next version.

## [1.17.0] - 2026-09-22

### Added

- Added the Guardian cross-platform user-service runtime (E15-S01, CR3,
  `docs/architecture/GUARDIAN_MODEL.md` "Runtime"): `cancellai_guardian::service` gives one
  engine (`GuardianService`) a consistent `install`/`uninstall`/`enable`/`disable`/`status`
  lifecycle over three real OS mechanisms selected at compile time - a `launchd` user agent on
  macOS, a `systemd --user` unit on Linux (falling back to an explicit `Unsupported` status,
  never a silent guess, when no session bus is reachable), and a `schtasks.exe` user-scoped
  scheduled task on Windows (parsed from its non-localized `/XML` output, not its
  display-language-dependent text formats). Each adapter shells real commands through
  `std::process::Command` behind a `CommandRunner` seam, never a shell string, so orchestration
  and status parsing are unit-tested with a fake runner on every host while a real smoke test
  additionally exercises the genuine mechanism on its own matching CI platform - the macOS one
  ran for real against this machine's own `launchctl` as part of this story's own verification.
  `cancellai-cli` does not depend on this crate, so a Guardian lifecycle failure cannot reach
  manual CLI operation (AC2). `run` (what an installed definition actually invokes) remains the
  E02-S01 skeleton; the detection/decision/authority loop is E15-S03/S04's scope.
- Added the Guardian notification abstraction (E15-S02, CR1,
  `docs/architecture/GUARDIAN_MODEL.md`): `cancellai_guardian::notification` delivers
  OS-appropriate notifications (`osascript` on macOS, `notify-send` on Linux, `msg.exe` on
  Windows) with a terminal fallback on any failure. AC1 ("notifications never include sensitive
  transcript/source content") holds by construction: `NotificationKind` carries no `String`/
  `PathBuf` field at all - every variant's payload is a closed enum (`PressureState`,
  `AnomalySeverity`) or a plain count, and `render` is a fixed template per variant, so there is
  no field or path through which caller-supplied text could reach a notification. AC2
  ("notification unavailability does not trigger stronger remediation") holds structurally too:
  `notify` returns a two-value `NotificationOutcome` (`Delivered`/`Fallback`) with no error
  variant, and the module imports no `AuthorityLevel`/`ActionClass` type, matching `pressure`'s
  own "cannot express an authority decision" argument. Not yet wired to a live orchestrator,
  matching this crate's own detection modules' precedent.
- Added the Guardian bounded remediation planner (E15-S03, CR4, SI-027, SI-028,
  `docs/architecture/GUARDIAN_MODEL.md` "Decision"/"Authority"): the currently crate-internal
  `cancellai_guardian::remediation::plan_remediation` never computes its own Effective Authority -
  each candidate's `reachable_authority` is `cancellai_policy::retention::ClassifiedArtifact`'s
  own field, the exact ceiling `cancellai_safety::authority::effective_authority` already derived
  through the same pipeline `cancellai-cli` uses, narrowed further only via a plain
  `std::cmp::min` against Guardian's own pressure-derived intent ceiling. AC1 ("Guardian action
  authority exactly equals or is below Effective Policy") and SI-028 hold by construction: `min`
  over two `AuthorityLevel` values cannot exceed either input, so there is no second,
  independently-derived comparison this module could get wrong. AC2 ("RED pressure cannot bypass
  artifact ceiling or unknown-state protections") holds the same way:
  `pressure_authority_ceiling` never returns above `AuthorityLevel::Quarantine` for any pressure
  state, `Red` included - `Delete` needs `AuthorityLevel::Govern`, a ceiling this function
  structurally cannot produce, so this planner can never emit a `Delete`-eligible plan regardless
  of pressure, and an artifact already capped by an unknown/protected lifecycle state stays
  capped under `min` no matter how high pressure climbs. Verified by an exhaustive 4 (pressure) x
  5 (`AuthorityLevel`) matrix. The planner and its authority-bearing conversion remain
  crate-internal until policy supplies an opaque, non-forgeable authority carrier; external
  callers therefore cannot mint a Guardian grant from a fabricated classification. It is not yet
  wired to sealing/execution - E15-S04's scope.
- Added the Guardian kill-switch and audit trail (E15-S04, CR3,
  `docs/architecture/GUARDIAN_MODEL.md` "Kill switch"/"Audit"): `cancellai_guardian::killswitch`
  is an immediate, local disable path - a marker file whose `engage`/`disengage` both *write* its
  content (`"engaged"`/`"disengaged"`) rather than creating/removing it, so it never calls
  `std::fs::remove_file` (`scripts/check_mutation_boundary.py`/SI-019 refuses that call outside
  `cancellai-platform/src/mutation.rs`, with no exemption for cancellAI's own local state).
  `is_engaged` is fail-safe: only a confirmed-absent marker, or one confirmed to read exactly
  `"disengaged"`, counts as disengaged - any other outcome, including an I/O error, reads as
  engaged. `apply_kill_switch` (AC1) is a pure, in-memory transform over an already-planned batch
  that forces every item to `Observe` when engaged, without ever touching execution - an in-flight
  mutation still runs entirely inside `cancellai_safety::mutation_executor`'s own transactional
  sequence, unaffected by anything here. `cancellai_guardian::audit::record_guardian_decision`
  (AC2) reuses `cancellai_store::EventLedger` (E13-S02) rather than a second ledger: a plan item
  that reached `Quarantine` is recorded as `EventKind::PlanCreated`, one that stayed at `Observe`
  as `EventKind::ActionBlocked` - both mutation-class kinds the ledger itself already refuses to
  accept without a plan reference and at least one piece of detection evidence, so this function
  cannot bypass that enforcement even if it tried.

## [1.16.0] - 2026-09-22

### Security

- The Rust engine's mutation boundary (`cancellai_safety::mutation_executor::execute`) now
  requires a live, freshly-observed provider-root layout immediately before every destructive
  mutation, refusing if it has drifted (or become unobservable) since the plan was sealed
  (SI-004, SI-013, ADR-0037). This closes a disclosed residual from the E14-S04/ADR-0036
  provider-layout work: a plan's own layout data was never re-checked at the one moment that
  matters. Independent review round 2 found and repaired three defects in the first version
  before it shipped: `execute`/`execute_all` were reachable from outside the crate with a
  fabricated observation (now `pub(crate)`, production-only via `execute_with_system_
  capabilities`); the fresh observation was not held through the mutation call itself (the
  check now runs immediately before it, narrowing but not fully closing that window - a
  disclosed residual); and `Restore` plans checked the quarantine store instead of the real
  destination provider root (now bound explicitly per action class). **Disclosed consequence:**
  the underlying observation capability is currently Unix-only, so every destructive mutation -
  Delete included - is now refused on non-Unix platforms, Windows included, until a verified
  Windows implementation lands.

### Fixed

- The Rust engine's `clean`/`plan`/`inspect` treated a Claude `~/.claude/projects` that exists
  but is not a directory (a regular file, a device node, etc.) as a structurally empty,
  known-clean scope - the same branch used for an absent or symlinked root - and reported a
  clean empty scan (`clean --yes` exited `0`) instead of withholding. The frozen Python
  reference records the resulting `ENOTDIR` and exits `4`. Found by independent review
  (`project/evidence/E21-S03-S07-INDEPENDENT-REVIEW-ROUND2.md`); repaired so this case is
  reported as unobservable evidence and withholds destructive work like every other unreadable
  scope root.

## [1.15.2] - 2026-09-21

### Fixed

- v1.15.1's tagged release workflow failed too, once the Windows tempdir-cleanup fix cleared
  `verify-rust`: `tests/test_release.py::ReleaseConsistencyTests::
  test_the_formula_never_lags_by_more_than_the_in_flight_window` hardcoded the legitimate
  in-flight formula target as `{cut[0], cut[1]}` - the two most recent changelog entries - which
  does not hold once a failed, unpublished release (v1.15.0) sits between two valid ones.
  `release.py`'s own `formula_should_point_at` already correctly skips past an unpublished
  version; the test now delegates to it directly instead of re-deriving a narrower version of the
  same rule by hand.

## [1.15.1] - 2026-09-21

### Fixed

- v1.15.0's tagged release workflow failed `verify-rust` on `windows-latest`: `cargo test
  --workspace` panicked in three `open_via_local_state_root_never_reaches_a_marker_bearing_
  mimic_elsewhere_on_disk` tests (`cancellai-store`'s `lib.rs`, `ledger.rs`, `rollup.rs`) during
  `std::fs::remove_dir_all`, with OS error 32 ("the process cannot access the file because it is
  being used by another process"). Each test dropped its verification `Connection` before
  cleanup, matching this module's own established Windows precedent, but never dropped the
  `CurrentStateStore`/`EventLedger`/`AnalyticalMemory` handle it opened and left alive on the same
  file - only macOS/Linux let an open file be unlinked out from under a live handle, so this never
  failed locally or on the other two CI platforms.

## [1.15.0] - 2026-09-21

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
  real digest under [ADR-0030](docs/adrs/0030-sha2-for-archive-integrity-in-cancellai-platform.md).
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
  ([ADR-0033](docs/adrs/0033-purge-tombstone-content-safety-is-a-disclosed-residual.md)). A third
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

## [1.14.0] - 2026-09-14

### Changed

- **Three `unsafe` blocks deleted rather than documented, and Miri run for the first time**
  (E27-S06, CR4). The independent review of E27-S01 found two of the executor's `SAFETY` arguments
  false - `FILE_STANDARD_INFO` was described as having no validity invariant, and in `windows-sys`
  0.61 its `DeletePending`/`Directory` fields are Rust `bool`, which has one. The conclusion held;
  the reason did not, and a reason is what the next reader extends. Underneath sat the better
  answer: `windows-sys` derives `Default` on that struct, so **41 `unsafe` blocks became 38** and a
  soundness argument that depended on a dependency's field types - and would have changed silently
  under `cargo update` - went with them.
  - **Miri: 197 tests across four crates execute with no undefined behaviour**, including the
    parsing code that reads third-party provider manifests. It runs weekly, not per-PR:
    `cancellai-tui` alone takes 333 seconds under the interpreter.
  - **And it helps least exactly where the risk is highest.** Miri cannot call foreign functions it
    has no shim for, so it cannot execute a single one of the 38 remaining `unsafe` blocks -
    `cancellai-sealedfs` and `cancellai-platform` stop at `statfs`, `cancellai-policy` at
    `fsetattrlist`, and `cancellai-safety` inside `sha2`'s aarch64 SHA-512 intrinsics. Recorded as
    a measured reach rather than left as "Miri never ran".

### Security

- **Recursive deletion now refuses where the platform cannot resist a symlink race** (E27-S04,
  CR3). The shipped Python reference deletes directories with `shutil.rmtree`, and every
  containment check before that call is path-based - a path re-checked immediately before use
  cannot close a race, because once the walk starts a subdirectory can be swapped for a symlink
  pointing anywhere. Python closes that only where the platform supports fd-relative removal and
  reports it as `shutil.rmtree.avoids_symlink_attacks`; **nothing here had ever read that flag**.
  On macOS it is `True`, so the shipped tool was protected by a property of the platform rather
  than by a decision. It is now a stated precondition: where the platform cannot remove safely,
  `clean` refuses the directory and says why, rather than removing it anyway. Behaviour on macOS
  and Linux is unchanged, which the committed characterization and the Python/Rust differential
  parity both confirm. This is the same gap
  [ADR-0017](docs/adrs/0017-sealed-root-handle-for-configuration-writes.md) built
  `cancellai-sealedfs` to close in the Rust engine.

### Added

- **Coverage is now a ratchet in the crates where falling matters** (E27-S02, CR1). Per-crate
  region coverage is recorded in `project/coverage_baseline.json` and
  `scripts/check_coverage.py check` fails when a kernel-ring crate covers less than it already did.
  Read the distribution, not the average: the workspace sits at 94.54%, `cancellai-safety` at
  98.23%, and **`cancellai-sealedfs` - the crate holding every `unsafe` block and the mutation
  boundary - at 92.54%**, the lowest of the gated set. `cancellai-guardian`'s 0% is correct and
  deliberately not gated: it is a sixteen-line skeleton that prints "not yet implemented".

### Changed

- **The Rust lint policy now states this project's thesis in a form the compiler checks**
  (E27-S01, CR4; [ADR-0028](docs/adrs/0028-lint-policy-states-the-safety-thesis-and-differs-by-ring.md)).
  `[workspace.lints]` held one line - `unsafe_code = "forbid"` - so `cargo clippy -D warnings` ran
  the default set and nothing else, in a workspace whose whole argument is that a tool deleting
  files must never act on an unproven assumption. Measured before changing anything: **41 `unsafe`
  blocks against 30 `SAFETY:` comments**, and **13 panic sites in production code**, six of them
  indexing strings that arrive from third-party provider manifests.
  - All 41 unsafe blocks now carry a written safety argument, enforced by the compiler and
    asserted by a test that does not depend on clippy. **Seven of the eleven undocumented ones
    exist only behind `cfg(windows)`** and no single-platform run could have found them.
  - Two of the thirteen panics were real rather than theoretical: a plan action naming no target
    artifact aborted the CLI part-way through a run, and the Windows rename path inside the
    mutation boundary sliced a buffer whose size was computed a few lines earlier.
  - The glob matcher that reads provider manifest patterns is panic-free by construction and is
    checked against a naive reference implementation over every pattern and input up to length
    four across an alphabet containing multi-byte characters - the differential method this
    repository already uses between its Python reference and its Rust port, applied to a parser
    that had never been tested that way.
  - **A structural defect the measurement exposed:** Cargo's `[lints]` table is all-or-nothing, so
    `cancellai-sealedfs` - which must declare its own to lift `forbid` - was the one crate a
    workspace policy could never reach. Every `unsafe` block in the repository sat in the only
    crate exempt from the rule about `unsafe` blocks. A gate now refuses any workspace lint a
    crate drops.
  - First coverage measurement in the project's history: **94.54% of regions**, with the worst
    kernel-ring file being `cancellai-sealedfs/src/lib.rs` at 92.57%.

## [1.13.4] - 2026-09-13

### Fixed

- Independent review of the released Atlas TUI and storage-accounting epics is now recorded. The
  TUI still has no mutation path, while the macOS filesystem probe's CR4 unsafe-boundary authority
  is explicitly documented in ADR-0027.
- Historical multi-story commits can no longer disappear from risk-floor auditing. New ambiguous
  batches are checked at their combined floor; older irrecoverable attribution gaps are visible,
  reasoned baselines.

## [1.13.3] - 2026-09-13

### Changed

- **The workspace minimum Rust version is 1.88.0** (E17-S08, CR4;
  [ADR-0026](docs/adrs/0026-raise-the-workspace-msrv-to-1-88.md), accepted). 1.85.0 was inherited
  from edition 2024 and never chosen on its own merits, and it had come to cost three things: an
  open advisory in `lru` that only `ratatui 0.30` clears, an accepted `cargo deny` waiver for an
  unmaintained `paste`, and a ban on let-chains in this workspace's own source that the safety
  kernel had already broken without anyone noticing. Building from source now needs rustc 1.88
  (released mid-2025). Nothing about the shipped binaries or the Homebrew formula
  changes.
  - **GHSA-rhfx-m35p-ff5j is closed.** `lru` 0.12.5 -> 0.18.4 through `ratatui` 0.29 -> 0.30.2,
    and `paste` leaves the graph, so `rust/deny.toml`'s ignore list is now **empty** - a
    supply-chain gate with no waivers.
  - **An intermediate upgrade resolution compiled two copies of `crossterm`** - 0.28 declared by `cancellai-tui`, 0.29 by
    `ratatui 0.30` - which means two copies of the crate owning raw mode and the event stream in
    one process. `cargo deny` only warns on duplicates. Now one.
  - Filed as CR2 and executed as CR4: clippy reads `rust-version`, so raising it enabled lints
    that had been silently skipped - five `collapsible_if` and one `manual_is_multiple_of`, three of
    the six sites in floored crates. The floor caught the story mid-flight and raised its own level. An
    MSRV bump is a code change here, not a configuration change.

### Added

- **A cut version whose release never published is now a state the repository can express**
  (E17-S10, CR1). Each release packet records `Published: yes | no | pending` with the reason when
  it failed, `release.py check` names every unpublished version, and the formula-lag rule skips one
  rather than demanding the impossible - v1.13.1 had to be finalized by hand before v1.13.2 could
  be prepared, because the tooling had no word for what had happened. Backfilling the record found
  a fourth: **v1.10.0** was cut and never published a month ago, and nobody had noticed.
- The toolchain report counts approved pack members as managed (E17-S11, CR1). It listed all eight
  approved skills as "decide or remove" in the one report the session-start ritual puts in front of
  the owner; `check` had always counted them correctly. Found by the E17-S08 independent review.

### Fixed

- Knowledge bundles now use the strict Ed25519 verification required by ADR-0024, rejecting
  weak keys/signatures without replacing the last accepted bundle (E17-S08 independent review).
- Cargo-deny now checks transitive unsoundness advisories explicitly; its default scope had
  missed `lru` even though RustSec already carried the advisory (E17-S08 independent review).

- The branch check added in 1.13.2 prints a status instead of a blank column (E26-S04, CR0).
  `gh run list --json conclusion` returns `""` rather than `null` while a run is still going, and
  jq's `//` falls back only on `null` - so every in-progress workflow reported as an empty field,
  which reads exactly like a clean sheet. A check whose failure mode is "looks fine" is worse than
  no check, which is the argument the step was added on.

## [1.13.2] - 2026-09-13

### Fixed

- The publish-time checksum guard hashes the archive, not the file that sorts first (E17-S09, CR3).
  The v1.13.1 release built all four platforms, verified every attestation, then refused to publish
  because all four checksums mismatched: the guard globbed `<artifact name>.*` and took the first
  match alphabetically, and since E17-S03 that has been `<name>.cdx.json` - so every declared
  checksum was compared against a JSON document describing the archive instead of the archive. It
  failed closed, which is the only reason this is a defect and not an incident. Latent since
  2026-09-08; the two releases in between died before this job ever ran.

### Changed

- The session-start ritual reads the branch it is about to build on (E26-S04, CR0). `orient` now
  reports every workflow conclusion on the default branch before a story is selected, and says
  *unknown* rather than nothing when it cannot tell. It exists because the MSRV leg of `rust.yml`
  failed on every push for four days, across two attempted releases, while every other leg stayed
  green and every session read the green ones. A failure nobody reads is indistinguishable from a
  gate nobody has.

## [1.13.1] - 2026-09-13

### Added

- **The engineering system is now measured and falsifiable, not only documented** (E25, E26; CR0-CR2,
  no user-visible product behavior change). Twelve findings from
  [the methodology review](docs/audits/2026-09-12-METHODOLOGY_REVIEW.md) against DO-178C, ISO 26262,
  IEC 61508, NPR 7150.2 and the software-measurement literature; eleven are now repaired.
  - `scripts/process_metrics.py` measures the process from artifacts the repository already commits:
    review yield per round, first-pass rejection rate split by reviewer independence, a
    Lincoln-Petersen residual estimate from reviewer overlap, evidence-ledger integrity, and
    documentation readership. The first result was that **47% of round-1 independent review verdicts
    are `FAIL`, on work whose executor had run every gate green**.
  - `scripts/check_risk_classification.py` gives the Change Risk Level a floor derived from the paths
    a change touches, enforced at `commit-msg` where the staged diff and the story are both visible.
    Four kernel surfaces are stated in code, where configuration may raise them and may never lower
    one. No safety standard lets the implementing party assign its own criticality level.
  - `scripts/gate_sensitivity.py` plants a violation of each named claim and records which gate
    caught it - Mills' error seeding, generalised from the two comparators already using it. Eleven
    mutants, all killed; the one that initially survived had edited the risk floor itself. A control
    pass runs every gate on an unmutated copy first, because a gate that fails on a clean export
    appears to kill everything it is pointed at, and a kill by such a gate is not counted.
  - `scripts/check_evidence.py` requires a row per acceptance criterion, a real residual-risk section
    at CR3 and above, and a Safety Verdict for a CR4 story at `done`.
  - `scripts/safety_oracle.py` checks protected-name enforcement, root capability and the retention
    rule against predicates written from the invariants rather than recorded from the implementation.
  - `scripts/check_ears.py` classifies acceptance criteria and requires a CR2+ story to say what
    happens when something is wrong. **13 of 366 criteria describe unwanted behaviour.**
  - `docs/security/HAZARD_ANALYSIS.md` adds an STPA pass over the mutation control loop: twenty
    unsafe control actions, five loss scenarios from process-model inconsistency, and three gaps the
    invariant set does not constrain.
- **The agent toolchain is governed as a dependency** (E26). `project/agent_toolchain.json` records
  every skill, hook, plugin, MCP server and language server with its source, pinned version, trust
  tier, capability surface, always-on token cost and a dated decision - plus what was rejected and
  why. Nothing unmanaged; a component that runs code must be first-party or a named vendor;
  decisions expire; and the always-on context cost is budgeted, by the same argument C-11 makes
  about storage. `scripts/check_agent_toolchain.py` also compares pins against upstream and records
  which components are actually used. **No agent installs, updates or removes a component.**

### Fixed

- The Rust quality gate is green against current stable clippy again (E06-S05, CR1). A descending
  sort in `cancellai-policy`'s atlas summary was written as a hand-rolled comparator, which the
  toolchain this workspace was developed against accepted and current stable denies - the v1.13.0
  release workflow failed at `verify-rust` on all three platforms because of it, and `publish` was
  correctly skipped. The ordering is unchanged. Recorded with the first use of the risk-floor
  override mechanism, since `cancellai-policy/src` carries a CR3 floor and this is presentation
  ordering downstream of every eligibility decision.

- The sensitivity harness plants each violation where the mutant aims it (E25-S12, CR2). Anchors are
  unique by construction: `except OSError` matched 26 sites in `cancellai.py` and only one is
  rewritten, so the SI-008 mutant had been editing a marker validator rather than the scan's
  completeness channel - and it therefore died on Python 3.13 and survived on 3.14, which is where
  CI found it. The report is now byte-identical on both, SI-008 is seeded in `Scan.record`, and a
  stale report prints the rows that differ instead of only saying that something moved.

- The MSRV promise holds again across a dependency update (E09-S05, CR1). Adding `ratatui` pulled in
  `instability` and `darling` releases that require rustc 1.88 while this workspace promises 1.85.0
  (ADR-0015), and the MSRV leg of `rust.yml` has failed on all three platforms on every push since -
  red on `main` across two release attempts, while every stable leg stayed green. The workspace now
  resolves with Cargo's MSRV-aware resolver (`resolver = "3"`), which will not select a version above
  the declared `rust-version`; the affected crates are locked to versions that support it. The Rust
  quality set passes unchanged.
- A release can carry a fix that closes no epic (E22-S07, CR3). The release contract could express
  only "this version closes an epic", so when the v1.12.0 and v1.13.0 workflows failed - a Windows
  packaging error, then a clippy denial - there was no way to cut the version carrying the fix, a
  published tag being immutable history. `release.py prepare` now takes `--fix <reason>` instead of
  `--epic`, and such a release must take the next patch number. Found while fixing it: a release was
  credited with covering an epic if the id appeared **anywhere** in the packet text, and a packet
  embeds the changelog - so PD-021's gate was satisfiable by a sentence. It now reads the declared
  `- Epic:` line. Four epics had been credited by prose; none was closed, so nothing had shipped
  wrongly.

- The risk-floor gate refuses a checkout it cannot reason over (E25-S14, CR2). It attributes
  committed changes to the story that declared them, and CI ran it against a depth-1 checkout: a
  shallow tip has no parent object, so `git log --name-only` reports the whole tree as its diff.
  All 539 tracked files were attributed to the last commit's story, and the gate refused a kernel
  crate's CR4 floor for a story that touched no Rust. It now refuses a shallow clone and names the
  checkout option that fixes it, a parentless commit attributes nothing, and the three CI jobs that
  run it fetch full history. The same defect pointing the other way is a silent pass, which is the
  version nobody would have found - and this is the second instance of the class, after the one
  that broke the v1.10.0 tag.
- A release packet's embedded links resolve from where the packet lives (E22-S09, CR3). A packet
  embeds the changelog section it ships, and a changelog link is written from the repository root
  while the packet sits two directories down - so `prepare` produced one pointing at `docs/adrs/...`
  from inside `project/evidence/`, and the documentation gate refused it. v1.13.0's packet carries
  the right form because someone rewrote it by hand, which is precisely the failure `release.py`'s
  own docstring warns about.
- The documents state the review rule that is actually in force (E25-S13, CR0). ADR-0025 replaced
  the two-round ceiling with a yield-based stopping rule, and three surfaces still said "at most
  twice": `AGENT_PROTOCOL.md` in two places, the `story-executor` skill, and PD-022 in the decision
  register, which is now superseded by PD-024. The skill is the one worth noting - the pack's rule
  is that a skill points at the contract and never restates it, and the single rule it restated is
  the one that drifted.

- The release workflow declares the shell for the step that broke v1.12.0 (E22-S08, CR1). That
  release packaged macOS and Linux, then its CycloneDX SBOM step - the one step in the job without
  `shell: bash` - ran under PowerShell on the Windows leg, where the backslashes continuing its
  command line are not continuations, and the release never published. A new workflow gate requires
  every multi-line `run:` in a job that can land on Windows to say which shell reads it. The gate's
  own first version reported clean: the step splitter was splitting on the matrix `include:` list
  rather than on `steps:`, so all four steps collapsed into one block and the missing declaration
  was masked by a sibling that had one.

- The knowledge-bundle verifier compiles on the promised toolchain again (E16-S07, CR4). Three
  `let`-chains written in E16-S02 are stable only from Rust 1.88, while the workspace promises
  1.85.0 (ADR-0015), so `cancellai-safety` has not compiled on the MSRV leg since 2026-09-09 -
  through two releases that failed to publish for other reasons that masked this one. Fixing the
  dependency side of the break removed the error that stopped cargo before it compiled anything,
  and this appeared underneath it. The conditions are rewritten with `is_some_and`, which the
  promised version supports, and the expiry boundary is now pinned in both places it appears.
  **This story stays at `ready_for_review`:** it is a CR4 change to the safety kernel, and the
  executor's own review cannot close one.

### Security

- **Open, low severity, not cleared:** `lru 0.12.5` is subject to
  [GHSA-rhfx-m35p-ff5j](https://github.com/advisories/GHSA-rhfx-m35p-ff5j) (`IterMut` violates
  Stacked Borrows), fixed in `lru 0.16.3`. It reaches this workspace through `ratatui 0.29`, which
  pins `lru ^0.12`; the version that takes the fix is `ratatui 0.30`, whose own minimum Rust is
  1.88.0 against this workspace's promised 1.85.0. It is an unsoundness report rather than a
  demonstrated exploit, and nothing here calls `lru::IterMut` - it is ratatui's internal render
  cache. `cargo deny check` does not see it: that gate reads the RustSec database and this advisory
  is in GitHub's, which is a narrower coverage than "advisories are checked" suggests.
  **Correction (2026-09-13):** the sentence above is wrong and is kept as published. GHSA-rhfx-m35p-ff5j
  aliases [RUSTSEC-2026-0002](https://rustsec.org/advisories/RUSTSEC-2026-0002.html), which the
  local RustSec database already carried. `cargo deny` missed it because its `unsound` check
  defaults to direct workspace dependencies and `lru` is transitive - a narrower scope than
  assumed, but not the one claimed. Found by the E17-S08 independent review and repaired with
  `unsound = "all"`; see ADR-0026 and the Unreleased section.
  [ADR-0026](docs/adrs/0026-raise-the-workspace-msrv-to-1-88.md) puts the choice - raise the
  minimum and clear it, or accept it explicitly - in front of the owner with the evidence. It is
  **proposed, not decided**; nothing in this release changes the minimum.

### Changed

- Review stops on **measured yield** rather than a fixed round count, and an epic that changes
  nothing in the shipped artifact closes as `done_no_release` instead of cutting an empty version
  ([ADR-0025](docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md),
  amending ADR-0014).
- `docs/development/AGENT_PROTOCOL.md` states what "independent" can mean here in the standards' own
  vocabulary - one owner and two model instances is IEC 61508's lowest rung - and defines what a
  self-review may and may not certify.
- `docs/development/VERIFICATION_STRATEGY.md` separates **regression detectors** from **correctness
  oracles**: a fixture recorded from an implementation detects change, never wrongness.

No user-visible product behavior changes in E24, E25 or E26.

### Added

- The engineering contract is now loadable by the agent harness that is supposed to execute it
  (E24, CR0). `.claude/skills/` carries seven Agent Skills in the open
  [Agent Skills](https://agentskills.io) format - `orient`, `story-executor`, `epic-verifier`,
  `adversarial-cases`, `risk-gate`, `rust-kernel-guard`, `evidence-packet` - so the same pack
  loads for the executor and for the independent reviewer, which `AGENTS.md` assigns to a
  different agent. A skill is a runner over the contract and never a second copy of it;
  `scripts/check_agent_skills.py` enforces that by requiring every repository path and every
  `scripts/*.py` command a skill names to resolve.
- A `PreToolUse` hook refuses a hand-edit to a generated document at the moment it is attempted,
  returning the regeneration command, instead of letting CI discover it after the work is done.
  It matches on the real, project-relative, case-folded path - `docs/BACKLOG.md`,
  `docs/backlog.md` and `docs/adrs/../BACKLOG.md` are one file on APFS - and fails open on any
  input it cannot interpret. It covers the structured file-writing tools only; the CI drift
  check remains the authority.

### Changed

- `AGENTS.md` states **diff discipline** alongside story discipline: no unrelated reformatting,
  no removal of pre-existing dead code (a safety rule here, since code that looks unreachable may
  be a barrier whose reachability is what is in dispute), removal limited to what a change
  orphaned, and flagging rather than fixing what is found outside the story.

No user-visible product behavior changes from E24.

## [1.13.0] - 2026-09-12

### Added

- Added a real peak-memory regression gate for the shipped discovery path (E10-S02, CR1). New
  `cancellai-cli/tests/performance_memory.rs` runs on every `cargo test` on Linux and asserts
  real peak RSS (`/proc/self/status`'s `VmHWM`, no new dependency) against a 128 MiB regression
  budget for the same synthetic tree `performance_shipped_path.rs`'s latency gate already uses;
  the gate's own module does not exist on macOS/Windows rather than reporting a fabricated
  number there. `performance_scheduled_shipped.rs`'s heavy-dataset trend artifact gained a
  `peak_rss_bytes` field (published for trend visibility, `None` off Linux, never gated) so the
  10k/100k/1M scheduled runs report memory alongside latency without blocking ordinary PRs on a
  noisy metric.
- Added a reclaimability estimator distinguishing logical size, allocated size, and clone/
  reflink-sharing uncertainty (E10-S01, CR2, library-level, no CLI/TUI surface yet). New
  `cancellai-platform::filesystem_kind::CloneSemantics` classifies a scope root's filesystem as
  `NotKnownToShare`, `PossiblyShared` (APFS, Btrfs, XFS, ZFS, ReFS - real, documented clone/
  reflink capability that can make a naive allocated-size sum overstate what deleting files
  actually frees), or `Unsupported` - real detection ships for macOS (`libc::statfs`'s
  `f_fstypename`, `cancellai-sealedfs::observe_filesystem_name`) and Linux (reusing `wsl::
  FilesystemContextObserver`'s `/proc/mounts` parsing for the raw fstype), with Windows
  disclosed as `Unsupported` pending its own story. An unrecognized filesystem name defaults to
  `PossiblyShared`, never the more confident label, so a future/unrecognized clone-capable
  filesystem is never silently trusted. New `cancellai-inventory::reclaim::estimate_reclaim`
  aggregates `FileFacts` into a `ReclaimEstimate`, excluding (and counting, never substituting
  with logical size) any file whose allocated size was itself unobservable, and labeling the
  result `Verified` only when every size was known and the filesystem is `NotKnownToShare` -
  any clone-capable or undetermined filesystem, or any excluded file, downgrades the estimate to
  `Estimated` with a named reason (this story's AC2: unknown APFS/reflink/shared-block effects
  are never presented as guaranteed savings). A self-review found and this same story's own fix
  closed a real Windows CI break before release: the classifier and its backing constants
  carried no `cfg` gate, so they were dead code on Windows (nothing there called them) and broke
  the mandatory `cargo clippy -D warnings` gate on `windows-latest` - now gated to
  `cfg(any(test, target_os = "macos", target_os = "linux"))`, the precedent this crate's own
  `wsl::classify_fstype` already set for the identical shape of gap.

- Added a real Atlas TUI shell and keyboard-first navigation to `cancellai-tui` (E09-S01, CR1,
  observational only), replacing the E02-S01 placeholder skeleton: `Tab`/`Shift+Tab`/`1`-`4`
  cycle four screens (`Home`, and stubs for `Atlas`/`Explain`/`Plan` pending E09-S02/S03/S04),
  `?` toggles a help overlay, `q`/`Esc` quits. Terminal capability detection
  (`NO_COLOR`/`TERM`/`COLORTERM` color tiers, `LANG`/`LC_ALL`/`LC_CTYPE` Unicode-vs-ASCII
  box-drawing, plus a `CANCELLAI_TUI_ASCII` escape hatch) degrades gracefully on any missing or
  unrecognized signal, and a too-small terminal renders a message instead of panicking. Built on
  `ratatui`/`crossterm` (outer-ring dependencies named for this epic in
  [ADR-0019](docs/adrs/0019-dependency-rings-per-crate.md)); the crate depends on no
  `cancellai-*` crate at all yet - no provider/filesystem access is possible from it by
  construction, and `cancellai-policy`'s engine query API is reintroduced only once E09-S02 has
  real view data to render.
- Added the Machine and project atlas screen to `cancellai-tui` (E09-S02, CR1, observational
  only): total footprint and an estimated-reclaimable subset shown as two visually distinct
  values (never blended), per-provider and per-project breakdowns with an explicit
  "Unattributed" bucket, and the individually largest contributors. An incomplete or unknown
  provider scan is flagged prominently rather than hidden inside the totals it could be
  undercounting. New `cancellai_policy::atlas::summarize` computes the summary from
  already-classified inventory (reusing the exact reclaimability test `plan`/`clean` already
  apply); `cancellai-tui` reintroduces `cancellai-policy` as its only new dependency to consume
  it - still no provider adapter or filesystem crate. Live-scan wiring into the running binary
  remains deferred; the screen shows an explicit "not loaded yet" state until that follow-up
  lands.
- Added the Artifact explain view to `cancellai-tui` (E09-S03, CR1, observational only): a
  selectable list of artifacts with, for the selected one, why it exists (project attribution and
  structural relationships), classification, evidence, risk, reversibility, allowed authority,
  and the concrete policy outcome. A destructive policy outcome always shows the real,
  human-readable reason `cancellai_policy::retention::build_actions` already produces for it -
  new `cancellai_policy::explain::explain` surfaces that reason rather than inventing a second
  explanation mechanism. Any confidence weaker than fully verified (an artifact's own or its
  project attribution's, independently) is flagged with a `[low confidence]` marker in the text
  itself, not only by color. Fixed a real rendering bug this story's own tests caught along the
  way: a long policy reason or provider summary line could be silently clipped instead of
  wrapping - both the Explain and Atlas detail panels now wrap.
- Added the Plan review workflow to `cancellai-tui` (E09-S04, CR3, SI-016): the Plan screen
  reviews the same selected artifact the Explain screen shows and requires stronger
  confirmation for irreversible actions than for others - one keypress confirms a
  non-irreversible recommendation, an irreversible one needs a second, and changing the
  selection or leaving the screen cancels any pending or completed confirmation. This is a
  review-only workflow: `cancellai-tui` still depends on neither `cancellai-safety` nor
  `cancellai-platform`, so nothing in it can construct a plan or execute a mutation - the
  confirmed state names `cancellai-cli clean` as the real, separate execution path rather than
  claiming to execute anything itself. `docs/architecture/TARGET.md` and
  `docs/security/SAFETY_INVARIANTS.md` (SI-016) record this scope decision explicitly. A
  self-review found and this same story's own fix closed a real input-handling gap before
  release: the confirmation guard matched `c` regardless of modifiers, so a real terminal's
  Ctrl+C (delivered as `Char('c')` + `CONTROL` once raw mode disables `ISIG`) could complete a
  pending irreversible confirmation instead of cancelling it - now a shared `is_plain_c` check
  makes any modified `c`, Ctrl+C included, always cancel and never arm/confirm.

## [1.12.0] - 2026-09-11

### Added

- Defined the canonical release artifact manifest contract (E17-S01):
  `project/schemas/release_manifest.schema.json` is a machine-verifiable, versioned document
  naming every distributed binary exactly once (`name`/`target_triple`/`sha256`), alongside
  `channel`, `source_sha`, `build_identity`, and `knowledge_compatibility`.
  `scripts/release_manifest.py check` validates it against a golden fixture corpus
  (`tests/fixtures/release_manifest/golden/`) and runs in `pre-commit` and CI. Generating a
  manifest from a real multi-platform build is E17-S02/E17-S03 scope.
- Automated the tier-1 cross-platform release build (E17-S02,
  [ADR-0021](docs/adrs/0021-hand-rolled-release-build-matrix.md)): `.github/workflows/release.yml`
  now builds `cancellai-cli` natively for `aarch64-apple-darwin`/`x86_64-apple-darwin`/
  `x86_64-unknown-linux-gnu`/`x86_64-pc-windows-msvc` on every `v*` tag, packages an archive
  plus SHA-256 checksum per target, smoke-tests each packaged binary by unpacking and running
  it on the platform that built it, and assembles/round-trips a real `release-manifest.json`
  (E17-S01) before `publish` attaches everything to the GitHub Release.
  `scripts/release_manifest.py` gained `generate` and `verify-checksums` subcommands for this.
  These binaries are not a shipping product yet - `cancellai-cli` stays a beta, source-built
  artifact until E06-S04's cutover gate opens.
- Added installation-source awareness to `cancellai-cli` (E17-S04, CR1, observational only):
  `cancellai-cli update --check` and `cancellai-cli version --source` report the installation
  source detected from the running binary's own resolved path (`homebrew`, `windows_package`,
  `linux_package`, `direct_download`, or `unknown`) and source-specific upgrade guidance that
  never names a different channel than the one detected. A bare `update` (no `--check`) is
  refused rather than silently implying a future auto-update default (SI-007); `version`'s bare
  output is unchanged - `--source` only ever appends lines.
- Added provenance, SBOM, and signed attestation to the release build (E17-S03,
  [ADR-0022](docs/adrs/0022-cyclonedx-sbom-via-cargo-cyclonedx.md)): `build-artifacts`
  generates a per-target CycloneDX 1.5 SBOM (`cargo-cyclonedx`, target-accurate - reflects
  each leg's own conditional dependencies rather than the host toolchain's) and signs a
  build-provenance attestation and an SBOM attestation for each canonical archive
  (`actions/attest-build-provenance`, `actions/attest` with `sbom-path` - not the deprecated
  `actions/attest-sbom`). A new `attestation-verify` job independently re-fetches and
  cryptographically verifies both attestations for every archive before `publish`, which now
  depends on it: a missing or invalid attestation fails the release rather than shipping
  silently.
- Bound release channel to maximum default authority (E17-S05, CR4, SI-030,
  [ADR-0023](docs/adrs/0023-release-channel-authority-as-opt-in-function.md)):
  `cancellai-safety::authority::effective_authority_for_channel` adds a `ReleaseChannelAuthority`
  constraint to the Effective Authority minimum, sourced from the new `cancellai-safety::
  BuildChannel` - an opaque wrapper (mirroring `TrustedTier`'s split from `ProviderTrust`,
  SI-021) whose only production constructor reads a `CANCELLAI_CHANNEL` value baked in at
  *compile* time, never a runtime environment variable a user could set to claim a higher
  channel than the build actually is. `stable` carries no additional cap; `beta` caps at
  `Govern` (reaches a confirmed `Delete`, never unattended `Autopilot`); `nightly` (and any
  unset/unrecognized value) caps at `Recommend`, strictly below what even `Quarantine`
  requires. `.github/workflows/release.yml`'s `build-artifacts` job (E17-S02) now sets
  `CANCELLAI_CHANNEL=stable` for every canonical tier-1 build. Wiring this into
  `cancellai-cli`'s own classification pipeline is deferred to E06-S04 (the cutover story) -
  see `docs/security/SUPPLY_CHAIN.md`'s "Release channels" section for why.
- Wrote the canonical-repository-topology migration runbook and made its identity claim
  enforced rather than only documented (E17-S06, CR2): `docs/RELEASING.md`'s "Repository
  topology transition" section now names the trigger condition, migration steps (history
  preservation, issue handling, release/tag continuity, and - critically - why
  `homebrew-cancellai` keeps its name so `brew tap`/`brew install cancellai` keeps working for
  existing users with zero action on their part), and an explicit "never silently retired" tap
  commitment, executing ADR-0011's already-accepted decision to defer the actual split rather
  than deciding it now. `scripts/check_repository_topology.py check` (new; `pre-commit` and CI)
  cross-checks that `Formula/cancellai.rb`, `scripts/release.py`'s `REPO` constant, and that
  document's own "current remote" claim all name the same repository.
- Defined the provider manifest schema v1 (E16-S01, `docs/architecture/PROVIDER_MODEL.md`
  "Manifest-only" integration level, SI-021): `rust/crates/cancellai-provider-api/src/manifest.rs`
  (`ProviderManifest`, versioned, `#[serde(deny_unknown_fields)]` on every struct) has no field
  anywhere that can express a capability, trust, or authority claim - a manifest can only ever
  describe *where* provider state lives (roots) and *what kind* of thing each matched file is
  (session/protected/cache), never claim delete capability or a trust level. `manifest_provider.rs`
  adds `ManifestProvider`, a generic `ProviderCapabilities` implementation driven entirely by a
  parsed manifest and resolved root paths - it answers `DETECT`/`FINGERPRINT_ROOT`/
  `INVENTORY_MAP` from real evidence (reusing the same root-confidence rule Claude/Codex's
  adapters use) and reports every other capability unconditionally `UNSUPPORTED`. A
  manifest-driven provider's authority is computed through the identical
  `cancellai_safety::TrustedTier` gate every hand-written adapter uses and defaults to
  `TrustedTier::untrusted()`.
- Added the OpenCode provider manifest (E16-S05, `docs/PROVIDERS.md` "Tier 2 ecosystem
  providers"): the first real "Manifest-only" integration, built from OpenCode's real,
  source-confirmed layout (`anomalyco/opencode`, formerly `sst/opencode`) - `auth.json`
  (credentials, protected) and a `storage/` tree of session/message/part/session_diff/project
  records under `$XDG_DATA_HOME/opencode`, plus declared (not yet scanned) config/cache roots.
  Ships committed and reviewed in this repository via `rust/crates/cancellai-provider-api::
  opencode_manifest`, but its provider still defaults to `TrustedTier::untrusted()` like every
  other provider - no code path here grants it anything a community-contributed manifest would
  not also have to earn through the identical trust pipeline. An unrecognized layout stays
  inspection-only.
- Added the Gemini CLI provider manifest (E16-S03, `docs/PROVIDERS.md` "Tier 2 ecosystem
  providers"): a second "Manifest-only" integration on the same E16-S01 engine, built from
  `google-gemini/gemini-cli`'s real source - `oauth_creds.json`/`google_accounts.json`/
  `settings.json`/`trustedFolders.json`/`projects.json` (protected) and `tmp/<projectIdentifier>/
  chats/*.jsonl` (session) under `GEMINI_CLI_HOME`-or-`~/.gemini`. A new `vendor_notes` manifest
  field cites the tool's documented built-in session retention policy in `EXPLAIN`'s evidence
  text without upgrading `EXPLAIN` out of `UNSUPPORTED` - it makes the honest answer more
  informative, never a claim to detect a user's actual configured value. Defaults to
  `TrustedTier::untrusted()` like every other provider.
- Added the GitHub Copilot CLI provider manifest (E16-S04, `docs/PROVIDERS.md` "Tier 2
  ecosystem providers"): a third "Manifest-only" integration, built from GitHub's published
  documentation (Copilot CLI is closed-source) - `config.json`/`settings.json` (protected) and
  `session-state/<sessionID>/events.jsonl` (session) under a `COPILOT_HOME` full-path override
  or `~/.copilot` default. Being manifest-only means there is no mutation path to begin with,
  satisfying "internal state is not mutated without documented native capability" by
  construction. The separate, platform-conditional `COPILOT_CACHE_HOME` directory is not
  modeled as a root in this schema version. Defaults to `TrustedTier::untrusted()`.
- Defined the signed knowledge bundle format (E16-S02, CR4,
  [ADR-0024](docs/adrs/0024-ed25519-dalek-for-knowledge-bundle-signatures.md),
  `docs/security/SUPPLY_CHAIN.md` "Knowledge updates"):
  `cancellai-safety::knowledge_bundle::KnowledgeBundle` packages provider/version/layout
  intelligence separately from the binary, verified with Ed25519 (verification-only dependency
  - no signing capability ships) against a caller-supplied `LocalTrustPolicy`. A verified
  bundle's trust tier always comes from local policy, never from the bundle itself - there is no
  field in the wire format that could assert one. `KnowledgeStore` refuses a stale/replayed
  update from the same publisher and supports rollback to the prior trusted bundle, re-checking
  expiry so a failed rollback never displaces the still-current bundle. Adversarially tested:
  tamper (payload, digest, signature), expiry, unknown-signer, and rollback-after-expiry cases.
- Automated the community provider verification workflow (E16-S06, `.github/CONTRIBUTING.md`
  "Provider contributions"): `scripts/check_provider_trust.py check` (`pre-commit` and CI) lints
  every manifest under `rust/crates/cancellai-provider-api/manifests/*.json` against that
  schema's own known field set - a smuggled `trust`/`capability`/`authority` field, anywhere in
  the manifest including nested inside a root/marker/artifact, fails here independently of the
  Rust parser's own `deny_unknown_fields` check - and validates the new
  `project/provider_trust.json` registry recording each shipped manifest's current
  `ProviderTrust` tier. A tier above `Untrusted` requires a non-empty `verified_by` and at least
  one `fixture_references` entry, mirroring `cancellai-safety::trust_promotion`'s own
  evidence-required rule; `project/provider_trust.json` is added to `CODEOWNERS` so only the
  project owner can merge a change to it. A community PR therefore cannot mark itself Built-in
  Verified through either a smuggled manifest field or a bare registry claim.
- Widened `cancellai-model::AgentArtifact` with `relationships: Vec<ArtifactRelationship>`
  (E08-S01, `docs/architecture/DOMAIN_MODEL.md` "`relationships` (E08-S01)"): each entry is a
  `{ kind: RelationshipKind, related_artifact_id: ArtifactId }` pair, with `RelationshipKind`
  carrying only `ChildOf` today. Populated from `cancellai_provider_codex::CodexSession::
  parent_session_id`, which `cancellai-policy::retention::resolve_codex` was already reading but
  discarding before this change; an unresolved `parent_thread_id` produces no relationship
  rather than a fabricated one. Claude sessions are flat and always report an empty list. New
  key, additive-only, in `--json` inventory output's `artifacts[]` entries.
- Widened `cancellai-model::AgentArtifact` with `project_attribution: Option<ProjectAttribution>`
  (E08-S02, CR2, SI-023, `docs/architecture/DOMAIN_MODEL.md` "`project_attribution` (E08-S02)"):
  `None` means `Unattributed`; a `Some` names the project (`ProjectRef`), the evidence category
  (`AttributionSource::ExplicitProviderMetadata` today - Claude's own `projects/<name>/`
  directory, taken verbatim, never decoded into a guessed real filesystem path), and a
  confidence that starts equal to the artifact's own and is downgraded alongside it by the
  existing partial-scan handling. A blank/whitespace-only project name resolves to
  `Unattributed` rather than a hollow reference. Codex sessions have no project concept this
  adapter observes and are always `Unattributed`. New key, additive-only, in `--json` inventory
  output's `artifacts[]` entries.
- Gave `cancellai-model::ActivityState::Orphaned` its first producer, and added
  `AgentArtifact::activity_signal: Option<ActivitySignal>` (E08-S03,
  `docs/architecture/DOMAIN_MODEL.md` "`activity_signal` / `ActivityState::Orphaned` (E08-S03)"):
  a Codex session declaring a `parent_session_id` this scan did not discover is now classified
  `Orphaned` (a dangling parent reference), with `activity_signal` naming the missing parent id;
  an ordinary `Stale` session's `activity_signal` names the observed mtime and cutoff. An
  unresolved parent only ever overrides what would otherwise be `Idle`/`Stale` - never `Active`
  or `Unknown` - preserving the existing authority-capping protection those two values already
  have in `cancellai-safety::authority::lifecycle_ceiling`. `activity_signal` is `None` for
  `Active`/`Idle`/`Unknown`. New key, additive-only, in `--json` inventory output's `artifacts[]`
  entries.
- Added `cancellai-policy::views` (E08-S04, CR1, `docs/architecture/TARGET.md` "Engine / Query
  API (E08-S04)"): machine/project/provider/artifact/session query views over one classified
  inventory (`by_machine`/`by_project`/`by_provider`/`by_artifact`/`by_session`), the first real
  occupant of the target architecture's "Engine / Query API" layer. Every view groups borrowed
  references rather than cloning artifact data; `by_project` carries an explicit `Unattributed`
  bucket for E08-S02's `None` attribution; `by_session` walks a full `ChildOf` chain to its
  ultimate root (E08-S01) rather than one edge, so a transitive Codex subagent tree lands in one
  bucket, with a target absent from the given slice or a cycle falling back to the artifact's own
  id instead of propagating an unbacked id (round-1 independent verifier review finding, repaired
  in the same round); Claude's flat sessions each stay their own bucket. `by_machine` is a single
  bucket today (no multi-machine support exists yet). A generic reconciliation test compares
  counted id occurrences, not a set, so a duplicate or dropped source row is observable in every
  view.

## [1.11.0] - 2026-09-08

### Fixed

- `.github/workflows/release.yml`'s `verify` job now checks out full git history
  (`fetch-depth: 0`) instead of GitHub Actions' default shallow checkout, so
  `scripts/check_platforms.py check`'s ancestor validation
  (`git merge-base --is-ancestor <verified_commit> HEAD`) can find each platform's
  `verified_commit` object at all. A shallow checkout made an older `verified_commit`
  entirely absent from the tagged commit's local history (not merely unreachable), which
  failed the `v1.10.0` tag's release workflow after the tag had already been pushed - the
  GitHub release was never published (`gh run 34252829459`). `scripts/check_workflows.py`
  now fails statically if the checkout step in the job running that provenance gate reverts
  to anything but `fetch-depth: 0` (E23-S01).

## [1.10.0] - 2026-09-08

### Added

- `cancellai-cli`'s target-engine Rust core now observes real Windows file/volume identity
  (`GetFileInformationByHandle`, via a new `windows-sys`-backed capability in
  `cancellai-sealedfs`) instead of reporting every Windows path as identity-`Unsupported`
  (E20-S01, [ADR-0020](docs/adrs/0020-windows-native-identity-via-windows-sys.md)). A
  Windows reparse point (symlink, junction, or any other reparse tag) is classified from its
  own attributes and never treated as, or compared using, Unix symlink semantics; the
  `cancellai-safety` filesystem/volume-boundary check (SI-018) now enforces a genuine Windows
  volume boundary instead of refusing unconditionally. As a direct consequence,
  `cancellai-inventory`'s scanner can now descend below a scope root on Windows (previously an
  accepted limitation, E20-S04). Allocated-size reporting, `cancellai-sealedfs::SealedRoot`'s
  no-follow root-establishment walk, Windows process observation, and real Windows file
  deletion were deliberately out of this story's scope and remained unimplemented at the time
  (see the E20-S05 entry below, which closes them).
- `cancellai-platform` now detects a WSL2 runtime environment explicitly (rather than treating
  it as generic Linux) and classifies a path's filesystem context as native Linux, a
  Windows-drive mount (`drvfs`, e.g. `/mnt/c`), or an unrecognized `Other` mount (E20-S02).
  Library-level only for now - no CLI surface yet, and no change to mutation/quarantine
  authority (the existing Unix device-identity boundary check already refuses crossing into a
  `/mnt/c`-style mount, since it is genuinely a different device).
- `docs/PLATFORMS.md` is now generated (`scripts/check_platforms.py`, from
  `project/platforms.json`) rather than hand-authored aspiration: each platform's tier is
  cross-validated against `.github/workflows/rust.yml`'s real CI matrix and against named test
  functions this script confirms actually exist, so a platform cannot be declared tier 1
  without CI and verified destructive-mutation evidence (E20-S03). Per that real bar, macOS,
  Linux, and (since E20-S05) Windows are tier 1; WSL2 (no dedicated CI runner) is tier 2.
- `cancellai-cli`'s target-engine Rust core now implements the remaining Windows capabilities
  E20-S01 scoped out (E20-S05): real running-process observation
  (`CreateToolhelp32Snapshot`, replacing an unconditional "incomplete" result), real
  allocated-size reporting (`GetFileInformationByHandleEx(FileStandardInfo)`, distinct from
  logical size), a verified no-follow, handle-relative root-establishment walk in
  `cancellai-sealedfs` for Windows (`NtCreateFile` via `OBJECT_ATTRIBUTES.RootDirectory`, the
  Windows analogue of `openat`, used by `configure` and by `clean`'s default-root
  re-verification), and real, identity-confirmed Windows file deletion
  (`FILE_DISPOSITION_INFO`/`SetFileInformationByHandle`, following the same open/re-check/
  handle-relative-delete/post-delete-corroboration shape ADR-0017/E21-S07 established on Unix,
  with the atomic-rename step using the native `NtSetInformationFile` rather than the Win32
  `SetFileInformationByHandle` wrapper - the latter does not reliably honor a non-null
  `RootDirectory` for handle-relative rename). Confirmed on real Windows CI (ADR-0020's own
  dated investigation record); `docs/PLATFORMS.md` now records Windows mutation as `verified`
  and the platform as tier 1, alongside macOS and Linux.

## [1.9.0] - 2026-09-04

### Note on how E22 was verified

Round 1 of independent adversarial review (Codex) returned `FAIL` for five of the six stories
(`project/evidence/E22-VERIFIER-REVIEW.md`): six independently reproduced `release.yml` drift
regressions the static checker missed, a real Codex retention semantic divergence (a stale
subagent-tree root beside a recent child was still an individual delete candidate), a golden
CLI snapshot gap, and a docs section a prior story had silently removed. Every finding was
repaired and re-verified, including real GitHub Actions/CodeQL/Dependabot evidence gathered
after pushing the repair commit (`project/evidence/E22-S0{1,2,3,4,5}/ROUND2-REPAIR.md`). The
epic was then closed by owner decision without spending the second independent review round
ADR-0014 permits - the same pattern v1.8.0's E21 closure used - so these repairs carry no
independent re-confirmation beyond the executor's own re-run of every gate the round-1 review
specified; `project/evidence/RELEASE-v1.9.0.md` records the residual risk this accepts.

### Added

- `cancellai-cli` (the beta target-engine CLI) now has a real `--help`/`-h`/`--version`
  surface and per-command help (`cancellai-cli clean --help`, etc.), matching the reference
  CLI's own top-level surface (E22-S03, `CR-TE-07`). Argument parsing moved from a hand-rolled
  loop in `main.rs` to `clap` ([ADR-0019](docs/adrs/0019-dependency-rings-per-crate.md)).

### Changed

- `cancellai-cli` now refuses a flag irrelevant to the chosen command (e.g. `status --dry-run`,
  `clean --claude-retention`) with exit code 2, instead of silently accepting and ignoring it
  as every command's flags did before this release. `--help`/`-h`/`--version` are an explicit
  exception: wherever they appear, they still short-circuit remaining validation and exit
  before any command runs, matching `clap`'s own precedence and common CLI convention (`git`,
  `cargo`) - see `docs/CLI_RUST.md`'s "Argument parsing" section.

### Fixed

- **A Codex subagent tree with a stale root and a recently-touched child is no longer an
  individual delete candidate for the stale member** (E22-S04). `cancellai-policy::retention`
  gated `--keep-latest` pinning on the tree's effective (max-of-members) mtime but evaluated
  each member's own staleness independently, so a tree the reference protects in full - any
  recent member protects the whole tree, not just the pinning rail - could still surface the
  old-looking member as a `Delete` action in the target engine. `resolve_codex` now applies the
  same tree-level cutoff gate `cancellai.py::choose_codex_old_sessions` does before classifying
  any member's staleness.

### Documentation

- Recorded that `cancellai-cli clean` deletes Codex sessions at the filesystem level only,
  even when the installed `codex` CLI advertises its own `--force`-capable delete: this is now
  a stated, permanent divergence from `cancellai.py` (which prefers the vendor command) rather
  than an unstated gap (E22-S05, `CR-TE-10`). See `docs/CLI_RUST.md`'s "Known gaps" for why -
  wiring it would add a second mutation primitive to the safety kernel and is deferred to a
  dedicated future story, not a side effect of this one.

## [1.8.0] - 2026-09-03

### Fixed

- **A directory the scan could not read no longer authorizes deletion** (E21-S03, `CR-TE-01`).
  On an ordinary tree - one directory without read permission - `cancellai-cli` deleted an
  eligible artifact and exited `0` while reporting the scope complete and its knowledge
  verified; `cancellai.py` withheld every destructive action for that tool and exited `4`. Both
  provider adapters discarded every failure to observe part of the tree with a bare
  `else { continue }`. They now record each one with a cause and a path, withhold the whole
  tool, and degrade `knowledge_confidence` for every artifact in the scope (SI-008, SI-009,
  SI-010, C-02). The Claude side was broader than previously recorded: E06-S02 had repaired only
  the companion-payload branch, and an unreadable **project** directory still passed silently,
  disclosed nowhere.
- `scan_completeness[].error_count` reports the real number of unobservable paths instead of
  `u32::from(!complete)`, which was only ever `0` or `1` (`CR-TE-10`).
- **The delete path prevents the path-swap race instead of detecting it** (E21-S07, `CR-TE-05`).
  The unlink is issued through `cancellai-sealedfs`'s handle-relative `unlinkat` against a
  directory descriptor opened once with `O_NOFOLLOW` at every component, so a rename or
  symlink-swap after validation cannot redirect it. Consequence, intended and user-visible: a
  provider root reached through a symlinked path component can no longer be cleaned - the rule
  E07-S09 already set for root establishment, now holding at the moment of mutation.
  `MutationOperation`'s two unconfirmed, unreachable variants (`Quarantine`,
  `DeleteDirectoryTree`) were removed rather than left armed for E12 to inherit (`CR-TE-11`).
- Rollout metadata reading honours the 512 KiB bound it documents instead of loading the whole
  transcript (E21-S06, `CR-TE-04`). Measured on a single 287 MB rollout: peak RSS **2.9 MB**,
  against 303 MB before - and below the Python reference's own 27.7 MB.

### Added

- Two fixtures the corpus never had - `codex-partial-tree` and `claude-partial-project` - both
  `NORMATIVE` and running through the differential gate in both root-origin scenarios (E21-S02).
  They were written to **fail** against the unrepaired engine, and did; the failing run is
  committed in their evidence packet. `scripts/check_fixtures.py` now refuses an undeclared
  category asymmetry between the two reference providers, because the corpus carrying
  `partial_tree` for Claude and not for Codex is what let the gate stay green while the engine
  deleted (`CR-TE-03`).
- Scope completeness is a shared *type* obligation on every provider adapter
  ([ADR-0018](docs/adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md),
  E21-S04). `ProviderResolution` hands out planning candidates only through a value carrying the
  scope's completeness, with a `compile_fail` regression proving the bare-candidates route is
  unreachable; `scripts/check_rust_workspace.py` fails if `cancellai-cli` stops being able to
  reach `cancellai-inventory` at all, which is how `CR-TE-02` went unnoticed.
- A performance gate on the discovery path the CLI actually executes (E21-S05). Every timing
  assertion is paired with an assertion on what the resolution produced, so a benchmark
  measuring an empty tree fails instead of reporting an excellent number.

### Note on how E21 was verified

Round 1 of independent adversarial review (Codex) returned `FAIL` for five of the seven stories
and reproduced a real escape the first implementation had left open: an unreadable Claude
`projects/` root was converted into a clean empty scan, so `clean --yes` exited `0` where the
frozen reference exits `4`. The completeness was computed correctly in discovery and discarded
one layer up - the same class of defect this epic exists to close. Every finding is repaired and
pinned by a regression written against the verifier's own reproduction; repairing them surfaced
one more instance of the same pattern (`Path::exists()` collapsing "not installed" into "not
readable"), also closed. The epic was closed by owner decision without spending the second
review round, which means these repairs carry no independent confirmation -
`project/evidence/E21-CLOSURE.md` records that and the residual risk it accepts.

- An independent target-engine review is committed as
  [`docs/audits/2026-09-03-CODE_REVIEW.md`](docs/audits/2026-09-03-CODE_REVIEW.md), with
  thirteen findings (`CR-TE-01`..`CR-TE-13`) converted into story contracts under two new
  epics rather than left as prose: **E21 Target Engine Trust Remediation** and **E22
  Engineering System Hardening**. They are new epics, not additions to E06, because E06 has
  already used both independent review rounds ADR-0014 permits.
- [ADR-0018](docs/adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md):
  scope completeness becomes a shared *type* obligation on every provider adapter, while the
  adapters keep their layout-specific traversal. `cancellai-inventory`'s completeness model is
  currently unreachable from the shipped binary, which is why the defect its own reviewer
  rejected in E04-S03 reappeared in the adapters that replaced it.
- [ADR-0019](docs/adrs/0019-dependency-rings-per-crate.md): the safety kernel stays
  dependency-free except by dedicated ADR; the experience and persistence crates may use
  mature, licence-checked libraries. This is what will give `cancellai-cli` the `--help`/
  `--version` surface it currently lacks entirely.

### Fixed

- `project/roadmap.json` declared `current_phase: "P0"` while both P0 epics were `done` and P1
  stood one epic from closing, so `PROJECT_STATUS.md` generated a phase the project had already
  left. Corrected to `P1` (`CR-TE-12`).

### Note

No user-visible behavior changed in this entry. `cancellai.py` remains the sole canonical,
shipping engine; the findings concern the beta Rust engine and the gates around it. `E06-S04`
now records `E21` and `E22-S01` among its blockers, so this file does not read, by omission, as
though the cutover checklist were unchanged.

## [1.7.0] - 2026-09-02

### Added

- E06-S01: `cancellai-cli` gains its first real command surface -
  `status`/`inspect`/`plan`/`clean`/`configure`/`version` against the Rust engine
  (`docs/CLI_RUST.md`). `status` is the read-only default (no subcommand or flag ever implies
  `clean`); `clean` is the only mutating command, gated by `--dry-run`/`--yes`/interactive
  confirmation and routed exclusively through `cancellai-safety`'s single mutation boundary.
  This is a beta command surface, not yet the canonical engine (`docs/development/
  MIGRATION_PYTHON_RUST.md`) - `cancellai.py` remains the shipping reference until E06 closes.
- E06-S02: a differential parity gate (`scripts/rust_python_parity.py`) runs the Python
  reference and the Rust CLI over the full `NORMATIVE` fixture corpus, comparing which
  sessions each engine would delete. Wired into pre-commit/CI. Building it surfaced and fixed
  two real E06-S01 defects: an incomplete companion-payload scan only withheld the one
  affected session instead of the whole tool (SI-008/SI-009), and a Claude home with no
  `projects/` directory was misreported as an incomplete scan instead of legitimately empty.
- E06-S03: documents and proves the beta side-by-side model for `cancellai-cli` -
  `version` identifies the engine, and `cancellai`/`cancellai-cli` share no install path or
  local state (`docs/RELEASING.md`, `docs/development/MIGRATION_PYTHON_RUST.md`), so rollback
  during beta is simply not invoking the Rust binary. Proven with new smoke tests
  (`rust/crates/cancellai-cli/tests/install_rollback.rs`): every read-only command, and even a
  real `clean`, touches nothing under `$HOME` outside the provider artifacts explicitly
  targeted.
- E06-S04: records the Rust cutover gate checklist (`docs/development/RELEASE_GATES.md` "Rust
  cutover gate status") and its current verdict - **not ready**; `cancellai.py` remains the
  sole canonical, shipping engine. No user-visible behavior changed in this entry; it exists so
  this file does not read, by omission, as though cutover had happened.
- E07-S07: `cancellai-cli clean`/`configure` refuse a default-named root
  (`$HOME/.claude`/`$HOME/.codex`, no override) that is itself a symlink/reparse point,
  independently re-checked immediately before establishing the root or writing configuration -
  not only at classification time (`docs/architecture/PLATFORM_MODEL.md` "Default-root
  authority never rests on a lexical name alone"). Closes an E06 verifier review round 2
  finding: authority previously followed the lexical `$HOME/.claude` name alone, so a symlinked
  default root was still treated as mutation-eligible.
- E07-S07 (round 2): closes an E07-S07 round-1 independent verifier review finding - `configure`'s
  own re-check above narrowed but did not close its TOCTOU: a default root swapped to a symlink
  *after* that check and before the raw path-based settings write reached outside the approved
  root. `configure` now routes every read/write through a new `cancellai-sealedfs` crate
  (`docs/adrs/0017-sealed-root-handle-for-configuration-writes.md`): the root is opened exactly
  once with `O_NOFOLLOW` and retained, with every following operation issued via
  `openat`/`renameat` against that descriptor rather than the original path, closing the race by
  construction. **Behavior change**: `configure` now refuses outright (rather than attempting an
  unprotected write) on every platform without a verified no-follow/handle-relative
  implementation - today, every non-Unix platform - matching `clean`'s existing fail-closed
  posture there.
- E07-S09: closes an E07-S07 round-2 independent verifier review finding - round-1's
  `O_NOFOLLOW` bound only `configure`'s final root component, so a *default* root reached
  through an intermediate symlink (e.g. `$HOME` itself being a link, with a real, non-symlink
  leaf directory underneath it) was still silently followed and written through
  (`docs/architecture/PLATFORM_MODEL.md` "Intermediate components need the same no-follow
  treatment as the leaf"). `cancellai-sealedfs::SealedRoot::establish` now walks every path
  component handle-relatively from the filesystem root, refusing the moment any component -
  intermediate or final - is a symlink/reparse point, and creating only the final absent
  component via `mkdirat` against an already-held parent descriptor. E07-S09's own round-1
  independent verifier review found this closure reached only `configure`: `clean` establishes
  its root through the separate `ApprovedRoot` capability, whose `canonicalize()` step still
  silently resolved through the identical intermediate link, so `clean --yes` could still purge
  a stale session reachable only through a symlinked `$HOME` (`docs/architecture/
  PLATFORM_MODEL.md` "The fix had to reach `clean`, not only `configure`"). Round 2 exports a
  read-only counterpart, `verify_no_intermediate_links`, used by `establish_verified_root`
  before `ApprovedRoot::establish` for the default root. The owner-authorized combined closure
  review found one further race in that handoff: a component could be swapped after the walk
  but before canonicalization. The walk now returns a retained final-directory handle, and
  cleanup refuses unless the subsequently established root has the same device/inode identity.
- E07-S08: `scripts/rust_python_parity.py`'s divergence allow-list is now structured
  (fixture/scenario/field-scoped, citation content-checked) rather than free-text, and its
  comparison surface grew from six to eight fields covering every discovered identity record,
  protection coverage, and root authority for every `NORMATIVE` fixture - closing an E06
  verifier review round 2 finding where any real, accepted ADR citation could suppress an
  unrelated divergence regardless of what it actually authorized
  (`docs/development/MIGRATION_PYTHON_RUST.md` M6).
- E07-S05: closes the intermittent Linux CI failure of `cancellai-platform`'s
  `identity::tests::toctou_file_deleted_and_recreated_with_identical_content_still_changes_
  identity` and `mutation::tests::confirmed_delete_rejects_a_target_already_swapped_before_open`.
  Reproduced natively in a real Linux container (not hypothesized): a zero-delay
  delete-and-recreate reuses the freed inode in ~98% of iterations and lands within the same
  ~1ms mtime clock tick, so `device`+`inode`+`kind`+whole-second-`modified` alone cannot always
  distinguish the two objects - a real `IdentityToken` gap, not only a fixture one.
  `IdentityToken::Unix` gains `modified_nanos` (the raw `st_mtime_nsec` sub-second remainder,
  not derivable from the shared whole-second `Timestamp` clock/retention type);
  `cancellai-platform::mutation`'s `confirmed_delete_file_inner` - which compared device+inode
  only, bypassing `IdentityToken` entirely - now also compares it at both its open-time and
  immediately-before-unlink checks (SI-013/SI-017). The two fixtures also had an
  over-specific/false-on-Linux assertion ("recreation must allocate a new inode") removed and
  gained a small real-world-realistic delay in place of an unrealistic zero-delay recreate,
  without weakening the byte-identical-content case either test verifies. Verified with 60
  consecutive passing runs (30 iterations of both tests) in a real Linux container - exceeding
  the story's own 20-consecutive-run bar.

### Fixed

- E20-S04 (formerly E07-S06): identified why `cancellai-inventory`'s
  `completeness::tests::ac1_a_fully_readable_tree_is_complete` and
  `scan::tests::ac1_one_traversal_visits_every_directory_exactly_once` fail on real Windows CI -
  `scan::walk_directory` only recurses into a child whose identity is *confirmed*
  (`IdentityObservation::Identity`, never `Unsupported`, per SI-017), and
  `SystemIdentityObserver` reports `Unsupported` unconditionally on Windows (E03-S01's
  pre-existing residual), so a real Windows scan currently visits only the scope root -
  correct, safety-driven behavior, not a traversal bug; weakening the identity-confirmed gate
  to make the old assertions pass would have been the wrong fix. Both tests gated `#[cfg(unix)]`
  with `#[cfg(windows)]` counterparts added asserting the actual current behavior
  (`Partial`/`directories_visited == 1`), and `docs/architecture/PLATFORM_MODEL.md` gains an
  "Accepted limitation" subsection - real Windows traversal depth requires E20-S01's native
  identity implementation.
- `cancellai-inventory/tests/performance_micro.rs`'s
  `scan_scope_completes_within_budget_for_a_small_dataset` had the identical E20-S04
  Windows-traversal assumption (an exact `paths_observed` count only reachable with confirmed
  identity) - gated that specific assertion `#[cfg(unix)]`; the time-budget and
  views-do-not-re-walk checks it also makes remain meaningful and run on every platform.
- `cancellai-safety`'s `mutation_executor`/`root_capability`/`sealed_plan` had 19 tests
  (`mutation_executor`'s entire test module, plus `root_capability::tests::bind_a_plain_child_
  succeeds`/`bind_the_root_itself_is_rejected`/`bind_a_path_outside_the_root_is_rejected`, plus
  `sealed_plan::tests::seal_derives_root_and_artifact_identity_from_real_capabilities`) that
  construct a real `ApprovedRoot` via the real `SystemIdentityObserver` and were not
  `#[cfg(unix)]`-gated - real Windows CI failed every one of them with the same
  `CandidateIdentityUnsupported` error (E03-S01's pre-existing residual, unrelated to and
  predating this session). `mutation_executor`'s entire `mod tests` is now `#[cfg(unix)]`
  (every test in it depended on the same real-root helper); the three `root_capability` tests
  and the one `sealed_plan` test are individually gated.
- `cancellai-cli/tests/cli_behavior.rs`'s
  `configure_writes_the_native_claude_retention_setting_and_preserves_other_keys` was not
  `#[cfg(unix)]`-gated, so real Windows CI ran it expecting a successful write - but
  `configure`'s write capability (`SealedRoot`) has no verified handle-relative implementation
  on non-Unix platforms and fails closed there by design (`docs/CLI_RUST.md`'s own "Known
  gaps", unrelated to this session's other changes). Gated the success-path test `#[cfg(unix)]`
  and added a `#[cfg(windows)]` counterpart asserting the disclosed refusal instead, matching
  the existing pattern for the symlinked-`$HOME` configure/clean tests.
- `cancellai-sealedfs` failed to build on Windows: `validate_child_name` and its `CString`
  import lived outside the `#[cfg(unix)]` boundary, so they became genuine dead code once
  `unix_impl` (the only caller) stopped compiling in on non-Unix targets - found on real
  Windows CI while verifying E07-S09, not caught locally since this executor's environment is
  macOS. Both are now `#[cfg(unix)]`-gated with the rest of the module they belong to.
- `cancellai-provider-codex::native_delete`'s `FakeCli`-based tests
  (`ac2_a_fake_cli_advertising_force_is_reported_supported` and three others) intermittently
  failed on Linux CI with `ProbeFailed { reason: "Text file busy (os error 26)" }` - reproduced
  directly in a real Linux container (not hypothesized): writing a fresh script then executing
  it from a highly parallel `cargo test` run can race a *different*, concurrently-forking test
  thread on the process's own shared file-descriptor table. Confirmed serial-safe (0/200
  failures at `--test-threads=1`) and only observed under real concurrency, so the fix is a
  bounded retry-on-`ProbeFailed` in the test harness itself (`codex_delete_supported_retrying`)
  rather than any change to `codex_delete_supported`'s production logic - `ProbeFailed` is
  already the correct, conservative production answer for "could not tell". Verified with 60
  consecutive passing runs at `--test-threads=8` in the same container (previously ~14% flake
  rate).
- `directory_size`/`safe_lstat_size` no longer count a symlink's own `lstat().st_size` toward
  a reported byte total. For a symlink that value is the byte length of the stored target path
  string, not real disk footprint - reporting it as "size" made coverage/size output for any
  entry containing a symlink depend on the absolute path length of wherever the symlink
  happened to live, silently differing by machine and even by which temp-directory prefix a
  test run used. Found via the `codex-symlink-escape`/`claude-symlink-protected-name`
  characterization fixtures diverging between macOS and Linux CI; a symlink already contributes
  nothing to deletion or discovery accounting elsewhere (E00-S02 / ADR-0013) and now
  consistently contributes nothing to size accounting either.

## [1.6.0] - 2026-08-31

### Changed

- Epic E05 implemented the Provider API and Reference Adapters: a nine-capability
  `ProviderCapabilities` contract (`cancellai-provider-api`) where capability absence is
  explicit and never inferred from provider identity, and every response carries evidence and
  confidence by construction; Built-in Verified/Community Verified/Local Custom/Untrusted
  provider trust wired into the Effective Authority lattice as its own constraint, gated by
  `TrustedTier`, an opaque type whose only public constructors are the safe `Untrusted` default
  and a checked, evidence-requiring promotion - closing the `ProviderTrustAuthority` gap
  `docs/architecture/DOMAIN_MODEL.md` had called out since E03-S04; Claude Code and Codex CLI
  reference adapters porting `cancellai.py`'s discovery/classification/session-relationship
  logic to Rust (root fingerprinting, the Unicode-canonical-caseless protected-name barrier,
  session/subagent-graph discovery, native-delete capability detection), each checked against
  the committed Python characterization corpus by reproducing its fixtures directly; and a
  generated, per-capability reference-provider compatibility matrix in `docs/PROVIDERS.md`. An
  independent review round found and this epic's own repair cycle closed a CR4 defect before
  close: the first version of provider trust typed its authority-lattice input as a bare,
  publicly-constructible enum, so an external caller could self-assign the highest trust tier
  with no promotion evidence at all - the exact self-assignment SI-021 prohibits. `cancellai.py`'s
  own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface
  yet.

## [1.5.0] - 2026-08-29

### Changed

- Epic E04 implemented the Single-Pass Inventory Engine: `FileFacts`, a per-path evidence record (`rust/crates/cancellai-inventory`) composed from independently-observed logical size, allocated/physical size (a new `AllocationObserver` platform seam distinguishing sparse/cloned/compressed allocation from logical length), identity, and filesystem-boundary facts, with every unsupported metric an explicit typed value rather than a fabricated zero or borrowed metric; `scan_scope`, a single recursive walk per scope whose status/top-consumers/planning report views are pure reads over one snapshot, never a re-walk, and which never follows a symlink or descends across a device/filesystem boundary (SI-018); scope-level completeness classification (`Complete`/`Partial`/`Unknown` with named permission/I/O/disappearance/unsupported-feature reasons, SI-008/SI-009) that a planning-facing view cannot be obtained without, enforced by construction and by a `compile_fail` regression proving the bare-candidates accessor is unreachable outside the crate; and a performance baseline (a CI microbenchmark plus scheduled 10k/100k/1M-entry benchmarks with a machine-readable trend artifact). An independent review round found and this epic's own repair cycle closed a CR3 defect before close: a `read_dir`-listed entry's unreadable/vanished observation was silently dropped instead of degrading scope completeness, and the bare planning-candidates accessor was reachable without the completeness it should always carry. `cancellai.py`'s own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface yet.

## [1.4.0] - 2026-08-29

### Changed

- Epic E03 implemented the Formal Safety Kernel: cross-platform artifact identity tokens with fail-closed Windows refusal (`rust/crates/cancellai-platform`, SI-013/SI-017); an immutable `SealedPlan` sealed only from a verified root/target capability pair, with fail-closed identity/root revalidation (SI-013/SI-016); typed `ApprovedRoot`/`BoundedPath` root and filesystem-boundary capabilities rejecting root-self deletion, escapes, symlink-escape tricks, and cross-device mounts (SI-002/SI-003/SI-018); a monotonic-minimum Effective Authority lattice with a deterministic explanation trace, collapsing unknown/active/protected/partial state to non-destructive authority (SI-001/SI-007/SI-008/SI-009); and the mutation executor itself - the sole, statically-enforced path to real filesystem deletion, checking root binding, authority, and reversibility before mutating, and confirming a plain file's identity via an open file descriptor immediately around the delete syscall (SI-013/SI-019/SI-020). An independent review round found and this epic's own repair cycle closed three CR4 defects before close: a raw mutation capability bypassing every one of the above checks, a plan executable against a target from a different root, and an executor that never consulted its own recorded authority/reversibility. `cancellai.py`'s own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface yet.

## [1.3.0] - 2026-08-29

### Changed

- Epic E02 bootstrapped the target Rust workspace ahead of the spec-first migration: twelve crates (`docs/architecture/TARGET.md`) with an acyclic dependency graph and no provider-specific code in `cancellai-model`/`cancellai-safety`; a quality baseline enforcing `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, and `cargo deny` (license allow-list, unknown-registry/git denial, MSRV 1.85.0) across macOS/Linux/Windows CI (`rust/deny.toml`, ADR-0015); a typed diagnostic model separating invalid-input/safety-block/incomplete-inventory/compatibility/mutation-failure/internal-fault with stable human/JSON error codes; and deterministic `Clock`/`FsObserver` seams (`rust/crates/cancellai-platform`) that keep the Python reference's absent-vs-unreadable filesystem distinction (SI-008/SI-009/SI-010) as a typed contract, including for a modification time the OS cannot report or represent - never silently substituted with a credible-looking epoch timestamp. `cancellai.py`'s own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface yet.

## [1.2.0] - 2026-08-28

### Changed

- Epic E01 turned the Python v1 CLI into a characterized, versioned executable reference ahead of the Rust migration: canonical domain vocabulary (`docs/architecture/DOMAIN_MODEL.md`), a synthetic Claude/Codex provider-layout fixture corpus (`tests/fixtures/`), versioned inventory/plan/explanation/result JSON contracts with an explicit compatibility policy (`docs/architecture/JSON_CONTRACTS.md`), a committed characterization of Python's actual behavior on that corpus classified normative/intentional-divergence/legacy-only/known-defect (`scripts/characterize.py`), and a differential comparison contract and self-testing harness for the eventual Python-vs-Rust migration gate (`scripts/diff_harness.py`, `docs/development/VERIFICATION_STRATEGY.md`). `cancellai.py`'s own runtime behavior is unchanged.
- `cancellai.py` is now maintenance-only (the Python reference freeze, `AGENTS.md`): only parity fixes against the committed characterization, safety/security fixes, and migration-support tooling are accepted going forward, not merely until this epic closed. New product capability targets the Rust implementation.

## [1.1.0] - 2026-08-28

### Security

- Protected names (`CLAUDE_PROTECTED_NAMES` / `CODEX_PROTECTED_NAMES`) are now an executable barrier instead of documentation. They are enforced when the plan is built and again inside `safe_remove`, immediately before any deletion, so a future discovery change cannot silently invalidate them. Comparison uses the Unicode canonical caseless form (NFD, casefold, NFD): APFS is case-insensitive and stores decomposed filenames, so neither raw string equality nor case folding alone is filename comparison (E00-S01).
- `--aggressive` no longer bypasses the age cutoff for Claude legacy directories and rebuildable cache files. It widens which categories are eligible; retention is applied independently (E00-S03).
- Only the provider's own default directory is mutated. A root relocated with `CODEX_HOME` or `CLAUDE_CONFIG_DIR` is fully inspectable but is never deleted from or written to: nothing observable in a filesystem proves a directory belongs to a provider, so this release refuses to act on structural resemblance. Two weaker schemes were tried and rejected by independent review before this one (E00-S02, ADR-0013 superseding ADR-0012).
- The protected-name barrier is applied to the path as written as well as after resolution, and matches case-insensitively. Previously a protected entry that was itself a symlink lost its protection entirely, and a candidate spelled `Plugins` bypassed the barrier on case-insensitive APFS (E00-S01).
- An unusable process observation is no longer read as "no provider is running". `ps` output that does not contain this process is not a full listing, so a missing, failing, filtered or stubbed enumeration refuses cleanup unless `--allow-running` is given (E00-S09).
- `history.jsonl` is never rewritten through a symlink. `os.replace` would have swapped the link for a regular file and silently detached whatever it pointed at (E00-S06).
- Filesystem observation errors are no longer silently flattened into zero. Every discovery guard goes through an `lstat` that separates "not there" from "could not look" - `Path.exists()` answers False for both, so using it as a guard turned an unreadable directory into an empty one. An unreadable path now withholds destructive authority for that provider, and `status` lists the unreadable paths and prints partial totals as lower bounds (E00-S05).
- Claude `history.jsonl` trimming now streams bytes instead of loading and re-encoding the file, so retained lines - including CRLF endings and a missing trailing newline - are preserved verbatim. It re-identifies the source immediately before the atomic replace and abandons the rewrite if a provider wrote concurrently. Trimming is skipped entirely while a Claude process is running, even under `--allow-running`, and a failed trim is reported instead of looking like "nothing to do" (E00-S06).

### Changed

- **Breaking:** flags without a subcommand no longer normalize to `clean`. `cancellai --days 14` now runs the read-only `status` view; deletion requires typing `clean`. An unrecognized verb is a usage error (E00-S04).
- **Breaking:** a relocated `$CODEX_HOME` / `$CLAUDE_CONFIG_DIR` can no longer be cleaned or configured, only inspected. This is a capability regression, taken deliberately: see ADR-0013. Default roots are unaffected.
- **Breaking:** `clean` exits `3` on mutation failure (previously `2`) and `4` when safety blocked or deferred the requested work. No failure path escapes the taxonomy: an unexpected bug also reports `3` rather than Python's exit code `1`, which automation cannot distinguish from a declined prompt. Exit `2` is now reserved for invalid usage and refused configuration roots. `--json` output carries `exit_code`, `blocked_tools` and `deferred` (E00-S04).
- `status` reads each provider root in a single pass instead of traversing it for the total and again for the largest entries.
- `status --json` and `clean --json` now report per-root `origin`, `confidence`, provider `markers` and `destructive_allowed`, plus a `scan` object and `withheld_tools`.

### Added

- `status --coverage` classifies every top-level provider entry as `selective`, `selective-aggressive`, `aggressive-only`, `trimmed`, `protected`, `reported` or `unknown`, with a legend. There is deliberately no state meaning "deleted as it stands", because no top-level entry is treated that way: `projects/` and `sessions/` are containers whose *contents* are selected by age and policy, and `history.jsonl` is trimmed rather than deleted. Unknown entries are reported so provider layout drift stays visible and are never cleanup candidates. The same classification is exposed in `status --json` (E00-S08).

### Changed

- Added the cancellAI Engineering Operating System (cEOS): product constitution, decision register, target architecture, threat model, safety invariants, evidence-gated development model, Claude/Codex executor-verifier protocol, and machine-readable roadmap/backlog control plane.
- Reframed the long-term product from a macOS Claude/Codex cleanup script to a local-first, cross-platform, provider-agnostic Agent State Control Plane while clearly separating that target from the currently released Python v1 feature set.
- Documented the spec-first Python-to-Rust migration and the P0 trust-floor work that must land before the reference implementation is frozen.
- Required status-check names in branch protection are now verified against the contexts the workflows can actually report. A required check named `test` was blocking every pull request permanently while a matrix produced `test (3.10)` and `test (3.14)`; a name that matches no job never reports and is indistinguishable from a slow check.
- Added governance/document integrity automation, story-specific executor/verifier briefs, CodeQL scanning, CODEOWNERS, incident response, synthetic-fixture policy, and supply-chain-aware CI foundations.
- Bumped the pinned `pytest` development dependency to 9.0.3, closing a Dependabot advisory about vulnerable tmpdir handling. Development tooling only; the shipped tool has no runtime dependencies.
- Replaced automatic Dependabot merge behavior with review-gated dependency updates and pinned first-party GitHub Actions to immutable revisions in active workflows.


## [1.0.2] - 2026-08-27

### Fixed

- `CODEX_PROTECTED_NAMES` now includes `plugins`, matching
  `CLAUDE_PROTECTED_NAMES`. Found by dogfooding against a real `~/.codex`:
  `plugins/` holds genuine installed-plugin state (`plugins/cache`,
  `plugins/.plugin-appserver`), not disposable cache. No code path sweeps
  it today, so this is a defense-in-depth fix, not a behavior change.

## [1.0.1] - 2026-08-27

### Added

- `AGENTS.md` / `CLAUDE.md`: repo-specific instructions for AI coding agents.
- `.github/CONTRIBUTING.md`, `.github/SECURITY.md`, `.github/CODE_OF_CONDUCT.md`,
  issue and pull request templates, and an issue template chooser that
  disables blank issues.
- `docs/ARCHITECTURE.md` and `docs/RELEASING.md`.
- `docs/CLI.md`: a command reference generated directly from the argparse
  definitions by the new `scripts/gen_docs.py`, checked for drift in CI.
- `pyproject.toml` dev-tooling config (`ruff`, `mypy` in strict mode) and a
  matching `.pre-commit-config.yaml`.
- `.editorconfig` and `.github/dependabot.yml` (GitHub Actions ecosystem).
- `.github/workflows/dependabot-auto-merge.yml`: auto-merges Dependabot PRs
  once the required `test`/`lint`/`homebrew` checks pass.
- CI now also runs `ruff check`, `ruff format --check`, `mypy --strict`, and
  the docs-drift check, in addition to the existing test suite.
- Repository hardening: branch protection on `main` (required status
  checks, no force-push/deletion), squash-only merges, Dependabot
  vulnerability alerts + security updates + automated fixes, private
  vulnerability reporting, and repo topics/description for discoverability.

### Changed

- Reorganized repository layout: `test_cancellai.py` moved to
  `tests/test_cancellai.py`; `CONTRIBUTING.md`, `SECURITY.md`, and
  `CODE_OF_CONDUCT.md` moved to `.github/` (a location GitHub recognizes
  natively for these files), decluttering the repo root.
- Modernized type hints to PEP 604 syntax (`X | None` instead of
  `Optional[X]`) and moved `Iterator`/`Sequence` imports to
  `collections.abc`.
- `active_processes()` now resolves `ps` to an absolute path via
  `shutil.which` instead of relying on `$PATH` resolution at call time.
- Replaced an internal `assert` in `delete_codex_via_cli` with an explicit
  `ValueError` guard (assertions can be optimized away with `python -O`;
  this is a real invariant, not a debug check).
- Simplified several `try`/`except ...: pass` blocks to
  `contextlib.suppress(...)`.
- `cancellai.py` is now tracked as executable in git (it has a shebang).

### Fixed

- The `tests` CI job never installed `pytest`, so it failed on every run
  since it was added; every CI job now also invokes tools via
  `python3 -m <tool>` so the installer and the invocation always share the
  same interpreter.
- `.gitignore` now excludes the local `.claude/` session directory so it
  can never end up tracked by accident.

## [1.0.0] - 2026-08-27

Initial public release.

### Added

- Safe cleanup CLI for old Codex CLI and Claude Code session data:
  `status` (read-only, default), `clean` (with dry-run, confirmation
  prompt, age cutoff, and keep-latest safety rail), and `configure` (sets
  Claude Code's own `cleanupPeriodDays`).
- Conservative-by-default safety model: protected name lists for
  auth/config/plugins/skills/memory, symlink-safe deletion, config-root
  validation, running-process detection, and preference for the official
  `codex delete --force` backend over raw filesystem deletion.
- MIT license, README, and a Homebrew formula (`Formula/cancellai.rb`) so
  the tool installs via `brew tap matteo-dritara/cancellai && brew install
  cancellai`.

[Unreleased]: https://github.com/matteo-dritara/homebrew-cancellai/compare/v1.0.2...HEAD
[1.0.2]: https://github.com/matteo-dritara/homebrew-cancellai/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/matteo-dritara/homebrew-cancellai/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/matteo-dritara/homebrew-cancellai/releases/tag/v1.0.0

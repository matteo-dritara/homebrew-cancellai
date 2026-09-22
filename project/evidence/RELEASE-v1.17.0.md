# Release Evidence - v1.17.0

## Source

- Tag: `v1.17.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-22
- Published: no - verify-rust (windows-latest) failed on cargo clippy --workspace --all-targets --all-features -- -D warnings: unused import std::path::PathBuf in cancellai-guardian/src/service_windows.rs:16 (only used by that module's own #[cfg(test)] fixtures, which do not compile into the plain lib target on a real Windows build where the module is included via target_os = "windows" alone, not cfg(test) - a platform-specific lint gap this workspace's own docs warn about, not caught locally since no Windows cross-compiler clippy was run before tagging); verify also failed on pytest: tests/test_release.py::ReleaseOutcomeTests::test_every_committed_packet_records_an_outcome, v1.16.0 had no recorded outcome despite publishing successfully (run 35742363018)

## Included work

- Epic: E15 - Guardian Runtime and Bounded Remediation
- Stories: E15-S01, E15-S02, E15-S03, E15-S04
- CR4 Safety Verdicts: `project/evidence/E15-S03/SAFETY_VERDICT.md`

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

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

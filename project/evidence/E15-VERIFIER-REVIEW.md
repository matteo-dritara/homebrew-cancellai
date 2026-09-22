# E15 Verifier Review - Round 1

- Review-Scope: epic
- Round: 1
- Review target: `b9409e7..dae7c32` (`dae7c32` at review start)
- Verifier: Codex (GPT-5)
- Date: 2026-09-22

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E15-S01 | FAIL | `cargo test --workspace` fails on the required real macOS smoke: `service_macos::tests::real_launchd_install_enable_status_disable_uninstall_smoke_test` calls `enable`, then observes `Disabled` rather than `Enabled` (`service_macos.rs:420`). Production `uninstall` additionally calls `std::fs::remove_file` in `service_macos.rs:130` and `service_linux.rs:147`, outside the SI-019 safety executor. `check_mutation_boundary.py` exited 0 only because its production-text splitter stops at the first `#[cfg(test)]` (lines 31/35), before both deletion calls; direct inspection confirmed the calls are not scanned. |
| E15-S02 | PASS | `NotificationKind` has only closed enum/count payloads, `render` uses fixed templates, and `Notifier::notify` returns only `Delivered` or `Fallback`; no authority/action type or escalation route is imported. `cargo test -p cancellai-guardian notification::tests -- --nocapture` passed all seven tests, including fallback and privacy rendering checks. |
| E15-S03 | FAIL | The trusted-input 4 x 5 matrix has the intended `min` behavior and cannot emit `Delete`, but its public `RemediationCandidate` lets any external caller fabricate `reachable_authority`. An external temporary integration probe constructed an `Autopilot` candidate without a `ClassifiedArtifact`; RED returned `Quarantine`/`Quarantine`. This violates AC1 and SI-028 (and undermines SI-027 for actual-policy inputs). See `project/evidence/E15-S03/SAFETY_VERDICT.md`. |
| E15-S04 | FAIL | `KillSwitch::is_engaged` uses `content.trim() != "disengaged"` (`killswitch.rs:99-103`), although the contract permits only content exactly equal to `"disengaged"` to disengage. A temporary adversarial test wrote `"disengaged\\n"` and asserted the switch stayed engaged; it failed, demonstrating that unrecognized modified marker content enables future autonomous actions. The temporary probe was removed after reproduction. |

## Required repairs

### E15-S01

- Repair the macOS lifecycle adapter so a successful `enable` is followed by an accurately
  observable `Enabled` state, or report a real unavailable launchd user domain as
  `Unsupported`/an error rather than returning a false `Disabled`. Restore a passing real macOS
  install/enable/status/disable/uninstall smoke test. This violates AC1 and its required
  platform-service smoke verification.
- Eliminate direct production `std::fs::remove_file` from the Guardian service adapters by
  routing service-definition deletion through an approved, reviewed mutation boundary (or obtain
  an explicit architectural/invariant decision for a different bounded primitive). Repair
  `scripts/check_mutation_boundary.py` so an early test-only cfg helper cannot hide following
  production text. This violates C-07/SI-019's one mutation boundary.

### E15-S03

- Make a remediation candidate's authority provenance non-forgeable: the planner must consume
  the actual shared-engine `ClassifiedArtifact` at its boundary, or an opaque equivalent that
  only that pipeline can mint. A plain public `AuthorityLevel` field cannot represent the claim
  that it is already-computed Effective Policy. Add an external-crate regression for the forged
  candidate and rerun the exhaustive matrix. This violates AC1, SI-027, and SI-028.

### E15-S04

- Change the marker comparison to accept only exact `"disengaged"` content; whitespace,
  malformed bytes/content, and every read failure must remain engaged. Add the failed
  newline-marker adversarial case permanently. This violates AC1 and C-03's ambiguity-never-
  escalates rule.

## Gate status

| Command actually run | Result |
| --- | --- |
| `python3 scripts/project_os.py check` (baseline) | PASS |
| `python3 scripts/project_os.py review` and four verifier briefs | PASS; all E15 stories were `ready_for_review` before review |
| `python3 scripts/check_agent_toolchain.py report` | PASS; no decision past review date |
| `gh run list --branch main --limit 5` | PASS; five latest main workflows were successful |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | FAIL; one macOS Guardian launchd smoke test failed, 144 Guardian tests passed |
| `cargo deny check` | PASS (pre-existing warnings only; advisories/bans/licenses/sources OK) |
| `python3 scripts/check_mutation_boundary.py check` | Exit 0, but NOT valid evidence for E15: source inspection reproduced a false negative caused by its first-`#[cfg(test)]` truncation |
| Targeted remediation, kill-switch, notification, and service tests | PASS before adversarial probes; insufficient to falsify the defects above |

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/BACKLOG.md`
- `docs/PRODUCT.md`
- `docs/PLATFORMS.md`
- `docs/architecture/GUARDIAN_MODEL.md`
- `docs/architecture/PLATFORM_MODEL.md`
- `docs/architecture/PERSISTENCE_MODEL.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/development/WORK_ITEM_MODEL.md`
- `docs/development/RELEASE_GATES.md`
- `docs/development/AGENT_TOOLCHAIN.md`
- `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`
- `project/epics/E15.json`
- `project/templates/SAFETY_VERDICT.md`
- `project/evidence/E15-S03/EVIDENCE.md`
- `project/evidence/E15-S04/EVIDENCE.md`

## Overall verdict

`REJECT` — three of four judged stories have findings (75% yield), above ADR-0025's 10% threshold.
Round 2 is required after the listed repairs and is the owner-capped final review round. No story
is eligible for epic closure or release in this round.

Status disposition: E15-S01 is `in_progress`; E15-S02, E15-S03, and E15-S04 are `blocked`.
E15-S02's verdict is a pass, but it cannot move to `done` while its same-epic predecessor is
`in_progress`; the control plane instead requires that dependent to be blocked. E15-S03 and
E15-S04 have their own failures as well and remain blocked behind E15-S01's repair, preserving
the dependency graph rather than recording a false completed state.

## Method defect

`scripts/check_mutation_boundary.py` treats every line after a file's first `#[cfg(test)]` as
test code. `service_macos.rs` and `service_linux.rs` place a test-only `for_test` helper before
production methods, so their direct `remove_file` calls are invisible to the SI-019 gate. The
finding and required repair are recorded above; no tool change was made during this review.

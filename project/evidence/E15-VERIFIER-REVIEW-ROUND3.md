# E15 Verifier Review - Owner-Authorized Targeted Confirmation

- Review scope: E15-S01 and E15-S03 round-2 repairs; E15-S02/E15-S04 dependency spot-checks
- Review target: `b9409e7..a72570c`, plus this reviewer closure/status commit
- Verifier: Codex (GPT-5)
- Date: 2026-09-22

## Per-story verdicts

| Story | Verdict | Concrete independent evidence |
| --- | --- | --- |
| E15-S01 | PASS_WITH_RESIDUALS | `LaunchdRuntime::enable` runs `launchctl load` then its own `confirm_registered` loop before its immediately adjacent `Ok(())` return; there is no intervening operation that can make a successful registration check read as a successful enable. Failure returns `ServiceError::CommandFailed` with the required literal `registration could not be confirmed`. Five exact real smoke invocations passed. Each took 2.04–2.10 seconds, which is the two-second confirmation bound and therefore identifies the accepted honest-fallback branch in this environment, rather than a genuine `Enabled` branch. The test catches only that literal error, cleans up, and otherwise panics. All ten `service_macos::tests::*` passed. The fallback is truthful and satisfies AC1 here: no `Ok` is returned for an unconfirmable registration; it is preferable to the prior contradictory success. |
| E15-S02 | PASS | No S02 source changed in `b9409e7..a72570c`. Workspace tests include the notification privacy and terminal-fallback tests; they pass. Its previous PASS verdict therefore remains valid. |
| E15-S03 | PASS_WITH_RESIDUALS | `RemediationCandidate`, `GuardianPlanItem`, and `plan_remediation` are `pub(crate)`, as are `killswitch::apply_kill_switch` and `audit::record_guardian_decision`; no public re-export exists. A separate external Cargo crate fails E0603 at importing `plan_remediation` and `RemediationCandidate`, before it can fabricate either carrier. The guardian doc-test run has only the unrelated structural doctest. All nine internal remediation tests pass, including `from_classified_artifact_carries_the_real_engine_authority_unmodified`. The prior forgeability of public `ClassifiedArtifact` is moot to this module because no external caller can reach its conversion. The appended round-3 CR4 Safety Verdict accepts this crate-local, primitive-only scope and records the requirement for an opaque upstream carrier before any future public exposure. |
| E15-S04 | PASS_WITH_RESIDUALS | The only S04-source changes are visibility narrowing of `apply_kill_switch` and `record_guardian_decision`, required because their `GuardianPlanItem` parameter is crate-private. Their bodies and tests are unchanged; workspace tests pass. The prior primitive-only/no-live-orchestrator residual remains documented, but no new behavior invalidates its verdict. |

## Targeted repair evidence

### E15-S01 — truthful launchd enable

- Inspected `service_macos.rs`: `confirm_registered` invokes the same `launchctl list <label>`
  status mechanism that `status()` uses, retries for at most two seconds, and `enable()` returns
  `Ok` only in its `true` branch. An unconfirmed registration cannot be reported as success.
- Exact real smoke command, five independent runs:

  `cargo test -p cancellai-guardian --lib service_macos::tests::real_launchd_install_enable_status_disable_uninstall_smoke_test -- --exact`

  All five passed (run durations: 2.10s, 2.07s, 2.06s, 2.07s, 2.04s). The timing is consistent
  with the expected accepted honest-fallback outcome; no unexpected-error panic occurred.
- `cargo test -p cancellai-guardian --lib service_macos::` passed 10/10 tests.

### E15-S03 — inaccessible remediation authority path

- Inspected `remediation.rs`, `killswitch.rs`, and `audit.rs`: all five named items are genuinely
  `pub(crate)`, not `pub`; `lib.rs` has no re-export that widens them.
- The external privacy probe's `cargo check --offline` failed with E0603: private function
  `plan_remediation` and private struct `RemediationCandidate`. It did not fail merely on a field
  constructor, so the former `ClassifiedArtifact` fabrication attack cannot reach this module.
- `cargo test --doc -p cancellai-guardian` passed and ran one unrelated structural doctest only.
- `cargo test -p cancellai-guardian --lib remediation::tests::` passed 9/9 tests, including the
  real-engine conversion test.

## Gate status

| Command actually run | Result |
| --- | --- |
| `python3 scripts/project_os.py check`, `status`, `next`, `review`; four verifier briefs | PASS; all E15 stories were `ready_for_review` before this verdict |
| `python3 scripts/check_agent_toolchain.py report` | PASS; no component is past its review date |
| `gh run list --branch main --limit 5` | PASS; all five listed `main` workflow runs succeeded |
| `git diff --check b9409e7..a72570c` | PASS |
| Five exact macOS launchd smoke runs | PASS; all accepted honest-fallback tests, no unexpected failure |
| `cargo test -p cancellai-guardian --lib service_macos::` | PASS (10 tests) |
| `cargo test --doc -p cancellai-guardian` | PASS (one unrelated structural doctest) |
| External Cargo privacy probe | PASS; E0603 visibility failure for both inaccessible names |
| `cargo test -p cancellai-guardian --lib remediation::tests::` | PASS (9 tests) |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS |
| `cargo deny check` | PASS; existing unmatched-license/duplicate warnings only |
| `python3 scripts/check_mutation_boundary.py check` | PASS |

## Documents opened

- `AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`
- `project/epics/E15.json`; all four E15 verifier briefs; `docs/development/ENGINEERING_SYSTEM.md`;
  `docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`;
  `docs/development/RELEASE_GATES.md`; `docs/development/AGENT_TOOLCHAIN.md`; `docs/RELEASING.md`
- `docs/architecture/GUARDIAN_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`;
  `docs/security/THREAT_MODEL.md`
- `project/evidence/E15-VERIFIER-REVIEW-ROUND2.md`; `project/evidence/E15-S03/SAFETY_VERDICT.md`;
  E15-S01 and E15-S03 executor evidence
- Guardian service/remediation/kill-switch/audit sources and `lib.rs`; the final repair diff
- `project/epics/E16.json` and its prior independent review, after closing E15 exposed that E16's
  sole declared epic dependency had become satisfied

## Overall verdict

`ACCEPT` — all four stories are `PASS` or `PASS_WITH_RESIDUALS`. The specific E15-S01 failure
mode is now an explicit error, never a contradictory success, and the E15-S03 external authority
fabrication route is closed at the crate boundary. E15 is eligible for closure and the required
minor release.

Closing E15 also resolves E16's sole declared epic dependency. E16 is moved from `blocked` to
`ready_for_review`, not closed: its already-completed work needs a separately authorized epic
closure/release decision and is not silently folded into this E15 release.

The same transition made E17-S07's prior `blocked_by.waiting_on: E15` stale. That satisfied wait
is removed and its summary now names the remaining E16 closure and documented operability gaps;
E17-S07 remains blocked.

## Release follow-through

`python3 scripts/release.py prepare --version 1.17.0 --epic E15` passed and produced the version
bump, cut changelog, and `RELEASE-v1.17.0.md`; `python3 scripts/release.py check` then confirmed
the prepared in-flight state. The reviewed preparation is committed as `chore(release): 1.17.0`,
then tagged `v1.17.0` and pushed with `--follow-tags`. `scripts/release.py finalize` is
intentionally deferred until the tag's CI-built archive exists, as the release runbook requires.

# E15 Verifier Review - Round 2

- Review-Scope: epic
- Round: 2 (owner-capped final round)
- Review target: `b9409e7..006c0ae` (`006c0ae` at review start)
- Verifier: Codex (GPT-5)
- Date: 2026-09-22

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E15-S01 | FAIL | Round-1 F1's SI-019 repair is effective: neither production macOS/Linux adapter calls a `remove_*` primitive; `uninstall` empties the definition and 0-byte definitions are `NotInstalled`. The repaired real macOS smoke nevertheless failed on **all five** independent repetitions after its 2-second poll: `enable()` succeeded but `poll_status_until(... Enabled)` returned `Disabled` at `service_macos.rs:461`. This violates AC1 and the required platform install/uninstall smoke. An empty file from a crash is conservative (`NotInstalled`); a nonempty partial file cannot enable successfully as a valid definition and a later `install` overwrites it, so this did not add a further authority gap. |
| E15-S02 | PASS | Spot check remains valid: `NotificationKind` admits only closed enums/counts, rendering uses fixed templates, and `Notifier::notify` can only return `Delivered` or `Fallback`; it imports no authority/action type. No repair altered this surface. Status is blocked only by failed E15-S01. |
| E15-S03 | FAIL | The direct struct-literal attack is now rejected (external compiler E0451; inspected `compile_fail` doctest passes), but the claimed provenance repair is incomplete. An external probe fabricated public `cancellai_policy::ClassifiedArtifact`/`AgentArtifact` values with `reachable_authority: Autopilot`, used `RemediationCandidate::from`, and got a RED `Quarantine` plan. This is a live SI-027/SI-028 and AC1/AC2 breach; see the appended round-2 Safety Verdict. |
| E15-S04 | PASS_WITH_RESIDUALS | Round-1 F1 is closed: `is_engaged` compares raw content to `"disengaged"`. Independent external probes with `"disengaged\\0"`, whitespace-surrounded content, and existing empty content all read engaged; absence remains the intentional disengaged default. The module still supplies audit/kill-switch primitives rather than a live sealed-plan/execution orchestrator, as declared by the architecture; no autonomous execution exists in this scope. Status is blocked by E15-S03. |

## Round-1 repair verification

### E15-S01 F1 - service-definition deletion / SI-019

- Inspected production regions of `service_macos.rs` and `service_linux.rs`: neither contains
  `std::fs::remove_file`, `remove_dir`, or `remove_dir_all`; their trailing test modules still
  remove only synthetic temporary directories, which is exempt.
- Both `uninstall()` implementations now write `""`; both `is_installed()` functions require
  metadata length greater than zero, so an empty definition is `NotInstalled` and `enable()`
  refuses it. A partial nonzero write is not silently considered enabled: `status()` checks the
  actual service mechanism and `enable()` must load the resulting definition; reinstall retries
  by overwriting the file. This is operationally imperfect (writes are not atomic) but does not
  create deletion or elevated authority.
- Added then removed an independent source probe containing a `#[cfg(test)] fn helper()` before a
  real production `std::fs::remove_file`, followed by a real `#[cfg(test)] mod tests`. The fixed
  checker rejected it at the production line. The repair closes round 1's false negative.
- The executor's broader claim is overstated: comparison against the former first-`#[cfg(test)]`
  split showed 104 files scanned, 9 widened, 95 unchanged, and none narrowed—not a strict
  widening on all 104. This wording error does not affect the repaired detection result.

### E15-S01 F2 - macOS lifecycle race

- Ran `cargo test -p cancellai-guardian --lib
  service_macos::tests::real_launchd_install_enable_status_disable_uninstall_smoke_test -- --exact`
  five times. Every run failed after about two seconds with `left: Disabled`, `right: Enabled`.
  The poll has not made service state truthful or the mandated smoke reliable.

### E15-S03 F1 - forgeable remediation candidate

- Verified the direct candidate literal is no longer compilable and that the relevant doctest is
  a real private-field construction test, not a vacuous compilation failure.
- Independently disproved the claimed closed boundary with a full external policy-artifact
  fabrication. `ClassifiedArtifact` remains public with public authority-relevant fields; its
  `From` implementation is therefore still a forgeable authorization input. This cannot be
  accepted as a CR4 residual because it directly negates SI-028.

### E15-S04 F1 - kill-switch exact marker comparison

- Verified raw `content != DISENGAGED_MARKER` comparison and independently tested null-bearing,
  whitespace-only/surrounded, and empty present markers. All fail safe to engaged; no modified
  marker content slipped through as disengaged.

## Gate status

| Command actually run | Result |
| --- | --- |
| `python3 scripts/project_os.py check` (baseline) | PASS |
| `python3 scripts/project_os.py review` and four verifier briefs | PASS; all E15 stories were `ready_for_review` before review |
| `python3 scripts/check_agent_toolchain.py report` | PASS; no component is past review date |
| `gh run list --branch main --limit 5` | UNKNOWN; GitHub API DNS/network access failed, so CI is not treated as green |
| `git diff --check b9409e7..006c0ae` | PASS |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | FAIL; 146 Guardian tests passed and the real macOS launchd smoke failed |
| `cargo deny check` | PASS; existing unmatched-license/duplicate warnings only; advisories, bans, licenses, and sources passed |
| `python3 scripts/check_mutation_boundary.py check` | PASS; 104 Rust source files scanned |
| Five exact macOS smoke repetitions | FAIL, 5/5 `Disabled` after successful `enable()` and poll |
| `cargo test --doc -p cancellai-guardian` | PASS; the candidate private-field `compile_fail` doctest and structural doctest passed |
| Independent external candidate probes | Direct literal correctly FAILS E0451; fabricated public `ClassifiedArtifact` correctly compiles/runs and reproduces the authority bypass |

## Documents opened

- `AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`
- `project/epics/E15.json` and all four verifier briefs
- `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md` (Verifier procedure); `docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`; `project/templates/SAFETY_VERDICT.md`
- `docs/architecture/GUARDIAN_MODEL.md`; `docs/architecture/PLATFORM_MODEL.md`; `docs/architecture/PERSISTENCE_MODEL.md`; `docs/PLATFORMS.md`; `docs/PRODUCT.md`
- `docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`
- `project/evidence/E15-VERIFIER-REVIEW.md`; `project/evidence/E15-S03/SAFETY_VERDICT.md`; executor evidence for E15-S01, E15-S03, and E15-S04
- Final diff and Guardian service/remediation/kill-switch/audit/notification sources; policy retention and ledger sources; `scripts/check_mutation_boundary.py`

## Overall verdict

`REJECT` — two of four stories remain defective in the owner-capped final round (50% measured
yield). E15-S01 returns to `in_progress`; E15-S03 is `blocked` by E15-S01 while also carrying
its own failed CR4 determination; E15-S02 and E15-S04 are `blocked` by their dependencies. The
epic remains `in_progress` and no release closure is permitted.

Accepted-to-record capped-round residuals requiring owner decision:

- **E15-S01:** do not ship or wire the macOS user-service capability until its successful enable
  produces a truthful enabled status (or an unavailable user domain is reported explicitly as
  unsupported/error). The required platform smoke is presently 0/5.
- **E15-S03:** do not wire the planner to execution. Its authority source must be redesigned as a
  policy-owned opaque, pipeline-minted carrier; the current public `ClassifiedArtifact` route
  lets an external caller forge pre-authorized quarantine. This is a live SI-027/SI-028 failure,
  not a passable CR4 residual.

No third verification round is requested: the owner capped this epic at two. The owner must
decide whether to fund the required policy/service repairs in a new tracked item or accept that
E15 remains unshippable without the affected capabilities.

# E07 Epic Verifier Review - Round 1

- Epic: E07 - Unix Cross-Platform Hardening
- Review target: `f9db57e..HEAD` on `e07-closure-release-1.7.0` (the final squash merge/tag provides the immutable endpoint)
- Verifier/executor: Codex (`/root`)
- Date: 2026-09-02
- Process exception: **Owner-authorized combined verify+fix+close round, 2026-09-02 - see conversation record.** The owner explicitly authorized combined independent verification, repair, self-reverification, CR4 verdict authorship, and direct closure for E07-S01/E07-S05/E07-S09 and E20-S04 only.

## Verdicts

| Story | Risk | Verdict | Independent result |
| --- | --- | --- | --- |
| E07-S01 | CR3 | `PASS_WITH_RESIDUALS` | Current macOS/Linux identity, allocation, process, path/filesystem, mutation, and sealed-root behavior is capability-scoped and unsupported facts lower authority. Notification/user-service seams remain deliberately deferred until their Guardian consumers exist. |
| E07-S05 | CR3 | `PASS_WITH_RESIDUALS` | Linux inode reuse and whole-second timestamp collision were real; `modified_nanos` is propagated into identity and confirmed deletion. Both named tests passed 20 consecutive local runs each. Residual: a replacement inside the filesystem's underlying timestamp tick can still collide. |
| E07-S09 | CR4 | `PASS_WITH_RESIDUALS` | Static intermediate links and non-Unix unsupported states fail closed. One additional cleanup handoff TOCTOU was found, fixed with a retained no-follow directory handle plus native identity match, and independently regression-tested. See `E07-S09/SAFETY_VERDICT.md`. |

E07-S07 was cancelled/superseded and E07-S08 was already done; neither was re-reviewed.

## Falsification and findings

The review started from story ACs, SI-002/SI-003/SI-013/SI-017/SI-019, TM-02/TM-03, and the `f9db57e..HEAD` diff, treating executor packets as claims. It inspected static and raced links, identity reuse, missing/unsupported observations, mount/device checks, boundary paths, configuration-write atomicity, cleanup binding, cross-target cfg behavior, and test gating.

One real CR4 defect was found: `verify_no_intermediate_links` returned and dropped its final descriptor before the separate path-based `ApprovedRoot::establish`. An intermediate component could be swapped in that interval, so “immediately before” was still check-then-use. The repair returns a `VerifiedPath` retaining the final descriptor; cleanup compares its device/inode with the canonicalized approved root and refuses mismatch. The deterministic regression `verified_path_detects_a_component_swapped_after_the_walk` swaps an intermediate component after verification and proves the replacement cannot inherit authority.

Control-plane generation then found E07 still depended at epic level on blocked E06, recreating
the operational cycle already documented in `E07-S01/DEPENDENCY_ESCALATION.md`: E06-S04 is a
cutover gate blocked on later platform/packaging capabilities, while E07 is one of the hardening
epics needed before cutover. The stale epic edge was removed and its rationale added to E07's
objective; individual story dependencies continue to express the actual prerequisites.

No code defect was found in E07-S01. E07-S05's remaining sub-clock-tick collision is real and explicit rather than silently claimed closed; eliminating it requires a different persistent handle/generation identity design, not another timestamp field. It remains bounded by the existing immediate revalidation/open checks and is recorded as a residual.

## Gates run by this reviewer

| Gate | Result |
| --- | --- |
| `cargo fmt --check` | PASS after formatting the verifier repair |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS |
| `cargo deny check` | PASS; only the three pre-existing unmatched BSD-2-Clause/BSD-3-Clause/ISC allowance warnings |
| `cargo check --workspace --all-targets --target x86_64-unknown-linux-gnu` | PASS |
| `cargo check --workspace --all-targets --target x86_64-pc-windows-gnu` | PASS |
| E07-S05 named identity test, 20 consecutive runs | PASS |
| E07-S05 named confirmed-delete test, 20 consecutive runs | PASS |
| E07-S09 targeted sealedfs + end-to-end cleanup tests | PASS |

The full Python and Rust release gate suites and the PR CI matrix are run again after control plane generation and release preparation; those results are recorded in `RELEASE-v1.7.0.md`.

## Closure

Under the owner-authorized process exception, E07-S01, E07-S05, and E07-S09 move directly to `done`; E07 itself moves to `done` because every remaining story is done or intentionally cancelled/superseded. Closing the epic triggers release v1.7.0 under ADR-0014.

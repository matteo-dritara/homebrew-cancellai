# Safety Verdict - E12-S01

- Change: Quarantine store semantics, round-6 independent verification of recovery-scope reduction
- Risk: CR4
- Review target: uncommitted working-tree changes on `82d30c1..f8dc4cc` (main)
- Independent verifier: Codex
- Verifier: Codex
- Date: 2026-09-16
- Brief-Checksum: eae5c25c2224707972f6731bae2035dcd9f16e291ce9bd7e5f41c76e3ecce734

Brief-Checksum: eae5c25c2224707972f6731bae2035dcd9f16e291ce9bd7e5f41c76e3ecce734
Verifier: Codex

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

The automated pending-sidecar scanner and identity witness are removed. The retained protocol writes the full contentless restore record durably to `.pending` before the no-clobber move, then finalizes its name after it.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-018 | Quarantine is same-volume and never copied. | Identity-confirmed handle-relative no-copy move tests, direct no-clobber probe, and full Rust suite pass. | PASS |
| SI-020 | No unverified automatic cleanup represents or authorizes irreversible action. | No scanner caller remains; failed finalize returns `Err` and leaves correct content pending rather than claiming completion. | PASS_WITH_RESIDUALS |

## Adversarial cases

- Forced pending temporary-name write failure: source stayed intact and destination absent.
- Pre-created move destination: source and destination stayed intact and pending record was cleaned up.
- Blocked finalization with a final-name directory: call returned `Err`, artifact remained intact, exact record remained under `.pending`.
- Repository-wide removal search found no executable `recover_pending_moves`, `PendingRecoveryOutcome`, `move-identity`, or identity-witness implementation/caller.

## Differential / compatibility evidence

`pytest` (665 passed, 547 subtests), Ruff, mypy, every AGENTS.md checker, Rust fmt, native and Windows/Linux-target Clippy, workspace check/test, and `cargo deny check` pass. Windows refuses quarantine explicitly.

## Known residual risks

- Post-move finalization failure leaves the correct restore record under `.pending`; no code completes it automatically. A human must inspect and resolve an obstruction before completing the rename. Future automation needs its own independently verified story.

## Rollback / recovery

Pre-move write or move failure leaves no moved-but-unrecorded object. Post-move finalization is deliberately manual-only; the pending record is durable recovery evidence.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS` — acceptable only for this unwired primitive, never as automatic-recovery or purge authority.

# Safety Verdict - E03-S01

- Change: Artifact identity tokens
- Risk: CR4
- Commit/PR: `df91ff9..f2a4080562410ec49673de7d1c21e1364a30bc0c`
- Independent verifier: Codex
- Date: 2026-08-29

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Introduces typed filesystem identity observation used to detect plan/target drift.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 | Identity can detect ordinary replacement. | Real Unix file-to-directory, directory-to-symlink, symlink-to-file, and recreate tests pass; verifier exercised revalidation at the executor boundary. | PASS |
| SI-017 | Unsupported platform identity lowers authority. | `SystemIdentityObserver` returns `Unsupported` off Unix; root binding and revalidation reject it. | PASS |

## Adversarial cases

- Verified replacement detection for files, directories, symlinks, and synthetic device changes.
- Unreadable, absent, and unsupported observations are all non-proceed outcomes in revalidation.

## Differential / compatibility evidence

- Rust fmt, clippy, check, test, and cargo-deny gates pass locally on macOS.

## Known residual risks

- Native Windows volume/file-index/reparse identity is deliberately unsupported, so destructive authority fails closed there. Windows CI implementation/verification remains required before enabling it.
- Observation is not atomic with a later path mutation; E03-S05 must repair that separate execution-boundary race before the epic can close.

## Rollback / recovery

No standalone mutation is performed by this story. Revert the identity seam if it incorrectly reports a supported identity state; unsupported remains fail-closed.

## Owner decision

`PENDING OWNER ACCEPTANCE` — verifier recommendation: accept with the recorded residuals.

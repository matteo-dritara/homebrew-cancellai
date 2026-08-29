# Safety Verdict - E03-S03

- Change: Root and boundary capabilities
- Risk: CR4
- Commit/PR: `d85ebda..f2a4080562410ec49673de7d1c21e1364a30bc0c`
- Independent verifier: Codex
- Date: 2026-08-29

## Verdict

`FAIL`

## Safety surface changed

Adds `ApprovedRoot` and `BoundedPath` for root, symlink, and device-boundary checks.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-002 | Mutation is under a validated root capability. | `ApprovedRoot::bind` checks this correctly, but a public raw mutation executor can be called without any root capability. | FAIL |
| SI-003 | Mutation cannot escape/delete the approved root. | `BoundedPath` is bypassable because `SystemMutationExecutor::mutate(&Path, ...)` is public and accepts an unconstrained raw path. | FAIL |
| SI-018 | Filesystem boundaries are explicit. | Bound-path device checks pass; raw mutation bypass means they are not universal. | FAIL |

## Adversarial cases

- A separate external consumer test created a temporary file and called `SystemMutationExecutor.mutate(&raw_path, DeleteFile)` directly. It deleted the file without `ApprovedRoot`, `BoundedPath`, `SealedPlan`, or safety executor.

## Differential / compatibility evidence

- Rust and repository gates pass; the static boundary script only looks for direct `std::fs::remove_*` calls and cannot see this public capability bypass.

## Known residual risks

- This is a blocking API bypass, not an accepted residual.

## Rollback / recovery

Do not expose the raw destructive executor to non-safety clients; its current API admits unrecoverable deletion of arbitrary paths.

## Owner decision

`REJECT`

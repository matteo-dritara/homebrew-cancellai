# Safety Verdict - E03-S05

- Change: Mutation executor isolation
- Risk: CR4
- Commit/PR: `f2a4080562410ec49673de7d1c21e1364a30bc0c`
- Independent verifier: Codex
- Date: 2026-08-29

## Verdict

`FAIL`

## Safety surface changed

Adds the public path from a sealed plan to filesystem deletion and the platform mutation seam.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 | Revalidation prevents mutation of a replacement. | A verifier observer performed a real rename-and-replace immediately after observing the planned identity; `execute` then called `SystemMutationExecutor` and deleted the replacement. | FAIL |
| SI-019 | All mutation routes through the safety executor. | An external consumer directly invokes the publicly exported `SystemMutationExecutor` with a raw path. The text scanner cannot detect trait-method calls. | FAIL |
| SI-020 | Irreversible delete is explicit and stronger-gated. | `execute` performs `ActionClass::Delete` even when the plan has `AuthorityLevel::Observe` and `Reversibility::Quarantinable`. | FAIL |

## Adversarial cases

- Direct raw-path delete outside the kernel succeeds.
- `Delete` + `Observe` + `Quarantinable` produces `ActionResult::Succeeded` with a matching synthetic identity.
- A plan for `root-a` executes against a separately bounded target under `root-b`.
- Swapping the target after identity observation and before `remove_file` deletes the replacement, proving the check/use race.

## Differential / compatibility evidence

- Rust and repository gates pass, demonstrating the test-suite/static-check gap rather than a build failure.

## Known residual risks

- These are blocking CR4 defects, not accepted residuals.

## Rollback / recovery

Do not expose or use this mutation path for destructive operations until its authority, root, and atomic TOCTOU guarantees are repaired.

## Owner decision

`REJECT`

# Safety Verdict - E03-S04

- Change: Action authority lattice
- Risk: CR4
- Commit/PR: `683be22..f2a4080562410ec49673de7d1c21e1364a30bc0c`
- Independent verifier: Codex
- Date: 2026-08-29

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Introduces a deterministic named-constraint minimum for effective authority.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-001 | Protected/unknown facts do not grant destructive authority. | Table tests and direct inspection show protected, pinned, active, unknown, partial, corrupt, and low/unknown inputs cap at `Recommend`. | PASS |
| SI-007 | User authority cannot independently elevate the result. | Minimum includes user authority and artifact ceiling; raising the former cannot exceed the latter. | PASS |
| SI-008 | Partial integrity is non-destructive. | `lifecycle_ceiling` returns `Recommend` for `Partial`. | PASS |
| SI-009 | Unknown activity/integrity is non-destructive. | `lifecycle_ceiling` returns `Recommend` for each unknown state. | PASS |

## Adversarial cases

- Repeated calculations preserve ordered trace output and report all tied constraints.
- Empty constraints fail closed to `Observe`.

## Differential / compatibility evidence

- Rust fmt, clippy, check, test, and cargo-deny pass locally on macOS.

## Known residual risks

- E03-S05 does not consume this result: a public plan can carry `Observe` authority with `Delete` and still be executed. This cross-story execution defect blocks the epic, but the lattice computation itself meets this story's contract.

## Rollback / recovery

The lattice performs no mutation. Do not enable destructive execution until it is made a required executor precondition.

## Owner decision

`PENDING OWNER ACCEPTANCE` — verifier recommendation: accept with the recorded residuals.

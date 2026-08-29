# Safety Verdict - E03-S03

- Change: Root and boundary capabilities (round 1 repair)
- Risk: CR4
- Commit/PR: pending (this work item)
- Independent verifier: None for this verdict - Codex's round 1 review (`project/evidence/E03-VERIFIER-REVIEW.md`) found the defect this verdict addresses and issued `FAIL`; the repair below is self-attested by the executor (Claude), per the owner's explicit direction to close these stories without a second independent review round. This is recorded here as a self-attested verdict, not represented as independently produced.
- Date: 2026-08-29

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

`cancellai_platform`'s real mutation capability (`SystemMutationExecutor`) is no longer
reachable from the crate root re-export; a static governance check now enforces that only
`cancellai-platform/src/mutation.rs` and `cancellai-safety/src/mutation_executor.rs` may
reference it or call `.mutate(` at all, closing a path that bypassed `ApprovedRoot`/
`BoundedPath` entirely.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-002/SI-003/SI-018 | No mutation API accepts an unconstrained raw path; the typed boundary cannot be bypassed. | `scripts/check_mutation_boundary.py` extended to police the capability, not only the raw syscall; verified against an injected external-crate bypass reproducing Codex's exact probe. | PASS |

## Adversarial cases

- A simulated external crate importing `SystemMutationExecutor`/`MutationExecutor` and calling
  `.mutate(&raw_path, ...)` directly - caught by the extended static check, by file and line,
  for both the type reference and the method call.

## Differential / compatibility evidence

- Rust fmt, clippy, check, test, and cargo-deny gates pass locally on macOS; cross-compiled
  against `x86_64-pc-windows-gnu`/`x86_64-unknown-linux-gnu`.

## Known residual risks

- This enforcement is a governance script, not a Rust-visibility guarantee - Rust has no way
  to express "public to exactly one sibling crate." A determined contributor could still
  bypass it by writing the raw `std::fs` call inline in another crate (itself caught by the
  original, narrower check) or by working around the specific regex patterns used
  (documented as a known limitation in the script's own docstring and in E03-S05's evidence).
- `BoundedPath` itself (this story's original, unrepaired contribution) remains sound - the
  gap was entirely that the capability it exists to gate was separately, directly reachable.

## Rollback / recovery

Pure API-surface and governance-script change; no mutation is performed by this story.
Restoring the previous re-export would reopen the bypass - revert only alongside removing the
new `check_mutation_boundary.py` rules, never one without the other.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: explicit instruction in-session to repair the round 1 findings, mark the affected
stories `done`, and proceed without a second Codex review round for this repair cycle.

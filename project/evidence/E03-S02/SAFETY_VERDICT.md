# Safety Verdict - E03-S02

- Change: Sealed cleanup plan (round 1 repair)
- Risk: CR4
- Commit/PR: pending (this work item)
- Independent verifier: None for this verdict - Codex's round 1 review (`project/evidence/E03-VERIFIER-REVIEW.md`) found the defect this verdict addresses and issued `FAIL`; the repair below is self-attested by the executor (Claude), per the owner's explicit direction to close these stories without a second independent review round. This is recorded here as a self-attested verdict, not represented as independently produced.
- Date: 2026-08-29

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

`SealedPlan`'s only public constructor now derives `root_identity`/`artifact_identity` from a
real `ApprovedRoot`/`BoundedPath` pair instead of accepting independent caller-supplied
values; `mutation_executor::execute` (E03-S05) enforces the resulting root-identity match at
execution time.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 | Identity (including root identity) is revalidated immediately before mutation. | `mutation_executor::tests::e03_verifier_round1_plan_for_one_root_cannot_execute_against_a_different_root` reproduces Codex's exact counterexample and asserts `SafelyBlocked`. | PASS |
| SI-016 | Mutations require a sealed plan carrying root identity, not a caller-fabricated claim. | `SealedPlan::seal` derives `root_identity` only from a real `ApprovedRoot`; `sealed_plan::tests::seal_derives_root_and_artifact_identity_from_real_capabilities`. | PASS |

## Adversarial cases

- Two real `ApprovedRoot`s established over two different directories; a plan sealed claiming
  root A's identity, executed against a target bound under root B - refused.

## Differential / compatibility evidence

- Rust fmt, clippy, check, test, and cargo-deny gates pass locally on macOS; cross-compiled
  against `x86_64-pc-windows-gnu`/`x86_64-unknown-linux-gnu`.

## Known residual risks

- `SealedPlan` does not yet carry policy explanation or provider capability (SI-016's full
  field list) - no policy engine or provider-adapter subsystem exists yet to supply them; this
  is a documented scope boundary (`sealed_plan.rs`'s own module docs,
  `docs/security/SAFETY_INVARIANTS.md`), not a silent gap.
- `SealedPlan::new` remains `pub(crate)` for within-crate test convenience; nothing outside
  `cancellai-safety` can reach it, verified by normal Rust crate-visibility rules (not a
  governance-script-dependent check, unlike the mutation-capability boundary).

## Rollback / recovery

No standalone mutation is performed by this story in isolation; `SealedPlan`'s shape change is
a pure-data/API change. Revert `seal`/`root_identity` if a defect is found; the check this
verdict addresses would then need to move back into `mutation_executor::execute` alone
(E03-S05's own repair still independently enforces the root match at execution time).

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: explicit instruction in-session to repair the round 1 findings, mark the affected
stories `done`, and proceed without a second Codex review round for this repair cycle.

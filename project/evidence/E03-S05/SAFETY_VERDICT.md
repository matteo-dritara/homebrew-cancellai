# Safety Verdict - E03-S05

- Change: Mutation executor isolation (round 1 repair)
- Risk: CR4
- Commit/PR: pending (this work item)
- Independent verifier: None for this verdict - Codex's round 1 review (`project/evidence/E03-VERIFIER-REVIEW.md`) found the three defects this verdict addresses and issued `FAIL`; the repairs below are self-attested by the executor (Claude), per the owner's explicit direction to close these stories without a second independent review round. This is recorded here as a self-attested verdict, not represented as independently produced.
- Date: 2026-08-29

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

`mutation_executor::execute` now checks root binding and authority/reversibility before ever
observing or mutating a target; `MutationExecutor::mutate` now performs an OS-level
identity-confirmed deletion for plain files (open + fresh pre-unlink recheck + post-unlink
link-count corroboration) instead of a bare path-based `remove_file`; the raw capability is no
longer reachable outside the safety kernel's own two files (see E03-S03's verdict).

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 | Identity revalidated immediately before mutation; a crafted swap-on-observe cannot produce a false `Succeeded`. | `mutation::tests::confirmed_delete_detects_a_target_swapped_between_open_and_unlink` and `confirmed_delete_rejects_a_target_already_swapped_before_open` reproduce the round 1 race at the OS-call layer and assert the replacement survives. | PASS |
| SI-019 | All filesystem mutations route through the safety executor. | `scripts/check_mutation_boundary.py`, both the raw-syscall and capability-reference checks. | PASS |
| SI-020 | Irreversible actions are explicitly, strongly gated - not disguised as weaker cleanup. | `mutation_executor::tests::e03_verifier_round1_observe_authority_cannot_execute_a_delete` and `execute_blocks_delete_claiming_quarantinable_reversibility_even_with_sufficient_authority` reproduce the round 1 counterexample (`Observe` authority, `Quarantinable` reversibility, `Delete` class) and assert the target survives. | PASS |

## Adversarial cases

- Plan with `AuthorityLevel::Observe` + `ActionClass::Delete` - refused, target survives.
- Plan with `Reversibility::Quarantinable` + `ActionClass::Delete` at full authority - refused,
  target survives.
- Plan sealed for root A executed against a target bound under root B - refused (see E03-S02's
  verdict for the root-binding half of this fix).
- A target swapped for a different object immediately before the confirmed-delete's open call
  - the replacement survives, the call fails closed.
- A target swapped for a different object *between* the confirmed-delete's open-time check and
  its unlink call (the exact round 1 construction) - the replacement survives, the call fails
  closed. A first implementation attempt (after-the-fact link-count check only) failed this
  exact test during development; fixed by adding the pre-unlink fresh recheck (see E03-S05's
  `EVIDENCE.md` "Verification Commands" for the failure and fix).
- Directory/symlink targets reaching `execute` with `ActionClass::Delete` - refused outright
  rather than deleted without the file-only confirmation guarantee.

## Differential / compatibility evidence

- Rust fmt, clippy, check, test, and cargo-deny gates pass locally on macOS; cross-compiled
  against `x86_64-pc-windows-gnu`/`x86_64-unknown-linux-gnu` (the confirmed-delete path is
  `#[cfg(unix)]`-gated; unreachable off-Unix in practice since `IdentityObserver` reports
  `Unsupported` there, E03-S01).

## Known residual risks

- The pre-unlink-recheck-to-unlink gap is narrowed but not perfectly closed - true prevention
  needs an OS-specific handle-relative unlink (`openat`/`unlinkat` with `O_NOFOLLOW`) via
  `unsafe` (forbidden by default, ADR-0015) or a reviewed dependency (`rustix`/`nix`, not
  present in this workspace). This is the one explicitly-acknowledged remaining gap in this
  story's SI-013 enforcement; recommending a dedicated follow-up story to close it, not
  claiming it is already closed.
- `ActionClass::Quarantine`/`ActionClass::Archive` remain refused, not implemented -
  `SealedPlan` carries no quarantine destination yet.
- The mutation-capability governance check (`scripts/check_mutation_boundary.py`) is a static
  script, not a Rust-visibility guarantee - see E03-S03's verdict for that residual risk in
  full.

## Rollback / recovery

Revert `mutation.rs`/`mutation_executor.rs` to the pre-repair versions to remove the stronger
checks; this reopens all three round 1 findings and must not be done selectively (e.g.
reverting only the confirmed-delete change while keeping the authority check active would
still leave SI-013's race open). No irreversible action has been taken by this repair itself -
these are code/logic changes, not data mutations.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: explicit instruction in-session to repair the round 1 findings, mark the affected
stories `done`, and proceed without a second Codex review round for this repair cycle.

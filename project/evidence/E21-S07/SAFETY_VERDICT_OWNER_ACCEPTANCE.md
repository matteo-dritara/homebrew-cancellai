# Safety Verdict - E21-S07 (owner acceptance)

- Change: handle-relative confirmed file deletion
- Risk: CR4
- Decided by: **project owner**, not an independent verifier
- Date: 2026-09-03
- Independent review history: round 1 (`SAFETY_VERDICT.md`, Codex) returned
  `PASS_WITH_RESIDUALS` on all four invariants, with the `fstatat`/`unlinkat` window confirmed
  as accurately documented rather than overclaimed.

## What this file is, and is not

Unlike E21-S03's, this file does not overturn anything: the independent verdict already passed.
It exists because the story was blocked by its failed dependency (E21-S01) and because closing a
CR4 story requires an owner-visible Safety Verdict recording a pass. The independent verdict is
committed beside it and stands on its own. See `project/evidence/E21-CLOSURE.md`.

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Unix confirmed deletion binds the target's parent with a component-by-component `O_NOFOLLOW`
walk and issues `fstatat(..., AT_SYMLINK_NOFOLLOW)` plus `unlinkat` against that retained
descriptor. The two unconfirmed `MutationOperation` variants were removed.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 | A path-component swap cannot redirect deletion | Independently confirmed in round 1 via `a_symlinked_intermediate_component_refuses_the_delete`, `the_unlink_refuses_a_name_that_no_longer_holds_the_confirmed_inode`, and the pre-existing before/open and mid-flight swap tests | PASS |
| SI-016 | Only a sealed, revalidated plan reaches mutation | Independently confirmed; no alternate mutation caller introduced | PASS |
| SI-019 | The removal primitive stays confined to the mutation boundary | `check_mutation_boundary.py` passes; one permitted file, `unsafe` in one crate | PASS |
| SI-020 | Irreversible deletion remains explicit and strongly gated | `MutationOperation` has a single variant; authority and reversibility tests pass | PASS |

## Adversarial cases

As recorded in the independent verdict, and unchanged by this round: symlinked intermediate
component refused; symlinked `$HOME` already non-destructive under E07-S09; relocated non-symlink
home unaffected; a Unix bind mount is not a reparse traversal and is not newly refused; the held
descriptor keeps naming the original directory after a rename; non-Unix fails closed.

## Differential / compatibility evidence

Re-run after E21's round-1 repairs, which touched neither this story's code nor its tests:
`cargo test --workspace` 327 passed, clippy `-D warnings` clean, `cargo deny check` clean,
mutation-boundary checker clean, parity gate 12 fixtures in both scenarios.

## Known residual risks

1. **The `fstatat`/`unlinkat` window remains open by construction.** POSIX has no "unlink only if
   this name still points at this inode" primitive. What is closed is the directory being
   swapped; what remains is an attacker with write access to that specific directory replacing
   the entry between the two syscalls. The mutation seam's held file descriptor and post-unlink
   link-count check still *detect* such a swap. Documented in ADR-0017 rather than claimed closed.
2. **A provider root reached through a symlinked path component can no longer be cleaned.** A
   deliberate, user-visible consequence, consistent with E07-S09, documented in
   `docs/architecture/PLATFORM_MODEL.md`.
3. Non-Unix platforms cannot delete at all, unchanged.

## Owner decision

Accepted as `PASS_WITH_RESIDUALS`, matching the independent verdict. No residual here is an
unresolved HIGH/CRITICAL safety risk; residual 1 is narrower than the surface that existed
before this story, not wider.

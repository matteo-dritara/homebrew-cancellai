# Safety Verdict - E21-S07

- Change: handle-relative confirmed file deletion
- Risk: CR4
- Review target: working tree against `c00f16f56534651e304c12c5040303984317ac3d` (the requested `c00f16f..HEAD` range is empty; the implementation was uncommitted)
- Independent verifier: Codex (`/root`)
- Date: 2026-09-03

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Unix confirmed deletion binds the target parent with a component-by-component
`O_NOFOLLOW` walk and invokes `fstatat(..., AT_SYMLINK_NOFOLLOW)` plus `unlinkat` relative to
that retained descriptor. The unconfirmed quarantine and recursive-directory variants were
removed.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 | A path-component swap cannot redirect deletion. | `a_symlinked_intermediate_component_refuses_the_delete`, `the_unlink_refuses_a_name_that_no_longer_holds_the_confirmed_inode`, and the existing before/open/mid-flight tests passed in `cargo test --workspace`. | PASS |
| SI-016 | Only a sealed, revalidated plan reaches mutation. | Existing safety executor tests passed; no alternate mutation caller was introduced. | PASS |
| SI-019 | The removal primitive remains confined to the mutation boundary. | `python3 scripts/check_mutation_boundary.py check` passed; it found the sole raw removal seam and the two allowed capability references. | PASS |
| SI-020 | Irreversible deletion remains explicit and strongly gated. | `MutationOperation` has only `DeleteFile`; safety tests for authority and reversibility passed. | PASS |

## Adversarial cases

- A symlinked intermediate component is refused; this is correct and already required by the
  E07-S09 provider-root rule. A symlinked `$HOME` therefore remains non-destructive, a relocated
  non-symlink home works normally, and a bind mount is not a symlink/reparse traversal and is
  not newly refused on Unix.
- The held descriptor continues to name the original directory after a path rename; the
  handle-relative operation therefore cannot be redirected through the original pathname.
- Non-Unix remains explicitly unsupported/fail-closed.

## Differential / compatibility evidence

`cargo fmt --check`, clippy with `-D warnings`, `cargo check`, `cargo test --workspace`,
`cargo deny check`, and the mutation-boundary checker passed in this review.

## Known residual risks

The ADR accurately describes the remaining POSIX window: an attacker able to write the already
held directory can replace the entry between `fstatat` and `unlinkat`. The operation cannot then
escape the held directory; the retained file descriptor's post-unlink link-count check detects
that the intended entry was not removed, but cannot undo removal of a replacement. This is a
smaller, explicitly disclosed residual, not a claim of full compare-and-unlink atomicity.

## Rollback / recovery

Reverting this change restores the broader path-resolution race and must not be done as a safety
rollback. The code change itself performed no provider mutation.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: the story is technically verified, but is blocked from closure by failed dependency
E21-S01; the overall epic remains open.

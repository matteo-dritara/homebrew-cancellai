# Safety Verdict - E07-S09

- Change: Provider-root intermediate-link containment
- Risk: CR4
- Review target: `f9db57e..HEAD` on `e07-closure-release-1.7.0` (final squash merge/tag identify the immutable endpoint)
- Verifier/executor: Codex (`/root`)
- Date: 2026-09-02
- Process exception: **Owner-authorized combined verify+fix+close round, 2026-09-02 - see conversation record.** The owner explicitly authorized Codex to act as verifier and executor, repair findings, re-verify, write this CR4 verdict, and close the named story for this round only.

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Unix provider-root establishment now refuses every symlinked path component. Configuration
operations remain handle-relative for their full lifetime. Cleanup performs the same no-follow
walk and, after the verifier repair in this round, retains the final directory descriptor until
`ApprovedRoot::establish` completes and requires its canonicalized device/inode identity to
match the held handle. Non-Unix implementations remain explicitly unsupported/fail-closed.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-002 | Mutation has a positively bounded provider root. | Static intermediate-link CLI fixtures; `verified_path_detects_a_component_swapped_after_the_walk`; held-handle identity comparison in `establish_verified_root`. | PASS |
| SI-003 | Mutation cannot escape through root link indirection. | Configure and clean outside sentinels remain unchanged; post-walk replacement identity is rejected. | PASS |
| SI-013 | Root identity is revalidated at the authority handoff. | The no-follow-opened directory descriptor survives canonicalization and must match the established root's device/inode. | PASS |
| SI-019 | No alternate artifact mutation path is introduced. | `python3 scripts/check_mutation_boundary.py check`; cleanup still routes deletion through `cancellai-safety`. | PASS |

## Adversarial cases

- Intermediate Unix symlink at `$HOME`: configuration and cleanup refuse; outside settings and stale-session sentinels remain untouched.
- Final-component link: refused by `O_NOFOLLOW`.
- Component swapped after the no-follow walk but before canonicalization: the retained handle and replacement identity differ, so authority is refused. This is the defect found and fixed during this combined round.
- Missing root: verification creates nothing and downstream establishment refuses absence.
- Relative and `.`/`..` paths: refused.
- Concurrent leaf creation during `mkdirat`: `EEXIST` returns to the same no-follow open.
- Windows/non-Unix: `Unsupported`; no junction/reparse mutation capability is claimed.

## Differential / compatibility evidence

- Local macOS: full Rust fmt/clippy/check/test/deny suite passed after the repair.
- Cross-target compilation passed for `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu`.
- The PR CI matrix must pass on macOS/Linux/Windows before squash merge; its results become release evidence, not a substitute for this local adversarial review.

## Known residual risks

- Windows native junction/reparse containment is not implemented; both cleanup identity and sealed-root establishment fail closed until E20-S01 supplies verified native semantics.
- Unix artifact unlink still has the previously documented final recheck-to-`unlink` race in `cancellai-platform::mutation`; it is not created or widened by E07-S09.
- E07-S05 documents the filesystem-clock-granularity limit of metadata identity tokens. This review found no route by which either residual turns an intermediate-link root into authority.

No unresolved HIGH/CRITICAL E07-S09-specific residual remains.

## Rollback / recovery

Revert the E07-S09 cleanup/SealedRoot changes to return to fail-closed/non-release state; do not restore the former check-only handoff. The change introduces no persisted format or migration. All adversarial tests use synthetic temporary roots and touched no user/provider data.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: The conversation authorizes Codex to complete this verdict and close the story for this round. Residuals above retain lower authority and are already assigned to explicit future platform/safety work.

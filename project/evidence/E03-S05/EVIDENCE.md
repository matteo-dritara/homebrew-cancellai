# Evidence Packet - E03-S05

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (round 1: FAIL, `project/evidence/E03-VERIFIER-REVIEW.md`)
- Change Risk: CR4
- Spec version/commit: `rust/crates/cancellai-platform/src/mutation.rs`, `rust/crates/cancellai-platform/src/lib.rs`, `rust/crates/cancellai-safety/src/mutation_executor.rs`, `scripts/check_mutation_boundary.py` as added/repaired in this change

## Outcome

PASS (after round 1 repair)

## This is the first story to perform a real filesystem mutation

Every prior E03 story (S01-S04) was read-only or pure computation. E03-S05 adds the first
code in this workspace that actually calls `std::fs::remove_file`/`remove_dir_all`/`rename` -
`cancellai-platform`'s new `MutationExecutor` seam (`mutation.rs`), mirroring the
`Clock`/`FsObserver`/`IdentityObserver`/`PathResolver` pattern the prior four stories
established: a real, OS-backed `SystemMutationExecutor` and a test-only
`SyntheticMutationExecutor` for fault injection. `cancellai-safety`'s new
`mutation_executor.rs` (`execute`/`execute_all`) is the orchestration that composes
everything the epic built - `SealedPlan`/`revalidate` (E03-S02), `BoundedPath` (E03-S03),
`effective_authority`/`minimum_authority_for`/`reversibility_allowed` (E03-S04) - into the one
call path from an approved plan to that capability.

## Round 1 repair - three independent defects Codex's review found

Codex's independent review (round 1, `project/evidence/E03-VERIFIER-REVIEW.md`) found `execute`
performed a real, irreversible deletion in three situations it should have refused:

1. **Authority/reversibility never checked at all.** A plan with `ActionClass::Delete`,
   `AuthorityLevel::Observe` (the *weakest* level), and `Reversibility::Quarantinable` (not
   even claiming to be irreversible) executed successfully. `execute` consulted neither
   `plan.authority()` nor `plan.reversibility()` before mutating.
2. **Root binding never checked.** See E03-S02's evidence for the full repair; `execute` is
   the enforcement point - it now refuses unless `plan.root_identity() == target.root_identity()`
   for the target *actually passed to `execute`*.
3. **Path-based revalidate-then-delete left an exploitable race.** A crafted `IdentityObserver`
   that answers "still matches" and then, as a side effect of that same call, swaps the
   object before the real `remove_file` runs, caused the *replacement* to be deleted while
   `execute` reported `Succeeded`. No amount of moving the revalidation call closer to the
   mutation defeats this specific construction, since the adversarial observer's swap is
   timed to land immediately after it answers, regardless of the gap.

Repair, in the same order:

1. `authority.rs` gained `minimum_authority_for(ActionClass) -> AuthorityLevel`
   (`Delete` requires `Govern`, strictly above `Quarantine`/`Archive`'s requirement - SI-020:
   irreversible actions are stronger-gated) and `reversibility_allowed(ActionClass,
   Reversibility) -> bool` (a plan claiming `Delete` must record `Reversibility::Irreversible`,
   not merely something weaker). `execute` checks both before ever observing the target's
   current state, let alone mutating it.
2. Repaired jointly with E03-S02 (see that story's evidence) - `execute` now compares
   `plan.root_identity()` against `target.root_identity()`.
3. `cancellai-platform::mutation::MutationExecutor::mutate` now takes the plan's expected
   `IdentityToken` and, for a plain file, performs three checks around one held file
   descriptor: open the target and confirm the descriptor's own device/inode match expected;
   immediately before the actual unlink, an independent fresh path lookup re-confirms the
   path still resolves to that identity (the check that actually stops a same-named
   replacement from being deleted - a bare *after-the-fact* link-count check alone cannot
   distinguish "my own unlink zeroed the original" from "a concurrent unlink already zeroed it
   before I touched a different object," and a first implementation attempt that only had the
   after-the-fact check was caught failing exactly this way during development, see
   "Verification Commands" below); then, after the unlink, re-stat the same descriptor as
   final corroboration. This narrows, but - being safe-Rust and dependency-free, per this
   workspace's `unsafe_code = "forbid"` (ADR-0015) and no new dependency having been reviewed
   for an `openat`/`unlinkat`-based approach - does not perfectly close, the remaining gap
   between the pre-unlink re-check and the unlink syscall itself. Where the guarantee cannot
   be established (directories, symlinks - `File::open` would follow a symlink rather than
   operate on it, and the technique does not generalize to a recursive tree), `execute` now
   refuses rather than deletes with a weaker guarantee: `delete_operation_for` returns `None`
   for anything but `FileKind::File`.

A related, independent defect Codex filed under E03-S03 (raw mutation capability bypassing
`BoundedPath` entirely) is also repaired in these same files - see E03-S03's evidence for the
AC-level writeup; summarized here since the fix lives in `mutation.rs`/`lib.rs`/
`check_mutation_boundary.py`:

- `cancellai_platform`'s crate root no longer re-exports `SystemMutationExecutor` (or the
  other `mutation` module items).
- `scripts/check_mutation_boundary.py` was extended to forbid referencing
  `SystemMutationExecutor` or calling `.mutate(` anywhere outside `mutation.rs` and
  `mutation_executor.rs`, in addition to its original raw-syscall check. Verified it both
  passes today and catches Codex's exact reproduction (an external-crate import-and-call),
  restoring the probed file afterward with no residual change (`git status` confirmed clean).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Provider/UI crates cannot directly unlink/rmtree/delete | `scripts/check_mutation_boundary.py` now enforces this at two layers: the raw `std::fs::remove_file`/`remove_dir`/`remove_dir_all` primitive is allowed only in `mutation.rs`, and the capability wrapping it (`SystemMutationExecutor`, `.mutate(`) is allowed only in `mutation.rs`/`mutation_executor.rs` - closing the round 1 gap where the capability itself, not just the raw syscall, was reachable from any crate. Verified against both an injected raw-syscall violation and an injected capability-reference violation, each caught by file/line, each cleaned up afterward. | PASS |
| AC2 - Executor revalidates all plan preconditions immediately before mutation | `execute` checks root binding, then authority/reversibility, then calls `observer.observe`/`revalidate` (E03-S02, reused) - all *before* ever constructing a `MutationOperation` or calling `executor.mutate`. `execute_never_calls_mutate_on_a_stale_plan` proves the OS call is never reached for a stale plan (a `SyntheticMutationExecutor` configured to fail loudly is never triggered). `mutate` itself (E03-S05's platform-layer repair) adds a second, OS-level revalidation immediately around the unlink syscall - see "Round 1 repair" above. | PASS |
| AC3 - Partial failures produce explicit per-action results and never hide skipped work | `execute_all` is `plans.iter().map(execute).collect()` - structurally incapable of skipping an element or stopping after the first failure. `execute_all_never_short_circuits_and_never_drops_a_result` drives three plans (one succeeds, one is blocked as stale, one fails) through one call and asserts exactly three results, each matching its corresponding fault. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 (one mutation boundary) | Any crate other than the two allowed files referencing the raw primitive or the capability wrapping it | `scripts/check_mutation_boundary.py`, both layers (see AC1) | PASS |
| SI-013 (revalidate immediately before mutation; fail closed on drift) | Identity changed since planning; artifact became absent; a crafted observer swaps the object as a side effect of answering "still matches" | `execute_blocks_a_stale_plan_instead_of_mutating`, `execute_never_calls_mutate_on_a_stale_plan`, and (at the OS-call layer) `mutation::tests::confirmed_delete_detects_a_target_swapped_between_open_and_unlink`/`confirmed_delete_rejects_a_target_already_swapped_before_open` | PASS |
| SI-020 (irreversible actions are explicit, not disguised) | `ActionClass::Delete` with `AuthorityLevel::Observe` and `Reversibility::Quarantinable` (the round 1 review's exact reproduction) | `mutation_executor::tests::e03_verifier_round1_observe_authority_cannot_execute_a_delete`, `execute_blocks_delete_claiming_quarantinable_reversibility_even_with_sufficient_authority` - both assert the target file survives, not merely that the call returns a non-success status | PASS |
| SI-002/SI-003/SI-018 via root binding | A plan sealed against one root's fingerprint, executed against a target bound under a different root | `mutation_executor::tests::e03_verifier_round1_plan_for_one_root_cannot_execute_against_a_different_root` (the round 1 review's exact reproduction) | PASS |
| Not vacuously fail-closed | A genuinely valid, sufficiently-authorized plan against a genuinely unchanged, real file | `execute_deletes_when_identity_still_matches`, `end_to_end_real_delete_through_the_full_stack_including_authority_and_root_checks` (real `SystemIdentityObserver` + real `SystemMutationExecutor`, no synthetic doubles) | PASS |
| Kinds this executor refuses rather than weakly-delete | A directory target reaching `execute` with `ActionClass::Delete` | `execute_refuses_directory_deletion_rather_than_delete_without_the_stronger_guarantee` - asserts the directory survives | PASS |

## Verification Commands

```text
# Python governance (repository-wide)
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py scripts/diff_harness.py scripts/check_rust_workspace.py scripts/check_mutation_boundary.py
python3 scripts/check_docs.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/project_os.py check

# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# Cross-platform compile verification (see E03-S01's evidence for why)
cargo check --target x86_64-pc-windows-gnu --all-targets
cargo check --target x86_64-unknown-linux-gnu --all-targets
cargo clippy --target x86_64-pc-windows-gnu --all-targets --all-features -- -D warnings
```

All passed. `cargo test -p cancellai-platform` runs 25 unit tests (23 prior + 2 new
`mutation` swap-detection tests). `cargo test -p cancellai-safety` runs 47 (37 prior + 10 new:
6 in `authority.rs` for the round-1 counterexamples, 4 in `mutation_executor.rs` for
root/authority/reversibility). `scripts/check_mutation_boundary.py` passes with both layers
active.

Two real bugs surfaced and were fixed during development, both worth recording:

- **A logic flaw in the first version of the post-deletion confirmation.** The initial
  implementation checked only the held file descriptor's link count *after* calling
  `remove_file`, reasoning that a swap would leave the original's link count nonzero. A test
  reproducing the exact round-1 race (`confirmed_delete_detects_a_target_swapped_between_open_and_unlink`)
  failed: the *hook* simulating the concurrent swap itself removed the original (dropping its
  link count to zero as a side effect of the swap, not of this code's own unlink call), so the
  after-the-fact check could not distinguish "I deleted the right thing" from "someone else
  already deleted the right thing before I deleted something else." Fixed by adding a second,
  independent, fresh path lookup immediately before the unlink itself (see "Round 1 repair"
  above) - this is the check that actually prevents the wrong object from being deleted, not
  merely one that detects it afterward. Rerun clean.
- **Test-infrastructure directory-name collisions** (already fixed and recorded before round 1
  review; unaffected by this repair, mentioned here only because the same `TempDir` test
  helper pattern is used again in `mutation.rs`'s own new tests, this time correctly using a
  per-call atomic counter from the start).

## Compatibility

- `MutationExecutor`'s production implementation (`SystemMutationExecutor`) uses
  `std::fs::remove_file`/`remove_dir_all`/`rename`/`File::open`/`fstat`-via-`Metadata`,
  available identically on macOS, Linux, and Windows at the API level; the confirmed-delete
  path is `#[cfg(unix)]`-gated because it uses `std::os::unix::fs::MetadataExt` for
  `dev()`/`ino()`/`nlink()` - on non-Unix targets `confirmed_delete_file` returns `Err`
  unconditionally (`#[cfg(not(unix))]`), which is unreachable in practice anyway since
  `execute` never reaches the point of building a real `BoundedPath` off-Unix (E03-S01's
  `Unsupported` identity posture).

## Performance / operability

- `execute` performs, in order: two field comparisons (root, authority/reversibility), one
  `observe` call, and (on the `Proceed` path) one `mutate` call - the latter now three syscalls
  (`open`+`fstat`, `symlink_metadata`, `remove_file`, `fstat`) instead of one, a deliberate
  cost for the stronger guarantee.

## Documentation updated

- `docs/architecture/TARGET.md` - "Forbidden dependency direction" section states the E03-S05
  mutation-boundary implementation, the E03-S02 dependency-check change (documented
  retroactively here since it was missed at the time), and (round 1 repair) the
  capability-reference bypass and its fix (the story's declared documentation impact).
- `docs/architecture/PLATFORM_MODEL.md` - "Boundary rules" section gained the round 1 repair
  note (E03-S03's declared documentation impact, since the defect was filed against that
  story's ACs too).
- `docs/security/SAFETY_INVARIANTS.md` - SI-013, SI-019, and SI-020 each gained or updated an
  implementation pointer reflecting the round 1 repairs.

## Residual risks

- `ActionClass::Quarantine`/`ActionClass::Archive` are refused outright by `execute`, not
  implemented - `SealedPlan` (E03-S02) does not carry a quarantine destination, and this
  story does not invent one. `MutationExecutor::mutate`'s `Quarantine` operation exists
  (`SystemMutationExecutor` handles it via `std::fs::rename`, unconfirmed) but has no
  production caller; a future story adding a destination field to `SealedPlan` is what wires
  `execute`'s `ActionClass::Quarantine` arm to it, and should extend the confirmation
  technique to it too rather than renaming blindly.
- The pre-unlink-to-unlink gap in `confirmed_delete_file` is narrowed, not eliminated (see
  "Round 1 repair" above) - true prevention needs an OS-specific handle-relative unlink
  (`openat`/`unlinkat` with `O_NOFOLLOW`) via `unsafe` or a reviewed dependency (`rustix`/
  `nix`), neither of which exists in this workspace today. This is now the one remaining,
  explicitly-acknowledged gap in SI-013's enforcement for file deletion; directories/symlinks
  are refused entirely rather than carrying this same narrowed-but-open gap silently.
  Recommending a dedicated follow-up story to add a reviewed syscall dependency, not creating
  that scope unilaterally now.
- `ActionResult::Succeeded` does not carry `observed reclaimed bytes` or post-action
  reconciliation state (`docs/architecture/DOMAIN_MODEL.md`'s `Results` section) - no
  inventory/reconciliation subsystem exists yet (E04) to supply either.
- `pre-commit` itself is not installed in this environment; the mutation-boundary hook was
  verified by running the underlying command directly and validating
  `.pre-commit-config.yaml`'s YAML syntax, not by an actual `pre-commit run`. CI runs the full
  hook set and will exercise it for real.

## Verifier verdict

Round 1 (Codex, independent): FAIL - see "Round 1 repair" above and
`project/evidence/E03-VERIFIER-REVIEW.md`.

Round 2: not run. Per explicit owner direction, the three findings above were repaired and
the story moved directly to `done` without a second independent verification pass. This is a
self-attested repair, not an independently re-verified one - recorded here rather than
silently presented as re-verified.

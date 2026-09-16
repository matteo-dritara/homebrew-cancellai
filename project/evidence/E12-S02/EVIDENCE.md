# Evidence Packet - E12-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E12 epic review
- Change Risk: CR4
- Spec version/commit: `docs/security/THREAT_MODEL.md` "TM-14 Restore overwrites new provider
  state"; E12-S01's own quarantine mechanism, reused in reverse

## Outcome

PASS

## Scope

Adds `ActionClass::Restore` - the reverse of `Quarantine`, at the same authority/reversibility
floor - and a `cancellai_platform::mutation::MutationOperation::Restore` that reuses E12-S01's
identity-confirmed, no-clobber move primitive (extracted into a shared `confirmed_move_inner`)
without writing any sidecar into the destination, since the destination here is the artifact's
original provider location, not cancellAI's own quarantine store. `SealedPlan::seal_restore` and
`mutation_executor::execute`'s new `Restore` arm reuse the exact same explicit same-device
boundary check E12-S01 built (SI-018), factored into a shared `destination_for` helper.
`ApprovedRoot::prepare_destination`'s return type is renamed `QuarantineDestination` ->
`MoveDestination`, since a not-yet-existing named location under an `ApprovedRoot` is now a
capability both directions need, not one specific to quarantine.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Restore never overwrites unrelated current provider state silently." | The restore destination is prepared via `ApprovedRoot::prepare_destination`, which refuses if anything already exists there (checked once at prepare-time via `IdentityObserver`), and the move itself now performs one atomic no-replace rename at the OS level (`renameat2`/`RENAME_NOREPLACE` on Linux, `renameatx_np`/`RENAME_EXCL` on macOS - round-1 repair, see below) rather than a separate absence check followed by a plain rename - never a silent overwrite, including in the window between an application-level check and the move. `confirmed_restore_move_refuses_when_the_original_destination_was_recreated` proves the exact TM-14 reproduction (destination recreated before the call); `confirmed_restore_move_refuses_when_the_destination_is_recreated_between_check_and_move` proves the narrower race the round-1 verifier reproduced (destination created in the window immediately before the move itself). | PASS |
| AC2 - "Conflict outcomes are explicit: refuse, alternate location, or provider-specific restore." | The kernel primitive's job is the mechanism, not the retry policy: a conflict produces an explicit, distinguishable `ActionResult`/`MutationError` reason (`"the destination name already refers to an existing object..."`, propagated as `Failed`), never a false `Succeeded`. Trying an alternate location or a provider-specific path is the caller's decision, made by calling `prepare_destination` again for a different name/location - the same mechanism, not a special-cased second code path. This design choice is stated in `docs/security/THREAT_MODEL.md`'s updated TM-14 entry. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-018 | Restore destination is on a different filesystem/volume than the quarantine-store root | `execute_blocks_a_restore_plan_whose_destination_crosses_a_filesystem_boundary` (synthetic identity, mismatched `device()`) - refused before the executor is ever invoked | PASS |
| SI-020 | A restore plan with insufficient authority (`Observe`) | `execute_refuses_a_restore_plan_with_insufficient_authority` - refused before destination/boundary logic runs | PASS |
| SI-013 (identity-conflict, story's own verification-contract item) | The quarantined artifact's identity drifted since the plan was sealed | `execute_blocks_a_stale_restore_plan_instead_of_moving` - the pre-existing, action-class-agnostic `revalidate` refuses, exactly as it already does for `Delete`/`Quarantine` | PASS |
| TOCTOU (reused technique) | Source identity changed before/after the open-time check | `confirmed_restore_move_rejects_a_target_already_swapped_before_open`, `confirmed_restore_move_detects_a_target_swapped_between_open_and_move` | PASS |
| Never clobber (destination-recreated, story's own verification-contract item) | The original destination was recreated after quarantine | `confirmed_restore_move_refuses_when_the_original_destination_was_recreated` (platform layer, exact TM-14 reproduction); `rename_child_matching_unix_identity`'s own no-clobber check, now a single atomic no-replace rename rather than check-then-act (E12-S01/round-1 repair) | PASS |
| SI-013 (round-1 repair - closes the check-then-rename window) | Provider state created at the destination *after* the application-level absence check but *before* the move syscall | `confirmed_restore_move_refuses_when_the_destination_is_recreated_between_check_and_move` (uses the `between_open_and_move` hook to create "new provider state" at exactly that window) - refused, provider bytes untouched; see "Round 1 independent review and repair" below | PASS |
| No cancellAI residue in provider space | A restore must not write a `.quarantine-record.json` sidecar at the destination | `system_executor_restores_a_real_file_confirmed_by_identity_and_writes_no_sidecar` asserts no sidecar file exists after a real restore | PASS |
| Mutation boundary (SI-019, reused gate) | Only one file may perform a real move | `scripts/check_mutation_boundary.py check` - unchanged: still only `cancellai-platform/src/mutation.rs` | PASS |

## Verification Commands

```text
$ cargo fmt --check                                                        clean
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-pc-windows-gnu -- -D warnings                         clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-unknown-linux-gnu -- -D warnings                      clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   39 suites, all "test result: ok", 0 failed
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
$ python3 scripts/project_os.py check                                      governance OK
$ python3 scripts/check_mutation_boundary.py check                         OK (unchanged: only mutation.rs moves/deletes)
$ python3 scripts/check_rust_workspace.py check                            OK (13 crates, acyclic)
$ python3 scripts/rust_python_parity.py check                              OK (unaffected, no cancellai.py change)
$ python3 scripts/check_docs.py check                                      OK (374 Markdown files)
$ python3 scripts/check_coverage.py check                                  OK - every ratcheted crate above its recorded floor
                                                                            (cancellai-safety 98.63%, cancellai-platform 96.08%), none re-recorded
```

## Compatibility

- Additive: a new `ActionClass::Restore` variant. Every existing exhaustive match over
  `ActionClass` in this workspace was updated to name it explicitly (compiler-forced, not a
  guess): `cancellai-safety::authority` (`minimum_authority_for`/`reversibility_allowed`),
  `cancellai-policy::explain` (folded into the same `Recommended` bucket as
  `Quarantine`/`Archive`/`Delete`), and `cancellai-cli::main` (folded into the same
  `safely_skipped`/`NOT_ELIGIBLE` bucket as `Quarantine`/`Archive` - the CLI does not build
  `Restore` plans yet, matching E12-S01's own "no CLI surface yet" scoping).
  `cancellai-policy::budget`'s eligibility filter already names its three action classes
  explicitly via `matches!` (no exhaustiveness requirement) and correctly does *not* include
  `Restore` - restoring increases active footprint, the opposite of what budget pressure
  selects for.
- `ApprovedRoot::prepare_destination`'s return type renamed `QuarantineDestination` ->
  `MoveDestination` - a mechanical rename this story's own reuse requires (the type was already
  direction-agnostic; only its name was quarantine-specific). No behavior change.
- No new crate dependency.
- Windows: `MutationOperation::Restore` refuses explicitly, mirroring `Quarantine`'s own
  residual (`windows_system_executor_refuses_a_restore_move_as_a_disclosed_residual`).
- No CLI/TUI surface consumes this yet (library-level capability only, matching E12-S01's own
  precedent).

## Performance / operability

- Identical cost shape to E12-S01's quarantine move: one `renameat`, no sidecar write, no copy.

## Documentation updated

- `docs/security/THREAT_MODEL.md` - TM-13's control statement corrected to match what E12-S01
  actually built (no copy capability exists); TM-14 states the real restore mechanism.
- `CHANGELOG.md` - `[Unreleased]` / Added.

## Method defects

- none

## Residual risks

- **Windows cannot restore at all**, by explicit refusal rather than a silent gap - a disclosed
  residual, matching `Quarantine`'s own Unix-only scope in this epic so far.
- **No orphaned-record cleanup.** A successful restore does not delete the
  `<name>.quarantine-record.json` sidecar E12-S01 left behind in the quarantine store next to
  the now-vacated location. This is harmless metadata (contentless, no safety implication) but
  is a housekeeping gap a later story could close.
- **"Alternate location" and "provider-specific restore" are policy, not mechanism, and this
  story does not implement either as a distinct code path** - see AC2's evidence row. A future
  orchestrator (not yet built) that retries at a different location, or hands off to
  provider-specific restore logic, is expected to call the primitives this story provides
  again, not a new kernel capability.
- Coverage ratchet was not tightened, though every ratcheted crate measured above its recorded
  floor. Left to the owner/reviewer per `AGENTS.md`'s own instruction.
- This packet is executor self-assessment. CR4 requires an independent adversarial pass and an
  owner-visible Safety Verdict - neither exists yet.

## Round 1 independent review and repair (2026-09-16)

Codex's round-1 review (`project/evidence/E12-VERIFIER-REVIEW.md`, `SAFETY_VERDICT.md` in this
directory) issued `FAIL`: `rename_child_matching_unix_identity` performed the destination-absence
check (`fstatat`) and the move (`renameat`) as two separate syscalls. The verifier reproduced
provider state created in that exact window being silently replaced by the restored artifact -
correct against SI-013, since restore's destination is provider state, not cancellAI's own
private store (the doc this story's own AC1 evidence row previously relied on had described this
window as bounded to "cancellAI's own private quarantine store," which is true for `Quarantine`
but not for `Restore`'s destination).

**Repair**: `rename_child_matching_unix_identity`
(`rust/crates/cancellai-sealedfs/src/lib.rs`) now performs one atomic no-replace rename per
platform instead of check-then-act - `renameat2` with `RENAME_NOREPLACE` on Linux,
`renameatx_np` with `RENAME_EXCL` on macOS - and refuses outright (`SealError::Unsupported`)
wherever that platform guarantee is unavailable (old kernel, unsupported filesystem), rather than
falling back to the unsafe two-syscall shape. This closes the race for `Quarantine` and `Archive`
as well, since all three share this one function.

## Verification Commands (round 1 repair)

```text
$ cargo fmt --check                                                        clean
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-pc-windows-gnu -- -D warnings                         clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-unknown-linux-gnu -- -D warnings                      clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   all suites "test result: ok", 0 failed (cancellai-sealedfs: 21 passed, cancellai-platform: 92 passed)
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
```

## Round 2 independent review (2026-09-16)

Codex's round-2 review (`project/evidence/E12-VERIFIER-REVIEW-ROUND2.md`, `SAFETY_VERDICT.md` in
this directory) issued `PASS_WITH_RESIDUALS`: the atomic no-replace rename genuinely closes the
race round 1 reproduced (an injected recreate-in-window regression protects the provider bytes),
with disclosed residuals for platforms/filesystems lacking the atomic primitive (an explicit
refusal, not a silent fallback) and Windows (still refuses entirely). No code change to this
story's own mechanism was required this round.

E12-S02 was nonetheless recorded `blocked` rather than `done` after round 2, because its
dependency E12-S01 was returned to `in_progress` for a separate, shared crash-recovery finding
(see E12-S01's own evidence). Round 3 repairs that finding
(`write_new_child_atomically` now also `fsync`s its containing directory, and the shared move
protocol writes sidecars durably before moving - see E12-S01's evidence for detail); this story's
own restore mechanism is unaffected by that repair beyond inheriting the stronger directory-fsync
guarantee for free, since `Restore` still writes no sidecar of its own.

## Verification Commands (round 3, unaffected re-run)

```text
$ cargo fmt --check                                                        clean
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-pc-windows-gnu -- -D warnings                         clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-unknown-linux-gnu -- -D warnings                      clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   all suites "test result: ok", 0 failed (cancellai-platform: 95 passed, cancellai-sealedfs: 21 passed)
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
```

## Round 3 independent review (2026-09-16)

Codex's round-3 review (`project/evidence/E12-VERIFIER-REVIEW-ROUND3.md`, `SAFETY_VERDICT.md` in
this directory) issued `PASS_WITH_RESIDUALS` again: the restore commit remains a single atomic
no-replace rename with explicit refusal where unavailable, unaffected by round 3's finding (which
was specific to `recover_pending_moves`'s recovery-side logic, something `Restore` never invokes
since it writes no sidecar). E12-S02 stays `blocked` because its dependency E12-S01 was rejected
again in round 3 for a defect in that same recovery logic - see E12-S01's own evidence for the
round-4 identity-witness repair.

## Round 4 independent review (2026-09-16)

Codex's round-4 review issued `PASS_WITH_RESIDUALS` again, unaffected by round 4's finding
(again specific to `recover_pending_moves`, which `Restore` never invokes). Still `blocked` on
E12-S01 - see that story's own evidence for the round-5 repair.

## Round 5 independent review and scope reduction (2026-09-16)

Codex's round-5 review found a third recovery-mechanism defect, again in `recover_pending_moves`
(which `Restore` never invokes - unaffected here). The owner's decision to remove that mechanism
outright is recorded in E12-S01's own evidence. `Restore` still writes no sidecar of its own, so
nothing about this reduction changes its behavior.

## Verifier verdict

pending (round 6, owner-authorized beyond the 3-round ceiling, reviewing a scope reduction rather than a further patch)

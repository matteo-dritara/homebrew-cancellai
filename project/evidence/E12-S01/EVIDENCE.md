# Evidence Packet - E12-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E12 epic review
- Change Risk: CR4
- Spec version/commit: `docs/architecture/PERSISTENCE_MODEL.md` "Quarantine store",
  `docs/architecture/PLATFORM_MODEL.md` "Boundary rules"; E21-S07's own note above
  `MutationOperation` ("re-adding either is that story's job, together with the confirmation
  technique it needs")

## Outcome

PASS

## Scope

Re-adds `MutationOperation::Quarantine` - removed by E21-S07 as an unconfirmed, unreachable
primitive - with the confirmation technique that removal asked for: the same open-time /
immediately-before three-check shape `confirmed_delete_file_inner` uses, ending in a
cross-directory `renameat` (via a new `cancellai_sealedfs::rename_child_matching_unix_identity`)
instead of an unlink, plus a contentless restore-metadata sidecar. Adds the destination a
quarantine plan needs (`ApprovedRoot::prepare_destination`, `QuarantineDestination`,
`SealedPlan::seal_quarantine`) and wires `mutation_executor::execute`'s `ActionClass::Quarantine`
arm to an explicit same-device boundary check before ever attempting a move. Unix-only; Windows
refuses explicitly (disclosed residual, matching `DeleteFile`'s own history before E20-S05).

No copy-based quarantine fallback exists or is planned by this story - "move-first" is read as
"move-only, fail closed otherwise," matching PERSISTENCE_MODEL.md's own "prefer same-volume
atomic move" alongside its silence on any copy path.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Quarantine never copies huge data when atomic/same-volume move is required by policy." | No copy path exists anywhere in `MutationOperation::Quarantine`/`confirmed_quarantine_move_inner` - the only OS operation performed is `renameat` (via `rename_child_matching_unix_identity`), which never touches file content. `execute_quarantines_a_real_file_through_the_full_stack` proves a real same-volume move end to end. | PASS |
| AC2 - "Insufficient space or cross-volume uncertainty fails closed." | Cross-volume: `mutation_executor::execute` compares `plan.root_identity().device()` against `plan.destination_root_identity().device()` *before* ever reaching the platform seam - `execute_blocks_a_quarantine_plan_whose_destination_crosses_a_filesystem_boundary` proves the mismatch refuses without the mutation executor ever being invoked. `renameat`'s own `EXDEV` remains the backstop (documented, not additionally unit-tested - a second real filesystem is not available to this executor, matching every other cross-device test in this workspace). Insufficient space: no separate pre-check exists; an OS failure (real `ENOSPC` or the fault-injection stand-in this workspace already uses elsewhere) propagates as `ActionResult::Failed`, never `Succeeded` - `execute_reports_failed_when_the_quarantine_mutation_itself_fails` proves this at the executor level. | PASS |
| AC3 - "Original identity and restore metadata are recorded contentlessly." | `mutation_executor::execute` composes a `QuarantineRecord` (`original_path`, `original_identity`, `root` - no artifact content) and `confirmed_quarantine_move_inner` writes it atomically (`SealedRoot::write_new_child_atomically`) as `<destination>.quarantine-record.json` once the move itself has already succeeded. `system_executor_quarantines_a_real_file_confirmed_by_identity` asserts the sidecar's real content. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-018 | Quarantine destination is on a different filesystem/volume than the source root | `execute_blocks_a_quarantine_plan_whose_destination_crosses_a_filesystem_boundary` (synthetic identity, mismatched `device()`) - refused before the executor is ever invoked | PASS |
| SI-018 | A quarantine destination reached through a symlinked intermediate directory component | `a_symlinked_intermediate_destination_component_refuses_the_quarantine_move` - refused with "without following a link"; nothing lands at the link's real target | PASS |
| SI-018 (source side, pre-existing mechanism reused) | Source reached through a symlinked intermediate component | `a_symlinked_intermediate_component_refuses_the_quarantine_move` | PASS |
| SI-020 | A quarantine plan with insufficient authority (`Observe`) | `execute_refuses_a_quarantine_plan_with_insufficient_authority` - refused before the destination/boundary logic runs at all, mirroring `e03_verifier_round1_observe_authority_cannot_execute_a_delete` for `Delete` | PASS |
| SI-013 (TOCTOU, reused technique) | Source identity changed before/after the open-time check | `confirmed_quarantine_move_rejects_a_target_already_swapped_before_open`, `confirmed_quarantine_move_detects_a_target_swapped_between_open_and_move` | PASS |
| Unknown-to-authority promotion | A quarantine destination fact is `Unreadable`/`Unsupported` rather than `Absent` | `prepare_destination_fails_closed_when_the_observation_is_unreadable`, `prepare_destination_fails_closed_when_the_observation_is_unsupported` - both refuse, neither is silently treated as "safe to use" | PASS |
| Never clobber | A name already exists at the destination | `rename_child_matching_unix_identity_refuses_to_clobber_an_existing_destination` (sealedfs), `confirmed_quarantine_move_refuses_to_clobber_an_existing_destination` (platform) - source and pre-existing destination both survive intact | PASS |
| SI-020 (round-1 repair) | The move itself succeeds but the restore-record sidecar write fails | `a_quarantine_move_that_succeeds_but_cannot_write_its_restore_record_is_rolled_back` - the move is now rolled back (source restored, destination left empty) rather than merely reported as `Err` with the artifact stranded unrecorded; see "Round 1 independent review and repair" below | PASS |
| Mutation boundary (SI-019, reused gate) | Only one file may perform a real deletion/move | `scripts/check_mutation_boundary.py check` - unchanged: still only `cancellai-platform/src/mutation.rs` | PASS |

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
$ python3 scripts/project_os.py check                                      governance OK: 24 decisions, 32 epics, 165 stories
$ python3 scripts/check_mutation_boundary.py check                         OK (only mutation.rs deletes/moves; only it and mutation_executor.rs reference the capability)
$ python3 scripts/check_rust_workspace.py check                            OK (13 crates, acyclic)
$ python3 scripts/rust_python_parity.py check                              OK (13 NORMATIVE fixtures, both scenarios) - unaffected, this story does not touch cancellai.py
$ python3 scripts/check_docs.py check                                      OK (373 Markdown files)
$ python3 scripts/check_schemas.py check                                   OK (unaffected by this story)
$ python3 scripts/check_fixtures.py check                                  OK (unaffected by this story)
$ python3 scripts/safety_oracle.py check                                   OK
$ python3 scripts/check_ears.py check                                      OK (pre-existing baselined warnings only, none new)
$ python3 scripts/check_risk_classification.py check                      OK (no new story below its floor)
$ python3 scripts/gate_sensitivity.py check                                OK (11/11 mutants killed)
$ python3 scripts/check_process.py check                                  OK (pre-existing baselined warnings only, none new)
$ python3 scripts/check_coverage.py check                                 OK - cancellai-safety 98.57% (floor 97.78%), cancellai-platform 95.73%
                                                                            (floor 94.39%), cancellai-sealedfs 93.54% (floor 91.79%); every
                                                                            ratcheted crate above its recorded floor, none re-recorded
```

## Compatibility

- Additive: `MutationOperation` gains a variant; no existing caller/behavior changes.
  `ActionClass::Quarantine` previously always refused (`mutation_executor` combined it with
  `Observe`/`Archive`) - it now performs a real move when a caller supplies a `seal_quarantine`
  plan, and continues to refuse (unchanged message) when it does not.
- No new crate dependency: the new `unsafe` code lives entirely in `cancellai-sealedfs`
  (already the sole `unsafe_code`-exempt crate, ADR-0017), reusing `libc` already in
  `rust/deny.toml`'s allow-list.
- Windows: `MutationOperation::Quarantine` refuses explicitly with a stated reason
  (`windows_system_executor_refuses_a_quarantine_move_as_a_disclosed_residual`), never falling
  back to an unconfirmed path-based rename.
- No CLI/TUI surface consumes this yet (library-level capability only, matching this workspace's
  own precedent for allocation/process-observation stories landing ahead of their consumer).

## Performance / operability

- One `renameat` plus one atomic sidecar write per quarantine action - both directory-entry
  operations, not proportional to file size (no copy is ever performed).
- One extra directory bind (`SealedRoot::bind_existing`) per side (source parent, destination
  parent), each a handle-relative walk proportional to path depth, matching `DeleteFile`'s
  existing cost shape.

## Documentation updated

- `docs/architecture/PERSISTENCE_MODEL.md` - "Quarantine store" section states the real
  mechanism and the Windows residual.
- `docs/architecture/PLATFORM_MODEL.md` - names this story as the "dedicated operation" its own
  boundary-rules bullet anticipated.
- `CHANGELOG.md` - `[Unreleased]` / Added.

## Method defects

- none

## Residual risks

- **Windows cannot quarantine at all**, by explicit refusal rather than a silent gap - a
  disclosed residual, not a defect. Follow-up story's job, mirroring `DeleteFile`'s own history
  before E20-S05.
- **No dedicated free-space pre-check.** "Insufficient space fails closed" is discharged by
  honest propagation of the OS's own failure (a same-volume `renameat` is a directory-entry
  operation and rarely triggers `ENOSPC` in practice) rather than a new `statvfs`-based
  observer. Adding one is a proportionate follow-up if real-world quarantine hits directory-entry
  exhaustion in practice; nothing found in this story's own testing motivates it yet.
- **Coverage ratchet was not tightened**, though every ratcheted crate measured above its
  recorded floor (`cancellai-safety` 98.57% vs 97.78%, `cancellai-platform` 95.73% vs 94.39%,
  `cancellai-sealedfs` 93.54% vs 91.79%). `scripts/check_coverage.py record` is a separate,
  explicit, reviewable diff per `AGENTS.md`'s own instruction - left to the owner/reviewer rather
  than bundled here.
- This packet is executor self-assessment. CR4 requires an independent adversarial pass and an
  owner-visible Safety Verdict - neither exists yet.

## Round 1 independent review and repair (2026-09-16)

Codex's round-1 review (`project/evidence/E12-VERIFIER-REVIEW.md`, `SAFETY_VERDICT.md` in this
directory) issued `FAIL`: forcing a collision at the restore-record sidecar's own temp name
(`<destination>.quarantine-record.json.tmp`) after the move had already completed reproduced
`source_exists=false; moved_exists=true; record_exists=false` - an artifact stranded outside its
original location with no restore metadata, violating SI-020/AC3. The evidence packet above had
disclosed this as a residual risk ("reported as `Failed`, not invisibly repaired"); the verifier
correctly judged that disclosure insufficient for a CR4 obligation.

**Repair**: `confirmed_move_inner` (`rust/crates/cancellai-platform/src/mutation.rs`) now returns
the source's own `SealedRoot` alongside the destination's. `confirmed_quarantine_move_inner` and
`confirmed_archive_move_inner` roll the move back (`rollback_move`, an identity-confirmed reverse
`renameat` using the same identity token - a same-device rename changes neither device/inode nor
mtime, so it still matches) whenever a sidecar write fails, and report whichever of "rolled back,
original intact" or "rollback also failed, manual recovery required" actually happened - never
the write failure alone. `a_quarantine_move_that_succeeds_but_cannot_write_its_restore_record_is_rolled_back`
replaces the test that had accepted the unsafe terminal state as an "honest failure."

## Verification Commands (round 1 repair)

```text
$ cargo fmt --check                                                        clean
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-pc-windows-gnu -- -D warnings                         clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-unknown-linux-gnu -- -D warnings                      clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   all suites "test result: ok", 0 failed (cancellai-platform: 92 passed)
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
```

## Round 2 independent review and round 3 repair (2026-09-16)

Codex's round-2 review (`project/evidence/E12-VERIFIER-REVIEW-ROUND2.md`, `SAFETY_VERDICT.md` in
this directory) issued `FAIL` again: round 1's rollback correctly handles a *synchronous* sidecar
write failure, but an isolated reproduction found no restart recovery for a genuine crash/power
loss between a successful rename and its sidecar write - nothing durable existed yet at that
point for a restart to recover from. Required repair: "persist and fsync an identity-bound
pre-move journal, make every state transition restart-recoverable, and add crash injection after
rename and every metadata transition."

**Repair**: round 1's write-after-move-then-rollback design is replaced outright, not patched.
`confirmed_move_prepare` (renamed from `confirmed_move_inner`) now stops before performing the
move; `commit_move_with_sidecars` writes every sidecar's real, final content to a `.pending` name
durably - `write_new_child_atomically` now `fsync`s the containing directory after its rename, not
only the file's own content - *before* the move is even attempted. The move becomes a plain
rename; finalizing is a second, same-directory rename
(`cancellai_sealedfs::SealedRoot::finalize_own_child`) with no new content write. A failure before
the move means nothing was attempted at all (`remove_own_child` cleans up any already-written
pending sidecars); a failure finalizing after a successful move leaves a state the new
`recover_pending_moves` function completes on restart, because the content it finalizes was
already durable before whatever interrupted it. Two new regression tests exercise both directions
directly: `recover_pending_moves_finalizes_a_move_interrupted_after_rename_but_before_finalize`
(the exact "crash after rename, before finalize" gap round 2 found) and
`recover_pending_moves_discards_a_pending_sidecar_whose_move_never_happened`. The two round-1
"is_rolled_back" tests are replaced with
`a_quarantine_sidecar_that_cannot_be_durably_recorded_never_moves_anything`/
`an_archive_sidecar_that_cannot_be_durably_recorded_never_moves_anything`, proving a durable-write
failure now means the move is never attempted, and a further
`a_later_archive_sidecar_failing_to_write_durably_cleans_up_the_earlier_ones_too` proves cleanup
covers every already-written pending sidecar, not only the one that failed.

`recover_pending_moves` is not wired to any startup caller yet, since none exists - the same
"primitive provided, orchestrator not yet built" scoping this story's own `verify_archive_
integrity` already used before any real purge caller existed.

## Verification Commands (round 3 repair)

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

## Round 3 independent review and round 4 repair (2026-09-16)

Codex's round-3 review (`project/evidence/E12-VERIFIER-REVIEW-ROUND3.md`, `SAFETY_VERDICT.md` in
this directory) issued `FAIL`: an isolated reproduction placed an existing artifact and its
already-finalized final record alongside a `.pending` record left behind by a *different,
conflicting* operation whose own no-replace move had been correctly refused (destination name
already taken) but whose pre-move pending sidecar was never cleaned up. `recover_pending_moves`
reported `Finalized` and silently replaced the legitimate record's content with the conflicting
operation's own - because it proved only that *some* object existed at the destination name, not
that this specific pending sidecar was the one that produced it. Required repair: "journal each
operation with a durable identity/operation binding, then verify that binding against the
destination artifact before finalizing. A pre-existing destination is not evidence that a pending
sidecar belongs to it."

**Repair**: `commit_move_with_sidecars` now also durably writes an identity witness sidecar
(`<destination>.move-identity.json`, content `"<device>:<inode>"`) alongside every operation's
other pending sidecars - the identity is the source's own, captured at `confirmed_move_prepare`
time; a same-device rename preserves it, so it is still correct once the object sits at its new
name. `recover_pending_moves` now groups every pending sidecar by destination name and resolves
each group together: it reads the group's witness (pending or already-finalized, for the
partial-recovery case), stats the real object currently at that name, and finalizes the *entire*
group only if the two identities match - a missing witness, an unreadable one, or a mismatch
discards the entire group instead. New regression test
`recover_pending_moves_never_overwrites_an_existing_record_with_an_unrelated_conflicting_one`
reproduces Codex's exact scenario and asserts the legitimate record survives untouched while the
conflicting group is discarded; the existing finalize/discard tests were updated to also write
the now-required witness.

This is the owner-authorized fourth review round, exceeding ADR-0025's three-round cost ceiling
by explicit owner decision (recorded in the conversation directing this repair), because round 3
found a genuine, well-scoped CR4 defect rather than a residual.

## Verification Commands (round 4 repair)

```text
$ cargo fmt --check                                                        clean
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-pc-windows-gnu -- -D warnings                         clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-unknown-linux-gnu -- -D warnings                      clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   all suites "test result: ok", 0 failed (cancellai-platform: 96 passed)
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
```

## Round 4 independent review and round 5 repair (2026-09-16)

Codex's round-4 review (`project/evidence/E12-VERIFIER-REVIEW-ROUND4.md`, `SAFETY_VERDICT.md` in
this directory) confirmed round 3's exact fully-formed-conflict reproduction is closed, but found
a narrower variant reachable through round 3's own repair: the fallback that let recovery accept
an *already-finalized* witness (added for a partial-recovery case) let a *different, later*
operation's own partial group - one that crashed before ever writing its own pending witness -
silently borrow an unrelated, older, already-fully-completed operation's finalized witness for
the same name, since that old witness's recorded identity still matched the untouched real
object. Required repair: "a pending group must not borrow an earlier finalized witness as proof
of a newer operation... order finalization so that it occurs last, or add a durable operation
identifier."

**Repair**: the fallback-to-finalized-witness path is removed outright, not narrowed further.
`commit_move_with_sidecars` already wrote and finalized the identity witness *last* among a
group's sidecars (an existing property, not new); this makes "the group's own witness is still
pending" and "this operation has something left for recovery to do" the same fact by
construction - an operation that reached finalizing its own witness had, by definition, already
finalized everything ahead of it, so there is nothing left pending to recover for it in the first
place. `recover_pending_moves` now reads *only* the still-pending witness as proof; an
already-finalized witness at a name is never treated as a current group's own evidence, only ever
as (at best) a different, earlier, unrelated operation's permanent record - exactly what must
never validate someone else's leftovers. New regression test
`recover_pending_moves_never_borrows_an_older_completed_operations_final_witness` reproduces
Codex's exact round-4 scenario.

This is the fifth review round, continuing the owner's explicit authorization to close what each
previous round found (`scripts/check_process.py`'s `REVIEW_ROUND_EXCEPTIONS["E12"]` records this).

## Verification Commands (round 5 repair)

```text
$ cargo fmt --check                                                        clean
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-pc-windows-gnu -- -D warnings                         clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-unknown-linux-gnu -- -D warnings                      clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   all suites "test result: ok", 0 failed (cancellai-platform: 97 passed)
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
```

## Round 5 independent review and scope reduction (2026-09-16)

Codex's round-5 review (`project/evidence/E12-VERIFIER-REVIEW-ROUND5.md`, `SAFETY_VERDICT.md` in
this directory) confirmed round 4's exact finding is closed, but found a third successive
variant of the same underlying hazard: `read_dir` gives no ordering guarantee, and the
per-group finalize loop could finalize the identity witness before a sibling sidecar's own
finalize had run and failed - a later recovery pass would then find that sibling still pending
but its own proof (the witness) no longer pending, and wrongly discard it as an orphan, losing
legitimate metadata for an artifact that had already, genuinely, moved.

A targeted repair for this exact finding was implemented and independently regression-tested
(finalize every non-witness sibling first; finalize the witness only if all of them succeed; on
any failure, leave the witness and unresolved siblings pending for a retry) - but round 5 was
the third consecutive round to find a genuine, distinct correctness hazard in the same automated
recovery mechanism (round 3: filename-only proof; round 4: an unrelated operation's finalized
witness accepted as proof; round 5: finalization ordering within one operation's own group).
Codex's own round-5 record named this pattern explicitly rather than treating it as three
unrelated bugs.

**Owner decision**: given three real defects in one mechanism across three rounds, the owner
chose to stop iterating on an automated recovery scanner rather than send a fourth attempt to a
sixth review round. `recover_pending_moves`, `PendingRecoveryOutcome`, the identity-witness
sidecar, and their five regression tests are removed outright. What remains, and is unaffected
by this reduction: the write-before-move/fsync-durable/finalize-after-move protocol itself
(rounds 1-2's own finding, closed since round 3, never itself the subject of rounds 3-5's
findings) - a sidecar's content is always durably recorded before the move is ever attempted, and
a finalize failure after a successful move leaves that content intact under its own `.pending`
name, safe for a human operator (or a future, independently verified tool, as its own dedicated
story) to complete. This is now a disclosed, deferred residual rather than an automated
capability - recorded here rather than left implicit.

## Verification Commands (round 5 scope reduction)

```text
$ cargo fmt --check                                                        clean
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-pc-windows-gnu -- -D warnings                         clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-unknown-linux-gnu -- -D warnings                      clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   all suites "test result: ok", 0 failed (cancellai-platform: 93 passed)
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
```

## Residual risks (added)

- **No automated crash recovery for an interrupted finalize.** Disclosed above and in
  `docs/architecture/PERSISTENCE_MODEL.md`. Three independent-review rounds (3-5) each found a
  genuine defect in successive attempts at this specific mechanism; a dedicated future story with
  its own direct state-machine tests (not exercised only through the normal commit path) is the
  right place to reattempt it, per Codex's own round-5 recommendation.

## Verifier verdict

pending (round 6, owner-authorized beyond the 3-round ceiling, reviewing a scope reduction rather than a further patch)

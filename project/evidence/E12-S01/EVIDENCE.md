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
| Crash/failure honesty | The move itself succeeds but the restore-record sidecar write fails | `a_quarantine_move_that_succeeds_but_cannot_write_its_restore_record_reports_failure_honestly` - `Err` is returned (never a false `Succeeded`), while the artifact is confirmed safely relocated, not lost | PASS |
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

- **POSIX has no atomic "rename only if the destination name is absent."** The destination-
  absence check and the `renameat` itself remain two syscalls in
  `rename_child_matching_unix_identity`. What is closed is either directory being swapped
  (both are held, no-follow descriptors); what remains is an attacker with write access to the
  destination directory planting an object at the destination name in that window. The
  destination directory is cancellAI's own private quarantine store, never a
  provider/user-writable location, which is a materially smaller surface than the source-side
  race this story's identity check already closes.
- **Windows cannot quarantine at all**, by explicit refusal rather than a silent gap - a
  disclosed residual, not a defect. Follow-up story's job, mirroring `DeleteFile`'s own history
  before E20-S05.
- **No dedicated free-space pre-check.** "Insufficient space fails closed" is discharged by
  honest propagation of the OS's own failure (a same-volume `renameat` is a directory-entry
  operation and rarely triggers `ENOSPC` in practice) rather than a new `statvfs`-based
  observer. Adding one is a proportionate follow-up if real-world quarantine hits directory-entry
  exhaustion in practice; nothing found in this story's own testing motivates it yet.
- **A restore-record write failure after a successful move is reported as `Failed`, not
  invisibly repaired.** The artifact is not lost (confirmed by
  `a_quarantine_move_that_succeeds_but_cannot_write_its_restore_record_reports_failure_honestly`),
  but a caller that only inspects `Succeeded` vs not, without a repair path for this specific
  `Failed` reason, would leave a quarantined artifact without a restore record. E12-S02 (Restore
  protocol) is the natural owner of a repair/reconciliation path for this state.
- **Coverage ratchet was not tightened**, though every ratcheted crate measured above its
  recorded floor (`cancellai-safety` 98.57% vs 97.78%, `cancellai-platform` 95.73% vs 94.39%,
  `cancellai-sealedfs` 93.54% vs 91.79%). `scripts/check_coverage.py record` is a separate,
  explicit, reviewable diff per `AGENTS.md`'s own instruction - left to the owner/reviewer rather
  than bundled here.
- This packet is executor self-assessment. CR4 requires an independent adversarial pass and an
  owner-visible Safety Verdict - neither exists yet.

## Verifier verdict

pending

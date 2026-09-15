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
| AC1 - "Restore never overwrites unrelated current provider state silently." | The restore destination is prepared via `ApprovedRoot::prepare_destination`, which refuses if anything already exists there (checked once at prepare-time via `IdentityObserver`, and again independently at move-time via `cancellai_sealedfs::rename_child_matching_unix_identity`'s own `fstatat` pre-check) - never a silent overwrite. `confirmed_restore_move_refuses_when_the_original_destination_was_recreated` proves the exact TM-14 reproduction: a real file pre-populated at the destination survives untouched, and the quarantined copy survives too. | PASS |
| AC2 - "Conflict outcomes are explicit: refuse, alternate location, or provider-specific restore." | The kernel primitive's job is the mechanism, not the retry policy: a conflict produces an explicit, distinguishable `ActionResult`/`MutationError` reason (`"the destination name already refers to an existing object..."`, propagated as `Failed`), never a false `Succeeded`. Trying an alternate location or a provider-specific path is the caller's decision, made by calling `prepare_destination` again for a different name/location - the same mechanism, not a special-cased second code path. This design choice is stated in `docs/security/THREAT_MODEL.md`'s updated TM-14 entry. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-018 | Restore destination is on a different filesystem/volume than the quarantine-store root | `execute_blocks_a_restore_plan_whose_destination_crosses_a_filesystem_boundary` (synthetic identity, mismatched `device()`) - refused before the executor is ever invoked | PASS |
| SI-020 | A restore plan with insufficient authority (`Observe`) | `execute_refuses_a_restore_plan_with_insufficient_authority` - refused before destination/boundary logic runs | PASS |
| SI-013 (identity-conflict, story's own verification-contract item) | The quarantined artifact's identity drifted since the plan was sealed | `execute_blocks_a_stale_restore_plan_instead_of_moving` - the pre-existing, action-class-agnostic `revalidate` refuses, exactly as it already does for `Delete`/`Quarantine` | PASS |
| TOCTOU (reused technique) | Source identity changed before/after the open-time check | `confirmed_restore_move_rejects_a_target_already_swapped_before_open`, `confirmed_restore_move_detects_a_target_swapped_between_open_and_move` | PASS |
| Never clobber (destination-recreated, story's own verification-contract item) | The original destination was recreated after quarantine | `confirmed_restore_move_refuses_when_the_original_destination_was_recreated` (platform layer, exact TM-14 reproduction); `rename_child_matching_unix_identity`'s own no-clobber check (E12-S01) reused unchanged | PASS |
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

## Verifier verdict

pending

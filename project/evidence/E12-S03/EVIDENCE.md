# Evidence Packet - E12-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E12 epic review
- Change Risk: CR4 (declared CR3 at planning time in `project/epics/E12.json`; raised at
  commit time when the risk-floor gate refused it - the change adds a new mutation capability,
  `MutationOperation::Archive`, inside `rust/crates/cancellai-platform/src/mutation.rs`, the
  sole SI-019 mutation-boundary seam. `project/epics/E12.json` records the reclassification and
  now names `SI-018`/`SI-019`/`SI-020` as this story's safety obligations)
- Spec version/commit: `docs/architecture/PERSISTENCE_MODEL.md` "Archive";
  `docs/adrs/0019-dependency-rings-per-crate.md` (dependency-ring constraint that scoped this
  story - see "Scope" below)

## Outcome

PASS

## Scope

Owner-narrowed before implementation (recorded, not silently decided): a real compressed
archive format needs a kernel-ring dependency, which ADR-0019 requires a dedicated, reviewed
ADR for - the `rust-kernel-guard` skill's verdict on even reusing `sha2` (already a
`cancellai-safety` dependency) in `cancellai-platform` was explicit: `REQUIRES ADR`, no
exception for a crate already vetted elsewhere in the kernel ring. The owner chose to scope this
story to the mechanism only: a real move into cancellAI's own archive store (reusing E12-S01's
exact identity-confirmed, no-clobber primitive, extracted further so `Quarantine`/`Archive`
share one `write_move_record` sidecar writer), an explicit format/version record, and a
dependency-free integrity signal (byte length, captured at open time, verified on demand). Real
byte compression and a cryptographic content hash are both disclosed residuals for a follow-up
story that will carry its own ADR.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Archive format/version is explicit." | `mutation_executor::execute`'s new `ActionClass::Archive` arm composes an `ArchiveRecord { format: "cancellai-archive-uncompressed", format_version: 1, ... }`, serialized into the caller-composed sidecar. `system_executor_archives_a_real_file_and_writes_both_sidecars` asserts the real sidecar content. | PASS |
| AC2 - "Compression never changes semantic classification to disposable." | No compression is implemented (see Scope), so nothing can change any classification as a side effect of it - vacuously true, and stated as such rather than silently assumed. What the mechanism *does* enforce: `reversibility_allowed(ActionClass::Archive, _)` already required `Reversibility::Archivable` specifically (pre-existing gate from an earlier story) and is now actually reachable; `execute_blocks_an_archive_plan_claiming_quarantinable_reversibility_instead_of_archivable` proves a plan cannot substitute a different reversibility class and still reach a real move. | PASS |
| AC3 - "Restore integrity is checked before source purge." | `verify_archive_integrity` provides the check function `docs/architecture/PERSISTENCE_MODEL.md` names, with a documented, dependency-free interpretation (byte-length comparison, not a cryptographic hash - see Residuals). It is not wired to a real purge caller, since E12-S04 (Purge) is blocked on E13-S02 and does not exist yet; this is the same "primitive provided, orchestrator not yet built" scoping E12-S01/S02 already used. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-018 (reused mechanism) | Archive destination on a different filesystem/volume | `execute_blocks_an_archive_plan_whose_destination_crosses_a_filesystem_boundary` | PASS |
| SI-020 (reversibility gate, AC2) | An archive plan claiming `Quarantinable` instead of `Archivable` | `execute_blocks_an_archive_plan_claiming_quarantinable_reversibility_instead_of_archivable` - refused before any move is attempted | PASS |
| Authority gate | An archive plan with insufficient authority (`Observe`) | `execute_refuses_an_archive_plan_with_insufficient_authority` | PASS |
| TOCTOU (reused technique) | Source identity changed before/after the open-time check | `confirmed_archive_move_rejects_a_target_already_swapped_before_open`, `confirmed_archive_move_detects_a_target_swapped_between_open_and_move` | PASS |
| Never clobber (reused mechanism) | A name already exists at the archive destination | `confirmed_archive_move_refuses_to_clobber_an_existing_destination` | PASS |
| Round-trip (story's own verification-contract item) | Archive then verify | `system_executor_archives_a_real_file_and_writes_both_sidecars` ends with `verify_archive_integrity(&destination) == Verified` | PASS |
| Corruption (story's own verification-contract item) | The archived copy is truncated/modified after archiving | `verify_archive_integrity_detects_a_truncated_or_corrupted_archive` - a real file rewritten to a different length is refused, with the exact recorded/actual lengths reported | PASS |
| Fail-closed on the unknown | Missing sidecar / missing archived copy / malformed sidecar content | `verify_archive_integrity_fails_closed_when_the_length_sidecar_is_missing`, `..._the_archived_copy_is_gone`, `..._the_sidecar_content_is_malformed` - every branch is a named refusal, never a silent "probably fine" | PASS |
| Mutation boundary (SI-019, reused gate) | Only one file may perform a real move | `scripts/check_mutation_boundary.py check` - unchanged | PASS |

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
$ python3 scripts/check_docs.py check                                      OK (375 Markdown files)
$ python3 scripts/check_coverage.py check                                  OK - every ratcheted crate above its recorded floor
                                                                            (cancellai-safety 98.68%, cancellai-platform 96.43%), none re-recorded
```

## Compatibility

- Additive: `MutationOperation::Archive` is a new variant; `ActionClass::Archive` previously
  always refused (grouped with `Observe`) and now performs a real move when a caller supplies a
  `seal_archive` plan, continuing to refuse (unchanged message) when it does not.
- No new crate dependency - the explicit governance reason is recorded in Scope above.
- `confirmed_move_inner`'s return type changed (now also returns the captured source byte
  length) - an internal, `cancellai-platform`-private signature change with no external
  behavior change; `confirmed_quarantine_move_inner`/`confirmed_restore_move_inner` were updated
  to match.
- Windows: `MutationOperation::Archive` refuses explicitly, mirroring
  `Quarantine`/`Restore`'s own residual
  (`windows_system_executor_refuses_an_archive_move_as_a_disclosed_residual`).
- No CLI/TUI surface consumes this yet (library-level capability only, matching this epic's own
  precedent so far).

## Performance / operability

- Identical cost shape to `Quarantine`: one `renameat`, two small atomic sidecar writes
  (record + length), no copy, no compression pass over the artifact's content.

## Documentation updated

- `docs/architecture/PERSISTENCE_MODEL.md` - "Archive" section states the real mechanism and
  the compression/hash residuals.
- `CHANGELOG.md` - `[Unreleased]` / Added.

## Method defects

- none

## Residual risks

- **No real byte compression.** Deliberately out of scope - see Scope above. A future story
  introducing a reviewed compression dependency under its own ADR is expected to extend
  `ArchiveRecord`'s `format`/`format_version` to a new value, not silently reinterpret version 1.
- **No cryptographic content hash.** The length-based integrity check
  (`verify_archive_integrity`) detects truncation and most in-place corruption but not a
  same-length content substitution. A future story adding a reviewed hashing dependency (or
  reusing `sha2` under its own ADR extending it to `cancellai-platform`) is the natural place to
  close this; recorded here rather than overclaimed as already closed.
- **Windows cannot archive at all**, by explicit refusal rather than a silent gap - matching
  `Quarantine`/`Restore`'s own Unix-only scope in this epic so far.
- **No orphaned-sidecar cleanup** if a later story ever deletes an archived copy directly - same
  residual class E12-S02 already disclosed for its own restore-record sidecar.
- Coverage ratchet was not tightened, though every ratcheted crate measured above its recorded
  floor. Left to the owner/reviewer per `AGENTS.md`'s own instruction.
- This packet is executor self-assessment. This story is CR4 (see the reclassification note
  above): an independent adversarial pass **and an owner-visible Safety Verdict** are required
  before `done`, neither of which exists yet.

## Verifier verdict

pending

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
| AC3 - "Restore integrity is checked before source purge." | `verify_archive_integrity` provides the check function `docs/architecture/PERSISTENCE_MODEL.md` names, checking both recorded byte length *and* an FNV-1a content fingerprint (round-1 repair, not a cryptographic hash - see Residuals and "Round 1 independent review and repair" below). It is not wired to a real purge caller, since E12-S04 (Purge) is blocked on E13-S02 and does not exist yet; this is the same "primitive provided, orchestrator not yet built" scoping E12-S01/S02 already used. | PASS |

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
| SI-020 (round-1 repair - equal-length corruption) | The archived copy is corrupted in place with no length change | `verify_archive_integrity_detects_an_equal_length_content_corruption` - the round-1 verifier's exact reproduction (`"0123456789"` -> `"abcdefghij"`, same length); the content fingerprint now catches what the length-only check accepted as `Verified` | PASS |
| SI-020 (round-1 repair - sidecar failure) | The move succeeds but a sidecar write (record or length/fingerprint) fails | `an_archive_move_that_succeeds_but_cannot_write_its_record_sidecar_is_rolled_back` - the move is rolled back rather than leaving the artifact stranded, unrecorded, in the archive store (same repair as E12-S01) | PASS |
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
  length, an optional content fingerprint gated by a new `compute_fingerprint` parameter added
  in round 1, and the source's own `SealedRoot` for rollback) - an internal,
  `cancellai-platform`-private signature change with no external behavior change; every caller
  was updated to match.
- Windows: `MutationOperation::Archive` refuses explicitly, mirroring
  `Quarantine`/`Restore`'s own residual
  (`windows_system_executor_refuses_an_archive_move_as_a_disclosed_residual`).
- No CLI/TUI surface consumes this yet (library-level capability only, matching this epic's own
  precedent so far).

## Performance / operability

- One `renameat`, three small atomic sidecar writes (record + length + fingerprint), no copy, no
  compression pass over the artifact's content.
- **Round-1 repair changed this**: computing the content fingerprint reads the entire source
  file once, at open time, in bounded 8 KiB chunks (never loading a whole large file into
  memory) - proportional to file size, unlike every other operation this seam performs. This is
  inherent to any content-based integrity signal (a cryptographic hash would pay the identical
  cost) and only archive requests it (`confirmed_move_inner`'s `compute_fingerprint` flag is
  `false` for `Quarantine`/`Restore`).

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
- **Cryptographic content hash: closed in round 3 (ADR-0030).** Round 1's FNV-1a fingerprint was
  judged insufficient in round 2 for a CR4 pre-purge predicate; `verify_archive_integrity` now
  checks a real SHA-256 digest instead, under a dedicated, reviewed kernel-ring dependency
  decision - see "Round 2 independent review and round 3 repair" below. No remaining
  cryptographic-hash residual for this story.
- **Windows cannot archive at all**, by explicit refusal rather than a silent gap - matching
  `Quarantine`/`Restore`'s own Unix-only scope in this epic so far.
- **No orphaned-sidecar cleanup** if a later story ever deletes an archived copy directly - same
  residual class E12-S02 already disclosed for its own restore-record sidecar.
- Coverage ratchet was not tightened, though every ratcheted crate measured above its recorded
  floor. Left to the owner/reviewer per `AGENTS.md`'s own instruction.
- This packet is executor self-assessment. This story is CR4 (see the reclassification note
  above): an independent adversarial pass **and an owner-visible Safety Verdict** are required
  before `done`, neither of which exists yet.

## Round 1 independent review and repair (2026-09-16)

Codex's round-1 review (`project/evidence/E12-VERIFIER-REVIEW.md`, `SAFETY_VERDICT.md` in this
directory) issued `FAIL`: `verify_archive_integrity` checked only byte length, so an archived
10-byte file rewritten to a different 10 bytes (`"0123456789"` -> `"abcdefghij"`) returned
`Verified` - a length match is a truncation/extension signal only, never a content one, and this
packet's own residual-risks section had already named the gap without treating it as
disqualifying for AC3.

**Repair**: `confirmed_move_inner` now optionally reads the source's content (only when
`compute_fingerprint` is set, which only `confirmed_archive_move_inner` does) through the same
already-open, already-identity-confirmed file descriptor its existing checks use, and computes
an FNV-1a-64 fingerprint in bounded chunks (`fingerprint_content`,
`rust/crates/cancellai-platform/src/mutation.rs`). `confirmed_archive_move_inner` writes it as a
third sidecar (`<destination>.archive-fingerprint.json`, hex `u64`, zero new dependencies) and
`verify_archive_integrity` now requires both length *and* fingerprint to match before returning
`Verified`. Sidecar-write failure for any of the three sidecars now rolls the move back, matching
E12-S01's own repair (both stories share `confirmed_archive_move_inner`'s write path).

This is disclosed, not overclaimed: FNV-1a is a fast, deterministic content fingerprint, not a
cryptographically collision-resistant digest. It closes the concrete finding (accidental/
incidental equal-length corruption, detected with overwhelming probability) without closing the
adversarial-preimage case, which still needs the ADR-0019-gated hashing dependency this packet's
own Scope section already deferred to a follow-up story before round 1 ever ran.

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
this directory) issued `FAIL` again, on two points: the equal-length corruption case is now
correctly caught, but (1) FNV-1a is expressly non-collision-resistant, which the verifier judged
unacceptable for a predicate meant to authorize a future irreversible source purge, and (2) this
story shares E12-S01's after-rename crash-recovery gap (see E12-S01's own evidence for that
finding in full). Required repair: "a reviewed ADR-0019 kernel-ring digest decision followed by a
versioned collision-resistant digest, plus E12-S01's durable journal/restart protocol for all
archive sidecars."

**Repair, digest**: [ADR-0030](../../../docs/adrs/0030-sha2-for-archive-integrity-in-cancellai-platform.md)
adds `sha2 = { version = "0.11.0", default-features = false }` to `cancellai-platform` - the exact
version/feature configuration `cancellai-safety` already uses for `KnowledgeBundle` digests, so
`Cargo.lock` resolves one shared `sha2`, not a new one. `digest_content` (superseding round 1's
`fingerprint_content`) computes a SHA-256 digest of the source's content through the same
already-open, already-identity-confirmed descriptor, streamed in bounded 8 KiB chunks.
`confirmed_archive_move_inner` writes it as `<destination>.archive-digest.json` (64 lowercase hex
characters), replacing the `archive-fingerprint` sidecar outright - a real digest supersedes the
fingerprint it stood in for; the two are not run alongside each other.
`verify_archive_integrity_detects_an_equal_length_content_corruption` still reproduces the exact
round-1 finding and now asserts `DigestMismatch` rather than `FingerprintMismatch`.

**Repair, crash recovery**: shared with E12-S01 - see that story's own evidence for the full
write-before-move/finalize/`recover_pending_moves` design. Archive now writes three pending
sidecars (record, length, digest) durably before the move;
`a_later_archive_sidecar_failing_to_write_durably_cleans_up_the_earlier_ones_too` proves a later
sidecar's write failure cleans up every earlier one already written, not only the one that failed.

## Verification Commands (round 3 repair)

```text
$ cargo fmt --check                                                        clean
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-pc-windows-gnu -- -D warnings                         clean
$ cargo clippy --workspace --all-targets --all-features \
    --target x86_64-unknown-linux-gnu -- -D warnings                      clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   all suites "test result: ok", 0 failed (cancellai-platform: 95 passed)
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
```

## Round 3 independent review and round 4 repair (2026-09-16)

Codex's round-3 review (`project/evidence/E12-VERIFIER-REVIEW-ROUND3.md`, `SAFETY_VERDICT.md` in
this directory) confirmed the round-2 SHA-256 digest repair closes that finding, but found the
round-3 crash-recovery repair itself carried a new defect shared with E12-S01: archive's three
pending sidecars (record, length, digest) could be finalized over an existing archive's
legitimate metadata by a stale, conflicting operation's pending sidecars, because recovery proved
only that *some* archived object existed at the destination name.

**Repair**: shared with E12-S01 - see that story's own evidence for the full identity-witness
design. Archive now writes a fourth pending sidecar per operation, the same
`<destination>.move-identity.json` witness quarantine writes, and `recover_pending_moves`
resolves all of a destination name's pending sidecars (record, length, digest, witness) as one
group, gated on that witness matching the real object's `(device, inode)`.

This is the owner-authorized fourth review round, exceeding ADR-0025's three-round cost ceiling
by explicit owner decision, because round 3 found a genuine, well-scoped CR4 defect rather than
a residual.

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
this directory) found the same shared recovery flaw E12-S01's evidence describes in full: a
partial new archive group could borrow an older, unrelated, fully-completed operation's finalized
witness. The fix is shared (`recover_pending_moves` no longer falls back to a finalized witness
at all) - see E12-S01's own evidence for the complete repair description.

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

Codex's round-5 review found the same shared recovery-ordering hazard E12-S01's evidence
describes in full (a third successive variant of the rounds-3/4 recovery hazard). The owner's
decision - to remove the automated `recover_pending_moves` mechanism outright rather than attempt
a fourth repair - is shared: see E12-S01's own evidence for the complete reasoning. Archive keeps
its write-before-move/fsync-durable/finalize-after-move protocol and its SHA-256 digest (both
unaffected by this reduction, and never themselves the subject of rounds 3-5's findings); a
finalize failure after a successful archive move now leaves that content durably recorded under
its own `.pending` name as a disclosed, deferred residual rather than an automated capability.

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

## Verifier verdict

pending (round 6, owner-authorized beyond the 3-round ceiling, reviewing a scope reduction rather than a further patch)

<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E06-S13
Rendered-by: Claude
Rendered-on: 2026-09-23
Brief-Checksum: 867c06801cf21e247190f5d88a7dbc7ea2ee9d08e699dcc6f61881702d181121

<!-- end handoff header -->
# Verifier Brief - E06-S13 - clean deletes on Windows through the identity-confirmed handle path

Status: ready_for_review | Change Risk: CR4
Outcome: E06-S09's kill harness, on its first Windows run, showed that `cancellai-cli clean` deletes nothing on Windows: every planned deletion is safely skipped with "identity-confirmed deletion is only implemented for plain files, not this target's kind". E20-S05 implemented Windows identity observation and a handle-relative `confirmed_delete_file` in `cancellai-platform`, and closed with an independent PASS, but `cancellai-safety::mutation_executor::delete_operation_for` still maps every `IdentityToken::Windows` to no operation - the second, independent backstop written before the Windows primitive existed, never revisited. The engine is therefore fail-closed on Windows, which is safe, while the cutover checklist reads G3 as ready, which is not true of the one mutating command. This story lets the executor select the delete operation for a Windows identity it can confirm is a regular file, and nothing else.
Dependencies: none

## Acceptance Criteria
- When the planned target's Windows identity is a regular, non-reparse file that the platform confirms at deletion time, the safety executor shall select the identity-confirmed delete operation and clean shall remove it.
- If a Windows target is a directory, a reparse point, a hard-linked file the identity cannot distinguish, or of any unconfirmed kind, then the executor shall refuse it as it does today.
- If the target is swapped between planning and deletion, then the deletion shall not reach the substitute, on Windows as on Unix.
- The kill harness and the CLI deletion tests shall run on Windows as well as Unix.

## Verification Contract
- Native Windows CI runs the CLI deletion tests and the full kill harness.
- Adversarial tests on Windows cover a directory, a reparse point and a target swap between plan and delete.
- An independent CR4 verifier reviews the executor change against E20-S05's primitive.

## Safety Obligations

### SI-013 Identity is revalidated immediately before mutation

A path alone is insufficient. Safety-critical object/root identity and relevant preconditions are re-observed at execution. Identity drift produces `STALE_PLAN`/block.

Implemented for artifact identity at `rust/crates/cancellai-safety/src/sealed_plan.rs::revalidate`
(E03-S02), consuming `cancellai-platform`'s `IdentityObserver` (E03-S01). Also implemented for
root identity: `mutation_executor::execute` (E03-S05) refuses unless `plan.root_identity()`
matches the actual target's bound root at execution time (E03 verifier review round 1 - see
SI-016 below). `cancellai-platform::mutation::MutationExecutor::mutate` (repaired in the same
round) additionally re-confirms a plain file's identity via an open file descriptor
immediately around the unlink syscall itself, narrowing (though, without an OS-specific
handle-relative unlink this workspace does not have, not perfectly closing) the residual
revalidate-then-delete race.

E07-S07 round-1 independent verifier review found the identical *shape* of race, but fully
closable this time, one layer up: `cancellai-cli::configure` re-checked `roots::is_symlink`
immediately before its own writes, and that re-check was still a separate syscall from the
path-based reads/writes that followed it, leaving a real window for a root-directory symlink
swap. Unlike the `MutationExecutor` file-unlink case above, `cancellai-sealedfs::SealedRoot`
(ADR-0017) closes this one completely rather than narrowing it: it retains an
`O_NOFOLLOW`-opened directory descriptor across every operation and issues them via
`openat`/`renameat` against that descriptor, so identity is not merely revalidated immediately
before mutation but bound for the mutation's entire duration - no path re-resolution ever
happens again after `establish` returns.

E07-S09 extends this to every component `establish` itself walks to reach that final
descriptor, not only the descriptor it ends with: each intermediate directory is opened via
`openat`/`O_NOFOLLOW` against the descriptor already held for its own parent, so an
intermediate symlink is refused at the instant it is reached rather than silently resolved by
the initial path-based lookup that used to precede the final `O_NOFOLLOW` open.

E07-S05 found that identity revalidation's own disambiguator could itself collide: real Linux
reproduction (a delete-and-recreate loop with no intervening delay, run inside a Docker
container to get a genuine Linux filesystem rather than macOS's) showed the freed inode reused
and `IdentityToken::Unix`'s whole-second `modified` unchanged in the overwhelming majority of
back-to-back iterations - `device`+`inode`+`kind`+`modified` alone is not always enough, exactly
as this invariant's own "identity is insufficient" framing already warns against trusting a
weaker fact. `modified_nanos` (`rust/crates/cancellai-platform/src/identity.rs`), the raw
sub-second `st_mtime_nsec` remainder, and its use in `cancellai-platform::mutation`'s own
`confirmed_delete_file_inner` open-time and immediately-before-unlink checks (which previously
compared device+inode only, not going through `IdentityToken` at all), close the gap for any
real-world timing gap larger than the underlying clock's own granularity (measured directly at
roughly 1ms in one containerized environment) - see SI-017 below for why an artificially
zero-delay test fixture, not the disambiguator itself, needed the accompanying correction.

### SI-017 Platform-native identity semantics

Unix inode/device assumptions are not applied to Windows reparse/file identity or other platforms without a verified mapping. Unsupported identity semantics lower authority.

E07-S05's real Linux reproduction also disproved an assumption this codebase's own tests had
been making about Unix identity itself: `identity.rs`'s `toctou_file_deleted_and_recreated_
with_identical_content_still_changes_identity` asserted the recreated file's inode specifically
must differ ("recreation must allocate a new inode") - true on macOS/APFS in this workspace's
own CI, false on Linux under a zero-delay delete-recreate (inode reuse in ~98% of iterations
measured directly). That assertion tested an incidental, platform-varying implementation detail
rather than the actual invariant (the *whole* `IdentityToken` differs) and has been removed; the
fixture now inserts a small real-world-realistic delay (SI-013's revalidate-before-mutate always
follows a scan+plan+policy+confirmation cycle, never a zero-delay same-instant recreate) so the
underlying clock genuinely advances, without weakening the "byte-identical content" case the
test exists to prove.

E20-S01 (ADR-0020) implemented the Windows side of this invariant: `IdentityToken::Windows`
(`rust/crates/cancellai-platform/src/identity.rs`), backed by `GetFileInformationByHandle` via
`cancellai-sealedfs::observe_identity`, carries volume-serial-number/file-index identity
distinct from `Unix`'s device/inode - a Windows reparse point is classified from
`FILE_ATTRIBUTE_REPARSE_POINT` alone, never by reusing or comparing against Unix symlink
semantics. Covered by real adversarial fixtures (a genuine NTFS junction, a directory symlink,
hard-links, a synthetic cross-volume boundary case) and confirmed on real `windows-latest` CI
(`project/platforms.json`'s `windows.capabilities.identity.state` is `"verified"`, citing a
real, `gh`-confirmed successful `rust.yml` run as `verified_commit` -
`scripts/check_platforms.py` is the enforced, re-checkable source of truth for this claim, not
this paragraph). E20-S01 round-1 independent verifier review found an earlier version of this
same claim false for the commit range it actually reviewed (the branch had never been pushed) -
repaired and re-confirmed in the same round; see `docs/adrs/0020-windows-native-identity-via-
windows-sys.md`'s "Round-1 independent verifier review" section for the full account. A
genuinely unsupported non-Unix, non-Windows target still reports
`IdentityObservation::Unsupported`, unchanged.

### SI-019 One mutation boundary, evidence-gated

All filesystem/vendor mutations route through the safety executor. CR4 changes to this boundary require independent verification and owner-visible Safety Verdict.

Implemented at `rust/crates/cancellai-safety/src/mutation_executor.rs::execute` (E03-S05),
the sole production caller of `cancellai-platform::mutation::MutationExecutor`.
`scripts/check_mutation_boundary.py` statically enforces that the raw OS primitive and the
capability wrapping it are referenced only from those two files - E03 verifier review round 1
found the capability itself was `pub`, re-exported at `cancellai_platform`'s crate root, and
directly callable (with an unconstrained raw path) by any crate that imported it; repaired by
removing the re-export and extending the static check (`docs/architecture/TARGET.md`,
`docs/architecture/PLATFORM_MODEL.md`).

## Documentation Impact
- docs/development/RELEASE_GATES.md
- docs/CLI_RUST.md
- CHANGELOG.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

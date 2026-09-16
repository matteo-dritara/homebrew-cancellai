<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E12-S02
Rendered-by: Claude (executor handoff to Codex)
Rendered-on: 2026-09-16
Brief-Checksum: 012e3eba7ba248700a0e05059af0773ce45b82f2a910312ece7ce419f62b0ea0

<!-- end handoff header -->
# Verifier Brief - E12-S02 - Restore protocol

Status: ready_for_review | Change Risk: CR4
Outcome: Restore quarantined artifacts only if destination safety and conflict policy are satisfied.
Dependencies: E12-S01

## Acceptance Criteria
- Restore never overwrites unrelated current provider state silently.
- Conflict outcomes are explicit: refuse, alternate location, or provider-specific restore.

## Verification Contract
- Destination-recreated and identity-conflict tests.

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

## Documentation Impact
- docs/security/THREAT_MODEL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

# Evidence Packet - E07-S09

- Commit/PR: `c519f86` (round 1), round-2 repair pending commit
- Executor: Claude
- Independent verifier: Codex (round 1: `project/evidence/E07-S09-VERIFIER-REVIEW.md`, `FAIL`;
  round 2 pending)
- Change Risk: CR4
- Spec version/commit: `project/evidence/E07-S07-VERIFIER-REVIEW-ROUND2.md` (the original
  reproduction and required repair), `project/evidence/E07-S09-VERIFIER-REVIEW.md` (round-1
  rejection: the repair did not reach `clean`), `docs/architecture/PLATFORM_MODEL.md`'s
  "Intermediate components need the same no-follow treatment as the leaf" and "The fix had to
  reach `clean`, not only `configure`" sections

## Outcome

PASS (executor self-assessment of the round-2 repair; independent verification pending, as CR4
requires)

## Round 2 repair (this update)

Round-1 independent verifier review (`project/evidence/E07-S09-VERIFIER-REVIEW.md`) found that
the whole-path walk closed the intermediate-link gap for `configure` (via `SealedRoot`) but not
for `clean`, which establishes its provider root through a different capability,
`cancellai-safety::ApprovedRoot::establish` - its own `canonicalize()` step (deliberate, for a
different purpose: catching a *candidate* escaping the root through a symlink, see
`root_capability.rs`'s module docs) silently resolves through the identical intermediate link.
Native reproduction confirmed: `$HOME` a symlink to an outside directory containing a real
`.claude` with a stale session - `clean --yes` deleted it while the already-repaired `configure`
correctly refused the same topology.

`cancellai-sealedfs` now exports `verify_no_intermediate_links(path)` - the identical
handle-relative, no-follow walk `SealedRoot::establish` performs, but read-only: it never calls
`mkdirat`, and returns `Ok(())` (not an error) when a component is absent, since `clean` must
never materialize a provider root that does not exist - the absence is left for
`ApprovedRoot::establish`'s own existing error to report. `cancellai-cli::establish_verified_root`
(the function `clean` already used for its leaf-only `roots::is_symlink` re-check) now calls
`verify_no_intermediate_links` immediately before `ApprovedRoot::establish`, for the default
root only (a custom root is never mutation-eligible for `clean` regardless of this check).

New end-to-end regression test, `clean_refuses_to_mutate_when_home_itself_is_a_symlink_to_a_real_dot_claude`
(`cancellai-cli/tests/cli_behavior.rs`) - the exact native reproduction from round 1, run as a
real built binary. Verified to fail without the fix: reverting only the `main.rs` change and
re-running this test reproduces round-1's exact escape (`clean --yes` exits `0`, deletes the
stale session; `succeeded: 1`) before the fix and exits `4` with the session intact after it.

E07-S07 round-1 bound only the sealed root's *final* path component with `O_NOFOLLOW`. Round-2
independent verifier review reproduced the consequence natively: with `$HOME` itself a symlink
to an outside directory and a real `.claude` directory underneath that outside target,
`configure --claude-retention 30` exited `0` and wrote through to the outside directory - the
leaf was a real, non-symlink directory, so round-1's check never had a reason to refuse it.

`cancellai-sealedfs::SealedRoot::establish` (`rust/crates/cancellai-sealedfs/src/lib.rs`) now
performs one handle-relative walk for the whole path, not only its last component: it opens the
filesystem root `/` (nothing upstream of it can have been swapped), then `openat`s each
subsequent component - intermediate or final - against the descriptor already held for its
parent, with `O_NOFOLLOW | O_DIRECTORY`, refusing the instant any of them is a symlink/reparse
point. Only the final, absent component may be created, via `mkdirat` against the already-held
parent descriptor - never `create_dir_all`'s path-based, potentially link-following recursive
creation. A relative path or a path containing a `.`/`..` component is refused outright
(`SealError::NotAbsolute`/`PathNotNormalized`) rather than resolved.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - A provider root reached through an intermediate Unix symlink is refused before any configuration or cleanup mutation reaches the symlink target | **configure**: `establish_refuses_a_root_reached_through_an_intermediate_symlink_component` (`cancellai-sealedfs/src/lib.rs`) - `establish(home_like.join("leaf"))` returns `Err(IsSymlinkOrReparsePoint)`; outside sentinel absent. **cleanup** (round-2 addition, closing the round-1 rejection): `clean_refuses_to_mutate_when_home_itself_is_a_symlink_to_a_real_dot_claude` (`cancellai-cli/tests/cli_behavior.rs`) - a real built `cancellai-cli clean --yes` against a symlinked `$HOME` with a real `.claude`/stale session underneath exits `4`; the session is asserted to still exist. Also unit-level: `verify_no_intermediate_links_refuses_an_intermediate_symlink`/`_refuses_a_symlinked_leaf_too` (`cancellai-sealedfs/src/lib.rs`). | PASS |
| AC2 - Unix root establishment walks every component handle-relatively from a trusted anchor, using no-follow directory opens and mkdirat-style creation only beneath an already-held parent handle | `establish_with_hook`'s walk: `open_root_dir()` opens `/` directly (the trusted anchor - `/` cannot itself be a symlink); every subsequent component is opened via `open_child_dir_nofollow`, which issues `openat(parent_fd, name, O_NOFOLLOW\|O_DIRECTORY)`; the leaf, if absent, is created via `libc::mkdirat(current.as_raw_fd(), leaf, 0o700)` against the already-held parent descriptor, then re-opened no-follow. Existing tests `establish_creates_an_absent_root_before_binding_it` and the new `establish_refuses_a_relative_path`/`establish_refuses_a_path_containing_dot_dot` cover the anchor/creation and refusal-to-resolve-`.`/`..` requirements. | PASS |
| AC3 - Windows junction/reparse handling either has equivalent verified handle-relative semantics with real junction fixtures or fails closed | Unchanged from E07-S07: `fallback_impl::SealedRoot::establish` on non-Unix targets always returns `SealError::Unsupported` - no code path attempts a Windows-specific walk. This is the disclosed fail-closed posture, not a new capability; a genuine Windows handle-relative walk remains E07-S02 scope (`docs/CLI_RUST.md`'s own "Known gaps"). | PASS (fail-closed, not implemented) |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-002 | Root positively bound only after every component - not only the leaf - is proven non-symlink, for both `configure` and `clean` | `establish_refuses_a_root_reached_through_an_intermediate_symlink_component`; `clean_refuses_to_mutate_when_home_itself_is_a_symlink_to_a_real_dot_claude` | PASS |
| SI-003 | Mutation cannot escape the approved root via an intermediate link, for both callers | Same tests; outside settings sentinel and outside stale session both asserted unchanged/present | PASS |
| SI-013 | Identity (of every component, not only the final one) is bound before any use, not merely re-checked | Handle-relative walk itself: each `openat` call resolves against an already-verified parent descriptor, never a re-derived path; `verify_no_intermediate_links` runs immediately before `ApprovedRoot::establish`'s own `canonicalize()` | PASS |
| SI-019 | Single mutation boundary unaffected | No change to `mutation_executor.rs`/`scripts/check_mutation_boundary.py`'s reach; `configure`'s `SealedRoot` write path and `clean`'s pre-`ApprovedRoot` check are the only things changed, neither is a new mutation path | PASS (`scripts/check_mutation_boundary.py check` unaffected, still passes) |

## Verification Commands

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check
python3 scripts/project_os.py check
python3 scripts/project_os.py generate
```

All green. `cargo deny check` shows only the three pre-existing unmatched BSD-2-Clause/
BSD-3-Clause/ISC license-allowance warnings (unrelated to this change, present before it).

## Compatibility

- Exercised on macOS (this executor's environment) and via the existing Linux/macOS tier-1 CI
  matrix (`rust.yml`) once merged. Windows is unaffected (still fails closed via
  `fallback_impl`, not exercised by this change).
- Test fixtures (`cancellai-sealedfs`'s own `TempDir`, and both `cancellai-cli/tests/
  cli_behavior.rs`'s and `cancellai-cli/tests/install_rollback.rs`'s `TempHome`) now
  canonicalize their temp base directory once, at creation, on a path the test harness itself
  just created - not a security-relevant resolution. Without it, macOS's `/tmp`/`/var`
  compatibility symlinks (`std::env::temp_dir()` returns a `/var/folders/...` path there, and
  `/var -> private/var`) would make the new strict whole-path walk refuse every existing
  `SealedRoot`/`configure`/`clean` test as "reached through an intermediate symlink" - a false
  positive from an OS-level symlink outside this story's threat model (same-user attacker
  control of paths under `$HOME`), not the attacker-planted symlinks these tests construct
  deliberately. Round 2 found `install_rollback.rs`'s copy of this same helper had been missed
  in round 1 - `cargo test --workspace` caught it immediately
  (`a_real_clean_touches_only_the_provider_artifact_it_deletes_nothing_else_anywhere` failed
  until it was fixed the same way).

## Performance / operability

- One `open`/`openat` syscall per path component instead of one `symlink_metadata` plus one
  `OpenOptions::open` for the whole path. For `$HOME/.claude`-depth paths (typically 3-5
  components) this is a small, fixed number of additional syscalls per `configure` invocation -
  not a hot path, no measurable operability impact expected.

## Documentation updated

- `docs/architecture/PLATFORM_MODEL.md` (declared documentation impact) - "Intermediate
  components need the same no-follow treatment as the leaf" section (round 1), plus new round-2
  subsection "The fix had to reach `clean`, not only `configure`".
- `docs/CLI_RUST.md` (declared documentation impact) - "Known gaps" gains the intermediate-
  component closure note, updated in round 2 to name both callers.
- `docs/security/SAFETY_INVARIANTS.md` (declared documentation impact) - SI-002 and SI-013
  sections extended with the E07-S09 closure, updated in round 2 to describe both the
  `SealedRoot`/`configure` and `verify_no_intermediate_links`/`clean` halves.
- `CHANGELOG.md` - Unreleased/Added entry, extended in round 2.
- `project/epics/E07.json`:
  - E07-S01's dependency corrected from `E06-S04` to `E03-S01`/`E04-S01` (see
    `project/evidence/E07-S01/DEPENDENCY_ESCALATION.md`'s "Resolution" section) - an
    independent, pre-existing control-plane conflict found and resolved as part of this same
    session, unrelated to this story's own fix but blocking otherwise.
  - E07-S09's own declared dependency on `E07-S07` was removed (now `[]`). `E07-S07`'s round-2
    independent verifier review is what created E07-S09 and its round budget for that
    *standalone story-level* review (distinct from the epic-wide review `scripts/
    check_process.py::check_review_rounds` mechanically caps - its regex `^(E\d{2})-VERIFIER-
    REVIEW.*\.md$` only matches epic-level files, not `E07-S07-VERIFIER-REVIEW*.md`) is already
    exhausted at 2 rounds, both `FAIL`; a same-epic dependency on `E07-S07` would require it to
    reach `ready_for_review` again to unblock E07-S09, which is indistinguishable from
    requesting a third round on E07-S07 itself - exactly what the round-2 record says not to do
    ("rather than requesting E07-S07 round 3"). The actual prerequisite - the `cancellai-
    sealedfs::SealedRoot` capability E07-S07 built - already exists in `main` regardless of
    E07-S07's own story status, so no dependency edge is needed for E07-S09 to build on it.
    Regenerated (`python3 scripts/project_os.py generate`).

## Residual risks

- Windows/reparse-point intermediate-component handling has no verified implementation
  (unchanged residual, E07-S02 scope) - `SealedRoot::establish` continues to fail closed there.
- The `mkdirat`-then-reopen sequence for an absent leaf has its own narrow window (between the
  no-follow lookup that produced `NotFound` and the `mkdirat` call) in which something could be
  concurrently created at that name; the code handles this by treating `EEXIST` from `mkdirat`
  as "something is there now" and falling through to the same no-follow open used everywhere
  else, which accepts a real directory and refuses a symlink either way - not a silent gap, but
  worth an independent verifier's own adversarial pass rather than only this packet's word for
  it.
- This packet is executor self-assessment of executor-produced work - `AGENT_PROTOCOL.md` is
  explicit that a verifier does not treat executor tests as proof. It is a starting point for
  independent review, not a substitute for it.

## Verifier verdict

PENDING

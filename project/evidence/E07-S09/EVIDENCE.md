# Evidence Packet - E07-S09

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, standalone CR4 carry-forward review, per E07-S07's own
  precedent)
- Change Risk: CR4
- Spec version/commit: `project/evidence/E07-S07-VERIFIER-REVIEW-ROUND2.md` (the reproduction
  and required repair this story closes), `docs/architecture/PLATFORM_MODEL.md`'s new
  "Intermediate components need the same no-follow treatment as the leaf" section

## Outcome

PASS (executor self-assessment; independent verification pending, as CR4 requires)

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
| AC1 - A provider root reached through an intermediate Unix symlink is refused before any configuration or cleanup mutation reaches the symlink target | New test `establish_refuses_a_root_reached_through_an_intermediate_symlink_component` (`cancellai-sealedfs/src/lib.rs`) - native reproduction of the round-2 verifier's `configure` counterexample at the `SealedRoot` layer: a `home-like` directory symlinked to an `outside` directory, with a real `outside/leaf` directory. `establish(home_like.join("leaf"))` returns `Err(IsSymlinkOrReparsePoint)`; the outside sentinel is asserted absent. Since `establish` fails, no `SealedRoot` instance exists to write through - `configure`'s write path (`configure_claude_retention`) cannot reach the outside directory by construction, not merely by this test's own assertion. | PASS |
| AC2 - Unix root establishment walks every component handle-relatively from a trusted anchor, using no-follow directory opens and mkdirat-style creation only beneath an already-held parent handle | `establish_with_hook`'s walk: `open_root_dir()` opens `/` directly (the trusted anchor - `/` cannot itself be a symlink); every subsequent component is opened via `open_child_dir_nofollow`, which issues `openat(parent_fd, name, O_NOFOLLOW\|O_DIRECTORY)`; the leaf, if absent, is created via `libc::mkdirat(current.as_raw_fd(), leaf, 0o700)` against the already-held parent descriptor, then re-opened no-follow. Existing tests `establish_creates_an_absent_root_before_binding_it` and the new `establish_refuses_a_relative_path`/`establish_refuses_a_path_containing_dot_dot` cover the anchor/creation and refusal-to-resolve-`.`/`..` requirements. | PASS |
| AC3 - Windows junction/reparse handling either has equivalent verified handle-relative semantics with real junction fixtures or fails closed | Unchanged from E07-S07: `fallback_impl::SealedRoot::establish` on non-Unix targets always returns `SealError::Unsupported` - no code path attempts a Windows-specific walk. This is the disclosed fail-closed posture, not a new capability; a genuine Windows handle-relative walk remains E07-S02 scope (`docs/CLI_RUST.md`'s own "Known gaps"). | PASS (fail-closed, not implemented) |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-002 | Root positively bound only after every component - not only the leaf - is proven non-symlink | `establish_refuses_a_root_reached_through_an_intermediate_symlink_component` | PASS |
| SI-003 | Mutation cannot escape the approved root via an intermediate link | Same test; sentinel file under `outside/leaf/settings.json` never created | PASS |
| SI-013 | Identity (of every component, not only the final one) is bound before any use, not merely re-checked | Handle-relative walk itself: each `openat` call resolves against an already-verified parent descriptor, never a re-derived path | PASS |
| SI-019 | Single mutation boundary unaffected | No change to `mutation_executor.rs`/`scripts/check_mutation_boundary.py`'s reach; `configure`'s own single write path (`SealedRoot`) is the only thing changed | PASS (`scripts/check_mutation_boundary.py check` unaffected, still passes) |

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
- Test fixtures (`cancellai-sealedfs`'s own `TempDir`, `cancellai-cli/tests/cli_behavior.rs`'s
  `TempHome`) now canonicalize their temp base directory once, at creation, on a path the test
  harness itself just created - not a security-relevant resolution. Without it, macOS's `/tmp`/
  `/var` compatibility symlinks (`std::env::temp_dir()` returns a `/var/folders/...` path there,
  and `/var -> private/var`) would make the new strict whole-path walk refuse every existing
  `SealedRoot`/`configure` test as "reached through an intermediate symlink" - a false positive
  from an OS-level symlink outside this story's threat model (same-user attacker control of
  paths under `$HOME`), not the attacker-planted symlinks these tests construct deliberately.

## Performance / operability

- One `open`/`openat` syscall per path component instead of one `symlink_metadata` plus one
  `OpenOptions::open` for the whole path. For `$HOME/.claude`-depth paths (typically 3-5
  components) this is a small, fixed number of additional syscalls per `configure` invocation -
  not a hot path, no measurable operability impact expected.

## Documentation updated

- `docs/architecture/PLATFORM_MODEL.md` (declared documentation impact) - new "Intermediate
  components need the same no-follow treatment as the leaf" section.
- `docs/CLI_RUST.md` (declared documentation impact) - "Known gaps" gains the intermediate-
  component closure note.
- `docs/security/SAFETY_INVARIANTS.md` (declared documentation impact) - SI-002 and SI-013
  sections extended with the E07-S09 closure.
- `CHANGELOG.md` - new Unreleased/Added entry.
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

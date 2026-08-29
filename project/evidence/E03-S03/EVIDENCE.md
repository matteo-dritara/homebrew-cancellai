# Evidence Packet - E03-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E03)
- Change Risk: CR4
- Spec version/commit: `rust/crates/cancellai-safety/src/root_capability.rs`, `rust/crates/cancellai-platform/src/path_resolver.rs` as added in this change

## Outcome

PASS

## A new capability seam this story needed: `PathResolver`

`docs/architecture/PLATFORM_MODEL.md` lists "path canonicalization/normalization" as its own
required platform capability, separate from filesystem object identity. `ApprovedRoot::bind`
needs to canonicalize a candidate (to resolve a symlink that might escape the root) before it
can check containment - calling `std::fs::canonicalize` directly from `cancellai-safety` would
have violated the very principle `root_capability.rs`'s own module doc states ("domain and
policy code consume capability results, not OS-specific syscalls") and the crate's now-updated
`lib.rs` claim ("this crate performs no OS calls of its own"). Added `PathResolver`
(`cancellai-platform/src/path_resolver.rs`), mirroring `Clock`/`FsObserver`/
`IdentityObserver`'s seam shape exactly: `SystemPathResolver` (real, wraps
`std::fs::canonicalize`) and `SyntheticPathResolver` (test-only injection). `ApprovedRoot::
establish`/`bind` take `&dyn PathResolver` alongside `&dyn IdentityObserver`.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - No mutation API accepts an unconstrained raw path | `BoundedPath` has private fields and exactly one path to construct one: a successful `ApprovedRoot::bind`. No other public constructor exists. A future mutation API (E03-S05) typed to take `BoundedPath` instead of `&Path`/`PathBuf` therefore cannot be called with an unconstrained raw path - this is a type-system fact, not a runtime check a call site could skip. | PASS (structural) |
| AC2 - Cross-root and root-self deletion are impossible through typed APIs | `bind_the_root_itself_is_rejected` (candidate == root, canonicalized) and `bind_a_path_outside_the_root_is_rejected` (candidate outside root's canonical prefix) both assert `Err`; `bind_a_symlink_that_escapes_the_root_is_rejected` proves the adversarial path-normalization case SI-003 names explicitly - a symlink lexically inside the root that resolves outside it is rejected, not silently followed, because `bind` canonicalizes (via `PathResolver`) before comparing. | PASS |
| AC3 - Mount/reparse boundary behavior is explicit per platform | Unix: `bind_rejects_a_candidate_on_a_different_device_via_synthetic_identity` proves a candidate whose observed device differs from the root's is refused (`CrossesFilesystemBoundary`), via `SyntheticIdentityObserver` since a real mount swap needs root privileges unavailable in this sandbox. Non-Unix: `establish`/`bind` call through `IdentityObserver`, which E03-S01 makes report `Unsupported` unconditionally off-Unix - `establish_fails_closed_when_root_identity_is_unsupported`/`bind_fails_closed_when_candidate_identity_is_unsupported` prove `Unsupported` is always refused, never treated as "boundary verified." This is an explicit refusal per platform (today: Unix has a real check, non-Unix refuses outright), not a silent no-op. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-002 (provider root positively bounded) | Root path does not exist / is unreadable / platform cannot establish its identity | `establish_fails_when_the_root_does_not_exist`, `establish_fails_closed_when_root_identity_is_unsupported` | PASS |
| SI-003 (mutation cannot escape or delete the approved root, including via link indirection) | Candidate is the root itself; candidate is outside the root; a symlink inside the root resolves outside it | `bind_the_root_itself_is_rejected`, `bind_a_path_outside_the_root_is_rejected`, `bind_a_symlink_that_escapes_the_root_is_rejected` | PASS |
| SI-018 (filesystem/volume boundaries explicit) | A candidate resolves onto a different device than the root (a mount swapped in under an approved root) | `bind_rejects_a_candidate_on_a_different_device_via_synthetic_identity` | PASS |
| SI-017 (unsupported identity semantics lower authority, not guessed) | Root or candidate identity is `Unsupported` | `establish_fails_closed_when_root_identity_is_unsupported`, `bind_fails_closed_when_candidate_identity_is_unsupported` | PASS |
| Fail-closed, not vacuous | A genuinely valid child of a genuinely valid root | `bind_a_plain_child_succeeds` - without this, every rejection test above would be trivially true of a function that always refuses, which would not prove the boundary check is *selective*, only that it blocks. | PASS |
| Racy candidate | Candidate cannot be resolved at bind time (a dangling symlink standing in for "vanished between listing and binding") | `bind_fails_when_the_candidate_no_longer_exists` | PASS |

## Verification Commands

```text
# Python governance (repository-wide, unaffected by this Rust-only change)
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py scripts/diff_harness.py scripts/check_rust_workspace.py
python3 scripts/check_docs.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/project_os.py check

# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# Cross-platform compile verification (see E03-S01's evidence for why)
cargo check -p cancellai-model -p cancellai-platform -p cancellai-safety --target x86_64-pc-windows-gnu --all-targets
cargo check -p cancellai-model -p cancellai-platform -p cancellai-safety --target x86_64-unknown-linux-gnu --all-targets
cargo clippy -p cancellai-model -p cancellai-platform -p cancellai-safety --target x86_64-pc-windows-gnu --all-targets --all-features -- -D warnings
```

All passed. `cargo test -p cancellai-platform` now runs 19 unit tests (12 prior + 7 new
`path_resolver` tests) plus the 4 `determinism.rs` integration tests. `cargo test -p
cancellai-safety` runs 16 (7 `sealed_plan` + 9 new `root_capability`, 2 of which -
`bind_a_symlink_that_escapes_the_root_is_rejected` and
`bind_fails_when_the_candidate_no_longer_exists` - are `#[cfg(unix)]`-gated since they create
real Unix symlinks).

The cross-target `cargo clippy` run for Windows caught a real defect during development: the
two `#[cfg(unix)]`-gated tests above initially had no `cfg` gate and called
`std::os::unix::fs::symlink` unconditionally, which does not exist under
`std::os::windows` - `error[E0433]: could not find 'unix' in 'os'`, a hard compile failure on
Windows CI's `windows-latest` matrix leg, not merely a warning. Fixed before this packet was
written (gated both tests `#[cfg(unix)]`, matching E03-S01's own precedent for the same class
of mistake), not left for CI to discover first.

## Compatibility

- Unix (macOS, Linux): fully implemented and tested against a real filesystem (root-self,
  outside-root, and symlink-escape cases) plus synthetic identity injection for the
  mount-boundary and unsupported-identity cases a sandbox cannot construct for real.
- Windows and any other non-Unix target: `establish`/`bind` refuse outright today, inheriting
  E03-S01's `Unsupported` posture (see AC3). No destructive capability can be granted on such
  a platform through this seam; this is unchanged risk, not new risk, since no mutation
  capability exists yet at all (E03-S05).

## Performance / operability

- `establish`/`bind` are each one `canonicalize` call plus one `observe` call; cost is
  dominated by whatever `PathResolver`/`IdentityObserver` implementation is wired in.

## Documentation updated

- `docs/architecture/PLATFORM_MODEL.md` - "Boundary rules" section now states the Rust
  implementation, the `PathResolver` seam, and the non-Unix refusal posture (the story's
  declared documentation impact).

## Residual risks

- The symlink-escape defense (`bind` canonicalizing the candidate before checking
  containment) is bind-time only. A symlink/mount swap that happens strictly *between* a
  successful `bind` and the eventual mutation is not this story's concern - closing that
  window is exactly SI-013/`revalidate`'s job (E03-S02), wired in immediately before mutation
  by E03-S05, which does not exist yet. `BoundedPath` alone does not yet guarantee
  end-to-end TOCTOU safety; it guarantees the boundary was correct at bind time.
- `ApprovedRoot`/`BoundedPath` are not wired into anything yet (same posture as E03-S01/
  E03-S02's types) - there is no real mutation API today for "no mutation API accepts an
  unconstrained raw path" (AC1) to protect; that claim is about the *type's* shape, provable
  now, not about an executor that exists to be tested against it (E03-S05).
- Root-directory recursive boundary crossing *inside* the root (a child directory that is
  itself a mount point for a different filesystem, several levels deep) is caught only if the
  specific candidate `bind` is called with resolves onto that mount - a caller that binds a
  parent directory and then recurses into it manually, without calling `bind` again per
  descendant, would not get this check for free. A real recursive-mutation walker (E03-S05 or
  later) needs to call `bind` per boundary-relevant descendant, not just once at the top.
- Compile-time enforcement of "no unconstrained raw path" (AC1) is verified by code
  inspection (no public `BoundedPath` constructor exists), not by an automated compile-fail
  test - this project does not currently depend on a `trybuild`-style crate, and adding one
  for a single story-level check was judged disproportionate scope for what inspection already
  establishes with high confidence.

## Verifier verdict

PENDING - epic E03 review runs once every story in E03 is `ready_for_review` (at most twice per epic, per ADR-0014).

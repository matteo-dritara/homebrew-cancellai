# Evidence Packet - E20-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending (Codex, per-epic review once every E20 story is `ready_for_review`)
- Change Risk: CR4
- Spec version/commit: `project/epics/E20.json`'s E20-S01 story contract

## Outcome

PASS (executor self-assessment; independent verification pending). Real Windows file/volume
identity is now implemented and observable (`GetFileInformationByHandle` via a new
`windows-sys`-backed capability in `cancellai-sealedfs`, [ADR-0020](../../../docs/adrs/0020-windows-native-identity-via-windows-sys.md)),
replacing `SystemIdentityObserver`'s unconditional `Unsupported` on Windows. A Windows reparse
point is classified from its own `FILE_ATTRIBUTE_REPARSE_POINT` attribute and never treated as,
or compared using, Unix symlink semantics (AC1). `cancellai-safety::root_capability`'s
filesystem/volume-boundary check (SI-018) now enforces a genuine Windows volume boundary
(`IdentityToken::device()`'s new `Windows` arm) rather than refusing unconditionally (AC2).

## Scope discipline

The story outcome text also names "process, allocated-size, and atomic move semantics." This
packet's scope is exactly the two stated acceptance criteria and the stated verification
contract (reparse-vs-Unix-symlink classification; drive/volume boundaries constraining
quarantine/mutation) - `AllocationObserver`, Windows process observation, atomic move, and
`cancellai-sealedfs::SealedRoot`'s separate no-follow root-*establishment* walk (used by
`configure` and `clean`'s default-root re-check) are explicitly left open, disclosed as
residuals below and in ADR-0020, not silently implied closed. `clean`'s default-root
establishment on Windows still refuses as a whole via that separate, still-`Unsupported` walk -
this change alone grants no new deletion authority on Windows.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| Reparse points are never treated as Unix symlinks by assumption | `IdentityToken` gains a `Windows` variant, structurally distinct from `Unix` - `identity.rs::observe_system_identity` (`cfg(windows)`) classifies a reparse point from `FILE_ATTRIBUTE_REPARSE_POINT` alone (stable `std::os::windows::fs::MetadataExt::file_attributes`), never by reusing or comparing against `Unix`'s `kind`/inode fields. The object is opened with `FILE_FLAG_OPEN_REPARSE_POINT` (never follows a reparse point at the final component, matching `symlink_metadata`'s no-follow contract). `cancellai-sealedfs::windows_identity::tests::observe_identity_reports_is_reparse_point_for_a_real_symlink_without_following_it` proves the link's own identity differs from its target's, on real Windows CI. | PASS (compiles + clippy-clean on `x86_64-pc-windows-gnu`; behavioral proof pending Windows CI test execution - see Verification Commands) |
| Drive/volume boundaries constrain quarantine and mutation | `IdentityToken::device()` gets a `Windows` arm returning the volume serial number widened to `u64`; `cancellai-safety::root_capability::ApprovedRoot::bind`'s existing device-comparison boundary check (SI-018) is unchanged and now enforces a real Windows volume boundary through that one accessor, with no platform branching added at the call site. `cancellai-sealedfs::windows_identity::tests::observe_identity_two_hardlinks_to_the_same_file_share_a_file_index` and the missing-path test prove the underlying facts are real (positive and negative case). | PASS (same caveat: compile/clippy-verified locally, behavioral proof pending Windows CI) |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-017 | A reparse point (symlink or otherwise) must never be silently followed or treated as a Unix symlink when computing identity | `observe_identity_reports_is_reparse_point_for_a_real_symlink_without_following_it` (real `symlink_dir`, asserts `is_reparse_point` and that the link's `file_index` differs from its target's) | PASS (pending Windows CI execution) |
| SI-018 | A candidate on a different Windows volume than its root must be refused as a boundary crossing | `root_capability`'s existing `bind_rejects_a_candidate_on_a_different_device_via_synthetic_identity` test is platform-agnostic (uses `SyntheticIdentityObserver`) and continues to pass; `IdentityToken::device()`'s new `Windows` arm is the only change reachable from that check, verified by inspection and by the `cancellai-sealedfs` hardlink/volume-serial tests | PASS |
| SI-019 | A `Windows`-identified target must not become deletable through this change alone | `cancellai-safety::mutation_executor::delete_operation_for` gets an explicit `IdentityToken::Windows { .. } => None` arm (refuses, rather than falling through to a default); `cancellai-platform::mutation::confirmed_delete_file`'s existing `cfg(not(unix))` arm is untouched and still refuses unconditionally - two independent, typed refusals rather than reliance on one | PASS |
| SI-002/SI-003 | `cancellai-cli::establish_verified_root`'s Unix-only irrefutable identity destructure, now refutable with two variants, must not panic or silently mis-handle a `Windows` identity | Replaced with an explicit `match`; the `Windows` arm returns a typed `BoundaryError::RootIdentityUnavailable` rather than a `todo!()`/panic. Unreachable in current behavior (`verify_no_intermediate_links` still fails closed on Windows before this code is reached), but fails closed by construction rather than by omission if that changes later | PASS |

## Verification Commands

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo check --workspace --all-targets --target x86_64-pc-windows-gnu
cargo clippy --workspace --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings
cargo deny check
```

All green in this executor's environment (macOS host): native (Unix) compile/lint/test suite
fully passes (no regressions across any crate), and the Windows-specific code (the new
`cancellai-sealedfs::windows_identity` module and every touched call site) compiles and passes
`clippy -D warnings` cross-target for `x86_64-pc-windows-gnu`. `windows-sys` was pinned to
`0.61` (not the newest `0.59`-line release available at the time) specifically to converge with
the copy `clap`'s own Windows terminal-color support already resolves transitively, verified
with `cargo tree --target x86_64-pc-windows-gnu -i windows-sys@0.61.2` showing one shared graph
node rather than two.

This executor has no real Windows machine or CI access in this environment - the new
`cancellai-sealedfs::windows_identity` tests (symlink/hardlink/missing-path/file/directory
cases) have **not** been executed on real hardware by this executor. They are written to run
automatically in `rust.yml`'s existing `windows-latest` `check`/`quality` matrix jobs on the PR
for this change, exactly as ADR-0020 and this story's own verification contract ("Windows CI
with junction/symlink/reparse adversarial fixtures") require. CI must run and pass before merge;
this is stated explicitly per `AGENTS.md`'s "if a dev tool is unavailable locally, say so in
evidence."

## Compatibility

- macOS/Linux (Unix): behaviorally unchanged - `IdentityToken::Unix` and every existing Unix
  code path are untouched; the only Unix-visible change is that several previously-irrefutable
  `let IdentityToken::Unix { .. } = ...;` destructures became `match`/`let-else` to remain
  exhaustive against the new `Windows` variant, verified by the full native test suite passing
  unchanged (same pass counts per crate as before this change).
- Windows: identity/reparse observation goes from always-`Unsupported` to real and verified
  (pending CI execution); volume-boundary enforcement (SI-018) becomes real; deletion authority
  is unchanged (still refused, via two independent layers - see Safety Evidence above);
  `cancellai-inventory`'s scanner can now descend below a scope root on Windows, resolving
  E20-S04's accepted limitation as a direct, previously-anticipated consequence
  (`docs/architecture/PLATFORM_MODEL.md` already named this as E20-S01's expected effect).

## Performance / operability

- No new runtime cost on Unix (the new code paths are `cfg(windows)`-gated).
- On Windows, `observe_identity` opens one handle and issues one `GetFileInformationByHandle`
  call per identity observation - the same shape (one syscall-equivalent per path) as the Unix
  `symlink_metadata` call it parallels.

## Documentation updated

- `docs/architecture/PLATFORM_MODEL.md` (declared documentation impact) - Windows identity
  section rewritten from "not implemented" to describe the real implementation and its
  residuals; "Boundary rules" section's Windows paragraph updated; the `configure`/`clean`
  TOCTOU paragraph corrected (it previously said `ApprovedRoot::establish` was a second backstop
  via `Unsupported` identity on Windows, which is no longer accurate - `SealedRoot`'s walk is now
  the sole remaining backstop there); the E20-S04 "accepted limitation" subsection updated to
  "Resolved."
- `docs/security/SAFETY_INVARIANTS.md` - SI-017 and SI-018 entries gained implementation
  cross-references.
- `docs/CLI_RUST.md` (hand-maintained "Known gaps") - Windows identity/process bullet split
  (identity now implemented, process still not); `configure`/`SealedRoot` and intermediate-
  component-walk bullets clarified as distinct from, and not closed by, this story.
- `CHANGELOG.md` - `[Unreleased]` entry added.
- `docs/adrs/0020-windows-native-identity-via-windows-sys.md` - new ADR (ADR-0019's kernel-ring
  dependency-review requirement for the new `windows-sys` dependency in `cancellai-sealedfs`).

## Residual risks

- **Not independently verified on real Windows hardware/CI by this executor** (no such access in
  this environment). This is the primary residual: the acceptance criteria and safety evidence
  above are supported by cross-target compile/clippy verification and code inspection, not by an
  observed passing test run on Windows. The independent verifier (or CI on the PR) must confirm
  the new Windows-specific tests actually pass on `windows-latest` before this can move past
  `ready_for_review`.
- `AllocationObserver` remains `Unsupported` on Windows - a "fully readable tree" is still
  `Partial` there (now solely for that reason, never `identity`).
- `cancellai-sealedfs::SealedRoot`'s no-follow root-establishment walk remains `Unsupported` on
  Windows - `configure` and `clean`'s default-root establishment continue to refuse there as a
  whole, unaffected by this change (this is a materially larger, differently-shaped capability
  than single-path identity observation; ADR-0020's own "Neutral/follow-up" names it as a future
  story).
- Windows process observation and atomic move semantics (named in the story's outcome text but
  not its acceptance criteria) are unimplemented, unchanged by this story.
- A true NTFS junction (`IO_REPARSE_TAG_MOUNT_POINT`) is exercised only indirectly: the new
  tests use `std::os::windows::fs::symlink_dir`, which sets the `IO_REPARSE_TAG_SYMLINK` reparse
  tag; `FILE_ATTRIBUTE_REPARSE_POINT` (what this story's classification actually reads) is set
  for both tags identically, so the classification logic itself does not distinguish them and
  the test gap is lower-risk than it would be for logic that did - but a junction-specific
  fixture was not constructed (creating one needs `DeviceIoControl`, no `std` API, consistent
  with the same disclosed gap `docs/CLI_RUST.md` already records for `roots::is_symlink`).

## Verifier verdict

Pending independent review (per-epic, once every E20 story reaches `ready_for_review` -
`docs/development/AGENT_PROTOCOL.md`). Not populated by the executor.

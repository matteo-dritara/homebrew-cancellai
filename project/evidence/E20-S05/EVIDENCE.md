# Evidence Packet - E20-S05

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: not yet run (this story is being set to `ready_for_review`; per
  `docs/development/AGENT_PROTOCOL.md`, epic-scope review runs once every story in E20 is
  `ready_for_review`)
- Change Risk: CR4
- Spec version/commit: `project/epics/E20.json`'s E20-S05 story contract
- Dependencies: E20-S01 (native Windows volume/file-index identity), which this story extends

## Outcome

Implementation complete against all four acceptance criteria. Local verification (native
macOS unit tests for the Unix/shared surface; Windows cross-compilation check/clippy for the
new `cfg(windows)` code) is green. Real Windows CI found three real issues, across two rounds,
that this session's own cross-compilation-only local checks could not: a stale
integration-test assumption (`configure` used to refuse outright on Windows; E20-S05 makes it
succeed - fixed by updating the test); a genuine over-broad Windows access-rights bug in
`nt_open_child` on the *read* path (`FILE_LIST_DIRECTORY` requested where only
`FILE_READ_ATTRIBUTES`/`FILE_TRAVERSE` were actually needed, denied by a GitHub Actions
runner's own workspace-ancestor directory ACL); and, once that fix let real CI reach the *write*
path for the first time, a second, independent `NtCreateFile` parameter-validation bug
(`FILE_OPEN_REPARSE_POINT` paired with `FILE_CREATE`, a combination NT rejects outright with
`STATUS_INVALID_PARAMETER`). See ADR-0020's own "Real Windows CI found a genuine over-broad
access-rights bug" and "Real Windows CI found a second, independent bug on the write path"
sections for the full analysis of each. All three are fixed in this commit range; **the write-
path fix itself has not yet been confirmed on real Windows CI** - this session's own
established pattern (E20-S01's round-1/round-2 history, now repeated twice more here) is that
cross-compilation catches compile errors but not runtime/ACL/parameter-validation logic bugs,
so `project/platforms.json`'s `windows.capabilities.mutation.state` is deliberately left at
`unsupported` in this commit and will only move to `verified` once a real, `gh`-confirmed
successful `rust.yml` run exists for a commit in this range (`scripts/check_platforms.py`'s
own enforced bar, not this packet's word).

## What changed

### AC1: Windows process observation

- `cancellai-sealedfs::windows_process` (new): `list_running_process_names()` via
  `CreateToolhelp32Snapshot`/`Process32FirstW`/`Process32NextW` (`kernel32.dll`) - no
  elevated privilege needed, unlike opening a handle to each process individually.
- `cancellai-platform::process::observe_system_processes` gained a `cfg(windows)` arm that
  calls it, strips the `.exe`/`.EXE` suffix, and matches case-insensitively; the existing
  `cfg(unix)` `ps`-based path is unchanged. A narrower `cfg(not(any(unix, windows)))` fallback
  keeps the old unconditional `complete: false` only for a genuinely exotic third platform.

### AC2: Windows allocated-size observation

- `cancellai-sealedfs::windows_allocation` (new): `observe_allocated_size()` via
  `GetFileInformationByHandleEx(FileStandardInfo)`'s `AllocationSize` field, against a handle
  opened with the same no-follow (`FILE_FLAG_OPEN_REPARSE_POINT`) primitive
  `windows_identity.rs` already established.
- `cancellai-platform::allocation` gained a matching `cfg(windows)` arm, with the
  `cfg(not(any(unix, windows)))` fallback narrowed the same way as AC1.

### AC3: Windows handle-relative, no-follow root-establishment walk

- `cancellai-sealedfs::windows_sealed` (new, the largest addition): a Windows `SealedRoot`/
  `VerifiedPath` implementation mirroring `unix_impl`'s shape and safety properties, built on
  the NT native API (`NtCreateFile`, reached via `windows-sys`'s `Wdk::Storage::FileSystem`
  feature module) rather than ordinary Win32 `CreateFileW` - `NtCreateFile`'s
  `OBJECT_ATTRIBUTES.RootDirectory` field is the handle-relative anchor Windows has no direct
  `openat`-equivalent for elsewhere. Each path component is opened relative to the descriptor
  already held for its parent, refusing outright (never following) the moment any component -
  intermediate or the leaf - is a reparse point, closing the same TOCTOU class ADR-0017/
  E07-S09/E21-S07 close on Unix.
- `establish`/`bind_existing`/`read_child_to_string`/`write_new_child_atomically` (the last via
  a new `rename_child` helper using `FILE_RENAME_INFO`/`SetFileInformationByHandle
  (FileRenameInfo)`, the handle-based analogue of `renameat`) all have real Windows
  implementations now; `verify_no_intermediate_links` does too.
- `cancellai-cli::main.rs`'s `establish_verified_root` and `configure_claude_retention` needed
  no new platform branching for the write/establish paths themselves - both already call
  `cancellai_sealedfs::SealedRoot::establish`/`bind_existing`, which resolves to the real
  Windows implementation automatically via `cancellai-sealedfs`'s existing `cfg`-gated type
  alias. `establish_verified_root`'s own identity-comparison block *did* need platform-specific
  branching (`VerifiedPath::matches_unix_identity` vs `matches_windows_identity` are different
  methods on different concrete types per platform), restructured with `cfg(unix)`/
  `cfg(windows)`/`cfg(not(any(unix, windows)))`-gated blocks.

### AC4: Real, identity-confirmed Windows file deletion

- `cancellai-sealedfs::windows_sealed::SealedRoot::unlink_child_matching_windows_identity`
  opens the child by name relative to the held directory descriptor, confirms it is not a
  reparse point and that its `(volume_serial_number, file_index)` match the caller's expected
  identity, then marks it for deletion via `FILE_DISPOSITION_INFO{DeleteFile: true}` +
  `SetFileInformationByHandle(FileDispositionInfo)` (the classic disposition, not the newer
  POSIX-semantics variant, matching this crate's stated preference for the broadly-compatible
  primitive). `SealedRoot::is_delete_pending` reads `FILE_STANDARD_INFO.DeletePending` off an
  already-open handle - the Windows analogue of the Unix path's post-unlink link-count check.
- `cancellai-sealedfs::windows_identity::open_and_observe_identity` (new) opens a path once and
  returns both the retained `File` handle and its facts, so the caller can hold the exact
  pre-delete handle for post-delete corroboration rather than a fresh, unprotected path-based
  reopen after the delete call.
- `cancellai-platform::mutation`'s `cfg(windows)` `confirmed_delete_file`/
  `confirmed_delete_file_inner` mirror the Unix path's three-check shape exactly: (1) open the
  target once via `open_and_observe_identity` and confirm `(volume_serial_number, file_index,
  last_write_time_ticks)` against the expected `IdentityToken::Windows`; (2) a second,
  independent, fresh path lookup (`cancellai_sealedfs::observe_identity`) immediately before
  the delete call re-confirms the same identity; (3) `SealedRoot::bind_existing` +
  `unlink_child_matching_windows_identity` perform the actual, handle-relative delete; (4)
  `SealedRoot::is_delete_pending`, queried against the still-open handle from step (1) *before*
  it is dropped, corroborates that the delete actually marked the confirmed object (not a
  different one) for removal. A `between_open_and_unlink` test hook, matching the Unix
  implementation's own, lets tests deterministically reproduce a mid-flight swap.
- `SealedRoot::is_delete_pending`'s visibility was widened from `pub(crate)` to `pub` (it was
  written in the same session as part of AC3's scaffolding, anticipating this exact caller) so
  `cancellai-platform` - a separate crate - can call it.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| Windows process/activity observation reports real facts | `cancellai_sealedfs::windows_process::tests::list_running_process_names_finds_this_test_processs_own_name`; `cancellai_platform::process` gains a `cfg(windows)` real-behavior test | Implemented; cross-compilation clean, real-CI confirmation pending |
| Allocated-size observation implemented, distinct from logical size | `cancellai_sealedfs::windows_allocation::tests` (nonzero/zero/missing cases); `cancellai_platform::allocation`'s `cfg(windows)` test | Implemented; cross-compilation clean, real-CI confirmation pending |
| Verified no-follow, handle-relative directory-establishment capability for Windows | `cancellai_sealedfs::windows_sealed::tests` (establish/bind_existing, real-junction refusal via `mklink /J`, intermediate-component refusal, read/write round-trip) | Implemented; cross-compilation clean, real-CI confirmation pending |
| Real Windows file deletion, identity-confirmed, passes adversarial fixtures (symlink swap, junction, TOCTOU) on real Windows CI before `docs/PLATFORMS.md` may record Windows mutation as verified | `cancellai_sealedfs::windows_sealed::tests::unlink_child_matching_windows_identity_deletes_only_on_a_real_identity_match`, `..._refuses_a_reparse_point_at_the_name`; `cancellai_platform::mutation::tests::windows_system_executor_deletes_a_real_file_confirmed_by_identity`, `windows_confirmed_delete_rejects_a_target_already_swapped_before_open`, `windows_confirmed_delete_detects_a_target_swapped_between_open_and_unlink`, `windows_the_unlink_refuses_a_name_that_no_longer_holds_the_confirmed_identity` | Implemented and locally test-passing (cross-compilation); `docs/PLATFORMS.md`/`project/platforms.json` intentionally NOT yet updated to `verified` - real Windows CI confirmation is this AC's own stated precondition, not yet met at commit time |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 (revalidate before mutate) | Target swapped for a different object between open-time confirmation and the delete call | `windows_confirmed_delete_detects_a_target_swapped_between_open_and_unlink`, `windows_the_unlink_refuses_a_name_that_no_longer_holds_the_confirmed_identity` - both assert the swap is refused and the replacement survives | PASS (cross-compiled; real-CI pending) |
| SI-013 (revalidate before mutate) | Target already swapped before the confirmed open even happens | `windows_confirmed_delete_rejects_a_target_already_swapped_before_open` | PASS (cross-compiled; real-CI pending) |
| SI-017 (platform-native identity, fail closed on the wrong shape) | A synthetically constructed `IdentityToken::Unix` passed to the Windows delete path (or vice versa) | `confirmed_delete_file_inner`'s `let IdentityToken::Windows { .. } = expected else { return Err(...) }` typed refusal, mirroring the existing Unix-side guard | PASS (structural; exercised by type system + existing Unix-side equivalent test pattern) |
| SI-018 (filesystem/volume boundary) | A reparse point (junction) planted at the child name being deleted | `unlink_child_matching_windows_identity_refuses_a_reparse_point_at_the_name` | PASS (cross-compiled; real-CI pending) |
| SI-019 (one mutation boundary) | New Windows delete code lives only inside `cancellai-platform/src/mutation.rs`, the one file `scripts/check_mutation_boundary.py` permits to delete | `python3 scripts/check_mutation_boundary.py check` | PASS |

## Verification Commands

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo check --workspace --all-targets --target x86_64-pc-windows-gnu
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
cargo deny check
python3 scripts/check_mutation_boundary.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/check_platforms.py check
python3 scripts/check_docs.py check
python3 scripts/check_process.py check
python3 scripts/release.py check
```

All green locally (macOS host; Windows via cross-compilation check/clippy, not execution).
Real Windows CI (`windows-latest`) execution of the new `#[test]` functions above is the
explicit precondition this story's own verification contract sets before `docs/PLATFORMS.md`
may record Windows mutation as verified, and is expected to run against the commit(s) this
packet accompanies once pushed - following this session's own established E20-S01 pattern
(cross-compilation missed a real runtime bug there; only actual `windows-latest` execution
caught it).

## Compatibility

- macOS/Linux (Unix): unaffected - every new file/arm is `cfg(windows)`-gated; the existing
  `cfg(unix)` code paths in `process.rs`/`allocation.rs`/`mutation.rs` are untouched.
- Windows: gains real process observation, allocated-size, handle-relative root establishment,
  and identity-confirmed deletion, where all four previously either refused unconditionally
  (`SealedRoot`, deletion) or reported an honest but permanently-incomplete/unsupported result
  (process, allocation).

## Documentation updated

- `docs/architecture/PLATFORM_MODEL.md` - the "Residual, deliberately out of E20-S01's scope"
  paragraph is replaced with a "Resolved by E20-S05" paragraph naming each new capability and
  its production call site; the allocated-size section, the SI-018 boundary section, and the
  `configure`/`clean` TOCTOU section's stale "remains `Unsupported` on Windows" claims are
  corrected in place.
- `docs/CLI_RUST.md` - the "Known gaps" entries for Windows process-liveness, the Windows
  `SealedRoot` walk, and Windows intermediate-component handling are updated to reflect real
  implementations rather than residuals.
- `docs/adrs/0020-windows-native-identity-via-windows-sys.md` - a new "E20-S05 extension"
  section, per this ADR's own "Supersession" clause, naming the new `windows-sys` surface added
  (no new dependency - the same pinned `0.61`).
- `docs/PLATFORMS.md` regenerated (`scripts/check_platforms.py generate`) - unchanged by this
  commit's own content, since `project/platforms.json` is deliberately not yet updated (see
  "Outcome" above); will be regenerated again once `windows.capabilities.mutation.state` moves
  to `verified` on real CI confirmation.
- `CHANGELOG.md` - new `[Unreleased]` entry naming all four capabilities.

## Residual risks

- **Real Windows CI has not yet run against this commit.** Every claim above about Windows
  behavior rests on cross-compilation (compiles, clippy-clean, correct per manual review of the
  `windows-sys`/NT-native-API surface against locally-cached crate source) and native-macOS
  execution of the Unix-side regression suite - not actual execution on Windows. This session's
  own E20-S01 history is direct evidence that cross-compilation alone is not sufficient: a real
  Windows CI run there caught a genuine runtime bug (a stale test) that every local check missed.
  This story is therefore *not* requesting closure without that confirmation - `project/
  platforms.json`'s `windows.capabilities.mutation.state` stays `unsupported` until it exists,
  and a follow-up commit will update it once a real, `gh`-confirmed successful `rust.yml` run is
  available, matching `scripts/check_platforms.py`'s own enforced bar.
- **`FILE_DISPOSITION_INFO`'s classic (non-POSIX-semantics) disposition** removes the directory
  entry once every handle to the object closes, not immediately on the `SetFileInformationByHandle`
  call itself - `confirmed_delete_file_inner` accounts for this (it queries `is_delete_pending`
  against the still-open pre-delete handle rather than assuming the object is already gone), but
  this remains a real difference from Unix `unlink`'s immediate directory-entry removal, worth
  keeping in mind for any future caller reasoning about ordering.
- **Atomic rename was implemented as an internal primitive** (`rename_child`, used by
  `write_new_child_atomically`), not as a directly caller-facing `SealedRoot` method - no
  production caller currently needs a standalone rename beyond the temp-name-to-final-name
  pattern `write_new_child_atomically` already performs. A future story needing a general
  Windows atomic move should confirm `rename_child`'s existing implementation meets that need
  before adding a second one.

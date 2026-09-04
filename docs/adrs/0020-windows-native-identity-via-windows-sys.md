# ADR-0020: Real Windows file/volume identity, via `windows-sys` inside `cancellai-sealedfs`

- Status: Accepted
- Date: 2026-09-04
- Owners: project owner
- Related: ADR-0015, ADR-0017, ADR-0019, E03-S01, E04-S01, E20-S01, SI-002, SI-017, SI-018

## Context

`cancellai-platform::identity::SystemIdentityObserver` has, since E03-S01, reported
`IdentityObservation::Unsupported` unconditionally on every non-Unix target rather than
guess at Windows volume/file-index/reparse identity without a Windows machine to verify it
against (SI-017, C-12). `docs/architecture/PLATFORM_MODEL.md` and ADR-0017's own
"Neutral/follow-up" section both name this as E20-S01's job once it can be exercised on
Windows CI (`rust.yml`'s `check`/`quality` jobs already run on `windows-latest`, real Windows,
not cross-compilation).

`std::os::windows::fs::MetadataExt` only stabilizes `file_attributes()`,
`creation_time()`/`last_access_time()`/`last_write_time()`, and `file_size()` (all since
1.1.0). `file_index()`, `volume_serial_number()`, and `number_of_links()` - the fields that
would give Windows an inode/device-strength identity - remain gated behind the
`windows_by_handle` nightly feature (tracking issue rust-lang/rust#63010), confirmed still
unstable on today's stable docs. Real Windows file/volume identity therefore cannot be
built from safe, stable `std` alone; it needs `GetFileInformationByHandle`
(`BY_HANDLE_FILE_INFORMATION`), a raw Win32 call `std` does not expose.

This is the same shape of gap ADR-0017 closed for `configure`'s TOCTOU: a kernel-ring crate
(`cancellai-platform`, per ADR-0019) needs one specific OS capability `std` cannot express
safely. `cancellai-sealedfs` is already this workspace's one FFI/`unsafe`-isolated crate
(`unsafe_code = "allow"` locally, `forbid` everywhere else); ADR-0017's own text names it as
the natural place to extend for "a genuine Windows reparse-safe handle implementation" rather
than adding a second unsafe-isolated crate or relaxing `cancellai-platform`'s own
`unsafe_code = "forbid"`.

Distinguishing a reparse point from an ordinary file/directory does **not** need this FFI call:
`file_attributes()` (stable) exposes the `FILE_ATTRIBUTE_REPARSE_POINT` bit directly, and
`OpenOptionsExt::custom_flags()` (stable) can request `FILE_FLAG_OPEN_REPARSE_POINT` so the
open itself does not follow the reparse point - both already safe, stable `std`. Only the
volume-serial-number/file-index pair needs the unsafe call.

## Decision

Add a Windows-only module to `cancellai-sealedfs` that:

- Opens the target with `std::fs::OpenOptions` plus `custom_flags(FILE_FLAG_BACKUP_SEMANTICS |
  FILE_FLAG_OPEN_REPARSE_POINT)` - `FILE_FLAG_BACKUP_SEMANTICS` is required to open a directory
  at all via `CreateFileW`; `FILE_FLAG_OPEN_REPARSE_POINT` is the Windows equivalent of
  `symlink_metadata`'s no-follow behavior, matching every other observer in this workspace
  (never silently resolving through the final reparse point).
- Calls `GetFileInformationByHandle` on the resulting handle (obtained safely via
  `std::os::windows::io::AsRawHandle`, owned and closed by the `File`'s own `Drop`) to read
  `BY_HANDLE_FILE_INFORMATION`'s `dwVolumeSerialNumber`, `nFileIndexHigh`/`nFileIndexLow`,
  `dwFileAttributes`, and `ftLastWriteTime`.
- Uses [`windows-sys`](https://crates.io/crates/windows-sys) `0.61` (MIT OR Apache-2.0,
  already inside `rust/deny.toml`'s allow-list) for the function signature and struct layout,
  rather than a hand-written `extern "system"` declaration. This mirrors ADR-0017's own
  reasoning for choosing `libc` over hand-rolled `extern "C"` syscalls, applied to the Windows
  side of the same tradeoff: `windows-sys` is Microsoft's own code-generated bindings
  (generated from the Win32 metadata that also produces the official C headers), used
  internally by `std` itself for its Windows backend, with zero risk of a hand-transcribed
  struct field, offset, or type being subtly wrong in a security-boundary crate. It is a
  raw-bindings-only crate (no runtime, no COM machinery, no build script beyond generated
  code), so it adds exactly the surface this one call needs and nothing else. `0.61` was
  chosen, rather than the newest-available `0.59` at the time this ADR was drafted, to
  converge with the copy `cancellai-cli`'s existing `clap` dependency already resolves
  transitively on Windows (via `anstream`/`anstyle-wincon`'s own `windows-sys` use) instead of
  adding a second, semver-incompatible resolved copy - verified with `cargo tree --target
  x86_64-pc-windows-gnu -i windows-sys@0.61.2` showing one shared node.
- Is scoped to `[target.'cfg(windows)'.dependencies]` in `cancellai-sealedfs/Cargo.toml`,
  mirroring `libc`'s existing `[target.'cfg(unix)'.dependencies]` scoping - non-Windows builds
  do not pull it in at all, and `cancellai-platform`/`cancellai-safety` gain no new dependency
  themselves (they already depend on `cancellai-sealedfs`).
- Feature-gates `windows-sys` to only `Win32_Foundation` and `Win32_Storage_FileSystem`, the
  two feature groups that contain `GetFileInformationByHandle`/`BY_HANDLE_FILE_INFORMATION`/
  the `FILE_FLAG_*` constants - `windows-sys` has no default features and no transitive
  dependencies of its own, so this stays a minimal, auditable addition.
- Keeps the unsafe surface to exactly one `unsafe` block (the FFI call itself), with a
  `// SAFETY:` comment, matching this crate's existing convention (`git grep unsafe
  rust/crates` still returns hits from exactly this one crate).

`cancellai-platform::identity::IdentityToken` gains a `Windows` variant (`volume_serial_number:
u32`, `file_index: u64`, `kind: FileKind`, `modified: Timestamp`, `modified_ticks: u64` - the
raw 100-nanosecond `FILETIME` remainder, the Windows analogue of the Unix variant's
`modified_nanos`, for the identical same-second delete-recreate disambiguation E07-S05 found
necessary there). `SystemIdentityObserver::observe` on `#[cfg(windows)]` calls the new
`cancellai-sealedfs` function instead of returning `Unsupported`; a genuinely unsupported/
exotic non-Unix, non-Windows target (`#[cfg(not(any(unix, windows)))]`) keeps the prior
fail-closed `Unsupported` behavior unchanged.

`IdentityToken::device()` is extended with a `Windows` arm returning `volume_serial_number as
u64`, rather than adding a second, parallel accessor - `cancellai-safety::root_capability`'s
SI-018 boundary check (`ApprovedRoot::bind`'s device comparison) then works unmodified for
Windows once identity is real, with no platform branching needed at that call site. (E03-S01's
original doc comment speculated a future Windows variant would need "its own volume identity
rather than reusing this accessor's meaning" - reusing it turned out simpler and avoids an
unused, duplicate accessor; the comment is updated in the same change.)

## What this does and does not close

- **Closed, and now actually confirmed on real Windows CI** (round-1 independent verifier
  review found this ADR had originally overstated this as already verified when it was not -
  see "Round-1 independent verifier review" below - this bullet is updated again now that it
  genuinely is): `IdentityObservation`/`IdentityToken` carry Windows identity strong enough to
  detect a TOCTOU replacement (SI-013) and a volume-boundary crossing (SI-018), not a
  plausible-but-unverified guess in the sense E03-S01 originally refused to ship.
  `project/platforms.json`'s `windows.capabilities.identity.state` is `"verified"`, citing
  `verified_commit` `8622405118127c723f559d5ccdffdd0b3d7e0568` - a real, `gh`-confirmed
  successful `rust.yml` run
  (https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/33886584899), not this
  prose, remains the enforced source of truth (`scripts/check_platforms.py check`) if this ever
  needs re-confirming after a future change. `cancellai-inventory`'s
  scanner (`scan::walk_directory`) can now descend below a scope root on Windows, since its own
  "an unconfirmed identity never earns a descend" gate (SI-017) is no longer permanently true
  there - the accepted limitation `docs/architecture/PLATFORM_MODEL.md` and E20-S04 recorded is
  resolved, not merely reduced.
- **Not closed, deliberately out of this ADR's/E20-S01's scope**: `AllocationObserver` remains
  `Unsupported` on Windows (a different Win32 call, `GetCompressedFileSizeW`, not needed for
  this story's acceptance criteria); `cancellai-sealedfs::SealedRoot::establish`/
  `verify_no_intermediate_links` (the handle-relative, no-follow *root-establishment* walk
  `configure` and `clean`'s default-root re-check use) still fail closed on Windows - this ADR
  gives Windows a way to *observe* identity/reparse status for a single path, not a
  no-follow, per-component walk from a trusted anchor to a leaf, which is a materially larger
  and differently-shaped capability (Windows has no direct `openat`-equivalent in the
  documented Win32 surface) left for E20-S05. `clean`'s default-root
  establishment on Windows therefore continues to refuse via that still-`Unsupported` walk,
  unaffected by this change. Native process observation and atomic move semantics (also named
  in E20-S01's outcome text) are likewise out of this change's scope; its acceptance criteria
  are exactly "reparse points are never treated as Unix symlinks by assumption" and "drive/
  volume boundaries constrain quarantine and mutation," both of which this ADR's identity work
  satisfies without needing either.

## Alternatives considered

### Hand-written `extern "system"` declaration for `GetFileInformationByHandle`, no new dependency

Avoids adding `windows-sys`. Rejected for the same reason ADR-0017 rejected hand-rolled
`extern "C"` libc signatures: a transcription error in a struct layout or calling convention in
a security-boundary crate risks silent, hard-to-detect incorrect behavior (a wrong field offset
would misread the volume serial number or file index and could make two different Windows
objects compare as the same identity) for no benefit over depending on Microsoft's own
generated bindings, which cannot have that specific error class.

### `winapi` instead of `windows-sys`

`winapi` is the older, now largely superseded crate for the same purpose; `windows-sys` is
Microsoft's current, actively maintained, code-generated replacement and is what `std` itself
now uses internally. Rejected in favor of the actively maintained option.

### The full `windows` crate instead of `windows-sys`

`windows` layers a higher-level, more ergonomic (COM-aware, `Result`-returning) API over the
same generated metadata `windows-sys` exposes raw. This call site needs exactly one flat
function call and one plain struct - `windows-sys`'s raw bindings are already sufficient and
avoid pulling in machinery (COM activation, higher-level error types) this crate has no other
use for.

### Add `unsafe_code` to `cancellai-platform` directly instead of extending `cancellai-sealedfs`

Rejected: this would create a second, independent unsafe surface in the kernel ring, undoing
ADR-0015/ADR-0017's stated goal that exactly one crate carries `unsafe`. `cancellai-platform`
already depends on `cancellai-sealedfs`, so routing through it costs nothing structurally and
keeps the audit surface in one place, exactly as ADR-0017's own follow-up note anticipated.

## Consequences

### Positive

- Windows file/volume identity is real code with real adversarial test coverage rather than a
  permanent, disclosed `Unsupported` gap - "CI-verified" specifically becomes true only once
  `project/platforms.json` records it, per the round-1 finding below.
- `cancellai-inventory`'s Windows traversal depth limitation (E20-S04) is resolved as a direct,
  anticipated consequence, not incidental scope creep - `docs/architecture/PLATFORM_MODEL.md`
  already named this as what E20-S01 would produce.
- The unsafe surface stays inside the one crate this workspace already trusts with it, and
  grows by exactly one call with a Microsoft-generated (not hand-transcribed) signature.

### Negative / cost

- A new external dependency (`windows-sys`), Windows-only, requiring the review this document
  constitutes (ADR-0019's kernel-ring bar).
- `AllocationObserver`, `SealedRoot`'s Windows walk, process observation, and atomic move
  remain open residuals on Windows, now more visible by contrast now that identity itself is
  resolved - tracked as follow-up story scope, not silently implied closed by this change.

### Neutral / follow-up

- A future story implementing `SealedRoot`'s Windows no-follow walk (unblocking `configure` and
  `clean`'s default-root establishment for real on Windows) may reuse this same `windows-sys`
  dependency and should re-read this ADR before choosing its own FFI approach.

## Safety and compatibility impact

- Change Risk: CR4 (E20-S01's own classification) - this changes what identity/boundary
  authority Windows can carry.
- Safety Invariants affected: SI-017, SI-018, SI-002 (root establishment now observes real
  identity on Windows where it previously always failed closed at the `ApprovedRoot` layer;
  `cancellai-cli`'s `establish_verified_root` still refuses Windows default-root establishment
  via `SealedRoot::verify_no_intermediate_links`'s continued `Unsupported`, so no new deletion
  authority is actually reachable on Windows from this change alone).
- Migration/rollback: reversible at the code level (revert to `Unsupported` on Windows); no
  persisted state or on-disk format changes.

## Supersession

If E20-S05 extends `cancellai-sealedfs`'s Windows surface to the no-follow walk itself
(closing the residual this ADR leaves open), record that extension here or in a superseding
ADR rather than silently expanding scope, mirroring how ADR-0017 records its own E21-S07
extension.

## Round-1 independent verifier review (2026-09-04)

Codex's round-1 review of this story's commit range (`project/evidence/E20-VERIFIER-REVIEW.md`,
`project/evidence/E20-S01/SAFETY_VERDICT.md`, verdict `FAIL`) found the original text of this
ADR, `docs/architecture/PLATFORM_MODEL.md`, `docs/security/SAFETY_INVARIANTS.md`, and this
crate's own module docs all stated Windows identity was "verified on real Windows CI" - false
for the commit range actually reviewed: the branch introducing this ADR had never been pushed,
so `git ls-remote origin refs/heads/main` showed `origin/main` still at the range base, and
`gh run list --commit <range head>` returned no runs at all. Every such claim in this ADR is
corrected in place above rather than left standing next to this note. The review also found two
concrete adversarial-fixture gaps this ADR's own "Decision" section did not disclose: no real
NTFS junction (`IO_REPARSE_TAG_MOUNT_POINT`) fixture existed (only a directory symlink,
`IO_REPARSE_TAG_SYMLINK`), and no test constructed an `IdentityToken::Windows` pair to exercise
`ApprovedRoot::bind`'s SI-018 boundary comparison (the existing cross-device test used Unix
tokens only).

Repaired in the same executor round: a real junction fixture
(`cancellai-sealedfs::windows_identity::tests::
observe_identity_reports_is_reparse_point_for_a_real_junction_without_following_it`), created
via the OS's own `mklink /J` rather than a hand-rolled `DeviceIoControl(FSCTL_SET_REPARSE_POINT)`
call - consistent with this ADR's own "prefer the audited primitive over a hand-transcribed one"
reasoning, applied to test fixtures as well as production code; a synthetic Windows-token
cross-volume boundary test pair in `cancellai-safety::root_capability`
(`bind_rejects_a_candidate_on_a_different_windows_volume_via_synthetic_identity` and its
same-volume positive counterpart); a best-effort real multi-volume test that probes actual
drive letters at runtime and disclosed-skips rather than assuming a specific one exists (GitHub's
own Windows runner `D:` drive has been added, undocumented, and removed across image versions);
and a `Timestamp::checked_sub` fix for a `FILETIME` pre-1970 saturation bug the verifier's Safety
Verdict separately flagged as a residual risk.

Structurally, `project/platforms.json`/`scripts/check_platforms.py` (E20-S03, hardened by this
same review round) is now the enforced source of truth for whether Windows identity is actually
CI-verified - it requires a `verified_commit` that is both a real git ancestor of `HEAD` and,
where `gh` can reach GitHub, a commit with a real successful `rust.yml` run, before
`identity.state` may say `"verified"`. This ADR's own prose is deliberately not that source of
truth any more, precisely because prose was what went stale here.

### Repair confirmed on real Windows CI (2026-09-04)

The repaired commit (`aaca5a0407d8731d837553e9bd7361cac63732b4`) was pushed and its real
`rust.yml` run (https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/33885998630)
found one genuine failure none of this session's cross-compile-only local checks could have
caught: `identity::tests::system_observer_reports_unsupported_off_unix`, a test predating this
story that still asserted the *old* pre-native-identity behavior. The failure's own captured
output is itself independent confirmation the new implementation is correct on real hardware -
`GetFileInformationByHandle` returned sane, real values
(`volume_serial_number: 4009161782, file_index: 281474976711284, modified: Timestamp(1788533409),
modified_ticks: 6553669`) - which is exactly why the stale assertion failed against it. Fixed in
a follow-up commit (`8622405118127c723f559d5ccdffdd0b3d7e0568`), whose own real `rust.yml` run
(https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/33886584899) passed every
job on all three platforms, both MSRV and stable. `project/platforms.json` cites this second
commit as `windows.verified_commit`, with `identity.state` now genuinely `"verified"`.

## E20-S05 extension: the Windows no-follow walk, allocation, process, and mutation

Per this ADR's own "Supersession" section above, E20-S05 extended `cancellai-sealedfs`'s
`windows-sys` surface (still the same pinned `0.61` dependency, no new crate) rather than
adding a second one:

- `windows_sealed.rs` implements `SealedRoot`'s handle-relative, no-follow root-establishment
  walk for Windows via the NT native API (`NtCreateFile`, `Wdk::Storage::FileSystem`, reached
  through `windows-sys`'s `Wdk` feature module), using `OBJECT_ATTRIBUTES.RootDirectory` as the
  handle-relative anchor - the Windows analogue of `openat`'s directory-fd parameter, which
  ordinary Win32 (`CreateFileW`) has no equivalent for. Deletion uses
  `FILE_DISPOSITION_INFO`/`SetFileInformationByHandle(FileDispositionInfo)`; atomic rename
  (needed internally by `write_new_child_atomically`) uses `FILE_RENAME_INFO`/
  `SetFileInformationByHandle(FileRenameInfo)`.
- `windows_allocation.rs` implements allocated-size reporting via
  `GetFileInformationByHandleEx(FileStandardInfo)`.
- `windows_process.rs` implements real running-process enumeration via
  `CreateToolhelp32Snapshot`/`Process32FirstW`/`Process32NextW`.
- `cancellai-platform::mutation::confirmed_delete_file_inner`'s `cfg(windows)` arm mirrors the
  Unix path's three-check TOCTOU shape (open-time identity confirmation, a fresh re-check
  immediately before the delete call, post-delete corroboration via the same retained handle)
  using `SealedRoot::unlink_child_matching_windows_identity` and
  `SealedRoot::is_delete_pending` (`FILE_STANDARD_INFO.DeletePending`, the Windows analogue of
  Unix's post-unlink link-count check).

A real NTFS junction fixture (`mklink /J`, the same test-fixture technique this ADR's round-1
repair already established) exercises the walk's reparse-point refusal;
`project/platforms.json`'s `windows.capabilities.mutation.state`/`verified_commit` remain the
enforced source of truth for whether this has been confirmed on real Windows CI, updated once
that run is green.

### Real Windows CI found a genuine over-broad access-rights bug (2026-09-04)

The first real `windows-latest` execution of this extension's own `cargo test --workspace`
(run 33899508063) failed two `cancellai-cli` integration tests with `[INTERNAL_FAULT] Access
is denied. (os error 5)` - not a stale-test assumption this time (the first push's failures,
fixed in the prior commit, *were* stale assumptions; this was a second, independent, real bug
local cross-compilation and this crate's own unit tests never exercised).

Root cause: `nt_open_child`'s desired access requested `FILE_LIST_DIRECTORY` on every
component of the handle-relative walk, including intermediate directories this process does
not own (a GitHub Actions runner's own workspace-ancestor directories - `D:\a`, in the
integration test's real path - never created by this crate's own test fixtures). Windows
distinguishes two different rights here that are easy to conflate: `FILE_TRAVERSE` ("pass
through this directory to reach something inside it") is bypassed for virtually every
real-world token via the default-granted "bypass traverse checking" privilege
(`SeChangeNotifyPrivilege`), which is exactly why ordinary path-based resolution (`std::fs`,
used by every test fixture's own setup, and by this crate's own `open_anchor`) never hits this
wall regardless of who owns an intermediate directory. `FILE_LIST_DIRECTORY` ("enumerate this
directory's contents") is a real, non-bypassed ACL check with no such exemption - and this
crate never actually enumerates a directory's contents; it only ever opens one *named* child
at a time via `NtCreateFile`. The access right was requested out of habit ("opening a
directory, so ask for directory-reading rights"), not because anything in this module needed
it, and every one of `windows_sealed.rs`'s own pre-CI unit tests happened to only ever walk
directories the same test process created (and therefore owns), which is precisely why this
had never failed before reaching real, foreign-owned directories on real infrastructure.

Fixed by requesting exactly what this module needs per hop: `FILE_READ_ATTRIBUTES` (every
caller immediately runs `GetFileInformationByHandle` for the reparse-point/directory check)
and `FILE_TRAVERSE` (so the handle can serve as the next hop's `RootDirectory`) instead of
`FILE_LIST_DIRECTORY`. `read_child_to_string`'s separate, file-opening `NtCreateFile` call
switched from the same numeric bit spelled `FILE_LIST_DIRECTORY` (0x1, aliased with
`FILE_READ_DATA` for a non-directory object) to `FILE_READ_DATA` directly - functionally
identical, but no longer reads as "list directory" on a file open. This is a least-privilege
correction as well as a bug fix: nothing in this crate ever needed directory-enumeration
rights, on any object, at any point.

### Real Windows CI found a second, independent bug on the write path (2026-09-04)

The access-rights fix above was itself pushed and re-run on real `windows-latest` CI (run
33900213433) before this ADR section was written - discipline this session held to throughout
E20-S05 rather than assuming a fix is correct because it is well-reasoned. That run made
further real progress (the read-only `configure` integration test now passes; only the one
that also writes still failed), and surfaced a second, independent, real bug: `[INTERNAL_FAULT]
The parameter is incorrect. (os error 87)` - `STATUS_INVALID_PARAMETER` - the first time real
Windows CI ever exercised `write_new_child_atomically` (`configure`'s actual settings-write
path), which none of `windows_sealed.rs`'s own pre-CI unit tests or the first CI round reached
before the access-rights bug was fixed.

Root cause: `nt_open_child`'s `create_options` unconditionally included
`FILE_OPEN_REPARSE_POINT` regardless of `disposition`. That flag means "if a reparse point
already exists at this name, open the reparse point itself rather than following it" - a
question that presupposes something might already exist. `FILE_CREATE` disposition
(`write_new_child_atomically`'s own O_CREAT|O_EXCL-equivalent exclusivity guarantee) already
requires nothing exist at that name at all, succeeding only if the name is free - so pairing
`FILE_OPEN_REPARSE_POINT` with `FILE_CREATE` asks NT to resolve a question that cannot apply,
which `NtCreateFile` rejects outright with `STATUS_INVALID_PARAMETER` rather than silently
ignoring. Fixed by omitting `FILE_OPEN_REPARSE_POINT` from `create_options` exactly when
`disposition == FILE_CREATE` - `write_new_child_atomically`'s own adversarial fixture
(`write_new_child_atomically_refuses_a_pre_planted_reparse_point_at_the_temp_name`) keeps
passing unmodified, because the refusal it tests comes entirely from `FILE_CREATE`'s own
exclusivity (`STATUS_OBJECT_NAME_COLLISION` the instant anything - reparse point or not -
already occupies the name), never from this flag.

This second finding is the reason this ADR records both rounds separately rather than folding
them into one retrospective paragraph: each was a genuinely distinct bug, found only by an
actual Windows machine actually reaching that specific code path, in that specific order (read
path first, write path second) - exactly the incremental-discovery pattern real infrastructure
produces and a single local review pass would not.

### The `FILE_OPEN_REPARSE_POINT` fix alone was insufficient (2026-09-04)

The fix above was itself re-run on real Windows CI (run 33900644210) and the *exact same*
`STATUS_INVALID_PARAMETER` persisted on the same test - proof the `FILE_OPEN_REPARSE_POINT`
theory, while correct and worth keeping (see below), was not the complete explanation. Isolated
by comparing `write_new_child_atomically`'s still-failing `FILE_CREATE` open against
`read_child_to_string`'s own, separately-coded, already-proven-working `NtCreateFile` call for
an existing file: the only `create_options` flag present in the former and absent from the
latter was `FILE_OPEN_FOR_BACKUP_INTENT`. That flag exists to let backup software open
*directories* without the usual traverse-checking friction (the same role
`FILE_FLAG_BACKUP_SEMANTICS` plays for `CreateFileW` in `windows_identity.rs::open_no_follow`,
required there "to open a directory at all" per that module's own doc comment) - `nt_open_child`
applied it unconditionally to every open, directory or not, and NT rejects it combined with
`FILE_NON_DIRECTORY_FILE` and `FILE_CREATE`. Fixed by scoping `FILE_OPEN_FOR_BACKUP_INTENT` to
directory opens only, matching what `read_child_to_string`'s independently-written file-open
code had already (correctly, if not by explicit reasoning at the time) omitted.

Both flag corrections stand together in the fix that follows this section: `FILE_OPEN_REPARSE_
POINT` dropped for `FILE_CREATE`, `FILE_OPEN_FOR_BACKUP_INTENT` scoped to directories only. Both
are correct, least-privilege improvements to `nt_open_child` in their own right - but, as the
next section records, neither was actually the cause of the failure still being chased at this
point. That correction was only confirmed wrong by pushing it and watching it fail identically
on real CI again.

### A real, necessary-but-not-sufficient fix: a `FILE_RENAME_INFO` buffer-length mismatch (2026-09-04)

The `FILE_OPEN_FOR_BACKUP_INTENT` fix was also re-run on real Windows CI (run 33901257787) and
the identical `STATUS_INVALID_PARAMETER` persisted on the identical test, byte-for-byte the
same failure text as every prior round. That repetition was the actual signal: two
independent, plausible, individually-correct `nt_open_child` fixes in a row changed nothing
observable, which means the defect was never in `nt_open_child` - it was somewhere else in the
same call chain that neither fix touched.

Re-reading `rename_child` (called by `write_new_child_atomically` immediately after the create
succeeds, and never touched by either of the two `nt_open_child` fixes) found the real bug:
`FILE_RENAME_INFO`'s `FileName` field is documented as exactly `FileNameLength` bytes of raw
UTF-16, with no trailing NUL terminator, and `SetFileInformationByHandle`'s own `BufferLength`
parameter must equal `header_len + FileNameLength` precisely. This code pushed a NUL
terminator onto `new_wide` before measuring it, sized the byte `buffer` to include that NUL
(`new_wide.len() * 2`, NUL included), but set `FileNameLength` to the *shorter*, NUL-excluding
length - so the buffer passed to `SetFileInformationByHandle` was always exactly one `u16` (2
bytes) longer than what `FileNameLength` declared. That length mismatch is precisely the shape
of defect `STATUS_INVALID_PARAMETER` exists to report, and it explains every observation: it is
independent of, and untouched by, both `nt_open_child` access-flag fixes (which is why neither
changed the outcome), it only manifests on the write path (the only caller of `rename_child`),
and it would have failed identically regardless of which `nt_open_child` flags were set,
because `SetFileInformationByHandle` never reaches an access check for a length-mismatched
call at all.

Fixed by not NUL-terminating `new_wide` in the first place - `buffer`'s length now equals
`header_len + FileNameLength` exactly, with no discrepancy. This was a genuine bug and a real
fix, kept as-is going forward - but, as the next section records, it was not the *only* defect
on this call, and real Windows CI confirmed that the hard way: the identical
`STATUS_INVALID_PARAMETER` persisted through this fix too.

### The actual root cause: `SetFileInformationByHandle` doesn't honor a non-null `RootDirectory` (2026-09-04)

The buffer-length fix above was pushed and re-run on real Windows CI (run 33902370090) and the
identical failure persisted a fourth time. At this point the pattern itself - four consecutive,
individually well-reasoned fixes to the same general area, none changing the observable
outcome at all - was strong enough evidence that continuing to reason about the same function
was the wrong move. A permanent instrumentation change (adding a `contextualize` helper that
prefixes each fallible step's error with which step it came from, committed separately) was
pushed *before* attempting a fifth guess, specifically to stop guessing and start knowing. The
next run's error changed from a bare `os error 87` to `SetFileInformationByHandle
(FileRenameInfo): The parameter is incorrect. (os error 87)` - proof, for the first time in
this investigation, of exactly which call raises it: not `nt_open_child`'s create step (already
ruled out - it never appears in the context prefix), but `SetFileInformationByHandle` itself,
with an otherwise byte-correct `FILE_RENAME_INFO` buffer.

This is a known, documented gap: `SetFileInformationByHandle`'s Win32-level implementation does
not reliably honor a non-null `RootDirectory` field for `FileRenameInfo`-class handle-relative
rename, regardless of how correctly the rest of the buffer is constructed - the same category
of "no real Win32 equivalent" gap this whole module's own docs already state as the reason
`NtCreateFile` (not `CreateFileW`) is used for the establishment walk. The native
`NtSetInformationFile` (`ntdll.dll`) does not share this limitation. Fixed by calling
`NtSetInformationFile`/`FileRenameInformation` (the `Wdk` module's native counterpart of the
Win32 `FileRenameInfo` class) directly instead of `SetFileInformationByHandle` - the
`FILE_RENAME_INFORMATION` struct `windows-sys` generates for it has the identical field layout
to `FILE_RENAME_INFO`, so the buffer-construction code (including the length-mismatch fix from
the section above, which remains necessary and correct) is otherwise unchanged.

**Process note, recorded because it is the more durable lesson than the bug itself**: three of
this investigation's five total fixes (the two `nt_open_child` flag corrections plus the
buffer-length fix) were each independently reasonable and are each kept as genuine
improvements, but none was the actual cause of this specific failure. What actually converged
on the real bug was not sharper reasoning about the same function - it was (a) noticing that
repeated fixes were producing an *identical*, unchanged failure, which is itself informative
(it rules out everything already changed), and (b) adding error-message context *before*
guessing again, which turned the fifth attempt from another guess into a targeted fix backed
by direct evidence of which exact API call was failing.

**Lesson recorded plainly**: the two `nt_open_child` fixes were reasonable given the evidence
available at each point (a real, correct bug existed in that function, twice), but their
persistence past both should have prompted looking *outside* the function being repeatedly
patched sooner. Real Windows CI, re-run after every single change rather than assumed correct,
is what actually surfaced this - a local reasoning process fixated on `nt_open_child` could
have kept "fixing" that same function indefinitely without ever finding the real defect one
call further down the same code path.

### The `NtSetInformationFile` fix confirmed on real Windows CI, and a sixth, unrelated stale test found (2026-09-04)

The `NtSetInformationFile` fix was pushed and re-run on real Windows CI (run 33903352661).
`cancellai-cli`'s own full test suite - including, for the first time, the write-path
integration test this entire investigation exists to fix - passed completely (26/26). The
handle-relative write path (`configure`'s actual settings update) is confirmed correct on real
Windows hardware.

The same run surfaced one further, unrelated stale test: `cancellai-inventory::completeness::
tests::ac1_a_fully_readable_tree_is_partial_on_windows_pending_allocated_size` (an E20-S04
fixture) still asserted `Partial` with an `allocated_size`-unsupported reason - correct when
E20-S01 had implemented Windows identity but not allocated-size, now stale since E20-S05
implemented real Windows allocated-size (`GetFileInformationByHandleEx(FileStandardInfo)`) too.
With both capabilities real, a fully-readable tree is genuinely `Complete` on Windows, the same
as Unix - confirmed directly by this same CI run's own captured output. Fixed by retiring the
Windows-specific variant and unifying it with the pre-existing Unix test
(`ac1_a_fully_readable_tree_is_complete`), which now runs unconditionally on every platform
rather than being `#[cfg(unix)]`-gated - matching this session's own established pattern for
exactly this class of finding (E20-S01's `system_observer_reports_unsupported_off_unix` had the
identical shape: a platform-conditional test whose assumption a later capability made stale).

### Safety-critical correction: the `FILE_OPEN_REPARSE_POINT`/`FILE_CREATE` fix was itself wrong (2026-09-04)

The very next push (adding only the stale-completeness-test fix above, no `windows_sealed.rs`
change) reached `cancellai-sealedfs`'s own unit tests on real Windows CI for the first time in
this entire investigation - `cargo test`'s per-crate ordering had never gotten past
`cancellai-cli` before this point, so none of this crate's own adversarial fixtures had ever
actually run on real hardware yet, only cross-compiled. One of them failed:
`windows_sealed::tests::write_new_child_atomically_refuses_a_pre_planted_reparse_point_at_the_
temp_name` - the exact fixture proving a symlink pre-planted at the temp name is refused, not
written through - reported success where it must report refusal.

This traces directly back to the *first* of this investigation's `nt_open_child` fixes (the
section above titled "Real Windows CI found a second, independent bug on the write path"),
which had dropped `FILE_OPEN_REPARSE_POINT` for `FILE_CREATE` on the reasoning that `FILE_
CREATE`'s own exclusivity makes "follow or don't follow an existing reparse point" a question
that cannot apply. **That reasoning was wrong, and the error was not caught until this run**:
without `FILE_OPEN_REPARSE_POINT`, `NtCreateFile` does not treat an existing reparse point at
`name` as a plain name collision - it *follows* the reparse point as part of ordinary name
resolution before `FILE_CREATE`'s own exclusivity check is ever reached, so a pre-planted
symlink silently redirected the create to the symlink's target (potentially a location entirely
outside the sealed directory) and reported success rather than refusing. This is precisely the
attack class SI-002/SI-003/E07-S07's own O_CREAT|O_EXCL-equivalent protection exists to close -
a genuine, if narrowly-scoped and never-shipped, safety regression, not a cosmetic test failure.
It reached this point only because `windows_sealed.rs`'s own adversarial fixtures had not yet
run on real hardware even once across five prior pushes in this investigation, each of which
verified only via cross-compilation (which cannot detect this: the flag change compiles and
type-checks identically either way) and via `cancellai-cli`'s integration tests (which never
happen to plant a reparse point at a temp name, so never exercised this path).

Fixed by restoring `FILE_OPEN_REPARSE_POINT` unconditionally, including for `FILE_CREATE` -
this does not reintroduce the `STATUS_INVALID_PARAMETER` the earlier fix believed it was
solving, because that failure's actual, confirmed cause (the "actual root cause" section above)
was always `SetFileInformationByHandle`'s Win32-level `RootDirectory` limitation in
`rename_child`, fixed independently by switching to `NtSetInformationFile`, and entirely
unrelated to this flag.

**This finding changes the process lesson from the sections above, not just the outcome**: this
investigation's own real-Windows-CI discipline - re-running every single change rather than
trusting reasoning about NT semantics - is what caught this regression before it could ship,
but only once execution order happened to finally reach the crate whose own tests would have
caught it *first*, every time, had cargo's default test-binary ordering reached it sooner. The
`unlink_child_matching_windows_identity_refuses_a_reparse_point_at_the_name` and other
`windows_sealed.rs` adversarial fixtures passed on this same run, confirming the rest of this
module's reparse-point handling was never affected - the regression was scoped exactly to the
one flag change this section corrects.

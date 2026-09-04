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

- **Closed**: `IdentityObservation`/`IdentityToken` now carry real, verified Windows identity
  strong enough to detect a TOCTOU replacement (SI-013) and a volume-boundary crossing
  (SI-018), on real Windows CI, not a plausible-but-unverified guess. `cancellai-inventory`'s
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
  documented Win32 surface) left for a dedicated future story. `clean`'s default-root
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

- Windows file/volume identity is real and CI-verified rather than a permanent, disclosed gap.
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

If a future story extends `cancellai-sealedfs`'s Windows surface to the no-follow walk itself
(closing the residual this ADR leaves open), record that extension here or in a superseding
ADR rather than silently expanding scope, mirroring how ADR-0017 records its own E21-S07
extension.

# Platform Model

Cross-platform safety is not achieved by replacing `/` with `\\`.

## Required platform capabilities

The platform layer owns the OS-specific implementation of:

- path canonicalization/normalization;
- filesystem object identity;
- filesystem/volume identity;
- symlink, mount, junction, and reparse-point classification;
- logical and allocated-size observation;
- atomic rename/move capability;
- process/activity observation;
- user service installation/runtime;
- notifications;
- permission/error translation.

Domain and policy code consume capability results, not OS-specific syscalls.

## Identity token

A plan must bind to the object observed, not just its path.

### Unix-like systems

Identity evidence may include device ID, inode, file type, relevant metadata timestamps, and filesystem boundary.

E03-S01 implements this at `rust/crates/cancellai-platform/src/identity.rs`: `IdentityToken::Unix`
carries device, inode, `FileKind` (file/directory/symlink/other), and modification time,
observed via `symlink_metadata` (never following the final symlink, matching `FsObserver`).
`IdentityObserver` mirrors `FsObserver`'s seam - `SystemIdentityObserver` (production,
OS-backed) and `SyntheticIdentityObserver` (test-only, injects facts a sandbox cannot
construct for real, such as a mount-boundary device change) - and `IdentityObservation` adds
`Unsupported` to the existing `Absent`/`Unreadable` split (see below).

### Windows

Use native file/volume identity and reparse metadata. Do not assume Unix inode/symlink semantics are sufficient. Junctions and reparse points require explicit classification.

If the platform cannot produce an identity strong enough for a requested irreversible action, authority is reduced.

E20-S01 (ADR-0020) implements native Windows volume/file-index identity:
`SystemIdentityObserver::observe` on `cfg(windows)` calls
`cancellai-sealedfs::observe_identity` (`GetFileInformationByHandle`, verified on real Windows
CI - `windows-latest` in `rust.yml`, not cross-compilation), populating a distinct
`IdentityToken::Windows` variant (`volume_serial_number`, `file_index`, `kind`, `modified`,
`modified_ticks`) rather than reusing or extending the `Unix` variant's fields - a reparse
point is detected from `FILE_ATTRIBUTE_REPARSE_POINT` alone (stable `std`,
`file_attributes()`), never inferred from or compared against Unix symlink semantics. The
object is opened with `FILE_FLAG_OPEN_REPARSE_POINT`, so a reparse point at the final path
component is observed as itself, never silently followed - matching `symlink_metadata`'s
no-follow contract used throughout this workspace's other observers. Only a genuinely exotic
non-Unix, non-Windows target still reports `IdentityObservation::Unsupported`, for the reason
E03-S01 originally chose it for Windows too: a plausible-but-unverified implementation of a
safety-critical equality check is a worse outcome than an honest refusal (SI-017, C-12
cross-platform truthfulness).

`IdentityToken::device()` (SI-018 boundary checks, `cancellai-safety::root_capability`) returns
the Windows volume serial number widened to `u64` for the `Windows` variant, so `ApprovedRoot`'s
existing device-comparison boundary check works unmodified on Windows once identity is real,
with no platform branching at that call site.

**Residual, deliberately out of E20-S01's scope**: `AllocationObserver` remains `Unsupported`
on Windows (`GetCompressedFileSizeW`, a different Win32 call); `cancellai-sealedfs::SealedRoot`'s
handle-relative, no-follow *root-establishment* walk (`establish`/`verify_no_intermediate_links`,
used by `configure` and `clean`'s default-root re-check) still fails closed on Windows - this is
a materially different, larger capability (a per-component walk from a trusted anchor, which
has no direct `openat`-equivalent in the documented Win32 surface) than observing one path's
identity, and is left for a dedicated future story (ADR-0020's own "Neutral/follow-up"). Native
process observation and atomic move semantics are likewise not part of this change.

#### Resolved: the inventory scanner can now descend below the scope root on Windows

E20-S04 (formerly E07-S06) found `cancellai-inventory::scan::walk_directory` only recurses into
a child whose identity is *confirmed* (`IdentityObservation::Identity`, never `Unsupported`)
and that does not cross the scope's device boundary (SI-017 - "an unconfirmed identity never
earns a descend by default"). Since `SystemIdentityObserver` reported `Unsupported`
unconditionally on Windows at the time, that condition was never true there, so a real Windows
scan visited only the scope root itself - a four-level nested fixture (`root/a/b/c`) produced
`directories_visited == 1`, not `4`. E20-S01's real Windows identity resolves this as a direct,
anticipated consequence (this document already named it as what E20-S01 would produce): the
shared traversal test (`scan::tests::ac1_one_traversal_visits_every_directory_exactly_once`) now
runs on every platform, Windows included. A "fully readable tree" is still `Partial` rather than
`Complete` on Windows, but now solely because `AllocationObserver` remains `Unsupported`, never
because of `identity` (`completeness::tests::
ac1_a_fully_readable_tree_is_partial_on_windows_pending_allocated_size`).

## Allocated-size observation

Logical size and allocated/physical size are different facts: a sparse file, a
copy-on-write clone, or a compressed filesystem can report a logical length far larger or
smaller than the disk blocks it actually occupies. E04-S01 implements this as its own seam,
`AllocationObserver`, at `rust/crates/cancellai-platform/src/allocation.rs` - mirroring
`FsObserver`/`IdentityObserver`'s `Absent`/`Unreadable`/`Unsupported` split rather than
folding an allocated-size field into `FsObserver` itself, so a platform/filesystem that
cannot report it distinctly is a typed fact, never a silent copy of the logical size or a
fabricated zero. `SystemAllocationObserver` uses Unix `st_blocks * 512` (the same
POSIX-standard convention `du` relies on); it reports `Unsupported` on non-Unix targets today,
the same fail-closed posture `IdentityObserver` uses for Windows identity until a verified
implementation exists.

## Boundary rules

- Never mutate the provider root itself.
- Never escape the approved root capability.
- Crossing a filesystem/volume boundary is explicit and normally prohibited for recursive mutation/quarantine unless a dedicated operation has verified semantics.
- Symlinks/reparse links are treated as link objects unless a specific read-only traversal capability says otherwise; mutation never follows an untrusted link target.

E03-S03 implements all four as one typed capability at
`rust/crates/cancellai-safety/src/root_capability.rs`: `ApprovedRoot::establish` binds a root
to the object identity observed for it (SI-002; fails closed if that identity is `Absent`/
`Unreadable`/`Unsupported`), and `ApprovedRoot::bind` is the *only* way to obtain a
`BoundedPath` under it - there is no other public constructor, so a future mutation API typed
to take `BoundedPath` instead of `&Path`/`PathBuf` cannot accept an unconstrained raw path.
`bind` resolves the candidate through the new `PathResolver` capability
(`docs/architecture/PLATFORM_MODEL.md`'s own "path canonicalization/normalization", split out
as its own seam alongside `IdentityObserver`) - resolving symlinks, so a candidate that
already escapes the root through a symlink component is rejected at bind time, not silently
followed - then refuses a candidate equal to the root itself, a candidate outside the root's
canonical prefix, and a candidate whose observed device differs from the root's (the explicit
Unix mount-boundary check, SI-018). E20-S01 (ADR-0020) gave `IdentityObserver` a real Windows
volume identity (the volume serial number, via `IdentityToken::device()`'s `Windows` arm), so
`bind`/`establish`'s device comparison now enforces a genuine Windows volume boundary rather
than refusing unconditionally through `Unsupported` - the same code path as Unix, no platform
branching added at this layer. This closes the *comparison* half of "explicit per platform";
`cancellai-sealedfs::SealedRoot`'s separate no-follow *root-establishment* walk (used by
`configure` and `clean`'s default-root re-check, see below) remains `Unsupported` on Windows,
so `clean`'s default-root establishment still refuses there as a whole, via that still-open
residual rather than via this boundary check. A later symlink/mount swap *after* a successful
`bind` is SI-013's job (E03-S02's `revalidate`, wired in immediately before mutation by
E03-S05), not this capability's.

E03 verifier review round 1 found `BoundedPath` alone did not fully close AC1 ("no mutation
API accepts an unconstrained raw path"): `cancellai-platform`'s real mutation capability
(`SystemMutationExecutor`) was itself `pub` and re-exported at that crate's root, so any crate
could import and call it directly against a raw path, bypassing `ApprovedRoot`/`BoundedPath`
entirely regardless of how carefully typed the "legitimate" path was. Repaired at E03-S05:
`SystemMutationExecutor` is no longer re-exported from `cancellai_platform`'s crate root, and
`scripts/check_mutation_boundary.py` statically verifies that only `cancellai-platform`'s
`mutation.rs` and `cancellai-safety`'s `mutation_executor.rs` reference it (or call
`.mutate(`) at all - Rust cannot express "public to exactly one sibling crate," so this
governance check, not type-level visibility alone, is what actually keeps the capability
reachable only through the safety kernel.

### Default-root authority never rests on a lexical name alone

`ApprovedRoot::establish`/`bind` bind to the object identity found *after* canonicalization
(SI-002/SI-003 above), which is exactly why E06 verifier review round 2 found a gap one layer
up: `cancellai-cli`'s own classification of `$HOME/.claude`/`$HOME/.codex` as the OS-default,
mutation-eligible root (ADR-0013) compared paths *before* canonicalization ever ran, so a
default-named leaf that was itself a symlink/reparse point to an attacker- or
operator-mistaken directory was still classified `origin=default` and mutated - the boundary
capability above never got a chance to refuse it, because nothing told it the root candidate
was a link in the first place.

Repaired at E07-S07 (`rust/crates/cancellai-cli/src/roots.rs`): `is_symlink` inspects the
literal leaf path (`symlink_metadata`, never following it) and `resolve_from` folds that fact
into `is_default` uniformly, whether the path came from `$HOME/.claude`/`$HOME/.codex` directly
or from an override that happens to name the same string. Classification alone is not trusted
at mutation time either - a root classified `Default` when a run started could be swapped for a
symlink during the interactive confirmation pause, so `main.rs::establish_verified_root` (used
by `clean`) and `cmd_configure` both re-run `is_symlink` fresh, immediately before establishing
the root or writing configuration, independent of the cached classification. `configure` in
particular does not go through `ApprovedRoot`/`IdentityObserver` at all (it is a vendor
settings-file write, not an artifact deletion), so on Windows, `is_symlink`'s own correctness
is the *only* thing standing between a symlinked default-named root and a write through it -
`cancellai-sealedfs::SealedRoot::establish` (which `configure` uses instead) still fails closed
there on its own account (its no-follow walk residual, not identity). `clean`'s deletion path
gets a second, independent backstop from `establish_verified_root`'s
`verify_no_intermediate_links` call failing closed on that same still-`Unsupported` walk (E20-S01
made `ApprovedRoot::establish`'s own identity check real on Windows - see above - so this second
backstop is no longer "identity `Unsupported`" but remains a genuine, independent refusal).

`is_symlink` uses `std`'s cross-platform `FileType::is_symlink()`, not a Unix-only syscall -
verified fixtures exist for a real Unix symlink and a real Windows directory symlink
(`std::os::windows::fs::symlink_dir`, cross-compile-clippy-verified for
`x86_64-pc-windows-gnu`; runs for real on this repo's Windows CI matrix). A true NTFS junction
(`IO_REPARSE_TAG_MOUNT_POINT`, creatable only via `DeviceIoControl` - no `std` API, and this
repo does not add a dependency merely to reach it) is not separately fixture-proven; `std`'s own
Windows implementation reports `is_symlink() == true` for that reparse tag too, so the same
refusal is expected, but this remains a disclosed residual rather than an empirically closed
case (`docs/CLI_RUST.md`'s own "Known gaps" section records the same disclosure).

#### `configure`'s TOCTOU: a re-checked path is not enough, only a retained handle is

E07-S07's round-1 independent verifier review found that the paragraph above's own `configure`
re-check did not actually close the gap it was added for: `cmd_configure` checked
`roots::is_symlink` once, then `configure_claude_retention` performed `create_dir_all`/
`read_to_string`/`OpenOptions::open`/`rename` against the same raw path again, several separate
syscalls later. A same-user attacker who atomically replaces the real default root with a
symlink in the gap between that check and the first of those path lookups causes every
following operation to silently follow the link and write outside the approved root - a
re-check immediately before use narrows this window but, being itself a separate syscall from
the operations that follow it, cannot close it to zero.

`cancellai-sealedfs` (ADR-0017) closes it by construction instead of by narrowing it:
`SealedRoot::establish` opens the root exactly once with `O_NOFOLLOW`, and every subsequent
child read/write/rename is issued via `openat`/`renameat` against that one retained directory
descriptor - the kernel resolves these relative to the descriptor's own bound object, not
whatever name currently occupies the original path, so a rename/symlink-swap of that path after
`establish` returns cannot redirect anything. This needed real `openat`/`renameat` FFI, which
`std` does not expose safely; `cancellai-sealedfs` is the one workspace crate ADR-0015
anticipated in the abstract ("isolated in a small, dedicated crate whose only job is that
unsafe boundary") and is now, concretely, not carrying `unsafe_code = "forbid"`. Non-Unix
platforms have no verified reparse-safe equivalent yet, so `SealedRoot::establish` there always
fails closed - `configure` now refuses on every non-Unix platform outright, the same posture
`clean` already had there via `ApprovedRoot::establish` failing closed on `Unsupported`
identity, closing the asymmetry the previous paragraph's "the only thing standing between a
symlinked default-named root and a write through it" language described.

A settings-file-level symlink (`$CLAUDE_HOME/settings.json` itself being a link, distinct from
the root directory case above) remains the already-verified E06 round-1 behavior: read through,
never written through (`O_EXCL` + `renameat` never follows a symlink at either name) - this ADR
did not change or re-scope that case.

#### Intermediate components need the same no-follow treatment as the leaf (E07-S09)

E07-S07 round-1's `SealedRoot::establish` bound the *leaf* with `O_NOFOLLOW`, but its own
pre-check (`symlink_metadata(path)`) and `OpenOptions::open(path)` still resolved every
component above the leaf through ordinary, link-following path resolution first. E07-S07
round-2 independent verifier review reproduced the consequence natively: with `$HOME` itself a
symlink to an outside directory and a real `.claude` directory sitting under that outside
target, `configure --claude-retention 30` exited `0` and wrote through to the outside
directory - the leaf was a real, non-symlink directory, so the round-1 check never had a
reason to refuse it.

`establish` now performs one handle-relative walk for the *entire* path, not only its last
component: it opens the filesystem root `/` (the one point in the walk with nothing upstream of
it to have been swapped), then `openat`s each subsequent component - intermediate or final -
against the descriptor already held for its parent, with `O_NOFOLLOW | O_DIRECTORY`, refusing
outright the moment any of them is a symlink or reparse point. Only the final component may be
created if absent, via `mkdirat` against the already-held parent descriptor, never
`create_dir_all`'s path-based recursive creation. A relative path or a path containing a `.`/`..`
component is refused outright (`SealError::NotAbsolute`/`PathNotNormalized`) rather than
resolved, since resolving either safely would need the same kind of path-based lookup this walk
exists to avoid.

This closes the class of gap for Unix. Windows/reparse-point handling still has no verified
handle-relative equivalent (`SealedRoot::establish` continues to fail closed there, per the
existing residual above) - a genuine junction/reparse-safe walk remains E20-S01's scope (moved
from E07 into a dedicated Windows/WSL epic pending real environment access).

##### The fix had to reach `clean`, not only `configure` (E07-S09 round 2)

E07-S09 round-1 independent verifier review found that the walk above closed the gap only for
`configure`'s write path. `clean` establishes its provider root through a different capability,
`cancellai-safety::ApprovedRoot::establish`, whose own `canonicalize()` step (deliberate -
`ApprovedRoot::bind` relies on it to catch a *candidate* escaping through a symlink component,
see this document's "Boundary rules" section above) silently resolves through the identical
intermediate link `SealedRoot`'s walk exists to refuse. Native reproduction: `$HOME` a symlink
to an outside directory containing a real `.claude` with a stale session underneath - `clean
--yes` deleted it while `configure` (already repaired) correctly refused the same topology.

`cancellai-sealedfs` exports a second, narrower entry point for exactly this shape of caller:
`verify_no_intermediate_links(path)` performs the identical handle-relative, no-follow walk as
`establish`, but never creates a missing component and returns a missing-path guard for one -
`clean` has no business materializing a provider root that does not exist, so a missing
component is left for `ApprovedRoot::establish`'s own existing "root does not exist" error to
report, not treated as this function's problem. `cancellai-cli`'s `establish_verified_root`
(used by `clean`, for the default root only - a custom root is never mutation-eligible
regardless of what this check would say about it) carries the walk's final directory descriptor
through `ApprovedRoot::establish` and compares its device/inode identity with the canonicalized
root before granting authority. The combined verifier/executor review found that merely calling
the walk immediately before `canonicalize()` left another check-then-use race: an intermediate
component could be swapped between those operations. The retained handle and identity match
turn that race into a refusal because the replacement path cannot become the object already
held open by the walk.

## Quarantine

Quarantine prefers metadata-preserving atomic/same-volume moves. Cross-volume copy+delete is a materially different action with more disk-pressure and failure risk and requires separate capability/policy.

## WSL

WSL is represented as:

```text
host_os: windows
environment: wsl2
guest_os: linux
filesystem_context: linux | windows-mounted | other
```

Operations across `/mnt/c` or similar mounts may have different identity, performance, permission, and atomicity semantics. These differences are surfaced rather than abstracted away.

E20-S02 implements the first two lines of that representation as explicit, typed facts in
`rust/crates/cancellai-platform/src/wsl.rs`:

- **`environment` (`RuntimeEnvironment::{Wsl2, Native}`)**: a WSL2 guest is a real Linux
  kernel, so every existing Unix seam in this crate (`IdentityObserver`, `AllocationObserver`,
  ...) already works correctly there without special-casing - what needed its own capability
  was knowing the process is running inside one at all. `SystemEnvironmentObserver` reads
  `/proc/sys/kernel/osrelease` on `cfg(target_os = "linux")` and checks for the "microsoft"
  marker both WSL1's (`...-Microsoft`) and WSL2's (`...-microsoft-standard-WSL2`) kernel
  release strings carry - the standard, widely-used heuristic, and the only positive signal
  available without an elevated check. Absence of the marker, including any read error, is
  `Native`, never a guessed `Wsl2` (C-03). `host_os`/`guest_os` are not carried as separate
  fields: both are implied constants of the `Wsl2` variant (`windows`/`linux` respectively)
  rather than state that could drift from it independently.
- **`filesystem_context` (`FilesystemContext::{Linux, WindowsMounted, Other}`)**:
  `SystemFilesystemContextObserver` parses `/proc/mounts` and finds the mount whose mountpoint
  is the longest matching prefix of the given (absolute) path - the same "most specific mount
  wins" resolution the kernel itself uses. A `drvfs` filesystem type (the WSL2 default for a
  mounted Windows drive, conventionally `/mnt/c`) classifies as `WindowsMounted`; a recognized
  native-Linux type (`ext4`, `overlay`, `tmpfs`, ...) classifies as `Linux`; anything else
  (e.g. a `9p`/network mount) is disclosed as `Other { fstype }` rather than silently folded
  into `Linux`. Classification is by real observed filesystem type, not a `/mnt/*` path-prefix
  guess - a mount could in principle sit anywhere. A relative path, or any platform without
  `/proc/mounts` (macOS, native Windows), is `Unsupported`, not a default.

Both are library-level capabilities (`cancellai-platform` seams with `Synthetic*` test doubles,
following this crate's existing pattern) without a CLI-facing surface yet - wiring
`RuntimeEnvironment`/`FilesystemContext` into `status`/`inspect` output, and using
`filesystem_context` to attach a performance/atomicity caveat to a scanned path, are left for a
future story; this story's documentation impact and acceptance criteria are scoped to explicit
detection/classification, not product surface. No safety-boundary decision (SI-018's device-
based volume-boundary check, `ApprovedRoot::bind`) depends on either capability: a `/mnt/c`
Windows-drive mount genuinely is a different device number from the WSL2 guest's own root
filesystem, so the existing Unix device-identity check (E20-S01/ADR-0020) already refuses
recursive mutation crossing that boundary without needing WSL-specific code - `filesystem_context`
is a descriptive/explanatory fact layered on top, not a second gate.

This executor has no real WSL2 guest to run against in this environment; both detectors split
their real observation (a single file read, `cfg(target_os = "linux")`-gated) from a pure
classification function exhaustively unit-tested with fabricated WSL2-shaped
`/proc/sys/kernel/osrelease`/`/proc/mounts` content, runnable and verified on any host - this is
what this story's "simulated path fixtures" verification contract means in practice.

## Handle-relative mutation (E21-S07)

The mutation seam's confirmed file deletion issues its unlink through
`cancellai-sealedfs`'s `unlinkat`, against a directory descriptor opened once with `O_NOFOLLOW`
at every component, rather than through the target's path
([ADR-0017](../adrs/0017-sealed-root-handle-for-configuration-writes.md)'s E21-S07 extension).
This is the same containment rule E07-S09 established for provider-root establishment, now
holding at the moment of mutation: a target reached through a symlinked intermediate component
is refused outright rather than followed, and a swap of any path component after validation
cannot redirect the removal.

The consequence is user-visible and intended: a provider root whose path crosses a link cannot
be cleaned. `cancellai-cli` already proves the default root link-free before establishing it, so
the deletion path simply meets the same bar the establishment path already set.

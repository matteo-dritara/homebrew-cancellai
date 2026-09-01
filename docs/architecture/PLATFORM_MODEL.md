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

E03-S01 deliberately does not implement native Windows volume/file-index/reparse identity yet:
`SystemIdentityObserver::observe` reports `IdentityObservation::Unsupported` unconditionally
on any non-Unix target. This is the "authority is reduced" outcome this section already
describes, not a placeholder standing in for it - a plausible-but-unverified implementation of
a safety-critical equality check (built and reviewed with no access to a real Windows runtime)
was judged a worse outcome than an honest `Unsupported` (SI-017, C-12 cross-platform
truthfulness). A follow-up story implements and verifies real Windows identity once it can be
exercised on Windows CI; until then, no artifact whose identity depends on Windows-native
evidence can receive destructive authority through this seam.

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
Unix mount-boundary check, SI-018). Real Windows volume/reparse boundary semantics inherit
E03-S01's `Unsupported` posture: since `IdentityObserver` reports `Unsupported` off-Unix
today, `bind`/`establish` refuse there too, rather than guessing a boundary they cannot
verify - "explicit per platform" resolving today to an explicit refusal on non-Unix targets,
not silent success. A later symlink/mount swap *after* a successful `bind` is SI-013's job
(E03-S02's `revalidate`, wired in immediately before mutation by E03-S05), not this
capability's.

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
settings-file write, not an artifact deletion), so on a platform where `IdentityObserver` is
`Unsupported` (Windows, today - see above), `is_symlink`'s own correctness is the *only* thing
standing between a symlinked default-named root and a write through it; `clean`'s deletion path
gets a second, independent backstop from `ApprovedRoot::establish` failing closed on
`Unsupported` identity.

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

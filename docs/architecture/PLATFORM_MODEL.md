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

## Boundary rules

- Never mutate the provider root itself.
- Never escape the approved root capability.
- Crossing a filesystem/volume boundary is explicit and normally prohibited for recursive mutation/quarantine unless a dedicated operation has verified semantics.
- Symlinks/reparse links are treated as link objects unless a specific read-only traversal capability says otherwise; mutation never follows an untrusted link target.

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

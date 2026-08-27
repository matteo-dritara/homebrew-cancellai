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

### Windows

Use native file/volume identity and reparse metadata. Do not assume Unix inode/symlink semantics are sufficient. Junctions and reparse points require explicit classification.

If the platform cannot produce an identity strong enough for a requested irreversible action, authority is reduced.

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

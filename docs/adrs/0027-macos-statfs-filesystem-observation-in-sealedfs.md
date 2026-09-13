# ADR-0027: macOS filesystem observation may use the sealed FFI boundary

- Status: Accepted
- Date: 2026-09-13
- Owners: project owner
- Related: ADR-0015, ADR-0017, E10-S01, E10-S03, SI-016

## Context

E10-S01 added `cancellai_sealedfs::observe_filesystem_name` on macOS. It calls
`libc::statfs` to read the kernel-provided `f_fstypename` for a path, allowing
`cancellai-platform` to downgrade reclaim estimates for filesystems that may share clone or
reflink blocks. The implementation uses two unsafe operations: zero-initialising
`libc::statfs` before it is used only as an output buffer, and passing that buffer and a
`CString` path to `statfs(2)`.

ADR-0017 isolated unsafe filesystem work in `cancellai-sealedfs`, but its original decision
described a handle-relative mutation capability. A filesystem-type probe is a separate job. A
comment explaining why `std` cannot expose the fact is not authority to broaden an exception to
the workspace's `unsafe_code = "forbid"` default.

## Decision

`cancellai-sealedfs` may also contain this one read-only macOS FFI observation capability. Its
authority is deliberately narrow:

- it takes an OS path, rejects an interior NUL through `CString::new`, and calls the platform
  `statfs` signature supplied by `libc`;
- its `libc::statfs` value is zero-initialised only as an output buffer and is inspected only
  after a successful return. The Darwin binding is a C representation made of integer fields
  and fixed `c_char` arrays, for which the all-zero bit pattern is valid;
- the `CString` and stack output buffer both remain valid throughout the synchronous call;
  `statfs` neither retains either pointer nor mutates application state;
- `f_fstypename` is read to its NUL terminator, casts signed `c_char` to its original byte value,
  and is decoded lossily. Invalid UTF-8 therefore reaches the conservative unknown-filesystem
  classification instead of panicking or being mistaken for a known filesystem;
- symlinks and a missing path retain Darwin `statfs` semantics: the probe follows a symlink to
  observe the backing filesystem and returns the OS error for a nonexistent target. It does not
  establish a root capability, authorize a plan, or perform mutation.

The platform classifier treats every unknown name, observation error, and unsupported platform
as `PossiblyShared` or `Unsupported`. Only `NotKnownToShare` plus fully known allocated sizes
can yield a `Verified` estimate. No filesystem observation can elevate mutation authority.

## Alternatives considered

### Move the FFI into `cancellai-platform`

Rejected. That crate remains under the workspace's `unsafe_code = "forbid"` rule. Relaxing it
would enlarge the unsafe trusted-computing base and create a second unsafe boundary.

### Use a path-based command such as `mount` or `df`

Rejected. It adds subprocess parsing, locale/output-shape drift, and a less direct path-to-mount
mapping where Darwin already provides the fact synchronously through `statfs`.

### Report every macOS filesystem as unsupported

Safe but rejected. It would make the accounting feature less useful without reducing mutation
authority, while the small, documented FFI above has a bounded ABI and failure surface.

## Consequences

- `cancellai-sealedfs` remains the only crate allowed to contain unsafe code. Every new unsafe
  operation there still needs an ADR and a local `SAFETY:` argument.
- The probe is CR4 because it broadens the authority of the unsafe exception, even though the
  operation itself is read-only and feeds only conservative accounting confidence.
- Miri is attempted for this code when its component is available. On the active stable macOS
  toolchain its component is unavailable, so native tests and platform CI remain the executable
  evidence; this is recorded in E10-S03's Safety Verdict.

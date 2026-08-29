//! OS capability interfaces and their per-platform implementations
//! (`docs/architecture/PLATFORM_MODEL.md`): path/identity, link/reparse, process, and
//! atomicity semantics for macOS, Linux, Windows, and WSL.
//!
//! `unsafe_code` is `forbid`-by-default at the workspace level (ADR-0015). If a platform
//! implementation genuinely needs an OS binding this crate cannot express safely otherwise,
//! that need is isolated in a small, separately-justified submodule/crate here - it is not
//! grounds for relaxing the default silently.
//!
//! E02-S04 adds the first two capability seams: [`Clock`] (time) and [`FsObserver`]
//! (filesystem observation), each with a real OS-backed production implementation and a
//! deterministic/synthetic test double. Neither abstraction hides the OS semantics that
//! matter for safety: `FsObserver` keeps `docs/architecture/AS_IS.md`'s absent-vs-unreadable
//! distinction (SI-008/SI-009/SI-010) as a typed contract, not an implementation convention.
//!
//! E03-S01 adds a third seam, [`IdentityObserver`], binding a plan to the object actually
//! observed (device/inode on Unix) rather than to a path alone (SI-013, SI-017). E03-S03
//! adds a fourth, [`PathResolver`], the "path canonicalization/normalization" capability
//! `docs/architecture/PLATFORM_MODEL.md` lists separately from filesystem identity. E03-S05
//! adds a fifth, `mutation::MutationExecutor` - the only seam in this crate whose real
//! implementation changes the filesystem (SI-019). E04-S01 adds a sixth,
//! [`AllocationObserver`], PLATFORM_MODEL.md's "logical and allocated-size observation" -
//! kept distinct from `FsObserver`'s logical `len` so a sparse/compressed/cloned file's
//! reclaim estimate is never silently assumed equal to its logical size.
//!
//! `mutation::SystemMutationExecutor` (the concrete, real-syscall implementation) is
//! deliberately *not* re-exported at this crate's root the way every other capability's
//! `System*` implementation is - reach it via the full `cancellai_platform::mutation::`
//! path. This is not itself the enforcement: `scripts/check_mutation_boundary.py`
//! statically verifies that only `mutation.rs` calls a filesystem removal primitive
//! directly, and that only it and `cancellai-safety`'s `mutation_executor.rs` reference
//! `SystemMutationExecutor`/`.mutate(` at all (E03 verifier review round 1 found the
//! previous crate-root re-export made the raw capability trivially importable, and thus
//! callable, from any crate that bypassed the safety kernel's root/authority/identity
//! checks entirely). Withholding the convenience re-export is defense in depth on top of
//! that check, not a substitute for it - Rust visibility cannot express "public to exactly
//! one sibling crate," so the check is the real boundary.

pub mod allocation;
pub mod clock;
pub mod fs_observer;
pub mod identity;
pub mod mutation;
pub mod path_resolver;
pub mod snapshot;

pub use allocation::{
    AllocationObservation, AllocationObserver, SyntheticAllocationObserver,
    SystemAllocationObserver,
};
pub use clock::{Clock, FrozenClock, SystemClock, Timestamp};
pub use fs_observer::{FsMetadata, FsObserver, Observation, SyntheticFsObserver, SystemFsObserver};
pub use identity::{
    FileKind, IdentityObservation, IdentityObserver, IdentityToken, SyntheticIdentityObserver,
    SystemIdentityObserver,
};
pub use path_resolver::{PathResolver, SyntheticPathResolver, SystemPathResolver};
pub use snapshot::{Snapshot, build_snapshot};

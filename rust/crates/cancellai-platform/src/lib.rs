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
//! adds a fifth, [`MutationExecutor`] - the only seam in this crate whose real
//! implementation changes the filesystem (SI-019); `scripts/check_mutation_boundary.py`
//! statically enforces that `mutation.rs` is the one production source file in the whole
//! workspace allowed to call a filesystem removal primitive directly.

pub mod clock;
pub mod fs_observer;
pub mod identity;
pub mod mutation;
pub mod path_resolver;
pub mod snapshot;

pub use clock::{Clock, FrozenClock, SystemClock, Timestamp};
pub use fs_observer::{FsMetadata, FsObserver, Observation, SyntheticFsObserver, SystemFsObserver};
pub use identity::{
    FileKind, IdentityObservation, IdentityObserver, IdentityToken, SyntheticIdentityObserver,
    SystemIdentityObserver,
};
pub use mutation::{
    MutationError, MutationExecutor, MutationOperation, SyntheticMutationExecutor,
    SystemMutationExecutor,
};
pub use path_resolver::{PathResolver, SyntheticPathResolver, SystemPathResolver};
pub use snapshot::{Snapshot, build_snapshot};

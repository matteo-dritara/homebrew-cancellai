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

pub mod clock;
pub mod fs_observer;
pub mod snapshot;

pub use clock::{Clock, FrozenClock, SystemClock, Timestamp};
pub use fs_observer::{FsMetadata, FsObserver, Observation, SyntheticFsObserver, SystemFsObserver};
pub use snapshot::{Snapshot, build_snapshot};

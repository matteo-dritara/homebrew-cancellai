//! An allocated/physical-size observation seam (E04-S01,
//! `docs/architecture/PLATFORM_MODEL.md`'s "logical and allocated-size observation").
//!
//! Logical size (`FsObserver`'s `FsMetadata::len`) and allocated/reclaimable size are
//! different facts - a sparse file, a copy-on-write clone, or a compressed filesystem can
//! report a logical length far larger or smaller than the disk blocks it actually occupies.
//! `docs/architecture/DOMAIN_MODEL.md`'s `AgentArtifact` keeps them as two separate optional
//! fields (`LogicalSize` vs `AllocatedSize?`) for exactly this reason, and this seam is what
//! lets `cancellai-inventory` (E04-S01) populate the second one honestly instead of reusing
//! the first. Mirrors [`crate::identity`]'s `Absent`/`Unreadable`/`Unsupported` split: a
//! platform/filesystem that cannot report allocated size is a distinct, typed fact, never a
//! fabricated `0` or a silent copy of the logical size.

use std::path::Path;
use std::{collections::BTreeMap, path::PathBuf};

/// What allocation observation for one path can tell us.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AllocationObservation {
    /// The path does not exist.
    Absent,
    /// Allocated/physical size in bytes, as reported by the platform.
    Allocated(u64),
    /// The path could not be examined (permission/I/O failure).
    Unreadable { reason: String },
    /// This platform/filesystem cannot report allocated size distinctly from logical size.
    /// A caller must not substitute the logical size here - that would silently claim a
    /// reclaim estimate this build never actually observed.
    Unsupported { reason: String },
}

/// A source of allocated-size facts. Production paths take `&dyn AllocationObserver` and use
/// [`SystemAllocationObserver`]; tests use the same trait and [`SyntheticAllocationObserver`].
pub trait AllocationObserver: Send + Sync {
    fn observe(&self, path: &Path) -> AllocationObservation;
}

/// The real, OS-backed observer.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemAllocationObserver;

impl AllocationObserver for SystemAllocationObserver {
    fn observe(&self, path: &Path) -> AllocationObservation {
        observe_system_allocation(path)
    }
}

/// Unix reports allocated size as 512-byte blocks (`st_blocks`) regardless of the
/// filesystem's own block size - this is the POSIX-standard convention `du` itself relies
/// on, not an assumption specific to any one filesystem.
#[cfg(unix)]
fn observe_system_allocation(path: &Path) -> AllocationObservation {
    use std::os::unix::fs::MetadataExt;

    match std::fs::symlink_metadata(path) {
        Ok(meta) => AllocationObservation::Allocated(meta.blocks() * 512),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => AllocationObservation::Absent,
        Err(e) => AllocationObservation::Unreadable {
            reason: e.to_string(),
        },
    }
}

/// `GetFileInformationByHandleEx(FileStandardInfo)` (E20-S05, extending ADR-0020) - handle-based
/// so it reuses `cancellai-sealedfs`'s existing no-follow open, matching identity observation's
/// own no-follow contract rather than the path-based (reparse-point-following)
/// `GetCompressedFileSizeW`.
#[cfg(windows)]
fn observe_system_allocation(path: &Path) -> AllocationObservation {
    match cancellai_sealedfs::observe_allocated_size(path) {
        Ok(bytes) => AllocationObservation::Allocated(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => AllocationObservation::Absent,
        Err(e) => AllocationObservation::Unreadable {
            reason: e.to_string(),
        },
    }
}

#[cfg(not(any(unix, windows)))]
fn observe_system_allocation(_path: &Path) -> AllocationObservation {
    AllocationObservation::Unsupported {
        reason: "allocated-size observation is not implemented on this platform; logical \
                 size remains available and is never substituted here"
            .to_string(),
    }
}

/// Test-only seam: synthesize allocation facts without touching the real filesystem. A path
/// with no fact explicitly `set` observes as `Absent`, matching every other observer in this
/// crate - never a silently-invented `Unsupported`/`Unreadable`.
#[derive(Debug, Default)]
pub struct SyntheticAllocationObserver {
    facts: BTreeMap<PathBuf, AllocationObservation>,
}

impl SyntheticAllocationObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(
        &mut self,
        path: impl Into<PathBuf>,
        observation: AllocationObservation,
    ) -> &mut Self {
        self.facts.insert(path.into(), observation);
        self
    }
}

impl AllocationObserver for SyntheticAllocationObserver {
    fn observe(&self, path: &Path) -> AllocationObservation {
        self.facts
            .get(path)
            .cloned()
            .unwrap_or(AllocationObservation::Absent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_observer_reports_absent_for_unset_paths() {
        let observer = SyntheticAllocationObserver::new();
        assert_eq!(
            observer.observe(Path::new("/never/configured")),
            AllocationObservation::Absent
        );
    }

    #[test]
    fn synthetic_observer_reports_exactly_what_was_set() {
        let mut observer = SyntheticAllocationObserver::new();
        observer.set("/synthetic/file", AllocationObservation::Allocated(4096));
        observer.set(
            "/synthetic/locked",
            AllocationObservation::Unreadable {
                reason: "permission denied".into(),
            },
        );
        observer.set(
            "/synthetic/exotic-fs",
            AllocationObservation::Unsupported {
                reason: "no allocation metric on this filesystem".into(),
            },
        );

        assert_eq!(
            observer.observe(Path::new("/synthetic/file")),
            AllocationObservation::Allocated(4096)
        );
        assert_eq!(
            observer.observe(Path::new("/synthetic/locked")),
            AllocationObservation::Unreadable {
                reason: "permission denied".into()
            }
        );
        assert_eq!(
            observer.observe(Path::new("/synthetic/exotic-fs")),
            AllocationObservation::Unsupported {
                reason: "no allocation metric on this filesystem".into()
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn system_observer_reports_a_nonzero_allocation_for_a_real_file() {
        let dir =
            std::env::temp_dir().join(format!("cancellai-allocation-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let file = dir.join("real-file.txt");
        std::fs::write(&file, vec![b'x'; 8192]).expect("write temp file");

        let observer = SystemAllocationObserver;
        match observer.observe(&file) {
            AllocationObservation::Allocated(bytes) => {
                assert!(bytes > 0, "an 8KB file must occupy at least one block");
            }
            other => panic!("expected Allocated for a real file, got {other:?}"),
        }
        assert_eq!(
            observer.observe(&dir.join("does-not-exist.txt")),
            AllocationObservation::Absent
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    // E20-S05 implemented real Windows allocated-size observation. Learning directly from
    // E20-S01's own round-1 repair (a stale `#[cfg(not(unix))]` "expected Unsupported" test
    // broke real Windows CI the moment identity stopped being genuinely unsupported there),
    // this test is written as a real Windows assertion from the start rather than left as a
    // now-false `Unsupported` expectation. The genuinely-exotic non-Unix-non-Windows fallback
    // has no real target this workspace runs CI on, so it stays untested here.
    #[cfg(windows)]
    #[test]
    fn system_observer_reports_a_real_allocation_on_windows() {
        let observer = SystemAllocationObserver;
        match observer.observe(Path::new(".")) {
            AllocationObservation::Allocated(_) => {}
            other => panic!("expected a real Allocated size on Windows, got {other:?}"),
        }
    }
}

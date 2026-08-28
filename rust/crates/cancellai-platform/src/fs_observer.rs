//! A deterministic filesystem observation seam (E02-S04).
//!
//! Mirrors the "absent vs unreadable" split `docs/architecture/AS_IS.md` documents for the
//! Python reference's `Scan`/`observe()` (SI-008, SI-009, SI-010): a path that does not
//! exist and a path this build could not examine are never the same fact, and neither may
//! collapse into a zero/empty result. `FsObserver` carries that distinction as a typed
//! contract rather than an implementation convention, so it cannot be silently lost the way
//! a bare `Path::exists()` guard would lose it (`AS_IS.md`'s safety-critical core, item 3).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::clock::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FsMetadata {
    pub is_dir: bool,
    pub is_symlink: bool,
    pub len: u64,
    pub modified: Timestamp,
}

/// What observing one path can tell us. Never collapsed to a boolean or a bare number -
/// `Absent` and `Unreadable` are distinct variants on purpose (SI-008/SI-009/SI-010).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Observation {
    /// The path does not exist. Distinct from `Unreadable` - absence of evidence is never
    /// treated as absence of active/protected data (SI-009).
    Absent,
    Metadata(FsMetadata),
    /// The path could not be examined (a permission/I/O failure, not a race where the path
    /// simply vanished). Must never be silently treated as `Absent` or as zero/empty facts.
    Unreadable {
        reason: String,
    },
}

/// A source of filesystem facts. Production paths take `&dyn FsObserver` and use
/// [`SystemFsObserver`]; tests use the same trait and [`SyntheticFsObserver`] to synthesize
/// facts without touching the real filesystem (AC1 of E02-S04).
pub trait FsObserver: Send + Sync {
    fn observe(&self, path: &Path) -> Observation;
}

/// The real, OS-backed observer. Production paths use this (AC2 of E02-S04) - `lstat`-like
/// semantics (`symlink_metadata`, never following the final symlink) so a symlink is
/// reported as itself, not silently resolved.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemFsObserver;

impl FsObserver for SystemFsObserver {
    fn observe(&self, path: &Path) -> Observation {
        match std::fs::symlink_metadata(path) {
            Ok(meta) => Observation::Metadata(FsMetadata {
                is_dir: meta.is_dir(),
                is_symlink: meta.file_type().is_symlink(),
                len: meta.len(),
                modified: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| Timestamp(d.as_secs()))
                    .unwrap_or(Timestamp::EPOCH),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Observation::Absent,
            Err(e) => Observation::Unreadable {
                reason: e.to_string(),
            },
        }
    }
}

/// Test-only seam: synthesize facts for specific paths without touching the real
/// filesystem. A path with no fact explicitly `set` observes as `Absent` - the same "I
/// looked and it wasn't there" answer a real observer gives for a genuinely missing path,
/// never `Unreadable` by default, since a synthetic observer that silently invents
/// unreadable paths would be lying about what the test actually configured.
#[derive(Debug, Default)]
pub struct SyntheticFsObserver {
    facts: BTreeMap<PathBuf, Observation>,
}

impl SyntheticFsObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, path: impl Into<PathBuf>, observation: Observation) -> &mut Self {
        self.facts.insert(path.into(), observation);
        self
    }
}

impl FsObserver for SyntheticFsObserver {
    fn observe(&self, path: &Path) -> Observation {
        self.facts.get(path).cloned().unwrap_or(Observation::Absent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_observer_reports_absent_for_unset_paths() {
        let observer = SyntheticFsObserver::new();
        assert_eq!(
            observer.observe(Path::new("/never/configured")),
            Observation::Absent
        );
    }

    #[test]
    fn synthetic_observer_reports_exactly_what_was_set() {
        let mut observer = SyntheticFsObserver::new();
        observer.set(
            "/synthetic/file",
            Observation::Metadata(FsMetadata {
                is_dir: false,
                is_symlink: false,
                len: 42,
                modified: Timestamp(1_000),
            }),
        );
        observer.set(
            "/synthetic/locked",
            Observation::Unreadable {
                reason: "permission denied".into(),
            },
        );

        assert_eq!(
            observer.observe(Path::new("/synthetic/file")),
            Observation::Metadata(FsMetadata {
                is_dir: false,
                is_symlink: false,
                len: 42,
                modified: Timestamp(1_000)
            })
        );
        assert_eq!(
            observer.observe(Path::new("/synthetic/locked")),
            Observation::Unreadable {
                reason: "permission denied".into()
            }
        );
        assert_eq!(
            observer.observe(Path::new("/synthetic/absent")),
            Observation::Absent
        );
    }

    #[test]
    fn system_observer_distinguishes_absent_from_a_real_file() {
        let dir =
            std::env::temp_dir().join(format!("cancellai-fs-observer-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let file = dir.join("real-file.txt");
        std::fs::write(&file, b"hello").expect("write temp file");

        let observer = SystemFsObserver;
        match observer.observe(&file) {
            Observation::Metadata(meta) => {
                assert!(!meta.is_dir);
                assert!(!meta.is_symlink);
                assert_eq!(meta.len, 5);
            }
            other => panic!("expected Metadata for a real file, got {other:?}"),
        }
        assert_eq!(
            observer.observe(&dir.join("does-not-exist.txt")),
            Observation::Absent
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}

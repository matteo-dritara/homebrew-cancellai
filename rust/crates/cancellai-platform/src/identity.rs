//! A cross-platform artifact identity seam (E03-S01, `docs/architecture/PLATFORM_MODEL.md`
//! "Identity token").
//!
//! A path alone is not a safety-critical precondition: between the moment a plan is built and
//! the moment it executes, whatever sits at that path can be deleted and replaced (SI-013). A
//! [`IdentityToken`] binds a plan to the object actually observed - on Unix, its device and
//! inode, which change whenever the underlying object is replaced even if the path, name, and
//! superficial metadata do not (a mount-point swap changes the device; a delete-and-recreate,
//! even with identical content, gets a new inode). Revalidating identity immediately before
//! mutation is how the safety kernel (E03-S02/E03-S05) turns "the path still looks right" into
//! "this is still the object I planned against."
//!
//! Real, verified Windows volume/file-index/reparse identity is deliberately not implemented
//! here yet (see [`IdentityObservation::Unsupported`] below) - this machine has no Windows
//! target to exercise it against, and a plausible-but-unverified implementation of a
//! safety-critical equality check is a worse outcome than an honest "cannot establish identity
//! strength here" (SI-017, C-12 cross-platform truthfulness). `SystemIdentityObserver` reports
//! `Unsupported` on any non-Unix platform for now; `docs/architecture/PLATFORM_MODEL.md`'s own
//! escape hatch - "if the platform cannot produce an identity strong enough ... authority is
//! reduced" - is exactly this state, not a workaround for it. A follow-up story lands the real
//! Windows implementation once it can be tested on Windows CI.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::clock::Timestamp;

/// The coarse shape of a filesystem object, captured at the same instant as its identity
/// evidence so a type change (file replaced by a directory, a symlink replaced by a real
/// file, ...) is itself part of what "identity changed" means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    File,
    Directory,
    Symlink,
    /// A device file, socket, FIFO, or anything else `symlink_metadata` reports that is
    /// none of the above. cancellAI never manages these as provider artifacts; the variant
    /// exists so identity capture never has to guess or silently drop such a path.
    Other,
}

/// Strong-enough-to-detect-replacement identity evidence for one filesystem object.
///
/// Equality (`PartialEq`) is the whole point: two tokens observed for the same path at
/// different times are equal only if they describe the same underlying object, not merely a
/// path that still resolves to *something*.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "platform", rename_all = "snake_case")]
pub enum IdentityToken {
    /// Device and inode uniquely identify an object on a given Unix filesystem for as long
    /// as it exists; `kind`/`modified` are included so a same-inode reuse (astronomically
    /// unlikely but not provably impossible after a delete) still shows up as a mismatch.
    Unix {
        device: u64,
        inode: u64,
        kind: FileKind,
        modified: Timestamp,
    },
}

impl IdentityToken {
    /// The coarse filesystem-object shape this token describes. A plain accessor rather than
    /// requiring every caller to match on the (currently single) variant directly, so a
    /// future non-Unix variant is a one-place change instead of every call site.
    pub fn kind(&self) -> FileKind {
        match self {
            IdentityToken::Unix { kind, .. } => *kind,
        }
    }

    /// The filesystem/volume this token's object lives on (E04-S01, SI-018 boundary checks).
    /// Unix-only today, matching this enum's only variant; a future Windows variant adds its
    /// own volume identity rather than reusing this accessor's meaning.
    pub fn device(&self) -> u64 {
        match self {
            IdentityToken::Unix { device, .. } => *device,
        }
    }
}

/// What identity observation for one path can tell us. Mirrors
/// [`crate::fs_observer::Observation`]'s absent-vs-unreadable split (SI-008/SI-009/SI-010) and
/// adds [`Unsupported`](IdentityObservation::Unsupported): a platform/filesystem that cannot
/// produce identity evidence strong enough to trust is a distinct, typed fact, never silently
/// collapsed into "equal" or "different" by comparing something weaker instead (SI-017).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum IdentityObservation {
    /// The path does not exist.
    Absent,
    Identity(IdentityToken),
    /// The path could not be examined (permission/I/O failure).
    Unreadable {
        reason: String,
    },
    /// The platform cannot produce identity evidence this codebase trusts for safety
    /// decisions. A caller must treat this as strictly weaker than any `Identity` result,
    /// never as "assume unchanged" - `Unsupported != Unsupported` would be the wrong lesson
    /// to draw from `PartialEq` here; callers reduce authority on `Unsupported` outright
    /// rather than comparing it (see `docs/architecture/PLATFORM_MODEL.md`).
    Unsupported {
        reason: String,
    },
}

/// A source of artifact identity facts. Production paths take `&dyn IdentityObserver` and use
/// [`SystemIdentityObserver`]; tests use the same trait and [`SyntheticIdentityObserver`] to
/// inject TOCTOU scenarios (a mount swap, a not-yet-implemented Windows reparse case) that are
/// impractical or impossible to construct against a real filesystem in a test sandbox.
pub trait IdentityObserver: Send + Sync {
    fn observe(&self, path: &Path) -> IdentityObservation;
}

/// The real, OS-backed observer.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemIdentityObserver;

impl IdentityObserver for SystemIdentityObserver {
    fn observe(&self, path: &Path) -> IdentityObservation {
        observe_system_identity(path)
    }
}

#[cfg(unix)]
fn observe_system_identity(path: &Path) -> IdentityObservation {
    use crate::fs_observer::modification_timestamp;
    use std::os::unix::fs::MetadataExt;

    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            let kind = if meta.file_type().is_symlink() {
                FileKind::Symlink
            } else if meta.is_dir() {
                FileKind::Directory
            } else if meta.is_file() {
                FileKind::File
            } else {
                FileKind::Other
            };
            match modification_timestamp(meta.modified()) {
                Ok(modified) => IdentityObservation::Identity(IdentityToken::Unix {
                    device: meta.dev(),
                    inode: meta.ino(),
                    kind,
                    modified,
                }),
                Err(reason) => IdentityObservation::Unreadable { reason },
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => IdentityObservation::Absent,
        Err(e) => IdentityObservation::Unreadable {
            reason: e.to_string(),
        },
    }
}

#[cfg(not(unix))]
fn observe_system_identity(_path: &Path) -> IdentityObservation {
    IdentityObservation::Unsupported {
        reason: "native volume/file-index/reparse identity is not yet implemented on this \
                 platform (E03-S01 residual risk); authority is reduced rather than guessed"
            .to_string(),
    }
}

/// Test-only seam: synthesize identity facts for specific paths without touching the real
/// filesystem, needed for TOCTOU scenarios a test sandbox cannot construct for real (a device
/// swap standing in for a mount-boundary replacement, a not-yet-implemented Windows reparse
/// case). A path with no fact explicitly `set` observes as `Absent`, matching the real
/// observer's answer for a genuinely missing path - never `Unreadable`/`Unsupported` by
/// default, since that would silently invent a fact the test never configured.
#[derive(Debug, Default)]
pub struct SyntheticIdentityObserver {
    facts: BTreeMap<PathBuf, IdentityObservation>,
}

impl SyntheticIdentityObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, path: impl Into<PathBuf>, observation: IdentityObservation) -> &mut Self {
        self.facts.insert(path.into(), observation);
        self
    }
}

impl IdentityObserver for SyntheticIdentityObserver {
    fn observe(&self, path: &Path) -> IdentityObservation {
        self.facts
            .get(path)
            .cloned()
            .unwrap_or(IdentityObservation::Absent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    struct TempDir(PathBuf);

    #[cfg(unix)]
    impl TempDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-identity-test-{label}-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    #[cfg(unix)]
    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn token_of(observation: IdentityObservation) -> IdentityToken {
        match observation {
            IdentityObservation::Identity(token) => token,
            other => panic!("expected an Identity observation, got {other:?}"),
        }
    }

    // --- TOCTOU: a real object replaced by a different kind of real object between two
    // observations of the same path, exactly the plan-time vs execute-time gap SI-013 exists
    // to close. Each case asserts the token actually differs, not merely that the two calls
    // ran without error - a test that only checked "it compiles" would not falsify a design
    // that accidentally ignored `kind` or `inode`.
    #[cfg(unix)]
    #[test]
    fn toctou_file_replaced_by_directory_is_detected() {
        let dir = TempDir::new("file-to-dir");
        let target = dir.path("target");
        std::fs::write(&target, b"hello").expect("create file");
        let observer = SystemIdentityObserver;
        let planned = token_of(observer.observe(&target));

        std::fs::remove_file(&target).expect("remove file");
        std::fs::create_dir(&target).expect("create directory in its place");
        let revalidated = token_of(observer.observe(&target));

        assert_ne!(
            planned, revalidated,
            "file-to-directory replacement must change identity"
        );
        let IdentityToken::Unix { kind, .. } = revalidated;
        assert_eq!(kind, FileKind::Directory);
    }

    #[cfg(unix)]
    #[test]
    fn toctou_directory_replaced_by_symlink_is_detected() {
        let dir = TempDir::new("dir-to-symlink");
        let target = dir.path("target");
        std::fs::create_dir(&target).expect("create directory");
        let observer = SystemIdentityObserver;
        let planned = token_of(observer.observe(&target));

        std::fs::remove_dir(&target).expect("remove directory");
        std::os::unix::fs::symlink(&dir.0, &target).expect("create symlink in its place");
        let revalidated = token_of(observer.observe(&target));

        assert_ne!(
            planned, revalidated,
            "directory-to-symlink replacement must change identity"
        );
        let IdentityToken::Unix { kind, .. } = revalidated;
        assert_eq!(kind, FileKind::Symlink);
    }

    #[cfg(unix)]
    #[test]
    fn toctou_symlink_replaced_by_regular_file_is_detected() {
        let dir = TempDir::new("symlink-to-file");
        let target = dir.path("target");
        std::os::unix::fs::symlink("/nonexistent-elsewhere", &target).expect("create symlink");
        let observer = SystemIdentityObserver;
        let planned = token_of(observer.observe(&target));

        std::fs::remove_file(&target).expect("remove symlink"); // unlink, not follow
        std::fs::write(&target, b"hello").expect("create regular file in its place");
        let revalidated = token_of(observer.observe(&target));

        assert_ne!(
            planned, revalidated,
            "symlink-to-file replacement must change identity"
        );
        let IdentityToken::Unix { kind, .. } = revalidated;
        assert_eq!(kind, FileKind::File);
    }

    #[cfg(unix)]
    #[test]
    fn toctou_file_deleted_and_recreated_with_identical_content_still_changes_identity() {
        // The sharpest case: everything an mtime/size-only check could see (name, content,
        // roughly-similar timestamp) looks unchanged, but the object is not the one that was
        // planned against - only device+inode catch this.
        let dir = TempDir::new("recreated-same-content");
        let target = dir.path("target");
        std::fs::write(&target, b"hello").expect("create file");
        let observer = SystemIdentityObserver;
        let planned = token_of(observer.observe(&target));

        std::fs::remove_file(&target).expect("remove file");
        std::fs::write(&target, b"hello").expect("recreate file with identical content");
        let revalidated = token_of(observer.observe(&target));

        assert_ne!(
            planned.clone(),
            revalidated,
            "a deleted-and-recreated object must not be trusted as the same identity even \
             when its content is byte-identical"
        );
        let (
            IdentityToken::Unix {
                inode: planned_inode,
                ..
            },
            IdentityToken::Unix {
                inode: revalidated_inode,
                ..
            },
        ) = (&planned, &revalidated);
        assert_ne!(
            planned_inode, revalidated_inode,
            "recreation must allocate a new inode"
        );
    }

    // --- TOCTOU cases a test sandbox cannot construct against a real filesystem (a
    // mount-boundary swap needs root; a Windows reparse-point swap needs Windows) are
    // constructed synthetically instead, proving the *comparison* is sound even where the
    // *real* observation is out of reach here.
    #[test]
    fn toctou_mount_boundary_swap_is_detected_via_synthetic_device_change() {
        let mut observer = SyntheticIdentityObserver::new();
        let path = PathBuf::from("/synthetic/mount-point/child");
        let before = IdentityToken::Unix {
            device: 1,
            inode: 42,
            kind: FileKind::Directory,
            modified: Timestamp(1_000),
        };
        observer.set(&path, IdentityObservation::Identity(before.clone()));
        let planned = token_of(observer.observe(&path));

        // A different filesystem/volume mounted at the same path: same inode number is
        // possible (inode numbers are only unique per-device), device is what changes.
        let after = IdentityToken::Unix {
            device: 2,
            inode: 42,
            kind: FileKind::Directory,
            modified: Timestamp(1_000),
        };
        observer.set(&path, IdentityObservation::Identity(after));
        let revalidated = token_of(observer.observe(&path));

        assert_ne!(
            planned, revalidated,
            "a mount-boundary swap must change identity"
        );
    }

    #[test]
    fn unsupported_identity_is_never_equal_to_a_real_identity() {
        // AC2: an unsupported platform must lower authority, not be treated as a wildcard
        // that happens to compare equal or unequal in a way a caller could rely on.
        let real = IdentityObservation::Identity(IdentityToken::Unix {
            device: 1,
            inode: 1,
            kind: FileKind::File,
            modified: Timestamp(0),
        });
        let unsupported = IdentityObservation::Unsupported {
            reason: "no verified Windows identity yet".into(),
        };
        assert_ne!(real, unsupported);
    }

    #[test]
    fn synthetic_observer_reports_absent_for_unset_paths() {
        let observer = SyntheticIdentityObserver::new();
        assert_eq!(
            observer.observe(Path::new("/never/configured")),
            IdentityObservation::Absent
        );
    }

    #[cfg(not(unix))]
    #[test]
    fn system_observer_reports_unsupported_off_unix() {
        let observer = SystemIdentityObserver;
        match observer.observe(Path::new(".")) {
            IdentityObservation::Unsupported { .. } => {}
            other => panic!("expected Unsupported off Unix, got {other:?}"),
        }
    }
}

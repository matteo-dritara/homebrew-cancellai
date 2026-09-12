//! Filesystem clone/reflink capability (E10-S01, `docs/architecture/PLATFORM_MODEL.md`'s
//! "logical and allocated-size observation" extended to reclaim accounting).
//!
//! [`crate::allocation::AllocationObserver`] reports one file's real allocated size, but
//! summing that number across many files is only a true reclaim estimate when the underlying
//! filesystem cannot make two files share the same disk blocks. APFS clones (`clonefile(2)`,
//! what every `cp` on macOS uses by default), Btrfs/XFS reflinks, and ZFS clones/dedup can all
//! make that assumption false: deleting one file may free none of its "allocated" bytes if a
//! surviving clone still references the same blocks. This module never tries to detect actual
//! sharing between specific files (that needs extent-level introspection, e.g. Linux's
//! `FS_IOC_FIEMAP`, which is out of this story's scope) - it only classifies whether the
//! filesystem *could* be sharing blocks at all, so a reclaim estimate downstream
//! (`cancellai-inventory::reclaim`) knows when it must not be presented as guaranteed (SI-008/
//! SI-009 generalized: unknown sharing is never silently equated with no sharing).

use std::path::Path;

/// Whether a filesystem is known to support clone/reflink sharing that could make a naive sum
/// of allocated sizes overstate real reclaim.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CloneSemantics {
    /// This filesystem type is not known to support block-level clone/reflink sharing between
    /// files - a real allocated-size sum is not knowingly inflated by it. Not a proof that
    /// sharing is architecturally impossible on every implementation of this filesystem, only
    /// that this classifier has no reason to doubt the sum.
    NotKnownToShare { filesystem: String },
    /// This filesystem type is known (or not positively ruled out) to support clone/reflink
    /// sharing between files - a real allocated-size sum may overstate what deleting these
    /// files would actually free.
    PossiblyShared { filesystem: String },
    /// This platform/path's filesystem clone capability could not be determined.
    Unsupported { reason: String },
}

/// A source of filesystem clone-capability facts. Mirrors this crate's other observer seams
/// (`IdentityObserver`, `AllocationObserver`, `wsl::FilesystemContextObserver`).
pub trait FilesystemKindObserver: Send + Sync {
    fn observe(&self, path: &Path) -> CloneSemantics;
}

/// The real, OS-backed observer.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemFilesystemKindObserver;

impl FilesystemKindObserver for SystemFilesystemKindObserver {
    fn observe(&self, path: &Path) -> CloneSemantics {
        observe_system_filesystem_kind(path)
    }
}

/// Filesystem type names with no known clone/reflink capability in ordinary use - deliberately
/// not exhaustive. A name absent from *both* lists below falls through to
/// [`CloneSemantics::PossiblyShared`] rather than this one, so an unrecognized filesystem is
/// disclosed as uncertain, never silently trusted (C-12 cross-platform truthfulness).
const KNOWN_NOT_SHARING: &[&str] = &[
    "ext2", "ext3", "ext4", "vfat", "msdos", "exfat", "fat32", "ntfs", "hfs", "tmpfs",
];

/// Filesystem type names with real, documented clone/reflink/dedup capability: APFS
/// (`clonefile(2)`), Btrfs and XFS (`FICLONE`/reflink, XFS's on by default since `xfsprogs`
/// enabled it years ago), and ZFS (dataset clones and block-level dedup).
const KNOWN_SHARING: &[&str] = &["apfs", "btrfs", "xfs", "zfs", "refs"];

/// Pure classification of a real filesystem type name, independently testable without any
/// filesystem access (mirrors [`crate::wsl::classify_fstype`]'s observation/classification
/// split).
fn classify_filesystem_name(name: &str) -> CloneSemantics {
    let lower = name.to_ascii_lowercase();
    if KNOWN_SHARING.contains(&lower.as_str()) {
        CloneSemantics::PossiblyShared {
            filesystem: name.to_string(),
        }
    } else if KNOWN_NOT_SHARING.contains(&lower.as_str()) {
        CloneSemantics::NotKnownToShare {
            filesystem: name.to_string(),
        }
    } else {
        CloneSemantics::PossiblyShared {
            filesystem: name.to_string(),
        }
    }
}

#[cfg(target_os = "macos")]
fn observe_system_filesystem_kind(path: &Path) -> CloneSemantics {
    match cancellai_sealedfs::observe_filesystem_name(path) {
        Ok(name) => classify_filesystem_name(&name),
        Err(e) => CloneSemantics::Unsupported {
            reason: e.to_string(),
        },
    }
}

#[cfg(target_os = "linux")]
fn observe_system_filesystem_kind(path: &Path) -> CloneSemantics {
    if !path.is_absolute() {
        return CloneSemantics::Unsupported {
            reason: "filesystem clone-capability classification requires an absolute path"
                .to_string(),
        };
    }
    match std::fs::read_to_string("/proc/mounts") {
        Ok(mounts) => match crate::wsl::longest_matching_mount_fstype(&mounts, path) {
            Some(fstype) => classify_filesystem_name(fstype),
            None => CloneSemantics::Unsupported {
                reason: "no matching entry in /proc/mounts for this path".to_string(),
            },
        },
        Err(e) => CloneSemantics::Unsupported {
            reason: format!("could not read /proc/mounts: {e}"),
        },
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn observe_system_filesystem_kind(_path: &Path) -> CloneSemantics {
    CloneSemantics::Unsupported {
        reason: "filesystem clone/reflink capability detection is not implemented on this \
                 platform; a reclaim estimate here must not be presented as guaranteed"
            .to_string(),
    }
}

/// Test-only seam: synthesize filesystem-kind facts without touching the real filesystem. A
/// path with no fact explicitly `set` observes as `Unsupported` - there is no honest default
/// answer for an unconfigured path, matching [`crate::wsl::SyntheticFilesystemContextObserver`]'s
/// own convention.
#[derive(Debug, Default)]
pub struct SyntheticFilesystemKindObserver {
    facts: std::collections::BTreeMap<std::path::PathBuf, CloneSemantics>,
}

impl SyntheticFilesystemKindObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(
        &mut self,
        path: impl Into<std::path::PathBuf>,
        observation: CloneSemantics,
    ) -> &mut Self {
        self.facts.insert(path.into(), observation);
        self
    }
}

impl FilesystemKindObserver for SyntheticFilesystemKindObserver {
    fn observe(&self, path: &Path) -> CloneSemantics {
        self.facts
            .get(path)
            .cloned()
            .unwrap_or_else(|| CloneSemantics::Unsupported {
                reason: "no synthetic fact configured for this path".to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apfs_is_classified_as_possibly_shared() {
        assert_eq!(
            classify_filesystem_name("apfs"),
            CloneSemantics::PossiblyShared {
                filesystem: "apfs".to_string()
            }
        );
    }

    #[test]
    fn btrfs_xfs_zfs_and_refs_are_all_classified_as_possibly_shared() {
        for fstype in ["btrfs", "xfs", "zfs", "refs"] {
            assert_eq!(
                classify_filesystem_name(fstype),
                CloneSemantics::PossiblyShared {
                    filesystem: fstype.to_string()
                },
                "{fstype} must be classified PossiblyShared"
            );
        }
    }

    #[test]
    fn ext4_and_ntfs_are_classified_as_not_known_to_share() {
        for fstype in ["ext4", "ntfs"] {
            assert_eq!(
                classify_filesystem_name(fstype),
                CloneSemantics::NotKnownToShare {
                    filesystem: fstype.to_string()
                },
                "{fstype} must be classified NotKnownToShare"
            );
        }
    }

    #[test]
    fn an_unrecognized_filesystem_defaults_to_possibly_shared_never_trusted_by_default() {
        assert_eq!(
            classify_filesystem_name("some-future-cow-fs"),
            CloneSemantics::PossiblyShared {
                filesystem: "some-future-cow-fs".to_string()
            }
        );
    }

    #[test]
    fn classification_is_case_insensitive_but_preserves_the_observed_spelling() {
        assert_eq!(
            classify_filesystem_name("APFS"),
            CloneSemantics::PossiblyShared {
                filesystem: "APFS".to_string()
            }
        );
    }

    #[test]
    fn synthetic_observer_reports_unsupported_for_unset_paths() {
        let observer = SyntheticFilesystemKindObserver::new();
        assert_eq!(
            observer.observe(Path::new("/never/configured")),
            CloneSemantics::Unsupported {
                reason: "no synthetic fact configured for this path".to_string()
            }
        );
    }

    #[test]
    fn synthetic_observer_reports_exactly_what_was_configured() {
        let mut observer = SyntheticFilesystemKindObserver::new();
        observer.set(
            "/vol",
            CloneSemantics::NotKnownToShare {
                filesystem: "ext4".to_string(),
            },
        );
        assert_eq!(
            observer.observe(Path::new("/vol")),
            CloneSemantics::NotKnownToShare {
                filesystem: "ext4".to_string()
            }
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn system_observer_reports_a_real_answer_on_macos() {
        let observer = SystemFilesystemKindObserver;
        if let CloneSemantics::Unsupported { reason } = observer.observe(Path::new(".")) {
            panic!("expected a real classification on macOS, got Unsupported: {reason}")
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn system_observer_reports_a_real_answer_on_linux_for_an_absolute_path() {
        let observer = SystemFilesystemKindObserver;
        if let CloneSemantics::Unsupported { reason } = observer.observe(Path::new("/")) {
            panic!("expected a real classification on Linux, got Unsupported: {reason}")
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn system_observer_reports_unsupported_for_a_relative_path_on_linux() {
        let observer = SystemFilesystemKindObserver;
        assert_eq!(
            observer.observe(Path::new("relative")),
            CloneSemantics::Unsupported {
                reason: "filesystem clone-capability classification requires an absolute path"
                    .to_string()
            }
        );
    }
}

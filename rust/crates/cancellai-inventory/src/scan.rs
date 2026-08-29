//! One traversal per scope (E04-S02): a single recursive walk of a scope root produces one
//! [`InventorySnapshot`], and every view (`status_summary`, `top_consumers`,
//! `planning_candidates`) is a pure read over that same snapshot - never a fresh walk of its
//! own. This replaces the pattern `docs/architecture/AS_IS.md` documents for the Python
//! reference, where status/planning/top-consumers each re-walked the same directory tree.
//!
//! Traversal never follows a symlink, and never crosses a filesystem/volume boundary
//! (`docs/architecture/PLATFORM_MODEL.md` "Boundary rules", SI-018): a mounted child is
//! recorded as a fact (with [`crate::file_facts::ScopeBoundary::CrossesBoundary`]) but its
//! own children are not visited. This is a read-only inventory pass, not a mutation - it
//! never calls anything in `cancellai-safety`/`cancellai-platform::mutation`.

use std::path::{Path, PathBuf};

use cancellai_platform::{AllocationObserver, FsObserver, IdentityObservation, IdentityObserver};

use crate::file_facts::{FactObservation, FileFacts, observe_file_facts};

/// A single directory this scan could not fully read - preserved with its raw cause so a
/// completeness rollup (E04-S03) has real evidence rather than a bare "something failed"
/// (SI-010: scan errors are visible, never collapsed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryError {
    pub path: PathBuf,
    pub kind: DirectoryErrorKind,
    pub message: String,
}

/// Distinguishes *why* a directory could not be fully listed. Unlike
/// [`cancellai_platform::Observation::Unreadable`] (a single opaque `reason` string), a
/// `read_dir` failure's `std::io::ErrorKind` is available directly at the call site here, so
/// this scan records it precisely instead of collapsing every failure into one generic kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryErrorKind {
    /// The directory existed when its parent listed it, but had vanished by the time this
    /// scan tried to read it (a listing-to-read race, not a permission problem).
    Disappeared,
    PermissionDenied,
    Other,
}

/// The result of walking one scope exactly once. Every view method below reads only these
/// fields - none re-touches the filesystem.
#[derive(Debug, Clone)]
pub struct InventorySnapshot {
    pub scope_root: PathBuf,
    /// The scope root's own device, if identity observation could establish one. `None` means
    /// every descendant's [`crate::file_facts::ScopeBoundary`] is `Unknown` rather than a
    /// guessed `WithinScope` (SI-017).
    pub root_device: Option<u64>,
    pub root_fact: FactObservation,
    pub facts: Vec<FileFacts>,
    pub directory_errors: Vec<DirectoryError>,
    pub directories_visited: usize,
    pub paths_observed: usize,
}

impl InventorySnapshot {
    /// A read-only rollup used by a status view: total known logical bytes across every fact
    /// this snapshot already holds, plus the traversal counters proving no extra work was
    /// done to answer it.
    pub fn status_summary(&self) -> StatusSummary {
        StatusSummary {
            total_entries: self.facts.len(),
            total_logical_bytes: self.total_logical_size(),
            directories_visited: self.directories_visited,
            paths_observed: self.paths_observed,
        }
    }

    pub fn total_logical_size(&self) -> u64 {
        self.facts
            .iter()
            .filter_map(|f| match f.logical_size {
                crate::file_facts::SizeMetric::Known { bytes } => Some(bytes),
                crate::file_facts::SizeMetric::Unsupported { .. } => None,
            })
            .sum()
    }

    /// The `n` facts with the largest known logical size, descending. A read-only sort over
    /// the existing `facts` vector - no additional filesystem access.
    pub fn top_consumers(&self, n: usize) -> Vec<&FileFacts> {
        let mut known: Vec<&FileFacts> = self
            .facts
            .iter()
            .filter(|f| matches!(f.logical_size, crate::file_facts::SizeMetric::Known { .. }))
            .collect();
        known.sort_by(|a, b| {
            let bytes_of = |f: &FileFacts| match f.logical_size {
                crate::file_facts::SizeMetric::Known { bytes } => bytes,
                crate::file_facts::SizeMetric::Unsupported { .. } => 0,
            };
            bytes_of(b).cmp(&bytes_of(a))
        });
        known.truncate(n);
        known
    }

    /// A placeholder planning-input view: every observed fact, alongside nothing more. This
    /// is *not* a real planning engine (that requires policy/classification, E05/E06); it
    /// exists only to prove a third named caller (AC1's "status/planning/top-consumers")
    /// reuses this same snapshot rather than re-scanning. `crate::completeness` (E04-S03)
    /// wraps this in a view that also carries scope completeness, which is the form a real
    /// planning caller must use - this bare accessor is intentionally not the public
    /// planning entry point once that exists.
    pub fn planning_candidates(&self) -> Vec<&FileFacts> {
        self.facts.iter().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusSummary {
    pub total_entries: usize,
    pub total_logical_bytes: u64,
    pub directories_visited: usize,
    pub paths_observed: usize,
}

/// Walks `scope_root` exactly once, depth-first, using the given platform seams. Symlinks are
/// recorded as facts but never followed; a directory whose identity crosses the scope's own
/// device boundary is recorded but not descended into (SI-018).
pub fn scan_scope(
    scope_root: &Path,
    fs: &dyn FsObserver,
    identity: &dyn IdentityObserver,
    allocation: &dyn AllocationObserver,
) -> InventorySnapshot {
    let root_fact = observe_file_facts(scope_root, fs, identity, allocation, None);
    let root_device = match &root_fact {
        FactObservation::Present(facts) => match &facts.identity {
            IdentityObservation::Identity(token) => Some(token.device()),
            _ => None,
        },
        _ => None,
    };

    let mut snapshot = InventorySnapshot {
        scope_root: scope_root.to_path_buf(),
        root_device,
        root_fact,
        facts: Vec::new(),
        directory_errors: Vec::new(),
        directories_visited: 0,
        paths_observed: 0,
    };

    // Only a real, on-scope directory root is worth descending into. A root that is absent,
    // unreadable, a plain file, or a symlink has nothing beneath it this walk should visit.
    let should_descend = matches!(
        &snapshot.root_fact,
        FactObservation::Present(f) if f.kind == cancellai_platform::FileKind::Directory
    );
    if should_descend {
        walk_directory(
            scope_root,
            fs,
            identity,
            allocation,
            root_device,
            &mut snapshot,
        );
    }

    snapshot
}

fn walk_directory(
    dir: &Path,
    fs: &dyn FsObserver,
    identity: &dyn IdentityObserver,
    allocation: &dyn AllocationObserver,
    scope_device: Option<u64>,
    snapshot: &mut InventorySnapshot,
) {
    snapshot.directories_visited += 1;

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            snapshot.directory_errors.push(DirectoryError {
                path: dir.to_path_buf(),
                kind: classify_io_error(&e),
                message: e.to_string(),
            });
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                snapshot.directory_errors.push(DirectoryError {
                    path: dir.to_path_buf(),
                    kind: classify_io_error(&e),
                    message: e.to_string(),
                });
                continue;
            }
        };
        let child_path = entry.path();
        snapshot.paths_observed += 1;

        let observation = observe_file_facts(&child_path, fs, identity, allocation, scope_device);
        let (kind, boundary, identity_observation) = match &observation {
            FactObservation::Present(facts) => (
                Some(facts.kind),
                Some(facts.boundary.clone()),
                Some(facts.identity.clone()),
            ),
            _ => (None, None, None),
        };
        if let FactObservation::Present(facts) = observation {
            snapshot.facts.push(facts);
        }

        let is_directory = kind == Some(cancellai_platform::FileKind::Directory);
        let crosses_boundary = matches!(
            boundary,
            Some(crate::file_facts::ScopeBoundary::CrossesBoundary)
        );
        let identity_confirmed_directory =
            matches!(identity_observation, Some(IdentityObservation::Identity(_)));
        // Descend only into a directory whose identity is actually confirmed and that does
        // not cross the scope's device boundary. An unconfirmed identity (Unreadable /
        // Unsupported / raced-Absent) never earns a descend by default (SI-017) - the
        // directory's own fact (with its degraded confidence) is still recorded above.
        if is_directory && identity_confirmed_directory && !crosses_boundary {
            walk_directory(
                &child_path,
                fs,
                identity,
                allocation,
                scope_device,
                snapshot,
            );
        }
    }
}

fn classify_io_error(e: &std::io::Error) -> DirectoryErrorKind {
    match e.kind() {
        std::io::ErrorKind::NotFound => DirectoryErrorKind::Disappeared,
        std::io::ErrorKind::PermissionDenied => DirectoryErrorKind::PermissionDenied,
        _ => DirectoryErrorKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_platform::{SystemAllocationObserver, SystemFsObserver, SystemIdentityObserver};

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-scan-test-{label}-{}",
                std::process::id()
            ));
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).expect("create temp root");
            Self(dir)
        }

        fn path(&self, rel: &str) -> PathBuf {
            self.0.join(rel)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn system_scan(root: &Path) -> InventorySnapshot {
        scan_scope(
            root,
            &SystemFsObserver,
            &SystemIdentityObserver,
            &SystemAllocationObserver,
        )
    }

    #[test]
    fn ac1_one_traversal_visits_every_directory_exactly_once() {
        let tree = TempTree::new("nested");
        std::fs::create_dir_all(tree.path("a/b/c")).unwrap();
        std::fs::write(tree.path("a/f1.txt"), b"hello").unwrap();
        std::fs::write(tree.path("a/b/f2.txt"), b"world").unwrap();
        std::fs::write(tree.path("a/b/c/f3.txt"), b"!").unwrap();

        let snapshot = system_scan(&tree.0);

        // scope root + a + a/b + a/b/c = 4 directories, visited exactly once each.
        assert_eq!(snapshot.directories_visited, 4);
        assert_eq!(snapshot.paths_observed, 6); // a, a/f1.txt, a/b, a/b/f2.txt, a/b/c, a/b/c/f3.txt
        assert_eq!(snapshot.facts.len(), 6);
    }

    #[test]
    fn ac1_status_top_consumers_and_planning_reuse_the_same_snapshot_without_rescanning() {
        let tree = TempTree::new("views");
        std::fs::write(tree.path("small.txt"), vec![b'x'; 10]).unwrap();
        std::fs::write(tree.path("big.txt"), vec![b'x'; 1000]).unwrap();

        let snapshot = system_scan(&tree.0);
        let before = (snapshot.directories_visited, snapshot.paths_observed);

        let status = snapshot.status_summary();
        let top = snapshot.top_consumers(1);
        let planning = snapshot.planning_candidates();

        // Calling all three views does not mutate the traversal counters - they are pure
        // reads over facts already collected by the one earlier walk.
        assert_eq!(
            before,
            (snapshot.directories_visited, snapshot.paths_observed)
        );
        assert_eq!(status.total_entries, 2);
        assert_eq!(status.total_logical_bytes, 1010);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].path, tree.path("big.txt"));
        assert_eq!(planning.len(), 2);
    }

    #[test]
    fn symlinks_are_recorded_but_never_descended_into() {
        let tree = TempTree::new("symlink");
        std::fs::create_dir_all(tree.path("real")).unwrap();
        std::fs::write(tree.path("real/inside.txt"), b"data").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(tree.path("real"), tree.path("link")).unwrap();

        #[cfg(unix)]
        {
            let snapshot = system_scan(&tree.0);
            // scope root + "real" = 2 directories; "link" is a symlink and is never
            // read_dir'd, so it does not add a third.
            assert_eq!(snapshot.directories_visited, 2);
            let link_fact = snapshot
                .facts
                .iter()
                .find(|f| f.path == tree.path("link"))
                .expect("symlink fact recorded");
            assert_eq!(link_fact.kind, cancellai_platform::FileKind::Symlink);
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_on_a_different_device_is_recorded_but_not_descended_into() {
        // A synthetic identity double stands in for a real mount-boundary swap (impractical
        // to construct in a sandboxed test, same rationale as
        // cancellai-platform::identity's own mount-boundary test).
        use cancellai_platform::{FileKind, IdentityToken, SyntheticIdentityObserver, Timestamp};

        let tree = TempTree::new("mount");
        std::fs::create_dir_all(tree.path("mounted/inside")).unwrap();
        std::fs::write(tree.path("mounted/inside/f.txt"), b"data").unwrap();

        let mut identity = SyntheticIdentityObserver::new();
        identity.set(
            &tree.0,
            IdentityObservation::Identity(IdentityToken::Unix {
                device: 1,
                inode: 1,
                kind: FileKind::Directory,
                modified: Timestamp(1_000),
            }),
        );
        identity.set(
            tree.path("mounted"),
            IdentityObservation::Identity(IdentityToken::Unix {
                device: 2, // a different device: a mounted filesystem
                inode: 2,
                kind: FileKind::Directory,
                modified: Timestamp(1_000),
            }),
        );

        let snapshot = scan_scope(
            &tree.0,
            &SystemFsObserver,
            &identity,
            &SystemAllocationObserver,
        );

        // Only the scope root itself was read_dir'd; "mounted" was recorded but not
        // descended into, so "mounted/inside" was never observed.
        assert_eq!(snapshot.directories_visited, 1);
        assert!(
            !snapshot
                .facts
                .iter()
                .any(|f| f.path == tree.path("mounted/inside"))
        );
        let mounted_fact = snapshot
            .facts
            .iter()
            .find(|f| f.path == tree.path("mounted"))
            .expect("mounted directory fact recorded");
        assert_eq!(
            mounted_fact.boundary,
            crate::file_facts::ScopeBoundary::CrossesBoundary
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_subdirectory_is_recorded_as_a_directory_error_not_silently_dropped() {
        use std::os::unix::fs::PermissionsExt;

        let tree = TempTree::new("locked");
        std::fs::create_dir_all(tree.path("locked")).unwrap();
        std::fs::write(tree.path("locked/secret.txt"), b"data").unwrap();
        std::fs::set_permissions(tree.path("locked"), std::fs::Permissions::from_mode(0o000))
            .unwrap();

        let snapshot = system_scan(&tree.0);

        // Restore permissions so TempTree::drop can clean up.
        std::fs::set_permissions(tree.path("locked"), std::fs::Permissions::from_mode(0o755))
            .unwrap();

        assert!(
            snapshot
                .directory_errors
                .iter()
                .any(|e| e.path == tree.path("locked")
                    && e.kind == DirectoryErrorKind::PermissionDenied),
            "expected a PermissionDenied directory error, got {:?}",
            snapshot.directory_errors
        );
        // The directory's own fact is still recorded (SI-010) even though its children are not.
        assert!(snapshot.facts.iter().any(|f| f.path == tree.path("locked")));
        assert!(
            !snapshot
                .facts
                .iter()
                .any(|f| f.path == tree.path("locked/secret.txt"))
        );
    }

    #[test]
    fn scanning_twice_over_an_unchanged_tree_is_deterministic() {
        let tree = TempTree::new("determinism");
        std::fs::create_dir_all(tree.path("a/b")).unwrap();
        std::fs::write(tree.path("a/f.txt"), b"stable").unwrap();

        let first = system_scan(&tree.0);
        let second = system_scan(&tree.0);

        assert_eq!(first.directories_visited, second.directories_visited);
        assert_eq!(first.paths_observed, second.paths_observed);
        assert_eq!(first.facts.len(), second.facts.len());
        assert_eq!(first.total_logical_size(), second.total_logical_size());
    }
}

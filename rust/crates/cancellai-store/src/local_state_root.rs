//! cancellAI's own local-state root (E13-S06, closing E13-S04's round-3 independent verifier
//! finding: `project/evidence/E13-VERIFIER-REVIEW-ROUND3.md`, "E13-S04 failure: static marker is
//! not ownership authority").
//!
//! Round 3 reproduced a provider-owned SQLite file, hand-crafted with the exact schema and the
//! exact compiled-in identity marker each of [`crate::CurrentStateStore::open`],
//! [`crate::ledger::EventLedger::open`] and [`crate::rollup::AnalyticalMemory::open`] checked for,
//! passed to those `open` functions, and had its one provider row deleted by the returned
//! handle's `reset()`. The marker is content inside the file a caller supplies; whoever supplies
//! the file controls the content, so checking it can only ever refuse a mimic that forgot the
//! marker, never one that copied it (the owner's own governance decision,
//! `feat(governance): register E13-S06...`, rejected a random per-install sidecar secret for the
//! identical reason: an attacker who fabricates both the sidecar and the marker together controls
//! both and can make them consistent regardless of which value this crate picks).
//!
//! The fix this module gives every layer's production `open()` is not a better check - it is no
//! path to check at all. [`LocalStateRoot::resolve`] is the only way to obtain a
//! [`LocalStateRoot`], and each layer's production `open()` (`crate::lib`/`ledger`/`rollup`)
//! derives its own database's location from it by joining a filename this crate alone fixes,
//! never a caller-suppliable path. A caller therefore names *which directory* is cancellAI's
//! own, but a provider-owned or mimicked file living anywhere else - including right next to it,
//! including bearing a byte-for-byte copy of the compiled-in marker - is never reachable from
//! that production entry point, because the entry point never constructs a handle anywhere but
//! `<resolved root>/<fixed filename>`. The compiled-in marker stays exactly what it always was
//! defensible as: a corruption/migration sanity check on a file this crate already knows it owns
//! by construction, never the thing that decides ownership.
//!
//! Each layer keeps its own `open_at_path` (crate-private) alongside this - the existing
//! migration/marker/reopen unit tests keep calling it directly with an explicit path, unchanged,
//! per this story's own AC3 ("existing unit tests keep exercising open()/reset() directly").
//! Only the *production* `open()` signature changes to take a [`LocalStateRoot`] instead of a
//! `Path`.
//!
//! ## Residual: the root directory itself is trusted, not identity-pinned
//!
//! [`LocalStateRoot::resolve`] canonicalizes the directory it is given once, but - unlike
//! `cancellai-safety::ApprovedRoot::establish`'s device/inode binding for provider roots
//! (`docs/architecture/PLATFORM_MODEL.md`) - it does not re-verify that identity on every use.
//! Reusing `ApprovedRoot` was considered and rejected: `docs/architecture/PERSISTENCE_MODEL.md`
//! states, as a deliberate isolation this story does not relax, that `cancellai-store` depends on
//! no part of `cancellai-safety` at all, so nothing this crate returns can reach the mutation
//! executor's authority even by accident. A root directory later replaced by a symlink between
//! resolution and a subsequent `open()` is therefore a disclosed residual, not a defect this
//! story closes - as is the narrower case of the fixed filename itself, inside an already-
//! resolved root, being a symlink to a file elsewhere: `Connection::open` follows it like any
//! other SQLite client would. Both share the same boundary this story draws: a caller who
//! already has write access to cancellAI's own resolved local-state root is a different,
//! unaddressed threat, not the one round 3 reproduced - that reproduction supplied an arbitrary
//! path to the *old* `open(path)` API from entirely outside cancellAI's own storage, which the
//! production API can no longer accept at all.

use std::path::{Path, PathBuf};

/// Why a [`LocalStateRoot`] could not be established - the directory could not be created, or an
/// existing path at that name could not be canonicalized (for example, it names a regular file,
/// not a directory).
#[derive(Debug)]
pub struct LocalStateRootError(String);

impl std::fmt::Display for LocalStateRootError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LocalStateRootError {}

/// cancellAI's own, established local-state root directory. The only public constructor is
/// [`LocalStateRoot::resolve`]; there is no `Default`, no `From<PathBuf>`, and the one field is
/// private, so a value of this type is proof that resolution actually ran.
pub struct LocalStateRoot {
    dir: PathBuf,
}

impl LocalStateRoot {
    /// Establishes `dir` as cancellAI's own local-state root: creates it if it does not already
    /// exist, then canonicalizes it once. This is the one reviewed resolution path this story's
    /// AC names - every production `open()` in this crate reaches its database file only by
    /// joining a fixed filename onto the [`LocalStateRoot`] this call returns, never by taking a
    /// caller-suppliable file path directly.
    pub fn resolve(dir: &Path) -> Result<Self, LocalStateRootError> {
        std::fs::create_dir_all(dir).map_err(|e| {
            LocalStateRootError(format!(
                "could not create cancellAI's local-state root at {}: {e}",
                dir.display()
            ))
        })?;
        let canonical = std::fs::canonicalize(dir).map_err(|e| {
            LocalStateRootError(format!(
                "could not resolve cancellAI's local-state root at {}: {e}",
                dir.display()
            ))
        })?;
        Ok(Self { dir: canonical })
    }

    /// Joins `filename` onto this root - `pub(crate)` because naming the filename stays each
    /// layer's own fixed constant, never a caller-suppliable value; this method only ever
    /// composes a path a caller cannot redirect.
    pub(crate) fn path_for(&self, filename: &str) -> PathBuf {
        self.dir.join(filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cancellai-store-test-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[test]
    fn resolve_creates_a_missing_directory() {
        let dir = unique_temp_dir("resolve-creates");
        assert!(!dir.exists());
        let root = LocalStateRoot::resolve(&dir).expect("resolve must create the directory");
        assert!(dir.exists());
        assert_eq!(root.path_for("x.sqlite3").file_name().unwrap(), "x.sqlite3");
        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    fn resolve_accepts_an_already_existing_directory() {
        let dir = unique_temp_dir("resolve-existing");
        std::fs::create_dir_all(&dir).expect("pre-create test dir");
        LocalStateRoot::resolve(&dir).expect("resolve must accept an existing directory");
        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    fn resolve_refuses_a_path_that_is_an_existing_regular_file() {
        let dir = unique_temp_dir("resolve-refuses-file");
        std::fs::create_dir_all(&dir).expect("create parent test dir");
        let file_path = dir.join("not-a-directory");
        std::fs::write(&file_path, b"not a directory").expect("create a plain file");
        assert!(
            LocalStateRoot::resolve(&file_path).is_err(),
            "resolve must refuse a path that already names a regular file"
        );
        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    fn resolve_refuses_an_empty_path() {
        // Boundary case: `std::fs::create_dir_all("")` is a silent no-op (nothing to create),
        // but `std::fs::canonicalize("")` fails - resolve must surface that failure rather than
        // treating an empty path as some implicit default (e.g. the current working directory).
        assert!(
            LocalStateRoot::resolve(Path::new("")).is_err(),
            "resolve must refuse an empty path rather than silently picking a default directory"
        );
    }

    #[test]
    fn path_for_joins_the_resolved_directory() {
        let dir = unique_temp_dir("path-for-joins");
        let root = LocalStateRoot::resolve(&dir).expect("resolve");
        let joined = root.path_for("current_state.sqlite3");
        assert_eq!(
            joined.parent().expect("parent"),
            dir.canonicalize().unwrap()
        );
        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }
}

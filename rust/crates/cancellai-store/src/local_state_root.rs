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
//! path to check at all. [`LocalStateRoot::resolve_platform_default`] is the only public way to
//! obtain a [`LocalStateRoot`], and each layer's production `open()`
//! (`crate::lib`/`ledger`/`rollup`) derives its own database's location from it by joining a
//! filename this crate alone fixes, never a caller-suppliable path. A provider-owned or mimicked
//! file living anywhere else - including right next to it, including bearing a byte-for-byte
//! copy of the compiled-in marker - is never reachable from that production entry point, because
//! the entry point never constructs a handle anywhere but `<resolved root>/<fixed filename>`. The
//! compiled-in marker stays exactly what it always was defensible as: a corruption/migration
//! sanity check on a file this crate already knows it owns by construction, never the thing that
//! decides ownership.
//!
//! **Round 4 independent review found this module's first attempt still let a caller decide the
//! `<resolved root>` half of that path**: `resolve`'s only public form took an arbitrary `&Path`
//! argument, so a caller could mint a [`LocalStateRoot`] over a real provider directory and reach
//! the identical `reset()`-erases-provider-data outcome round 3 had already reproduced, just one
//! call earlier. [`LocalStateRoot::resolve_platform_default`] takes no path argument at all - it
//! computes cancellAI's own configured location itself (`$CANCELLAI_HOME/state`, or
//! `$HOME/.cancellai/state`) - so a caller can choose *whether* to establish cancellAI's
//! local-state root, never *which directory* is it. The old arbitrary-path `resolve` survives,
//! narrowed to `pub(crate)`, purely so this module's and the other layers' existing direct-path
//! unit tests keep exercising `open()`/`reset()` without going through real environment variables
//! (this story's own AC3); a `compile_fail` doctest below proves it is unreachable from outside
//! the crate.
//!
//! Round 4 also demonstrated a second, independent route to the same outcome: a symlink planted
//! at the fixed leaf filename, inside an otherwise-legitimate resolved root, redirecting a
//! production `open()`/`reset()` call to a file elsewhere. [`LocalStateRoot::path_for`] now
//! refuses to hand out a path that already exists as a symlink, closing that specific,
//! no-race-required reproduction (see "Residual" below for what this does not close).
//!
//! Each layer keeps its own `open_at_path` (crate-private) alongside this - the existing
//! migration/marker/reopen unit tests keep calling it directly with an explicit path, unchanged,
//! per this story's own AC3 ("existing unit tests keep exercising open()/reset() directly").
//! Only the *production* `open()` signature changes to take a [`LocalStateRoot`] instead of a
//! `Path`.
//!
//! ## Residual: TOCTOU and a symlinked root are not closed
//!
//! [`LocalStateRoot::resolve_platform_default`]/[`LocalStateRoot::resolve`] canonicalize the
//! directory once, but - unlike `cancellai-safety::ApprovedRoot::establish`'s device/inode
//! binding for provider roots (`docs/architecture/PLATFORM_MODEL.md`) - do not re-verify that
//! identity on every use. Reusing `ApprovedRoot` was considered and rejected:
//! `docs/architecture/PERSISTENCE_MODEL.md` states, as a deliberate isolation this story does not
//! relax, that `cancellai-store` depends on no part of `cancellai-safety` at all, so nothing this
//! crate returns can reach the mutation executor's authority even by accident. Two things remain
//! open, both requiring a caller who already has write access to cancellAI's own resolved
//! local-state directory - a narrower, different threat than round 3/4 reproduced, not the one
//! this story closes:
//!
//! - a root directory later replaced by a symlink between resolution and a subsequent `open()`;
//! - a race between [`LocalStateRoot::path_for`]'s symlink check and the `open()` call that
//!   follows it (the check closes the reproduced attack, which required no race - the symlink
//!   was already in place - but does not make the two operations atomic).
//!
//! Closing either fully needs an fd-relative/`openat`-style mechanism this crate does not have
//! today (`rusqlite::Connection::open` takes a path, not an already-open directory handle), and
//! is left to a future story rather than attempted here.

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
/// [`LocalStateRoot::resolve_platform_default`], which takes no path argument; there is no
/// `Default`, no `From<PathBuf>`, and the one field is private, so a value of this type is proof
/// that resolution actually ran against cancellAI's own configured location, not an arbitrary
/// caller-chosen one.
///
/// This doctest is the regression proving the arbitrary-path constructor round 4 independent
/// review defeated is unreachable from outside this crate:
///
/// ```compile_fail
/// # use std::path::Path;
/// use cancellai_store::LocalStateRoot;
/// // resolve(dir) is pub(crate): an external caller cannot mint a root over a directory of its
/// // own choosing, only cancellAI's own crate code can (and only this module's tests do).
/// let _ = LocalStateRoot::resolve(Path::new("/some/provider/directory"));
/// ```
pub struct LocalStateRoot {
    dir: PathBuf,
}

/// The pure resolution rule behind [`LocalStateRoot::resolve_platform_default`], isolated from
/// reading real environment variables so tests never have to mutate process-wide global state -
/// matches `cancellai_cli::roots::resolve_from`'s own precedent and its stated reason:
/// `std::env::set_var` is `unsafe`, and this workspace forbids unsafe code outright.
/// `cancellai_home` wins when set; otherwise `home.join(".cancellai")`; `None` when neither is
/// available.
fn platform_default_base(
    cancellai_home: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    cancellai_home.or_else(|| home.map(|home| home.join(".cancellai")))
}

impl LocalStateRoot {
    /// Establishes cancellAI's own local-state root at its one configured platform location -
    /// `$CANCELLAI_HOME/state` if `CANCELLAI_HOME` is set, else `$HOME/.cancellai/state` -
    /// creating it if it does not already exist. This is the only public constructor: unlike
    /// the round-3/round-4-defeated design, it takes no path argument at all, so a caller can
    /// choose *whether* to establish cancellAI's local-state root, never *which directory* is
    /// it. Unix-only for now, matching `cancellai-cli::roots`'s own precedent for provider-root
    /// resolution (`$CODEX_HOME`/`$CLAUDE_CONFIG_DIR` override, `$HOME/...` default).
    pub fn resolve_platform_default() -> Result<Self, LocalStateRootError> {
        let base = platform_default_base(
            std::env::var_os("CANCELLAI_HOME").map(PathBuf::from),
            std::env::var_os("HOME").map(PathBuf::from),
        );
        let Some(base) = base else {
            return Err(LocalStateRootError(
                "cannot resolve cancellAI's own local-state directory: neither CANCELLAI_HOME \
                 nor HOME is set"
                    .to_string(),
            ));
        };
        Self::resolve(&base.join("state"))
    }

    /// The same root as [`Self::resolve_platform_default`], but only if it already exists:
    /// `Ok(None)` when it does not, and nothing is created (E06-S07). A read-only command such
    /// as `plan` consults cancellAI's own state without leaving a directory behind as a side
    /// effect, and an absent state directory is a known-empty state, not missing evidence.
    pub fn existing_platform_default() -> Result<Option<Self>, LocalStateRootError> {
        let base = platform_default_base(
            std::env::var_os("CANCELLAI_HOME").map(PathBuf::from),
            std::env::var_os("HOME").map(PathBuf::from),
        );
        let Some(base) = base else {
            return Err(LocalStateRootError(
                "cannot resolve cancellAI's own local-state directory: neither CANCELLAI_HOME \
                 nor HOME is set"
                    .to_string(),
            ));
        };
        let dir = base.join("state");
        match std::fs::symlink_metadata(&dir) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(LocalStateRootError(format!(
                "could not observe cancellAI's local-state root at {}: {error}",
                dir.display()
            ))),
            Ok(_) => Self::resolve(&dir).map(Some),
        }
    }

    /// Establishes `dir` as cancellAI's own local-state root: creates it if it does not already
    /// exist, then canonicalizes it once. `pub(crate)` - round 4 independent review of E13-S06
    /// found this accepting an arbitrary caller-supplied `dir` as its only public constructor
    /// let a caller mint a root over a real provider directory, defeating this story's own AC
    /// ("never accepted as an arbitrary caller-supplied argument on the production entry
    /// point"). [`Self::resolve_platform_default`] is now the only public entry point; this
    /// method survives, narrowed to the crate, purely so this module's and the other layers'
    /// existing direct-path unit tests keep working (this story's own AC3).
    pub(crate) fn resolve(dir: &Path) -> Result<Self, LocalStateRootError> {
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

    /// Joins `filename` onto this root, refusing if the result already exists as a symlink -
    /// `pub(crate)` because naming the filename stays each layer's own fixed constant, never a
    /// caller-suppliable value. Round 4 independent review of E13-S06 demonstrated a
    /// fixed-filename symlink, planted at this exact leaf before a production `open()` call,
    /// redirecting that call outside the resolved root; refusing here closes that specific,
    /// no-race-required reproduction (the symlink already existed before the call it defeated).
    /// This is not full TOCTOU immunity - a race between this check and the `open()` call that
    /// follows it remains, and a *root* directory later replaced by a symlink is a disclosed
    /// residual this module's own doc already names - but it closes the reproduced attack.
    pub(crate) fn path_for(&self, filename: &str) -> Result<PathBuf, LocalStateRootError> {
        let path = self.dir.join(filename);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => Err(LocalStateRootError(format!(
                "refusing to open {} - it is a symlink, not cancellAI's own database file",
                path.display()
            ))),
            _ => Ok(path),
        }
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
    fn platform_default_base_prefers_cancellai_home_when_set() {
        assert_eq!(
            platform_default_base(
                Some(PathBuf::from("/custom/cancellai")),
                Some(PathBuf::from("/home/someone"))
            ),
            Some(PathBuf::from("/custom/cancellai"))
        );
    }

    #[test]
    fn platform_default_base_falls_back_to_home_dot_cancellai() {
        assert_eq!(
            platform_default_base(None, Some(PathBuf::from("/home/someone"))),
            Some(PathBuf::from("/home/someone/.cancellai"))
        );
    }

    #[test]
    fn platform_default_base_is_none_when_neither_is_set() {
        assert_eq!(platform_default_base(None, None), None);
    }

    #[test]
    fn resolve_creates_a_missing_directory() {
        let dir = unique_temp_dir("resolve-creates");
        assert!(!dir.exists());
        let root = LocalStateRoot::resolve(&dir).expect("resolve must create the directory");
        assert!(dir.exists());
        assert_eq!(
            root.path_for("x.sqlite3")
                .expect("not a symlink")
                .file_name()
                .unwrap(),
            "x.sqlite3"
        );
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
        let joined = root
            .path_for("current_state.sqlite3")
            .expect("not a symlink");
        assert_eq!(
            joined.parent().expect("parent"),
            dir.canonicalize().unwrap()
        );
        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    #[cfg(unix)]
    fn path_for_refuses_a_symlink_planted_at_the_requested_leaf() {
        // Round 4 independent review of E13-S06: a fixed-filename symlink, planted at this
        // exact leaf before a production open() call, redirected that call outside the
        // resolved root. path_for must refuse to hand out such a path at all.
        let dir = unique_temp_dir("path-for-refuses-symlink");
        let elsewhere = unique_temp_dir("path-for-refuses-symlink-target");
        std::fs::create_dir_all(&elsewhere).expect("create symlink target dir");
        let target = elsewhere.join("provider-owned.sqlite3");
        std::fs::write(&target, b"not cancellAI's file").expect("create symlink target file");

        let root = LocalStateRoot::resolve(&dir).expect("resolve");
        std::os::unix::fs::symlink(&target, dir.join("leaf.sqlite3")).expect("plant symlink");

        assert!(
            root.path_for("leaf.sqlite3").is_err(),
            "a symlinked leaf must be refused, not silently followed"
        );

        std::fs::remove_dir_all(&dir).expect("clean up test dir");
        std::fs::remove_dir_all(&elsewhere).expect("clean up target dir");
    }

    #[test]
    fn path_for_accepts_a_leaf_that_does_not_exist_yet() {
        // The common, legitimate case: nothing has been written to this location yet.
        // symlink_metadata returning an error (not-found) must not be mistaken for "refused".
        let dir = unique_temp_dir("path-for-accepts-fresh-leaf");
        let root = LocalStateRoot::resolve(&dir).expect("resolve");
        assert!(root.path_for("fresh.sqlite3").is_ok());
        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }
}

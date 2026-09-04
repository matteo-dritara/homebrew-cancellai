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
//! Real Windows volume/file-index/reparse identity is implemented (E20-S01, ADR-0020), via
//! `cancellai-sealedfs::observe_identity` (`GetFileInformationByHandle`). E20-S01 round-1
//! independent verifier review found this module's own docs, and several others, had claimed
//! this was "verified on real Windows CI" before the branch introducing it had ever actually
//! been pushed and run there - compile/lint-clean cross-target and passing adversarial fixture
//! *code* is not the same evidence as a real `windows-latest` CI run, and this repository does
//! not conflate the two after that finding. `project/platforms.json`'s `windows` entry is the
//! one source of truth for whether that verification has actually happened
//! (`scripts/check_platforms.py check`) - consult it rather than this comment for current
//! status. Only a genuinely exotic non-Unix, non-Windows target still reports
//! [`IdentityObservation::Unsupported`], for the same reason E03-S01 originally chose it for
//! Windows too: a plausible-but-unverified implementation of a safety-critical equality check
//! is a worse outcome than an honest "cannot establish identity strength here" (SI-017, C-12
//! cross-platform truthfulness). `docs/architecture/PLATFORM_MODEL.md`'s own escape hatch - "if
//! the platform cannot produce an identity strong enough ... authority is reduced" - remains
//! exactly this state there.

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
    /// as it exists; `kind`/`modified`/`modified_nanos` are included so a same-inode reuse
    /// still shows up as a mismatch. E07-S05 found this was not merely a theoretical
    /// "astronomically unlikely" case: on Linux, a delete-and-recreate within the same
    /// wall-clock second (routine under fast test/CI timing) both reused the just-freed inode
    /// and left `modified` - `Timestamp`'s deliberately whole-second resolution, correct for
    /// its own cross-cutting clock/retention use (E02-S04) but too coarse here - identical,
    /// making `device`+`inode`+`kind`+`modified` alone unable to distinguish the two objects.
    /// `modified_nanos` is the sub-second remainder of the same modification time, read
    /// directly from the platform's raw `st_mtime_nsec` rather than derived from `Timestamp` -
    /// two writes far enough apart to be genuinely different operations are reliably
    /// nanoseconds apart even when they land in the same second (verified: a real
    /// delete-and-recreate on Linux produced distinct nanosecond components while sharing
    /// both inode and whole-second `modified`).
    Unix {
        device: u64,
        inode: u64,
        kind: FileKind,
        modified: Timestamp,
        modified_nanos: u32,
    },
    /// `GetFileInformationByHandle`'s volume serial number and file index uniquely identify
    /// an object on a given Windows volume for as long as it exists, the same role
    /// device+inode play for `Unix` (E20-S01, ADR-0020). `modified_ticks` is the raw
    /// 100-nanosecond `FILETIME` remainder of the last-write time - the Windows analogue of
    /// `Unix::modified_nanos`, needed for the identical same-second delete-recreate
    /// disambiguation E07-S05 found necessary there. Observed without following a reparse
    /// point at the final path component (`FILE_FLAG_OPEN_REPARSE_POINT`), matching
    /// `symlink_metadata`'s no-follow contract - a reparse point (symlink, junction, or any
    /// other Windows reparse tag) is never treated as, or compared using, Unix symlink
    /// semantics: `kind` is set from `FILE_ATTRIBUTE_REPARSE_POINT` alone, independent of the
    /// `Unix` variant entirely.
    Windows {
        volume_serial_number: u32,
        file_index: u64,
        kind: FileKind,
        modified: Timestamp,
        modified_ticks: u64,
    },
}

impl IdentityToken {
    /// The coarse filesystem-object shape this token describes. A plain accessor rather than
    /// requiring every caller to match on the variant directly, so a future platform variant
    /// is a one-place change instead of every call site.
    pub fn kind(&self) -> FileKind {
        match self {
            IdentityToken::Unix { kind, .. } => *kind,
            IdentityToken::Windows { kind, .. } => *kind,
        }
    }

    /// The filesystem/volume this token's object lives on (E04-S01, SI-018 boundary checks):
    /// the Unix device number, or the Windows volume serial number widened to `u64`. A single
    /// cross-platform accessor rather than a parallel Windows-specific one, so
    /// `cancellai-safety::root_capability`'s boundary check needs no platform branching of its
    /// own (E20-S01 reconsidered E03-S01's original doc comment here, which speculated a
    /// separate accessor - reusing this one turned out simpler once a second variant existed).
    pub fn device(&self) -> u64 {
        match self {
            IdentityToken::Unix { device, .. } => *device,
            IdentityToken::Windows {
                volume_serial_number,
                ..
            } => *volume_serial_number as u64,
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
                    // `mtime_nsec()` is the sub-second remainder of the same modification
                    // time `modified` above already captured whole-seconds-only (via
                    // `modification_timestamp`/`Timestamp`) - always in `[0, 999_999_999]`
                    // per POSIX, so the truncating cast is exact, never wrapping.
                    modified_nanos: meta.mtime_nsec() as u32,
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

/// Windows `FILETIME` is 100-nanosecond ticks since 1601-01-01, not Unix-epoch seconds; this
/// workspace's `Timestamp` is Unix-epoch seconds (E02-S04). `WINDOWS_EPOCH_OFFSET_SECONDS` is
/// the fixed, well-known offset between the two epochs, used only to populate `Timestamp`'s
/// whole-second field for display/retention purposes elsewhere - `modified_ticks` (the raw
/// `FILETIME` remainder, returned separately) is what identity comparison actually relies on,
/// exactly as `Unix::modified_nanos` supplements rather than replaces `Unix::modified`.
///
/// Returns `None` for a real pre-1970 `FILETIME` (valid back to 1601) rather than silently
/// clamping it to `Timestamp(0)` (E20-S01 round-1 independent verifier review's residual
/// finding: the original `saturating_sub` misrepresented the object's real modification date as
/// 1970-01-01 - a wrong fact, not a rounding artifact). Pure and platform-independent, so it is
/// directly unit-testable with fabricated `FILETIME` values on any host, matching this crate's
/// `wsl` module's own split between real OS observation and testable pure logic.
#[cfg(any(test, windows))]
fn windows_filetime_to_unix_timestamp(ticks: u64) -> Option<(Timestamp, u32)> {
    const WINDOWS_EPOCH_OFFSET_SECONDS: u64 = 11_644_473_600;
    const TICKS_PER_SECOND: u64 = 10_000_000;

    let whole_seconds = ticks / TICKS_PER_SECOND;
    let seconds = whole_seconds.checked_sub(WINDOWS_EPOCH_OFFSET_SECONDS)?;
    Some((Timestamp(seconds), (ticks % TICKS_PER_SECOND) as u32))
}

#[cfg(windows)]
fn observe_system_identity(path: &Path) -> IdentityObservation {
    match cancellai_sealedfs::observe_identity(path) {
        Ok(facts) => {
            let kind = if facts.is_reparse_point {
                FileKind::Symlink
            } else if facts.is_directory {
                FileKind::Directory
            } else {
                FileKind::File
            };
            let Some((modified, modified_ticks)) =
                windows_filetime_to_unix_timestamp(facts.last_write_time_ticks)
            else {
                return IdentityObservation::Unreadable {
                    reason: format!(
                        "modification time predates the Unix epoch (raw FILETIME {} is before \
                         1970-01-01); not reported as Timestamp(0)",
                        facts.last_write_time_ticks
                    ),
                };
            };
            IdentityObservation::Identity(IdentityToken::Windows {
                volume_serial_number: facts.volume_serial_number,
                file_index: facts.file_index,
                kind,
                modified,
                modified_ticks: modified_ticks.into(),
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => IdentityObservation::Absent,
        Err(e) => IdentityObservation::Unreadable {
            reason: e.to_string(),
        },
    }
}

#[cfg(not(any(unix, windows)))]
fn observe_system_identity(_path: &Path) -> IdentityObservation {
    IdentityObservation::Unsupported {
        reason: "native volume/file-index/reparse identity is not implemented on this \
                 platform; authority is reduced rather than guessed"
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

    #[test]
    fn windows_filetime_epoch_converts_to_timestamp_zero() {
        // 11_644_473_600 seconds * 10_000_000 ticks/sec is exactly the Unix epoch itself.
        let (modified, modified_ticks) =
            windows_filetime_to_unix_timestamp(11_644_473_600 * 10_000_000)
                .expect("the Unix epoch itself must convert, not refuse");
        assert_eq!(modified, Timestamp(0));
        assert_eq!(modified_ticks, 0);
    }

    #[test]
    fn windows_filetime_after_the_epoch_converts_correctly() {
        // One day (86_400 seconds) after the Unix epoch, plus a nonzero sub-second remainder.
        let ticks = (11_644_473_600 + 86_400) * 10_000_000 + 1_234_567;
        let (modified, modified_ticks) = windows_filetime_to_unix_timestamp(ticks)
            .expect("a real post-epoch FILETIME must convert");
        assert_eq!(modified, Timestamp(86_400));
        assert_eq!(modified_ticks, 1_234_567);
    }

    #[test]
    fn windows_filetime_before_the_unix_epoch_is_refused_not_clamped() {
        // E20-S01 round-1 independent verifier review: a pre-1970 FILETIME (valid back to
        // 1601) must not be silently saturated to Timestamp(0) - that would misrepresent the
        // real modification date. One tick before the epoch.
        assert_eq!(
            windows_filetime_to_unix_timestamp(11_644_473_600 * 10_000_000 - 1),
            None
        );
    }

    #[test]
    fn windows_filetime_zero_is_refused() {
        // FILETIME 0 is 1601-01-01, far before the Unix epoch - must not become Timestamp(0)
        // (which would misleadingly read as 1970-01-01, not 1601-01-01).
        assert_eq!(windows_filetime_to_unix_timestamp(0), None);
    }

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
        let IdentityToken::Unix { kind, .. } = revalidated else {
            panic!("SystemIdentityObserver must report Unix on a cfg(unix) test target");
        };
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
        let IdentityToken::Unix { kind, .. } = revalidated else {
            panic!("SystemIdentityObserver must report Unix on a cfg(unix) test target");
        };
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
        let IdentityToken::Unix { kind, .. } = revalidated else {
            panic!("SystemIdentityObserver must report Unix on a cfg(unix) test target");
        };
        assert_eq!(kind, FileKind::File);
    }

    #[cfg(unix)]
    #[test]
    fn toctou_file_deleted_and_recreated_with_identical_content_still_changes_identity() {
        // The sharpest case: everything an mtime/size-only check could see (name, content,
        // roughly-similar timestamp) looks unchanged, but the object is not the one that was
        // planned against.
        //
        // E07-S05: this test used to also assert the inode specifically changed ("recreation
        // must allocate a new inode") and relied on whole-second `modified` alone for the rest.
        // A real Linux CI reproduction (measured directly, not hypothesized: a tight
        // delete-recreate loop with no intervening delay) found *both* claims false in
        // general - the freed inode was reused in ~98% of iterations, and this container's own
        // mtime clock only advances in ~1ms steps, so a zero-delay recreate routinely lands in
        // the same tick too. Neither is the actual safety property this codebase depends on;
        // that property is "the whole `IdentityToken` differs", which `modified_nanos` (added
        // for exactly this) now provides given any gap larger than the underlying clock's own
        // granularity - a brief sleep here reflects the real-world case this guards
        // (SI-013's revalidate-before-mutate happens after a scan+plan+policy+confirmation
        // cycle, never in the same instant), not a weakening of "byte-identical content".
        let dir = TempDir::new("recreated-same-content");
        let target = dir.path("target");
        std::fs::write(&target, b"hello").expect("create file");
        let observer = SystemIdentityObserver;
        let planned = token_of(observer.observe(&target));

        std::fs::remove_file(&target).expect("remove file");
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&target, b"hello").expect("recreate file with identical content");
        let revalidated = token_of(observer.observe(&target));

        assert_ne!(
            planned, revalidated,
            "a deleted-and-recreated object must not be trusted as the same identity even \
             when its content is byte-identical"
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
            modified_nanos: 0,
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
            modified_nanos: 0,
        };
        observer.set(&path, IdentityObservation::Identity(after));
        let revalidated = token_of(observer.observe(&path));

        assert_ne!(
            planned, revalidated,
            "a mount-boundary swap must change identity"
        );
    }

    #[test]
    fn same_second_delete_and_recreate_still_differs_via_nanosecond_resolution() {
        // E07-S05: real Linux reproduction found `device`+`inode`+`kind`+`modified` alone can
        // legitimately collide - a delete-and-recreate within the same wall-clock second both
        // reused the just-freed inode and left the whole-second `modified` unchanged. This is
        // the synthetic proof that `modified_nanos` is what actually saves the comparison in
        // that exact shape, since a real filesystem's own sub-nanosecond-fast pair (needed to
        // reproduce a genuine collision on both fields for real) is not something this test
        // suite can force deterministically.
        let same_second_but_different_nanos_before = IdentityToken::Unix {
            device: 1,
            inode: 42,
            kind: FileKind::File,
            modified: Timestamp(1_700_000_000),
            modified_nanos: 163_341_198,
        };
        let same_second_but_different_nanos_after = IdentityToken::Unix {
            device: 1,
            inode: 42,
            kind: FileKind::File,
            modified: Timestamp(1_700_000_000),
            modified_nanos: 166_347_725,
        };
        assert_ne!(
            same_second_but_different_nanos_before, same_second_but_different_nanos_after,
            "identical device/inode/kind/modified must still differ when modified_nanos differs"
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
            modified_nanos: 0,
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

    // E20-S01's round-1 independent verifier repair found this test itself stale on real
    // Windows CI: it predates E20-S01/ADR-0020 and asserted the *old*, pre-native-identity
    // behavior (`Unsupported` on every non-Unix target) - since superseded by real Windows
    // identity, so it started failing for real (not a false alarm) the moment this crate's own
    // code stopped being wrong. Windows now gets its own positive assertion below; the
    // genuinely-exotic non-Unix-non-Windows fallback (`observe_system_identity`'s third `cfg`
    // arm) has no real target this workspace runs CI on to exercise it against, so it remains
    // untested here rather than asserted from a host that cannot actually reach that branch.
    #[cfg(windows)]
    #[test]
    fn system_observer_reports_a_real_identity_on_windows() {
        let observer = SystemIdentityObserver;
        match observer.observe(Path::new(".")) {
            IdentityObservation::Identity(IdentityToken::Windows { .. }) => {}
            other => panic!("expected a real Windows Identity, got {other:?}"),
        }
    }
}

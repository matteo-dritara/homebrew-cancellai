//! Filesystem mutation as its own OS capability seam (E03-S05,
//! `docs/architecture/PLATFORM_MODEL.md`'s "atomic rename/move capability", SI-019).
//!
//! This is the one seam in this crate whose real implementation ([`SystemMutationExecutor`])
//! actually changes the filesystem - every other capability here (`Clock`, `FsObserver`,
//! `IdentityObserver`, `PathResolver`) only observes. `scripts/check_mutation_boundary.py`
//! (E03-S05) statically enforces that no other production source file in this workspace
//! calls `std::fs::remove_file`/`remove_dir`/`remove_dir_all` directly - this file is the
//! one place that call is allowed to exist (SI-019: "all filesystem/vendor mutations route
//! through the safety executor"). `cancellai-safety`'s orchestration (`mutation_executor.rs`)
//! is the *only* production caller of this trait, and only after SI-002/SI-003/SI-013 have
//! already been checked - this seam itself performs no safety-policy check of its own
//! (authority/reversibility gating is `cancellai-safety`'s job), but it does prove, as far as
//! a safe-Rust, dependency-free implementation can, that what it deletes is what the caller
//! told it to expect.
//!
//! E03 verifier review round 1 found the original `mutate(target, operation)` signature
//! (no expected identity) let a path-based revalidate-then-delete race succeed silently: an
//! `IdentityObserver` that reports a matching identity and then, as a side effect, swaps the
//! object before the actual `remove_file` call, caused the *replacement* to be deleted while
//! `execute` reported `Succeeded`. `mutate` now takes `expected: &IdentityToken`, and
//! [`SystemMutationExecutor`]'s file-deletion path performs three checks around one held
//! file descriptor: (1) open the target once and confirm the descriptor's own device/inode
//! match `expected`; (2) immediately before the actual unlink, a second, independent, fresh
//! path lookup (not through the descriptor) re-confirms the path still resolves to that same
//! identity - this is the check that actually stops a same-named replacement from being
//! deleted, since a bare *after-the-fact* link-count check alone cannot distinguish "my own
//! unlink zeroed the original" from "a concurrent unlink already zeroed the original before I
//! touched a different object at this path," and both would otherwise look identical; (3)
//! after the unlink, re-stat the *same, already-open* descriptor as final corroboration that
//! its link count dropped to zero. This narrows, but does not perfectly close, the race:
//! step (2) and the unlink itself are still two separate syscalls with a small gap between
//! them, and true prevention (not merely detection) needs an OS-specific handle-relative
//! unlink (`openat`/`unlinkat` with `O_NOFOLLOW`, e.g. via a reviewed `rustix`/`nix`
//! dependency, or `unsafe` libc calls) that this workspace does not have (`unsafe_code` is
//! forbidden by default, ADR-0015, and no such dependency has been reviewed/added). Where
//! that guarantee
//! cannot be established this way, `mutate` reports `Err` rather than a possibly-false
//! `Ok(())` - "refuse where the guarantee cannot be established," not guess. Consequently
//! only `FileKind::File` deletion is confirmed this way; `cancellai-safety::mutation_executor`
//! only ever requests it for that kind (directories/symlinks are refused before reaching this
//! seam at all - see that module's own docs).

use std::path::Path;

use crate::identity::IdentityToken;

/// One class of real mutation this seam can perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationOperation<'a> {
    /// Move to a quarantine location on the same filesystem (reversible). Not yet driven by
    /// any production caller - `SealedPlan` (E03-S02) does not carry a quarantine
    /// destination yet (E03-S05's own residual risk); the operation exists so this seam's
    /// contract does not have to grow again the day that field lands. Not identity-confirmed
    /// the way `DeleteFile` is (see module docs) - a future story implementing it for real
    /// should extend the confirmation technique to it too, not merely rename blindly.
    Quarantine { to: &'a Path },
    /// Permanently remove a regular file. The only operation this seam confirms by
    /// open-file-descriptor identity (see module docs); the only one
    /// `cancellai-safety::mutation_executor` currently requests.
    DeleteFile,
    /// Permanently remove a directory tree. Not identity-confirmed the way `DeleteFile` is -
    /// no production caller currently requests this either (see module docs).
    DeleteDirectoryTree,
}

/// Why a real mutation attempt failed. Always the underlying OS error text, or this seam's
/// own identity-confirmation failure text - this seam does not otherwise interpret or
/// classify failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationError(pub String);

/// A sink for real filesystem mutation. `expected` is the identity the caller already
/// confirmed via `IdentityObserver` immediately before calling this (see module docs for how
/// [`SystemMutationExecutor`] uses it).
pub trait MutationExecutor: Send + Sync {
    fn mutate(
        &self,
        target: &Path,
        expected: &IdentityToken,
        operation: MutationOperation<'_>,
    ) -> Result<(), MutationError>;
}

/// The real, OS-backed executor. The only place in this crate - and, per
/// `scripts/check_mutation_boundary.py`, in this entire workspace outside this one file -
/// that calls a filesystem removal primitive directly.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemMutationExecutor;

impl MutationExecutor for SystemMutationExecutor {
    fn mutate(
        &self,
        target: &Path,
        expected: &IdentityToken,
        operation: MutationOperation<'_>,
    ) -> Result<(), MutationError> {
        match operation {
            MutationOperation::Quarantine { to } => {
                std::fs::rename(target, to).map_err(|e| MutationError(e.to_string()))
            }
            MutationOperation::DeleteFile => confirmed_delete_file(target, expected),
            MutationOperation::DeleteDirectoryTree => {
                std::fs::remove_dir_all(target).map_err(|e| MutationError(e.to_string()))
            }
        }
    }
}

#[cfg(unix)]
fn confirmed_delete_file(target: &Path, expected: &IdentityToken) -> Result<(), MutationError> {
    confirmed_delete_file_inner(target, expected, || {})
}

/// The real logic, parameterized by a hook that runs between the open-time identity
/// confirmation and the actual `remove_file` call. In production this hook is a no-op
/// (`confirmed_delete_file` above); tests use it to deterministically reproduce the exact
/// race the round-1 review found - swapping the target *after* it has been confirmed open
/// but *before* it is unlinked - without needing real thread-timing luck.
#[cfg(unix)]
fn confirmed_delete_file_inner(
    target: &Path,
    expected: &IdentityToken,
    between_open_and_unlink: impl FnOnce(),
) -> Result<(), MutationError> {
    use std::os::unix::fs::MetadataExt;

    let IdentityToken::Unix {
        device: expected_device,
        inode: expected_inode,
        modified: expected_modified,
        modified_nanos: expected_modified_nanos,
        ..
    } = expected;

    let file = std::fs::File::open(target)
        .map_err(|e| MutationError(format!("could not open target for confirmed deletion: {e}")))?;
    let before = file
        .metadata()
        .map_err(|e| MutationError(format!("could not stat open target before deletion: {e}")))?;
    // E07-S05: device+inode alone was found, on real Linux, insufficient - a delete-and-
    // recreate that happened to land within the same wall-clock second reused the just-freed
    // inode too, so a swapped-in replacement could pass this check purely by chance. Comparing
    // the sub-second modification-time remainder as well (the same disambiguator
    // `IdentityToken::Unix::modified_nanos` now carries) catches what device+inode+whole-second
    // `modified` cannot.
    if before.dev() != *expected_device
        || before.ino() != *expected_inode
        || before.mtime() != expected_modified.0 as i64
        || before.mtime_nsec() as u32 != *expected_modified_nanos
    {
        return Err(MutationError(
            "target identity changed between revalidation and deletion (open-time check)"
                .to_string(),
        ));
    }

    between_open_and_unlink();

    // A second, independent, fresh path lookup (not through the held fd) immediately before
    // the actual unlink - this is what actually catches a swap that happened after the
    // open-time check above (as in `between_open_and_unlink`): a bare nlink check *after*
    // `remove_file` cannot distinguish "my own unlink zeroed the original's link count" from
    // "a concurrent unlink of the original already zeroed it before I ever touched the
    // (different) object now sitting at this path" - both produce the same post-hoc nlink
    // reading. Refusing here, *before* calling `remove_file` at all, is what keeps a
    // same-named replacement from being deleted as collateral damage.
    let just_before = std::fs::symlink_metadata(target).map_err(|e| {
        MutationError(format!(
            "could not re-stat target immediately before deletion: {e}"
        ))
    })?;
    if just_before.dev() != *expected_device
        || just_before.ino() != *expected_inode
        || just_before.mtime() != expected_modified.0 as i64
        || just_before.mtime_nsec() as u32 != *expected_modified_nanos
    {
        return Err(MutationError(
            "target identity changed immediately before deletion (path re-check failed); refusing to delete a different object".to_string(),
        ));
    }

    std::fs::remove_file(target).map_err(|e| MutationError(format!("delete failed: {e}")))?;

    // Final corroboration via the fd opened at the very start: an open fd stays valid after
    // its directory entry is unlinked (Unix semantics), so if `remove_file` above really
    // unlinked the object this fd holds, that object's link count is now 0. This narrows,
    // but - being itself a check *after* the mutation - cannot on its own fully close, the
    // remaining gap between the immediately-preceding re-check and the unlink syscall.
    let after = file
        .metadata()
        .map_err(|e| MutationError(format!("could not stat held handle after deletion: {e}")))?;
    if after.nlink() != 0 {
        return Err(MutationError(
            "deletion removed a different filesystem object than the one confirmed open \
             (post-deletion link-count check failed); the intended target may still exist"
                .to_string(),
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn confirmed_delete_file(_target: &Path, _expected: &IdentityToken) -> Result<(), MutationError> {
    Err(MutationError(
        "confirmed file deletion is not implemented on this platform".to_string(),
    ))
}

/// Test-only seam: synthesize a mutation outcome for a specific path without touching the
/// real filesystem - the fault-injection double this story's verification contract names
/// ("fault-injection tests"). A path with no fact explicitly `set` succeeds, since a
/// mutation double that silently fails paths the test never configured to fail would hide
/// exactly the class of bug fault injection exists to find.
#[derive(Debug, Default)]
pub struct SyntheticMutationExecutor {
    outcomes: std::collections::BTreeMap<std::path::PathBuf, Result<(), MutationError>>,
}

impl SyntheticMutationExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(
        &mut self,
        path: impl Into<std::path::PathBuf>,
        outcome: Result<(), MutationError>,
    ) -> &mut Self {
        self.outcomes.insert(path.into(), outcome);
        self
    }
}

impl MutationExecutor for SyntheticMutationExecutor {
    fn mutate(
        &self,
        target: &Path,
        _expected: &IdentityToken,
        _operation: MutationOperation<'_>,
    ) -> Result<(), MutationError> {
        self.outcomes.get(target).cloned().unwrap_or(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    struct TempDir(std::path::PathBuf);

    #[cfg(unix)]
    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "cancellai-mutation-test-{label}-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn path(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }

    #[cfg(unix)]
    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[cfg(unix)]
    fn identity_of(path: &Path) -> IdentityToken {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::symlink_metadata(path).expect("stat path for test identity");
        IdentityToken::Unix {
            device: meta.dev(),
            inode: meta.ino(),
            kind: crate::identity::FileKind::File,
            // The real mtime/nanos, not a placeholder: `confirmed_delete_file_inner` (E07-S05)
            // now compares both against the live file's own metadata, so a hardcoded
            // `Timestamp(0)` here would make every legitimate (no-swap) test below fail this
            // helper's own identity capture, not just the swap-detection tests that want it to.
            modified: crate::clock::Timestamp(meta.mtime() as u64),
            modified_nanos: meta.mtime_nsec() as u32,
        }
    }

    #[cfg(unix)]
    #[test]
    fn system_executor_deletes_a_real_file_confirmed_by_identity() {
        let dir = TempDir::new("delete-confirmed");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = identity_of(&file);

        let executor = SystemMutationExecutor;
        executor
            .mutate(&file, &expected, MutationOperation::DeleteFile)
            .expect("delete should succeed");
        assert!(!file.exists());
    }

    #[cfg(unix)]
    #[test]
    fn system_executor_reports_the_os_error_for_a_missing_target() {
        let dir = TempDir::new("missing-target");
        let missing = dir.path("does-not-exist");
        // Any well-formed expected token: open() fails before it would ever be consulted.
        let expected = IdentityToken::Unix {
            device: 0,
            inode: 0,
            kind: crate::identity::FileKind::File,
            modified: crate::clock::Timestamp(0),
            modified_nanos: 0,
        };
        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(&missing, &expected, MutationOperation::DeleteFile)
            .expect_err("deleting a missing file must fail, not silently succeed");
        assert!(!err.0.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_delete_rejects_a_target_already_swapped_before_open() {
        let dir = TempDir::new("swapped-before-open");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = identity_of(&file); // captured identity of the ORIGINAL

        // Swap before any deletion attempt: replace the original with a different file at
        // the same path (same as an attacker winning the race entirely before this call).
        // E07-S05: a real Linux reproduction found a zero-delay swap can reuse the freed inode
        // and land within the same ~1ms mtime clock tick, defeating even the nanosecond-aware
        // comparison this test wants to exercise - a brief sleep reflects a real race's actual
        // timing (a separate attacker process needs at least a scheduling/syscall round trip),
        // not a weakening of the swap this test constructs.
        std::fs::remove_file(&file).expect("remove original");
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&file, b"replacement").expect("create replacement");

        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(&file, &expected, MutationOperation::DeleteFile)
            .expect_err("a target swapped before open must be rejected, not deleted");
        assert!(err.0.contains("open-time check"), "reason was: {}", err.0);
        assert!(
            file.exists(),
            "the replacement must survive - it was never the intended target"
        );
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_delete_detects_a_target_swapped_between_open_and_unlink() {
        // The exact race E03 verifier review round 1 exploited: the target is confirmed
        // open and matching, then swapped - simulating an observer whose "still matches"
        // answer stops being true at the worst possible moment, which no amount of
        // "revalidate right before mutating" alone can defeat (only proof of what was
        // actually deleted can).
        let dir = TempDir::new("swapped-mid-flight");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = identity_of(&file);

        let result = confirmed_delete_file_inner(&file, &expected, || {
            std::fs::remove_file(&file).expect("simulate concurrent removal of the original");
            std::fs::write(&file, b"replacement").expect("simulate concurrent replacement");
        });

        assert!(
            result.is_err(),
            "a mid-flight swap must never be reported as a successful, correct deletion"
        );
        assert!(
            file.exists(),
            "the replacement must survive - only the confirmed original may ever be deleted"
        );
        std::fs::read_to_string(&file)
            .map(|contents| assert_eq!(contents, "replacement"))
            .expect("replacement content must be intact");
    }

    #[test]
    fn synthetic_executor_succeeds_by_default_for_unconfigured_paths() {
        let executor = SyntheticMutationExecutor::new();
        let expected = IdentityToken::Unix {
            device: 0,
            inode: 0,
            kind: crate::identity::FileKind::File,
            modified: crate::clock::Timestamp(0),
            modified_nanos: 0,
        };
        assert_eq!(
            executor.mutate(
                Path::new("/never/configured"),
                &expected,
                MutationOperation::DeleteFile
            ),
            Ok(())
        );
    }

    #[test]
    fn synthetic_executor_injects_exactly_the_configured_fault() {
        let mut executor = SyntheticMutationExecutor::new();
        executor.set(
            "/synthetic/disk-full",
            Err(MutationError("No space left on device".into())),
        );
        let expected = IdentityToken::Unix {
            device: 0,
            inode: 0,
            kind: crate::identity::FileKind::File,
            modified: crate::clock::Timestamp(0),
            modified_nanos: 0,
        };
        assert_eq!(
            executor.mutate(
                Path::new("/synthetic/disk-full"),
                &expected,
                MutationOperation::DeleteFile
            ),
            Err(MutationError("No space left on device".into()))
        );
        assert_eq!(
            executor.mutate(
                Path::new("/synthetic/unrelated"),
                &expected,
                MutationOperation::DeleteFile
            ),
            Ok(())
        );
    }
}

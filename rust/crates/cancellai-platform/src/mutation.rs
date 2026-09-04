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
//! unlink (`openat`/`unlinkat` with `O_NOFOLLOW`). Where that guarantee cannot be established,
//! `mutate` reports `Err` rather than a possibly-false `Ok(())` - "refuse where the guarantee
//! cannot be established," not guess. Consequently only `FileKind::File` deletion is confirmed
//! this way; `cancellai-safety::mutation_executor` only ever requests it for that kind
//! (directories/symlinks are refused before reaching this seam at all - see that module's own
//! docs).
//!
//! ## Why this is still detection, and what changed underneath it (E21-S01)
//!
//! This module used to justify the residual by stating that the required capability did not
//! exist here: no reviewed FFI dependency, and `unsafe_code` forbidden workspace-wide by
//! ADR-0015. **That premise was superseded and the text was not updated.**
//! `cancellai-sealedfs` (E07-S07/E07-S09, ADR-0017) now exists, depends on `libc`, is the one
//! crate exempted from `unsafe_code = "forbid"`, and already implements `openat`/`renameat`/
//! `mkdirat` with `O_NOFOLLOW` plus a component-by-component handle-relative walk. Adding
//! `unlinkat` to it is a small extension well inside that crate's stated mandate, not a new
//! dependency decision.
//!
//! The consequence, recorded plainly because the 2026-09-03 target-engine review
//! (`docs/audits/2026-09-03-CODE_REVIEW.md`, `CR-TE-05`) found it: the risk ordering is
//! currently inverted. Writing one JSON key into Claude Code's own settings file is protected
//! by a retained no-follow handle; irreversibly deleting a user's file is not. The deletion
//! path is narrow rather than open - the three checks above refuse rather than deleting the
//! wrong object - but it is detection where prevention is now available.
//!
//! `E21-S07` carries the repair. Until it lands, this is a disclosed residual, not an
//! unavailable capability.

use std::path::Path;

use crate::identity::IdentityToken;
#[cfg(any(test, unix))]
use crate::wsl::RuntimeEnvironment;
#[cfg(unix)]
use crate::wsl::{EnvironmentObserver, SystemEnvironmentObserver};

/// One class of real mutation this seam can perform.
///
/// Exactly one variant, on purpose (E21-S07). This enum previously also carried `Quarantine`
/// (a bare `fs::rename`) and `DeleteDirectoryTree` (a bare `fs::remove_dir_all`), neither
/// identity-confirmed and neither requested by any production caller. They were added so the
/// contract "would not have to grow again" when E12 lands - but an unconfirmed, unreachable
/// deletion primitive sitting in the one file the workspace permits to delete is an armed
/// surface with no test protecting it, and `cancellai-safety::mutation_executor` refuses both
/// action classes upstream anyway. The 2026-09-03 review recorded them as a rising risk
/// (`CR-TE-11`); they are removed rather than left to be inherited by whoever implements E12.
/// Re-adding either is that story's job, together with the confirmation technique it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationOperation {
    /// Permanently remove a regular file. Confirmed by open-file-descriptor identity *and*
    /// performed relative to a held, no-follow directory descriptor (see module docs).
    DeleteFile,
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
        operation: MutationOperation,
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
        operation: MutationOperation,
    ) -> Result<(), MutationError> {
        match operation {
            MutationOperation::DeleteFile => confirmed_delete_file(target, expected),
        }
    }
}

/// Refuses a WSL2 guest, independent of the platform-specific check below. A WSL2 guest runs a
/// real Linux kernel, so the Unix confirmed-delete path in `confirmed_delete_file_inner` would
/// very likely work correctly there - but "very likely" is an inference, not this codebase's
/// own evidence (E20-S02/E20-S03 round-1 independent verifier review: neither this crate's WSL
/// detection nor its `/mnt` filesystem-context classification was ever consulted on the
/// deletion path, so a WSL2 guest silently took the identical, unverified-there Unix delete
/// route as native Linux - contradicting `docs/PLATFORMS.md`'s own claim that a non-tier-1
/// environment remains inspect-only). Pure and environment-independent so it is directly
/// unit-testable with a fabricated [`RuntimeEnvironment`] on any host, matching this crate's
/// `wsl` module's own split between real OS observation and testable pure logic.
#[cfg(any(test, unix))]
fn refuse_unverified_wsl2_mutation(env: RuntimeEnvironment) -> Result<(), MutationError> {
    if env == RuntimeEnvironment::Wsl2 {
        return Err(MutationError(
            "confirmed deletion is refused on a WSL2 guest: this codebase's own Unix mutation \
             path has not been independently verified there yet (E20-S02/E20-S03 residual, \
             SI-017/SI-018) - authority is reduced rather than inferred from generic Linux"
                .to_string(),
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn confirmed_delete_file(target: &Path, expected: &IdentityToken) -> Result<(), MutationError> {
    refuse_unverified_wsl2_mutation(SystemEnvironmentObserver.detect())?;
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

    // This Unix-only deletion path has no verified interpretation of a `Windows` identity
    // token (Windows has its own `confirmed_delete_file_inner` below, E20-S05). A caller
    // passing one anyway - only possible via a synthetically constructed `IdentityToken` in a
    // test, never a real `SystemIdentityObserver` result on a Unix host - gets a typed
    // refusal, not a panic, matching this codebase's fail-closed posture for an unexpected
    // identity shape (SI-017).
    let IdentityToken::Unix {
        device: expected_device,
        inode: expected_inode,
        modified: expected_modified,
        modified_nanos: expected_modified_nanos,
        ..
    } = expected
    else {
        return Err(MutationError(
            "confirmed file deletion on this platform requires a Unix identity token".to_string(),
        ));
    };

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

    // E21-S07: the unlink itself is now issued relative to a directory descriptor opened once,
    // with `O_NOFOLLOW` at every component, rather than through the target's path. That is the
    // difference between detecting the swap and preventing it: `std::fs::remove_file(target)`
    // re-resolved the whole path through the kernel's normal, link-following name resolution
    // every time it ran, so a rename or symlink-swap of any intermediate directory in the
    // window above redirected the removal. `cancellai-sealedfs` (ADR-0017) cannot be redirected
    // that way, because there is no path left to redirect.
    //
    // The identity comparison inside `unlink_child_matching_unix_identity` is deliberately not
    // a replacement for the two checks above - it is a third, handle-relative one, and the
    // held file descriptor's post-unlink link-count check below still runs. Defence in depth
    // over one clever check, as everywhere else in this kernel.
    let parent = target.parent().ok_or_else(|| {
        MutationError("target has no parent directory; refusing to delete".to_string())
    })?;
    let file_name = target.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
        MutationError(
            "target's file name is not representable as UTF-8; refusing to delete a name \
                 this seam cannot bind handle-relatively"
                .to_string(),
        )
    })?;
    let sealed = cancellai_sealedfs::SealedRoot::bind_existing(parent).map_err(|e| {
        MutationError(format!(
            "could not bind the target's parent directory without following a link: {e}"
        ))
    })?;
    sealed
        .unlink_child_matching_unix_identity(file_name, *expected_device, *expected_inode)
        .map_err(|e| MutationError(format!("delete failed: {e}")))?;

    // Final corroboration via the fd opened at the very start: an open fd stays valid after
    // its directory entry is unlinked (Unix semantics), so if the unlink above really removed
    // the object this fd holds, that object's link count is now 0.
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

#[cfg(windows)]
fn confirmed_delete_file(target: &Path, expected: &IdentityToken) -> Result<(), MutationError> {
    confirmed_delete_file_inner(target, expected, || {})
}

/// The Windows counterpart of the Unix `confirmed_delete_file_inner` above (E20-S05,
/// ADR-0020's own residual: "the deletion path is narrow rather than open" - this closes that
/// gap for Windows the same way E21-S07 closed it for Unix). Same three-check shape: an
/// open-time identity confirmation against a retained handle, a fresh path-based re-check
/// immediately before the actual delete call, and a post-delete corroboration against the
/// *same* retained handle rather than a fresh reopen. `cancellai_sealedfs::SealedRoot`'s
/// `NtCreateFile`-based, handle-relative child open (see that crate's `windows_sealed` module)
/// plays the same role `openat`/`O_NOFOLLOW` plays on Unix: the actual delete call cannot be
/// redirected by a rename/reparse-point swap of any path component, because no path is
/// resolved for it at all.
#[cfg(windows)]
fn confirmed_delete_file_inner(
    target: &Path,
    expected: &IdentityToken,
    between_open_and_unlink: impl FnOnce(),
) -> Result<(), MutationError> {
    // This Windows-only deletion path has no verified interpretation of a `Unix` identity
    // token - see the identical reasoning in the Unix implementation above, mirrored here for
    // the same fail-closed reason (SI-017).
    let IdentityToken::Windows {
        volume_serial_number: expected_volume,
        file_index: expected_file_index,
        modified_ticks: expected_modified_ticks,
        ..
    } = expected
    else {
        return Err(MutationError(
            "confirmed file deletion on this platform requires a Windows identity token"
                .to_string(),
        ));
    };

    let (file, before) = cancellai_sealedfs::open_and_observe_identity(target)
        .map_err(|e| MutationError(format!("could not open target for confirmed deletion: {e}")))?;
    if before.volume_serial_number != *expected_volume
        || before.file_index != *expected_file_index
        || before.last_write_time_ticks != *expected_modified_ticks
    {
        return Err(MutationError(
            "target identity changed between revalidation and deletion (open-time check)"
                .to_string(),
        ));
    }

    between_open_and_unlink();

    // A second, independent, fresh path lookup immediately before the actual delete call -
    // the same reasoning as the Unix path's `just_before` re-check: this is what catches a
    // swap that happened after the open-time check above, before `remove_file`/its Windows
    // equivalent has any chance to act on the wrong object.
    let just_before = cancellai_sealedfs::observe_identity(target).map_err(|e| {
        MutationError(format!(
            "could not re-observe target immediately before deletion: {e}"
        ))
    })?;
    if just_before.volume_serial_number != *expected_volume
        || just_before.file_index != *expected_file_index
        || just_before.last_write_time_ticks != *expected_modified_ticks
    {
        return Err(MutationError(
            "target identity changed immediately before deletion (path re-check failed); \
             refusing to delete a different object"
                .to_string(),
        ));
    }

    let parent = target.parent().ok_or_else(|| {
        MutationError("target has no parent directory; refusing to delete".to_string())
    })?;
    let file_name = target.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
        MutationError(
            "target's file name is not representable as UTF-8; refusing to delete a name \
                 this seam cannot bind handle-relatively"
                .to_string(),
        )
    })?;
    let sealed = cancellai_sealedfs::SealedRoot::bind_existing(parent).map_err(|e| {
        MutationError(format!(
            "could not bind the target's parent directory without following a link: {e}"
        ))
    })?;
    sealed
        .unlink_child_matching_windows_identity(file_name, *expected_volume, *expected_file_index)
        .map_err(|e| MutationError(format!("delete failed: {e}")))?;

    // Final corroboration via the handle opened at the very start - queried *before* it is
    // dropped, since a closed handle cannot itself be queried. `unlink_child_matching_windows_
    // identity` only marks the object for deletion (it does not itself hold the last handle);
    // actual removal happens once every handle to it closes, which this function's own `file`
    // still is at this point.
    let pending = cancellai_sealedfs::SealedRoot::is_delete_pending(&file).map_err(|e| {
        MutationError(format!(
            "could not confirm deletion via the held handle: {e}"
        ))
    })?;
    drop(file);
    if !pending {
        return Err(MutationError(
            "deletion did not mark the confirmed handle for removal (post-deletion check \
             failed); the intended target may still exist"
                .to_string(),
        ));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
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
        _operation: MutationOperation,
    ) -> Result<(), MutationError> {
        self.outcomes.get(target).cloned().unwrap_or(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuse_unverified_wsl2_mutation_refuses_on_wsl2() {
        let err = refuse_unverified_wsl2_mutation(RuntimeEnvironment::Wsl2)
            .expect_err("a WSL2 environment must refuse confirmed deletion");
        assert!(err.0.contains("WSL2"));
    }

    #[test]
    fn refuse_unverified_wsl2_mutation_allows_native() {
        refuse_unverified_wsl2_mutation(RuntimeEnvironment::Native)
            .expect("a native (non-WSL2) environment must not be refused by this gate");
    }

    #[cfg(unix)]
    struct TempDir(std::path::PathBuf);

    #[cfg(unix)]
    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            // Canonicalized, because E21-S07's handle-relative unlink refuses to traverse a
            // symlinked path component - and on macOS `std::env::temp_dir()` returns
            // `/var/folders/...`, where `/var` is itself a link to `/private/var`. That refusal
            // is the intended behaviour, not a test problem: E07-S09 already decided a provider
            // root reached through an intermediate link may not carry destructive authority, and
            // `cancellai-cli` proves the real root link-free before establishing it. The test
            // tree simply has to meet the same bar the production path already meets.
            // `a_symlinked_intermediate_component_refuses_the_delete` below pins the refusal
            // itself, so canonicalizing here does not hide it.
            let base = std::fs::canonicalize(std::env::temp_dir())
                .unwrap_or_else(|_| std::env::temp_dir());
            let dir = base.join(format!(
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

    #[cfg(unix)]
    #[test]
    fn a_symlinked_intermediate_component_refuses_the_delete() {
        // E21-S07's containment property, stated as a test rather than as a comment: the unlink
        // is issued relative to a directory descriptor walked component-by-component with
        // `O_NOFOLLOW`, so a path that reaches the target through a link is refused outright
        // rather than followed. Same rule E07-S09 established for provider-root establishment,
        // now holding at the moment of mutation too.
        let dir = TempDir::new("symlinked-intermediate");
        let real = dir.0.join("real");
        std::fs::create_dir_all(&real).expect("create real dir");
        let file = real.join("target.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = identity_of(&file);

        let link = dir.0.join("via-link");
        std::os::unix::fs::symlink(&real, &link).expect("create symlink");
        let through_link = link.join("target.txt");

        let err = SystemMutationExecutor
            .mutate(&through_link, &expected, MutationOperation::DeleteFile)
            .expect_err("a target reached through a symlinked component must be refused");
        assert!(
            err.0.contains("without following a link"),
            "reason was: {}",
            err.0
        );
        assert!(file.exists(), "the target must survive a refused deletion");
    }

    #[cfg(unix)]
    #[test]
    fn the_unlink_refuses_a_name_that_no_longer_holds_the_confirmed_inode() {
        // The handle-relative identity check, exercised directly: the directory descriptor is
        // sound, but the *entry* now points somewhere else. `SealError::IdentityMismatch` is the
        // refusal, and the replacement must survive.
        let dir = TempDir::new("entry-swapped");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = identity_of(&file);

        let result = confirmed_delete_file_inner(&file, &expected, || {
            std::fs::remove_file(&file).expect("simulate concurrent removal");
            std::thread::sleep(std::time::Duration::from_millis(10));
            std::fs::write(&file, b"replacement").expect("simulate concurrent replacement");
        });

        assert!(result.is_err(), "a swapped entry must never be deleted");
        assert_eq!(
            std::fs::read_to_string(&file).expect("replacement must be intact"),
            "replacement"
        );
    }

    #[cfg(windows)]
    struct WindowsTempDir(std::path::PathBuf);

    #[cfg(windows)]
    impl WindowsTempDir {
        fn new(label: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "cancellai-mutation-windows-test-{label}-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn path(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }

    #[cfg(windows)]
    impl Drop for WindowsTempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[cfg(windows)]
    fn windows_identity_of(path: &Path) -> IdentityToken {
        let facts =
            cancellai_sealedfs::observe_identity(path).expect("observe path for test identity");
        IdentityToken::Windows {
            volume_serial_number: facts.volume_serial_number,
            file_index: facts.file_index,
            kind: crate::identity::FileKind::File,
            modified: crate::clock::Timestamp(0),
            modified_ticks: facts.last_write_time_ticks,
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_system_executor_deletes_a_real_file_confirmed_by_identity() {
        let dir = WindowsTempDir::new("delete-confirmed");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = windows_identity_of(&file);

        let executor = SystemMutationExecutor;
        executor
            .mutate(&file, &expected, MutationOperation::DeleteFile)
            .expect("delete should succeed");
        assert!(!file.exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_system_executor_reports_the_os_error_for_a_missing_target() {
        let dir = WindowsTempDir::new("missing-target");
        let missing = dir.path("does-not-exist");
        let expected = IdentityToken::Windows {
            volume_serial_number: 0,
            file_index: 0,
            kind: crate::identity::FileKind::File,
            modified: crate::clock::Timestamp(0),
            modified_ticks: 0,
        };
        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(&missing, &expected, MutationOperation::DeleteFile)
            .expect_err("deleting a missing file must fail, not silently succeed");
        assert!(!err.0.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn windows_confirmed_delete_rejects_a_target_already_swapped_before_open() {
        let dir = WindowsTempDir::new("swapped-before-open");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = windows_identity_of(&file); // captured identity of the ORIGINAL

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

    #[cfg(windows)]
    #[test]
    fn windows_confirmed_delete_detects_a_target_swapped_between_open_and_unlink() {
        let dir = WindowsTempDir::new("swapped-mid-flight");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = windows_identity_of(&file);

        let result = confirmed_delete_file_inner(&file, &expected, || {
            std::fs::remove_file(&file).expect("simulate concurrent removal of the original");
            std::thread::sleep(std::time::Duration::from_millis(10));
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
        assert_eq!(
            std::fs::read_to_string(&file).expect("replacement content must be intact"),
            "replacement"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_the_unlink_refuses_a_name_that_no_longer_holds_the_confirmed_identity() {
        let dir = WindowsTempDir::new("entry-swapped");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = windows_identity_of(&file);

        let result = confirmed_delete_file_inner(&file, &expected, || {
            std::fs::remove_file(&file).expect("simulate concurrent removal");
            std::thread::sleep(std::time::Duration::from_millis(10));
            std::fs::write(&file, b"replacement").expect("simulate concurrent replacement");
        });

        assert!(result.is_err(), "a swapped entry must never be deleted");
        assert_eq!(
            std::fs::read_to_string(&file).expect("replacement must be intact"),
            "replacement"
        );
    }
}

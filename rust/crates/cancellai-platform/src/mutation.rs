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
//! already been checked - this seam itself performs no safety check of its own, exactly like
//! `FsObserver`/`IdentityObserver` perform no safety check of their own; it is a raw OS
//! capability, not the safety boundary itself.

use std::path::Path;

/// One class of real mutation this seam can perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationOperation<'a> {
    /// Move to a quarantine location on the same filesystem (reversible). Not yet driven by
    /// any production caller - `SealedPlan` (E03-S02) does not carry a quarantine
    /// destination yet (E03-S05's own residual risk); the operation exists so this seam's
    /// contract does not have to grow again the day that field lands.
    Quarantine { to: &'a Path },
    /// Permanently remove a file.
    DeleteFile,
    /// Permanently remove a directory tree.
    DeleteDirectoryTree,
}

/// Why a real mutation attempt failed. Always the underlying OS error text - this seam does
/// not interpret or classify failures, only reports what the OS said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationError(pub String);

/// A sink for real filesystem mutation.
pub trait MutationExecutor: Send + Sync {
    fn mutate(&self, target: &Path, operation: MutationOperation<'_>) -> Result<(), MutationError>;
}

/// The real, OS-backed executor. The only place in this crate - and, per
/// `scripts/check_mutation_boundary.py`, in this entire workspace outside this one file -
/// that calls a filesystem removal primitive directly.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemMutationExecutor;

impl MutationExecutor for SystemMutationExecutor {
    fn mutate(&self, target: &Path, operation: MutationOperation<'_>) -> Result<(), MutationError> {
        let result = match operation {
            MutationOperation::Quarantine { to } => std::fs::rename(target, to),
            MutationOperation::DeleteFile => std::fs::remove_file(target),
            MutationOperation::DeleteDirectoryTree => std::fs::remove_dir_all(target),
        };
        result.map_err(|e| MutationError(e.to_string()))
    }
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
        _operation: MutationOperation<'_>,
    ) -> Result<(), MutationError> {
        self.outcomes.get(target).cloned().unwrap_or(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_executor_deletes_a_real_file() {
        let dir =
            std::env::temp_dir().join(format!("cancellai-mutation-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let file = dir.join("target.txt");
        std::fs::write(&file, b"hello").expect("create file");

        let executor = SystemMutationExecutor;
        executor
            .mutate(&file, MutationOperation::DeleteFile)
            .expect("delete should succeed");
        assert!(!file.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn system_executor_reports_the_os_error_for_a_missing_target() {
        let missing = std::env::temp_dir().join("cancellai-mutation-test-missing-target");
        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(&missing, MutationOperation::DeleteFile)
            .expect_err("deleting a missing file must fail, not silently succeed");
        assert!(!err.0.is_empty());
    }

    #[test]
    fn synthetic_executor_succeeds_by_default_for_unconfigured_paths() {
        let executor = SyntheticMutationExecutor::new();
        assert_eq!(
            executor.mutate(
                Path::new("/never/configured"),
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
        assert_eq!(
            executor.mutate(
                Path::new("/synthetic/disk-full"),
                MutationOperation::DeleteFile
            ),
            Err(MutationError("No space left on device".into()))
        );
        assert_eq!(
            executor.mutate(
                Path::new("/synthetic/unrelated"),
                MutationOperation::DeleteFile
            ),
            Ok(())
        );
    }
}

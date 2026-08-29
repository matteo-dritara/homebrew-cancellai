//! The safety executor: the sole orchestration path from a [`SealedPlan`] to a real
//! mutation (E03-S05, SI-019, SI-020, C-07 "one safety kernel").
//!
//! [`execute`] is the whole contract: revalidate the plan's identity precondition
//! immediately before mutation (SI-013, reusing E03-S02's `revalidate` - this story does not
//! reimplement it), refuse a non-mutating action class, then delegate the real OS call to
//! `cancellai-platform`'s [`MutationExecutor`] (E03-S05's platform-layer addition) - this
//! module itself never calls `std::fs::remove_file`/`remove_dir_all` (verified by
//! `scripts/check_mutation_boundary.py`, which allows exactly one file in the whole
//! workspace to do that, and it is not this one). [`execute_all`] runs `execute` over a
//! batch without ever short-circuiting or dropping a result (AC3): every plan gets exactly
//! one [`ActionResult`], `Vec::map`/`collect` cannot silently skip an element the way a loop
//! with an early `return`/`?` could.

use cancellai_model::ActionClass;
use cancellai_platform::{
    FileKind, IdentityObserver, IdentityToken, MutationExecutor, MutationOperation,
};

use crate::root_capability::BoundedPath;
use crate::sealed_plan::{RevalidationOutcome, SealedPlan, revalidate};

/// The outcome of attempting one [`SealedPlan`]. Every branch is explicit - there is no
/// "probably fine" case, and a caller cannot mistake a safety block for a success (SI-014's
/// same principle, applied to a single action rather than a whole run).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ActionResult {
    /// The mutation was actually performed.
    Succeeded,
    /// Safety refused to perform the mutation - a stale plan, or a non-mutating action
    /// class. Never equivalent to `Succeeded` for a caller aggregating results.
    SafelyBlocked { reason: String },
    /// The mutation was attempted, safety allowed it, and the OS call itself failed.
    Failed { reason: String },
}

/// Execute exactly one sealed plan against exactly one already-boundary-checked target.
///
/// `target` is a [`BoundedPath`] (E03-S03), not a raw path - SI-002/SI-003/SI-018 are
/// already established by the time a caller can construct one at all. This function's own
/// job is the two things boundary-checking cannot do at bind time: re-verify identity
/// immediately before mutation (SI-013 - the object could have changed *after* a successful
/// `bind`), and perform the mutation itself through the one allowed capability.
pub fn execute(
    plan: &SealedPlan,
    target: &BoundedPath,
    observer: &dyn IdentityObserver,
    executor: &dyn MutationExecutor,
) -> ActionResult {
    let current = observer.observe(target.path());
    if let RevalidationOutcome::StalePlan { reason } = revalidate(plan, &current) {
        return ActionResult::SafelyBlocked { reason };
    }

    let operation = match plan.action_class() {
        ActionClass::Delete => match delete_operation_for(target.identity()) {
            Some(op) => op,
            None => {
                return ActionResult::SafelyBlocked {
                    reason: "target identity does not describe a file or directory this executor can delete".to_string(),
                };
            }
        },
        // AC's scope (see module/crate docs): Quarantine needs a destination `SealedPlan`
        // does not carry yet, and Archive/Observe have no OS-primitive mapping this story
        // defines. Refusing is the fail-closed answer, not a guess at what either would do.
        ActionClass::Observe | ActionClass::Quarantine | ActionClass::Archive => {
            return ActionResult::SafelyBlocked {
                reason: format!(
                    "{:?} is not an action class this executor performs yet",
                    plan.action_class()
                ),
            };
        }
    };

    match executor.mutate(target.path(), operation) {
        Ok(()) => ActionResult::Succeeded,
        Err(e) => ActionResult::Failed { reason: e.0 },
    }
}

fn delete_operation_for(identity: &IdentityToken) -> Option<MutationOperation<'static>> {
    let IdentityToken::Unix { kind, .. } = identity;
    match kind {
        FileKind::Directory => Some(MutationOperation::DeleteDirectoryTree),
        FileKind::File | FileKind::Symlink | FileKind::Other => Some(MutationOperation::DeleteFile),
    }
}

/// Run [`execute`] over every `(plan, target)` pair, in order, never stopping early and
/// never omitting a result - the input and output slices/vectors are always the same
/// length. A caller that only inspected the *first* failure, or that used an early-return
/// loop instead of this, could silently hide every action after it (AC3's "never hide
/// skipped work").
pub fn execute_all(
    plans: &[(SealedPlan, BoundedPath)],
    observer: &dyn IdentityObserver,
    executor: &dyn MutationExecutor,
) -> Vec<ActionResult> {
    plans
        .iter()
        .map(|(plan, target)| execute(plan, target, observer, executor))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_model::{AuthorityLevel, KnowledgeConfidence, Reversibility};
    use cancellai_platform::{
        Clock, FrozenClock, IdentityObservation, MutationError, SyntheticIdentityObserver,
        SyntheticMutationExecutor,
    };
    use std::path::PathBuf;

    fn fingerprint() -> cancellai_model::RootFingerprint {
        cancellai_model::RootFingerprint {
            root_id: "root-1".into(),
            provider_id: "codex".into(),
            confidence: KnowledgeConfidence::Verified,
        }
    }

    fn plan_with(identity: IdentityToken, action_class: ActionClass) -> SealedPlan {
        SealedPlan::new(
            fingerprint(),
            identity,
            action_class,
            AuthorityLevel::Govern,
            Reversibility::Irreversible,
        )
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            // A counter, not just process id: this helper is called multiple times per test
            // and by multiple tests running in parallel threads within the same process, so
            // process id alone collides (one test's Drop cleanup racing another's still-live
            // directory of the same name).
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "cancellai-mutation-executor-test-{label}-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn real_bounded_file() -> (TempDir, BoundedPath, IdentityToken) {
        let dir = TempDir::new("target-root");
        let file = dir.0.join("target.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let resolver = cancellai_platform::SystemPathResolver;
        let observer = cancellai_platform::SystemIdentityObserver;
        let root = crate::root_capability::ApprovedRoot::establish(&dir.0, &resolver, &observer)
            .expect("establish root");
        let bound = root.bind(&file, &resolver, &observer).expect("bind file");
        let identity = bound.identity().clone();
        (dir, bound, identity)
    }

    #[test]
    fn execute_deletes_when_identity_still_matches() {
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with(identity.clone(), ActionClass::Delete);

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let executor = SyntheticMutationExecutor::new();

        let result = execute(&plan, &target, &observer, &executor);
        assert_eq!(result, ActionResult::Succeeded);
    }

    #[test]
    fn execute_blocks_a_stale_plan_instead_of_mutating() {
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with(identity, ActionClass::Delete);

        // Execute-time identity differs from plan-time identity (SI-013 TOCTOU case).
        let mut observer = SyntheticIdentityObserver::new();
        observer.set(
            target.path(),
            IdentityObservation::Identity(IdentityToken::Unix {
                device: 1,
                inode: 999,
                kind: FileKind::File,
                modified: FrozenClock::at(1_000).now(),
            }),
        );
        let executor = SyntheticMutationExecutor::new();

        let result = execute(&plan, &target, &observer, &executor);
        assert!(matches!(result, ActionResult::SafelyBlocked { .. }));
    }

    #[test]
    fn execute_never_calls_mutate_on_a_stale_plan() {
        // Stronger than the above: prove the executor is never even invoked, not merely
        // that its result was ignored - a synthetic executor configured to fail loudly for
        // this path would catch a bug that mutates first and checks staleness after.
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with(identity, ActionClass::Delete);

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Absent);
        let mut executor = SyntheticMutationExecutor::new();
        executor.set(
            target.path(),
            Err(MutationError(
                "this must never be observed - mutate() should not have been called".into(),
            )),
        );

        let result = execute(&plan, &target, &observer, &executor);
        assert_eq!(
            result,
            ActionResult::SafelyBlocked {
                reason: "artifact no longer exists".to_string()
            }
        );
    }

    #[test]
    fn execute_reports_failed_when_the_mutation_itself_fails() {
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with(identity.clone(), ActionClass::Delete);

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let mut executor = SyntheticMutationExecutor::new();
        executor.set(
            target.path(),
            Err(MutationError("No space left on device".into())),
        );

        let result = execute(&plan, &target, &observer, &executor);
        assert_eq!(
            result,
            ActionResult::Failed {
                reason: "No space left on device".to_string()
            }
        );
    }

    #[test]
    fn execute_refuses_a_non_mutating_action_class() {
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with(identity.clone(), ActionClass::Observe);

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let executor = SyntheticMutationExecutor::new();

        let result = execute(&plan, &target, &observer, &executor);
        assert!(matches!(result, ActionResult::SafelyBlocked { .. }));
    }

    #[test]
    fn execute_deletes_a_directory_tree_when_target_kind_is_directory() {
        let dir = TempDir::new("directory-target");
        let child_dir = dir.0.join("target-dir");
        std::fs::create_dir(&child_dir).expect("create directory");
        let resolver = cancellai_platform::SystemPathResolver;
        let observer_real = cancellai_platform::SystemIdentityObserver;
        let root =
            crate::root_capability::ApprovedRoot::establish(&dir.0, &resolver, &observer_real)
                .expect("establish root");
        let target = root
            .bind(&child_dir, &resolver, &observer_real)
            .expect("bind directory");
        let identity = target.identity().clone();
        let plan = plan_with(identity.clone(), ActionClass::Delete);

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let executor = SyntheticMutationExecutor::new();

        let result = execute(&plan, &target, &observer, &executor);
        assert_eq!(result, ActionResult::Succeeded);
    }

    #[test]
    fn execute_all_never_short_circuits_and_never_drops_a_result() {
        // A mix of one success, one blocked (stale), and one failed - proving the batch
        // aggregation is genuinely per-action (AC3), not a first-error-wins loop that would
        // silently produce fewer results than inputs.
        let (dir_a, target_a, identity_a) = real_bounded_file();
        let (dir_b, target_b, identity_b) = real_bounded_file();
        let (dir_c, target_c, identity_c) = real_bounded_file();

        let plan_a = plan_with(identity_a.clone(), ActionClass::Delete);
        let plan_b = plan_with(identity_b, ActionClass::Delete); // will be reported stale
        let plan_c = plan_with(identity_c.clone(), ActionClass::Delete);

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target_a.path(), IdentityObservation::Identity(identity_a));
        observer.set(target_b.path(), IdentityObservation::Absent); // stale
        observer.set(target_c.path(), IdentityObservation::Identity(identity_c));

        let mut executor = SyntheticMutationExecutor::new();
        executor.set(
            target_c.path(),
            Err(MutationError("No space left on device".into())),
        );

        let plans = vec![(plan_a, target_a), (plan_b, target_b), (plan_c, target_c)];
        let results = execute_all(&plans, &observer, &executor);

        assert_eq!(
            results.len(),
            3,
            "every plan must produce exactly one result"
        );
        assert_eq!(results[0], ActionResult::Succeeded);
        assert!(matches!(results[1], ActionResult::SafelyBlocked { .. }));
        assert_eq!(
            results[2],
            ActionResult::Failed {
                reason: "No space left on device".to_string()
            }
        );

        drop((dir_a, dir_b, dir_c));
    }
}

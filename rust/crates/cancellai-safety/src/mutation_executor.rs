//! The safety executor: the sole orchestration path from a [`SealedPlan`] to a real
//! mutation (E03-S05, SI-013, SI-019, SI-020, C-07 "one safety kernel").
//!
//! [`execute`] is the whole contract, in order: verify the target was bound under the same
//! root the plan was sealed against (E03 verifier review round 1 - see below), verify the
//! plan's recorded authority/reversibility actually permit its action class (same review
//! round), revalidate the plan's identity precondition immediately before mutation (SI-013,
//! reusing E03-S02's `revalidate` - this story does not reimplement it), then delegate the
//! real OS call to `cancellai-platform`'s [`MutationExecutor`] (E03-S05's platform-layer
//! addition) - this module itself never calls `std::fs::remove_file`/`remove_dir_all`
//! (verified by `scripts/check_mutation_boundary.py`, which allows exactly one file in the
//! whole workspace to do that, and it is not this one). [`execute_all`] runs `execute` over
//! a batch without ever short-circuiting or dropping a result (AC3): every plan gets exactly
//! one [`ActionResult`], `Vec::map`/`collect` cannot silently skip an element the way a loop
//! with an early `return`/`?` could.
//!
//! E03 verifier review round 1 found three independent defects in this module's original
//! version, all repaired here:
//!
//! - nothing compared a plan's recorded root to the target actually passed to `execute` at
//!   execution time, so a plan sealed against one root's fingerprint executed successfully
//!   against a target bound under a different root entirely - `execute` now refuses unless
//!   `plan.root_identity() == target.root_identity()` (both populated from real
//!   `ApprovedRoot`/`BoundedPath` capabilities, not caller-suppliable strings - see
//!   `sealed_plan.rs`/`root_capability.rs`);
//! - `execute` never consulted the plan's own recorded `authority`/`reversibility` at all, so
//!   a plan carrying `ActionClass::Delete` with `AuthorityLevel::Observe` and
//!   `Reversibility::Quarantinable` executed as a real, irreversible deletion - `execute` now
//!   refuses unless `plan.authority() >= minimum_authority_for(plan.action_class())` and
//!   `reversibility_allowed(plan.action_class(), plan.reversibility())` (both from
//!   `authority.rs`);
//! - path-based revalidate-then-delete had an unclosed race between the identity check and
//!   the actual unlink syscall - `cancellai-platform`'s `MutationExecutor::mutate` (E03-S05,
//!   repaired) now takes the plan's expected identity and confirms it via an open file
//!   descriptor immediately around the unlink itself (see that module's own docs for exactly
//!   what this does and does not close); `execute` only ever requests `DeleteFile` for a
//!   target whose observed kind is a plain file, since that is the only kind this
//!   confirmation technique is implemented for - directories and symlinks are refused rather
//!   than deleted with a weaker guarantee.

use cancellai_model::ActionClass;
use cancellai_platform::mutation::{MutationExecutor, MutationOperation};
use cancellai_platform::{FileKind, IdentityObserver, IdentityToken, ProcessObserver};

use crate::authority::{minimum_authority_for, reversibility_allowed};
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
    /// Safety refused to perform the mutation - a root/authority/reversibility mismatch, a
    /// stale plan, or a non-mutating action class. Never equivalent to `Succeeded` for a
    /// caller aggregating results.
    SafelyBlocked { reason: String },
    /// The mutation was attempted, safety allowed it, and the OS call itself failed.
    Failed { reason: String },
}

/// Execute exactly one sealed plan against exactly one already-boundary-checked target.
///
/// `target` is a [`BoundedPath`] (E03-S03), not a raw path - SI-002/SI-003/SI-018 are
/// already established by the time a caller can construct one at all. This function's own
/// job is everything boundary-checking at bind time and sealing at plan-time cannot do on
/// their own: verify `plan` and `target` actually correspond to the same root (see module
/// docs), verify the plan's authority/reversibility actually permit its action, re-verify
/// identity immediately before mutation (SI-013 - the object could have changed *after* a
/// successful `bind`/`seal`), and perform the mutation itself through the one allowed
/// capability.
pub fn execute(
    plan: &SealedPlan,
    target: &BoundedPath,
    observer: &dyn IdentityObserver,
    executor: &dyn MutationExecutor,
    process: &dyn ProcessObserver,
) -> ActionResult {
    if plan.root_identity() != target.root_identity() {
        return ActionResult::SafelyBlocked {
            reason: "plan's root identity does not match the target's bound root".to_string(),
        };
    }

    let required_authority = minimum_authority_for(plan.action_class());
    if plan.authority() < required_authority {
        return ActionResult::SafelyBlocked {
            reason: format!(
                "authority {:?} is insufficient for {:?} (requires at least {required_authority:?})",
                plan.authority(),
                plan.action_class()
            ),
        };
    }
    if !reversibility_allowed(plan.action_class(), plan.reversibility()) {
        return ActionResult::SafelyBlocked {
            reason: format!(
                "reversibility {:?} is inconsistent with action class {:?}",
                plan.reversibility(),
                plan.action_class()
            ),
        };
    }

    let current = observer.observe(target.path());
    if let RevalidationOutcome::StalePlan { reason } = revalidate(plan, &current) {
        return ActionResult::SafelyBlocked { reason };
    }

    // SI-013/SI-014's TOCTOU principle applied to process liveness, not only filesystem
    // identity (E06 verifier review round 1): `process_not_running` was recorded as an
    // `execution_preconditions` entry in the emitted plan document but never actually
    // re-checked here - a provider process could start between plan-build time and this
    // moment. `ProcessObservation::is_running` already fails closed (an incomplete probe reads
    // as "possibly running"), so this refuses exactly like a real positive.
    if let Some(names) = plan.process_guard() {
        let liveness = process.observe(names);
        if names.iter().any(|name| liveness.is_running(name)) {
            return ActionResult::SafelyBlocked {
                reason: "a provider process guarding this artifact is running (or its liveness \
                         could not be confirmed) immediately before deletion"
                    .to_string(),
            };
        }
    }

    let operation = match plan.action_class() {
        ActionClass::Delete => match delete_operation_for(target.identity()) {
            Some(op) => op,
            None => {
                return ActionResult::SafelyBlocked {
                    reason: "identity-confirmed deletion is only implemented for plain files, not this target's kind".to_string(),
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

    match executor.mutate(target.path(), plan.artifact_identity(), operation) {
        Ok(()) => ActionResult::Succeeded,
        Err(e) => ActionResult::Failed { reason: e.0 },
    }
}

/// Only a plain file gets a real deletion operation - `cancellai-platform::mutation`'s
/// identity-confirmed delete is implemented (and tested) for `FileKind::File` only.
/// Directories and symlinks are refused rather than deleted with a weaker, unconfirmed
/// guarantee (see that module's own docs for why: the open-file-descriptor confirmation
/// technique does not generalize the same way to a symlink, which `File::open` would follow
/// rather than operate on itself, or to a recursive directory tree).
fn delete_operation_for(identity: &IdentityToken) -> Option<MutationOperation<'static>> {
    let IdentityToken::Unix { kind, .. } = identity;
    match kind {
        FileKind::File => Some(MutationOperation::DeleteFile),
        FileKind::Directory | FileKind::Symlink | FileKind::Other => None,
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
    process: &dyn ProcessObserver,
) -> Vec<ActionResult> {
    plans
        .iter()
        .map(|(plan, target)| execute(plan, target, observer, executor, process))
        .collect()
}

/// Executes `plan` against `target` using the real, OS-backed
/// [`cancellai_platform::SystemIdentityObserver`]/[`cancellai_platform::mutation::
/// SystemMutationExecutor`] - the one place outside this file allowed to reach a
/// mutation-capable executor at all, per `scripts/check_mutation_boundary.py` (SI-019).
/// `cancellai-cli` (E06-S01) is this function's reason for existing: a production caller that
/// needs `clean` to perform a real deletion cannot itself name `SystemMutationExecutor` (the
/// boundary check forbids referencing it - or calling `.mutate(` at all - from any file but
/// this one and `cancellai-platform/src/mutation.rs`), so it calls this wrapper instead of
/// [`execute`] directly. Test code continues to use [`execute`]/[`execute_all`] with
/// [`cancellai_platform::mutation::SyntheticMutationExecutor`], never this function - a real
/// filesystem mutation in a unit test would defeat the point of the synthetic seam.
pub fn execute_with_system_capabilities(plan: &SealedPlan, target: &BoundedPath) -> ActionResult {
    execute(
        plan,
        target,
        &cancellai_platform::SystemIdentityObserver,
        &cancellai_platform::mutation::SystemMutationExecutor,
        &cancellai_platform::SystemProcessObserver,
    )
}

// Unix-only: every test in this module ends up calling `real_bounded_file()`/`ApprovedRoot::
// establish` with the real `SystemIdentityObserver`, which cannot succeed on Windows yet
// (E03-S01's disclosed residual) - found via real Windows CI (E20 verification session).
// Gating the whole module, not each function individually, avoids leaving every shared test
// helper (`plan_with`, `TempDir`, ...) as dead code there.
// `scripts/check_mutation_boundary.py`'s `TEST_MODULE_MARKER` looks for the literal string
// `#[cfg(test)]` to find where to stop scanning for direct filesystem mutation - it must stay
// its own attribute, not folded into `cfg(all(test, unix))`, or this module's `TempDir::drop`
// cleanup (a legitimate test-only `remove_dir_all`) gets scanned and flagged as a boundary
// violation. Stacked `#[cfg]` attributes on one item combine with AND, so this is exactly
// equivalent to `cfg(all(test, unix))`.
#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use cancellai_model::{AuthorityLevel, KnowledgeConfidence, Reversibility};
    use cancellai_platform::mutation::{
        MutationError, SyntheticMutationExecutor, SystemMutationExecutor,
    };
    use cancellai_platform::{
        Clock, FrozenClock, IdentityObservation, SyntheticIdentityObserver,
        SyntheticProcessObserver, SystemProcessObserver,
    };
    use std::path::PathBuf;

    fn fingerprint() -> cancellai_model::RootFingerprint {
        cancellai_model::RootFingerprint {
            root_id: "root-1".into(),
            provider_id: "codex".into(),
            confidence: KnowledgeConfidence::Verified,
        }
    }

    /// The low-level, within-crate-only constructor, given a matching root identity - most
    /// tests here are about `execute`'s own logic, not about `SealedPlan::seal`'s derivation
    /// (covered in `sealed_plan.rs`'s own tests), so they build a plan whose `root_identity`
    /// is deliberately set to match whatever real `ApprovedRoot` `real_bounded_file` used,
    /// unless a test is specifically about a root mismatch.
    fn plan_with(
        root_identity: IdentityToken,
        artifact_identity: IdentityToken,
        action_class: ActionClass,
    ) -> SealedPlan {
        plan_with_authority(
            root_identity,
            artifact_identity,
            action_class,
            AuthorityLevel::Govern,
            Reversibility::Irreversible,
        )
    }

    fn plan_with_authority(
        root_identity: IdentityToken,
        artifact_identity: IdentityToken,
        action_class: ActionClass,
        authority: AuthorityLevel,
        reversibility: Reversibility,
    ) -> SealedPlan {
        SealedPlan::new_with_process_guard(
            fingerprint(),
            root_identity,
            artifact_identity,
            action_class,
            authority,
            reversibility,
            None,
        )
    }

    fn plan_with_process_guard(
        root_identity: IdentityToken,
        artifact_identity: IdentityToken,
        process_guard: &'static [&'static str],
    ) -> SealedPlan {
        SealedPlan::new_with_process_guard(
            fingerprint(),
            root_identity,
            artifact_identity,
            ActionClass::Delete,
            AuthorityLevel::Govern,
            Reversibility::Irreversible,
            Some(process_guard),
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

    /// Returns the temp dir (kept alive for cleanup), a real `BoundedPath` for a file inside
    /// it, and that file's own identity - `target.root_identity()` is a real `ApprovedRoot`'s
    /// identity, needed by tests that must supply a *matching* root identity to `plan_with`.
    /// Unix-only: `ApprovedRoot::establish` with the real `SystemIdentityObserver` cannot
    /// succeed on Windows yet (E03-S01's disclosed residual) - every caller of this helper is
    /// `#[cfg(unix)]` for that reason, found via real Windows CI (E20 verification session).
    #[cfg(unix)]
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

    #[cfg(unix)]
    #[test]
    fn execute_deletes_when_identity_still_matches() {
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with(
            target.root_identity().clone(),
            identity.clone(),
            ActionClass::Delete,
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert_eq!(result, ActionResult::Succeeded);
    }

    #[cfg(unix)]
    #[test]
    fn execute_blocks_a_stale_plan_instead_of_mutating() {
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with(
            target.root_identity().clone(),
            identity,
            ActionClass::Delete,
        );

        // Execute-time identity differs from plan-time identity (SI-013 TOCTOU case).
        let mut observer = SyntheticIdentityObserver::new();
        observer.set(
            target.path(),
            IdentityObservation::Identity(IdentityToken::Unix {
                device: 1,
                inode: 999,
                kind: FileKind::File,
                modified: FrozenClock::at(1_000).now(),
                modified_nanos: 0,
            }),
        );
        let executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert!(matches!(result, ActionResult::SafelyBlocked { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn execute_never_calls_mutate_on_a_stale_plan() {
        // Stronger than the above: prove the executor is never even invoked, not merely
        // that its result was ignored - a synthetic executor configured to fail loudly for
        // this path would catch a bug that mutates first and checks staleness after.
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with(
            target.root_identity().clone(),
            identity,
            ActionClass::Delete,
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Absent);
        let mut executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());
        executor.set(
            target.path(),
            Err(MutationError(
                "this must never be observed - mutate() should not have been called".into(),
            )),
        );

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert_eq!(
            result,
            ActionResult::SafelyBlocked {
                reason: "artifact no longer exists".to_string()
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn execute_reports_failed_when_the_mutation_itself_fails() {
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with(
            target.root_identity().clone(),
            identity.clone(),
            ActionClass::Delete,
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let mut executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());
        executor.set(
            target.path(),
            Err(MutationError("No space left on device".into())),
        );

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert_eq!(
            result,
            ActionResult::Failed {
                reason: "No space left on device".to_string()
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn execute_refuses_a_non_mutating_action_class() {
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with(
            target.root_identity().clone(),
            identity.clone(),
            ActionClass::Observe,
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert!(matches!(result, ActionResult::SafelyBlocked { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn execute_refuses_directory_deletion_rather_than_delete_without_the_stronger_guarantee() {
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
        let plan = plan_with(
            target.root_identity().clone(),
            identity.clone(),
            ActionClass::Delete,
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert!(
            matches!(result, ActionResult::SafelyBlocked { .. }),
            "a directory must be refused, not deleted without the file-only identity confirmation"
        );
        assert!(child_dir.exists());
    }

    #[cfg(unix)]
    #[test]
    fn e03_verifier_round1_plan_for_one_root_cannot_execute_against_a_different_root() {
        // The exact reproduction from the round-1 review: a plan sealed with the identity of
        // root A's target must not execute successfully against a target bound under a
        // *different* root B, even when the artifact identity itself matches.
        let (_dir_a, target_under_root_a, identity) = real_bounded_file();
        let (_dir_b, target_under_root_b, _identity_b) = real_bounded_file();

        // A plan claiming root A's identity, but the caller passes a target bound under root B.
        let plan = plan_with(
            target_under_root_a.root_identity().clone(),
            identity,
            ActionClass::Delete,
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(
            target_under_root_b.path(),
            IdentityObservation::Identity(target_under_root_b.identity().clone()),
        );
        let executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());

        let result = execute(&plan, &target_under_root_b, &observer, &executor, &process);
        assert!(
            matches!(result, ActionResult::SafelyBlocked { .. }),
            "a plan for one root must never execute against a target bound under a different root"
        );
    }

    #[cfg(unix)]
    #[test]
    fn e03_verifier_round1_observe_authority_cannot_execute_a_delete() {
        // The exact reproduction from the round-1 review: AuthorityLevel::Observe with
        // ActionClass::Delete and Reversibility::Quarantinable must never succeed.
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with_authority(
            target.root_identity().clone(),
            identity.clone(),
            ActionClass::Delete,
            AuthorityLevel::Observe,
            Reversibility::Quarantinable,
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert!(matches!(result, ActionResult::SafelyBlocked { .. }));
        assert!(
            target.path().exists(),
            "the target must survive an insufficiently-authorized plan"
        );
    }

    #[cfg(unix)]
    #[test]
    fn execute_blocks_delete_claiming_quarantinable_reversibility_even_with_sufficient_authority() {
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with_authority(
            target.root_identity().clone(),
            identity.clone(),
            ActionClass::Delete,
            AuthorityLevel::Autopilot,
            Reversibility::Quarantinable, // inconsistent with Delete
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert!(matches!(result, ActionResult::SafelyBlocked { .. }));
        assert!(target.path().exists());
    }

    #[cfg(unix)]
    #[test]
    fn execute_all_never_short_circuits_and_never_drops_a_result() {
        // A mix of one success, one blocked (stale), and one failed - proving the batch
        // aggregation is genuinely per-action (AC3), not a first-error-wins loop that would
        // silently produce fewer results than inputs.
        let (dir_a, target_a, identity_a) = real_bounded_file();
        let (dir_b, target_b, identity_b) = real_bounded_file();
        let (dir_c, target_c, identity_c) = real_bounded_file();

        let plan_a = plan_with(
            target_a.root_identity().clone(),
            identity_a.clone(),
            ActionClass::Delete,
        );
        let plan_b = plan_with(
            target_b.root_identity().clone(),
            identity_b,
            ActionClass::Delete,
        ); // will be reported stale
        let plan_c = plan_with(
            target_c.root_identity().clone(),
            identity_c.clone(),
            ActionClass::Delete,
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target_a.path(), IdentityObservation::Identity(identity_a));
        observer.set(target_b.path(), IdentityObservation::Absent); // stale
        observer.set(target_c.path(), IdentityObservation::Identity(identity_c));

        let mut executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());
        executor.set(
            target_c.path(),
            Err(MutationError("No space left on device".into())),
        );

        let plans = vec![(plan_a, target_a), (plan_b, target_b), (plan_c, target_c)];
        let results = execute_all(&plans, &observer, &executor, &process);

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

    #[cfg(unix)]
    #[test]
    fn end_to_end_real_delete_through_the_full_stack_including_authority_and_root_checks() {
        // Ties every E03 story together with the real, OS-backed identity observer AND the
        // real, OS-backed mutation executor (not synthetic doubles) - the actual production
        // call path this executor exists to provide.
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with(
            target.root_identity().clone(),
            identity,
            ActionClass::Delete,
        );

        let observer = cancellai_platform::SystemIdentityObserver;
        let executor = SystemMutationExecutor;
        let process = SystemProcessObserver;

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert_eq!(result, ActionResult::Succeeded);
        assert!(!target.path().exists());
    }

    #[cfg(unix)]
    #[test]
    fn execute_blocks_when_the_guarded_process_is_reported_running() {
        // E06 verifier review round 1: `process_not_running` was recorded in the plan document
        // but never actually revalidated immediately before deletion. This proves it now is.
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with_process_guard(
            target.root_identity().clone(),
            identity.clone(),
            &["claude"],
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::complete(vec!["claude".to_string()]);

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert!(matches!(result, ActionResult::SafelyBlocked { .. }));
        assert!(
            target.path().exists(),
            "the target must survive when its guarding process is running"
        );
    }

    #[cfg(unix)]
    #[test]
    fn execute_blocks_when_the_process_probe_is_incomplete_fail_closed() {
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with_process_guard(
            target.root_identity().clone(),
            identity.clone(),
            &["claude"],
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::incomplete();

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert!(
            matches!(result, ActionResult::SafelyBlocked { .. }),
            "an incomplete process probe must never be read as \"not running\""
        );
        assert!(target.path().exists());
    }

    #[cfg(unix)]
    #[test]
    fn execute_never_calls_mutate_when_the_process_guard_blocks() {
        // Stronger than the above: the mutation executor must not even be invoked - a stale
        // identity check passing first must not let a running-process block be bypassed by
        // reaching the OS call anyway.
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with_process_guard(
            target.root_identity().clone(),
            identity.clone(),
            &["claude"],
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let mut executor = SyntheticMutationExecutor::new();
        executor.set(
            target.path(),
            Err(MutationError(
                "this must never be observed - mutate() should not have been called".into(),
            )),
        );
        let process = SyntheticProcessObserver::complete(vec!["claude".to_string()]);

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert!(matches!(result, ActionResult::SafelyBlocked { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn execute_proceeds_when_the_guarded_process_is_confirmed_not_running() {
        let (_dir, target, identity) = real_bounded_file();
        let plan = plan_with_process_guard(
            target.root_identity().clone(),
            identity.clone(),
            &["claude"],
        );

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(target.path(), IdentityObservation::Identity(identity));
        let executor = SyntheticMutationExecutor::new();
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());

        let result = execute(&plan, &target, &observer, &executor, &process);
        assert_eq!(result, ActionResult::Succeeded);
    }
}

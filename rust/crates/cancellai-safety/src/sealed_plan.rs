//! `SealedPlan`: an immutable, identity-bound, capability-bound, policy-explained mutating
//! plan (E03-S02, `docs/architecture/DOMAIN_MODEL.md` "SealedPlan", SI-013, SI-016).
//!
//! A `SealedPlan` records exactly what it was approved against - which root capability
//! (`RootFingerprint`), which object (`IdentityToken`), what class of action, at what
//! authority, with what reversibility. Nothing here mutates anything; this crate has no
//! filesystem access at all yet (E03-S05, Mutation executor isolation, is where a plan turns
//! into a real `Result`). What this story does provide is [`revalidate`]: the fail-closed
//! answer to "does the object I am about to act on still match what I planned against?"
//! (SI-013) - `docs/architecture/AS_IS.md`'s deferred requirement "sealed identity-bound
//! plans and stronger TOCTOU defense," now real.
//!
//! Scope note: `docs/architecture/DOMAIN_MODEL.md`'s full `SealedPlan` also carries an
//! inventory snapshot ID, a batch of `Action`s (this crate models exactly one target per
//! plan for now), evidence references, and knowledge-bundle version references - none of
//! which exist as real subsystems yet (E04 inventory engine, provider knowledge). Building
//! those fields as placeholders nothing produces would not make this story's revalidation
//! logic any more correct; they land with the stories that actually populate them.
//! `artifact_identity` doubles as this plan's one implemented execution precondition (the AC
//! and SI-013 both single out identity specifically); other precondition kinds (activity
//! state, provider capability) are future stories' concern once those facts exist to check.

use cancellai_model::{ActionClass, AuthorityLevel, Reversibility, RootFingerprint};
use cancellai_platform::{IdentityObservation, IdentityToken};

/// An immutable, sealed mutating plan for exactly one target artifact.
///
/// Immutability is enforced by API shape, not by a runtime check: fields are private, the
/// only constructor is [`SealedPlan::new`], and every accessor is `&self` - there is no
/// method here that could mutate a `SealedPlan` once built (SI-016). A caller that wants a
/// "different" plan builds a new one; it cannot edit this one in place.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SealedPlan {
    root: RootFingerprint,
    artifact_identity: IdentityToken,
    action_class: ActionClass,
    authority: AuthorityLevel,
    reversibility: Reversibility,
}

impl SealedPlan {
    pub fn new(
        root: RootFingerprint,
        artifact_identity: IdentityToken,
        action_class: ActionClass,
        authority: AuthorityLevel,
        reversibility: Reversibility,
    ) -> Self {
        Self {
            root,
            artifact_identity,
            action_class,
            authority,
            reversibility,
        }
    }

    pub fn root(&self) -> &RootFingerprint {
        &self.root
    }

    /// The identity this plan was sealed against - also this plan's execution precondition
    /// (see module docs). [`revalidate`] compares a fresh [`IdentityObservation`] to this.
    pub fn artifact_identity(&self) -> &IdentityToken {
        &self.artifact_identity
    }

    pub fn action_class(&self) -> ActionClass {
        self.action_class
    }

    pub fn authority(&self) -> AuthorityLevel {
        self.authority
    }

    pub fn reversibility(&self) -> Reversibility {
        self.reversibility
    }
}

/// The result of checking a [`SealedPlan`]'s preconditions immediately before mutation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RevalidationOutcome {
    /// The freshly observed identity still matches what the plan was sealed against.
    Proceed,
    /// Something about the artifact changed - or could not be re-established - since the
    /// plan was sealed. Execution must refuse (SI-013); this is never a "probably fine."
    StalePlan { reason: String },
}

/// Revalidate a plan's identity precondition against a freshly observed identity fact.
///
/// Fail-closed by construction: this match is exhaustive over every
/// [`IdentityObservation`] variant, and exactly one arm - an exact [`IdentityToken`] match -
/// returns [`RevalidationOutcome::Proceed`]. Every other branch, including ones a future
/// variant would add, has to be handled explicitly and defaults to none of them being
/// written as "proceed anyway" (there is no wildcard `_ => Proceed` arm to silently cover a
/// case this function's author didn't think through - the compiler refuses to build this
/// function at all until every `IdentityObservation` variant is named here).
pub fn revalidate(plan: &SealedPlan, current: &IdentityObservation) -> RevalidationOutcome {
    match current {
        IdentityObservation::Identity(token) if *token == plan.artifact_identity => {
            RevalidationOutcome::Proceed
        }
        IdentityObservation::Identity(token) => RevalidationOutcome::StalePlan {
            reason: format!(
                "artifact identity changed since the plan was sealed: planned {:?}, observed {token:?}",
                plan.artifact_identity
            ),
        },
        IdentityObservation::Absent => RevalidationOutcome::StalePlan {
            reason: "artifact no longer exists".to_string(),
        },
        IdentityObservation::Unreadable { reason } => RevalidationOutcome::StalePlan {
            reason: format!(
                "artifact could not be re-examined immediately before mutation: {reason}"
            ),
        },
        IdentityObservation::Unsupported { reason } => RevalidationOutcome::StalePlan {
            reason: format!(
                "platform cannot re-verify artifact identity immediately before mutation: {reason}"
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_model::KnowledgeConfidence;
    use cancellai_platform::{
        Clock, FileKind, FrozenClock, IdentityObserver, SyntheticIdentityObserver,
    };

    fn fingerprint() -> RootFingerprint {
        RootFingerprint {
            root_id: "root-1".into(),
            provider_id: "codex".into(),
            confidence: KnowledgeConfidence::Verified,
        }
    }

    fn token(inode: u64) -> IdentityToken {
        IdentityToken::Unix {
            device: 1,
            inode,
            kind: FileKind::File,
            modified: FrozenClock::at(1_000).now(),
        }
    }

    fn plan_with(identity: IdentityToken) -> SealedPlan {
        SealedPlan::new(
            fingerprint(),
            identity,
            ActionClass::Delete,
            AuthorityLevel::Govern,
            Reversibility::Irreversible,
        )
    }

    #[test]
    fn proceeds_when_identity_is_unchanged() {
        // Not vacuously fail-closed: prove the mechanism actually allows the matching case
        // through, or every other test in this module would be meaningless.
        let plan = plan_with(token(1));
        let outcome = revalidate(&plan, &IdentityObservation::Identity(token(1)));
        assert_eq!(outcome, RevalidationOutcome::Proceed);
    }

    #[test]
    fn blocks_when_identity_token_differs() {
        let plan = plan_with(token(1));
        let outcome = revalidate(&plan, &IdentityObservation::Identity(token(2)));
        assert!(matches!(outcome, RevalidationOutcome::StalePlan { .. }));
    }

    #[test]
    fn blocks_when_artifact_became_absent() {
        let plan = plan_with(token(1));
        let outcome = revalidate(&plan, &IdentityObservation::Absent);
        assert!(matches!(outcome, RevalidationOutcome::StalePlan { .. }));
    }

    #[test]
    fn blocks_when_artifact_became_unreadable() {
        let plan = plan_with(token(1));
        let outcome = revalidate(
            &plan,
            &IdentityObservation::Unreadable {
                reason: "permission denied".into(),
            },
        );
        assert!(matches!(outcome, RevalidationOutcome::StalePlan { .. }));
    }

    #[test]
    fn blocks_when_platform_identity_is_unsupported() {
        let plan = plan_with(token(1));
        let outcome = revalidate(
            &plan,
            &IdentityObservation::Unsupported {
                reason: "no verified Windows identity yet".into(),
            },
        );
        assert!(matches!(outcome, RevalidationOutcome::StalePlan { .. }));
    }

    #[test]
    fn end_to_end_toctou_through_a_real_synthetic_observer_fails_closed() {
        // Ties E03-S01's observer directly to E03-S02's revalidation: plan against one
        // token, then have the *observer itself* report a different fact at "execute" time
        // (standing in for SystemIdentityObserver re-observing a real filesystem swap,
        // which identity.rs's own tests already cover against the real filesystem).
        let path = std::path::PathBuf::from("/synthetic/target");
        let mut observer = SyntheticIdentityObserver::new();
        observer.set(&path, IdentityObservation::Identity(token(1)));
        let plan = plan_with(token(1));

        observer.set(&path, IdentityObservation::Identity(token(2)));
        let revalidated = observer.observe(&path);
        assert_eq!(
            revalidate(&plan, &revalidated),
            RevalidationOutcome::StalePlan {
                reason: format!(
                    "artifact identity changed since the plan was sealed: planned {:?}, observed {:?}",
                    token(1),
                    token(2)
                )
            }
        );
    }

    #[test]
    fn sealed_plan_exposes_every_field_the_acceptance_criteria_names() {
        let plan = plan_with(token(1));
        assert_eq!(plan.root(), &fingerprint());
        assert_eq!(plan.artifact_identity(), &token(1));
        assert_eq!(plan.action_class(), ActionClass::Delete);
        assert_eq!(plan.authority(), AuthorityLevel::Govern);
        assert_eq!(plan.reversibility(), Reversibility::Irreversible);
    }
}

//! Artifact explain view (E09-S03) - the third occupant of `docs/architecture/TARGET.md`'s
//! "Engine / Query API" layer, after [`crate::views`] (E08-S04) and [`crate::atlas`] (E09-S02).
//! [`explain`] turns one classified artifact plus the plan's own [`Action`] list into a
//! human-readable account of why it exists, how it is classified, what evidence backs it, its
//! risk and reversibility, the authority it can actually reach, and the concrete policy outcome.
//!
//! AC1 ("Every destructive recommendation has a human-readable explanation path") is not a new
//! explanation mechanism: [`retention::build_actions`] already produces exactly one [`Action`]
//! per artifact, every time, carrying a `reason: String` that is never silently omitted
//! (SI-007, that function's own doc). [`explain`] finds that artifact's action and surfaces its
//! `reason` verbatim - the same sentence a `plan` document would show, not a second, potentially
//! diverging narrative invented for this screen.

use cancellai_model::{
    Action, ActionClass, ArtifactId, ArtifactRelationship, AuthorityLevel, EvidenceId,
    KnowledgeConfidence, Reversibility, RiskClass,
};

use crate::retention::ClassifiedArtifact;

/// AC2 ("Low-confidence data is visibly differentiated"). `Verified` is the only tier this
/// returns `false` for - matching `cancellai_safety::authority::confidence_ceiling`'s own
/// distinction, where `Verified` is the sole tier that reaches the top authority ceiling and
/// every other tier is already a reduced-trust case in the engine's own vocabulary. Applied to
/// both an artifact's own [`ExplainView::knowledge_confidence`] and its
/// [`AttributedProject::confidence`] independently - one can be low while the other is not.
pub fn is_low_confidence(confidence: KnowledgeConfidence) -> bool {
    confidence != KnowledgeConfidence::Verified
}

/// "Why it exists" - the project this artifact is attributed to, and how confidently.
#[derive(Debug, PartialEq, Eq)]
pub struct AttributedProject<'a> {
    pub project_ref: &'a str,
    pub confidence: KnowledgeConfidence,
}

/// The concrete policy outcome for this artifact (outcome's own "policy outcome" field).
#[derive(Debug, PartialEq, Eq)]
pub enum PolicyOutcome<'a> {
    /// A destructive/mutating action was proposed - AC1's case.
    Recommended {
        action_class: ActionClass,
        reason: &'a str,
    },
    /// This artifact was classified `Observe` - no destructive action is proposed, with the
    /// reason why (also never fabricated - the same `Action::reason` field).
    ObservationOnly { reason: &'a str },
    /// No `Action` in the given slice targets this artifact at all - distinct from
    /// `ObservationOnly`: it means the plan `explain` was given simply never covered this
    /// artifact (e.g. a filtered/partial action list), not that policy evaluated it and chose
    /// not to act. Never silently reported as if it were `ObservationOnly`.
    NotEvaluated,
}

/// One artifact's full explanation - the outcome's six named facets (why it exists,
/// classification, evidence, risk, reversibility, allowed authority) plus the policy outcome.
#[derive(Debug, PartialEq, Eq)]
pub struct ExplainView<'a> {
    pub artifact_id: &'a ArtifactId,
    pub provider_id: &'a str,
    /// "Classification".
    pub artifact_type: &'a str,
    /// "Why it exists", part 1: project attribution (`None` is the explicit `Unattributed`
    /// case, never dropped - the same rule `views::by_project` established).
    pub project: Option<AttributedProject<'a>>,
    /// "Why it exists", part 2: structural relationships (e.g. a Codex subagent's `ChildOf`
    /// parent, E08-S01).
    pub relationships: &'a [ArtifactRelationship],
    /// "Risk".
    pub risk_class: RiskClass,
    /// "Reversibility".
    pub reversibility: Reversibility,
    pub knowledge_confidence: KnowledgeConfidence,
    /// "Evidence" - opaque references only (C-09: contentless by default), never the
    /// underlying transcript/prompt/source content itself.
    pub evidence_ids: &'a [EvidenceId],
    /// "Allowed authority".
    pub reachable_authority: AuthorityLevel,
    pub binding_constraints: &'a [&'static str],
    pub policy_outcome: PolicyOutcome<'a>,
}

/// Build one artifact's explanation. `actions` is normally the same plan's `build_actions`
/// output the artifact was classified into - see [`PolicyOutcome::NotEvaluated`] for what
/// happens when it is not (e.g. a filtered slice), which this function reports rather than
/// silently treating as "nothing to act on."
pub fn explain<'a>(classified: &'a ClassifiedArtifact, actions: &'a [Action]) -> ExplainView<'a> {
    let artifact = &classified.artifact;

    let policy_outcome = actions
        .iter()
        .find(|action| action.target_artifact_ids.contains(&artifact.artifact_id))
        .map(|action| match action.action_class {
            ActionClass::Observe => PolicyOutcome::ObservationOnly {
                reason: action.reason.as_str(),
            },
            ActionClass::Quarantine | ActionClass::Archive | ActionClass::Delete => {
                PolicyOutcome::Recommended {
                    action_class: action.action_class,
                    reason: action.reason.as_str(),
                }
            }
        })
        .unwrap_or(PolicyOutcome::NotEvaluated);

    ExplainView {
        artifact_id: &artifact.artifact_id,
        provider_id: artifact.provider_id.as_str(),
        artifact_type: artifact.artifact_type.as_str(),
        project: artifact
            .project_attribution
            .as_ref()
            .map(|attribution| AttributedProject {
                project_ref: attribution.project_ref.0.as_str(),
                confidence: attribution.confidence,
            }),
        relationships: &artifact.relationships,
        risk_class: artifact.risk_class,
        reversibility: artifact.reversibility,
        knowledge_confidence: artifact.knowledge_confidence,
        evidence_ids: &artifact.evidence_ids,
        reachable_authority: classified.reachable_authority,
        binding_constraints: &classified.binding_constraints,
        policy_outcome,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retention::{ProviderResolution, build_actions};
    use cancellai_inventory::completeness::ScopeObservation;
    use cancellai_model::{
        ActivityState, AgentArtifact, AttributionSource, IntegrityState, ProjectAttribution,
        ProjectRef, ProtectionState, RelationshipKind, ResidencyState,
    };
    use std::path::PathBuf;

    #[allow(clippy::too_many_arguments)]
    fn artifact(
        id: &str,
        project: Option<&str>,
        knowledge_confidence: KnowledgeConfidence,
        activity_state: ActivityState,
        reachable_authority: AuthorityLevel,
        binding_constraints: Vec<&'static str>,
    ) -> ClassifiedArtifact {
        ClassifiedArtifact {
            artifact: AgentArtifact {
                artifact_id: ArtifactId::new(id),
                identity_token: format!("codex-cli:{id}"),
                provider_id: "codex-cli".to_string(),
                artifact_type: "session".to_string(),
                risk_class: RiskClass::R3Resumable,
                reversibility: Reversibility::Irreversible,
                knowledge_confidence,
                activity_state,
                residency_state: ResidencyState::Hot,
                protection_state: ProtectionState::Normal,
                integrity_state: IntegrityState::Healthy,
                authority_ceiling: AuthorityLevel::Govern,
                evidence_ids: vec![EvidenceId::new(format!("evidence-{id}"))],
                relationships: Vec::new(),
                project_attribution: project.map(|name| ProjectAttribution {
                    project_ref: ProjectRef::new(name),
                    source: AttributionSource::ExplicitProviderMetadata,
                    confidence: KnowledgeConfidence::Inferred,
                }),
                activity_signal: None,
            },
            path: PathBuf::from(format!("/synthetic/{id}")),
            size_bytes: 1_000,
            reachable_authority,
            binding_constraints,
        }
    }

    fn actions_for(classified: &ClassifiedArtifact) -> Vec<Action> {
        let resolution = ProviderResolution::for_test(
            "codex-cli",
            vec![classified.clone()],
            ScopeObservation::complete(),
        );
        build_actions(&[resolution.planning_view()])
    }

    #[test]
    fn a_stale_eligible_artifact_gets_the_real_delete_reason_from_build_actions() {
        let delete_minimum =
            cancellai_safety::authority::minimum_authority_for(ActionClass::Delete);
        let classified = artifact(
            "stale-eligible",
            Some("proj-x"),
            KnowledgeConfidence::Verified,
            ActivityState::Stale,
            delete_minimum,
            Vec::new(),
        );
        let actions = actions_for(&classified);
        let view = explain(&classified, &actions);

        match view.policy_outcome {
            PolicyOutcome::Recommended {
                action_class,
                reason,
            } => {
                assert_eq!(action_class, ActionClass::Delete);
                assert!(
                    reason.contains("past the retention cutoff"),
                    "AC1: must surface build_actions' real reason verbatim, got: {reason}"
                );
            }
            other => panic!("expected Recommended, got {other:?}"),
        }
    }

    #[test]
    fn a_non_stale_artifact_is_observation_only_with_its_real_reason() {
        let classified = artifact(
            "fresh",
            None,
            KnowledgeConfidence::Verified,
            ActivityState::Idle,
            AuthorityLevel::Govern,
            Vec::new(),
        );
        let actions = actions_for(&classified);
        let view = explain(&classified, &actions);

        match view.policy_outcome {
            PolicyOutcome::ObservationOnly { reason } => {
                assert!(reason.contains("retention window"));
            }
            other => panic!("expected ObservationOnly, got {other:?}"),
        }
    }

    #[test]
    fn a_stale_but_authority_blocked_artifact_explains_which_constraint_blocked_it() {
        let classified = artifact(
            "blocked",
            None,
            KnowledgeConfidence::Verified,
            ActivityState::Stale,
            AuthorityLevel::Observe,
            vec!["provider_trust_authority"],
        );
        let actions = actions_for(&classified);
        let view = explain(&classified, &actions);

        match view.policy_outcome {
            PolicyOutcome::ObservationOnly { reason } => {
                assert!(reason.contains("provider_trust_authority"));
            }
            other => {
                panic!("expected ObservationOnly naming the blocking constraint, got {other:?}")
            }
        }
    }

    #[test]
    fn an_artifact_absent_from_the_given_actions_is_not_evaluated_not_silently_fine() {
        let classified = artifact(
            "orphan-from-a-filtered-view",
            None,
            KnowledgeConfidence::Verified,
            ActivityState::Stale,
            AuthorityLevel::Govern,
            Vec::new(),
        );
        let view = explain(&classified, &[]);
        assert_eq!(view.policy_outcome, PolicyOutcome::NotEvaluated);
    }

    #[test]
    fn unattributed_project_is_explicit_none_not_a_default_name() {
        let classified = artifact(
            "no-project",
            None,
            KnowledgeConfidence::Verified,
            ActivityState::Idle,
            AuthorityLevel::Govern,
            Vec::new(),
        );
        let view = explain(&classified, &[]);
        assert!(view.project.is_none());
    }

    #[test]
    fn an_attributed_project_carries_its_own_confidence_independent_of_the_artifact() {
        let classified = artifact(
            "attributed",
            Some("proj-x"),
            KnowledgeConfidence::Verified,
            ActivityState::Idle,
            AuthorityLevel::Govern,
            Vec::new(),
        );
        let view = explain(&classified, &[]);
        let project = view.project.expect("project must be attributed");
        assert_eq!(project.project_ref, "proj-x");
        // The fixture's attribution confidence (Inferred) differs from the artifact's own
        // (Verified) - proving the two are tracked and reported independently (AC2).
        assert_eq!(project.confidence, KnowledgeConfidence::Inferred);
        assert!(is_low_confidence(project.confidence));
        assert!(!is_low_confidence(view.knowledge_confidence));
    }

    #[test]
    fn every_non_verified_confidence_tier_is_flagged_low_and_verified_is_not() {
        assert!(!is_low_confidence(KnowledgeConfidence::Verified));
        assert!(is_low_confidence(KnowledgeConfidence::Observed));
        assert!(is_low_confidence(KnowledgeConfidence::Inferred));
        assert!(is_low_confidence(KnowledgeConfidence::LowUnknown));
    }

    #[test]
    fn relationships_are_exposed_verbatim_for_the_why_it_exists_narrative() {
        let mut classified = artifact(
            "child",
            None,
            KnowledgeConfidence::Verified,
            ActivityState::Idle,
            AuthorityLevel::Govern,
            Vec::new(),
        );
        classified.artifact.relationships = vec![ArtifactRelationship {
            kind: RelationshipKind::ChildOf,
            related_artifact_id: ArtifactId::new("parent"),
        }];
        let view = explain(&classified, &[]);
        assert_eq!(view.relationships.len(), 1);
        assert_eq!(view.relationships[0].kind, RelationshipKind::ChildOf);
    }
}

//! The one seam this crate touches `cancellai-policy`'s "Engine / Query API" through (E09-S02).
//! `ui::draw` takes an [`EngineData`] rather than importing `cancellai_policy` types in every
//! screen-drawing function - E09-S04's plan review reads it through [`EngineData::plan_context`]
//! rather than widening how many places in this crate know the engine crate's shape.

use cancellai_policy::{PolicyOutcome, Reversibility};

pub use cancellai_policy::{AtlasSummary, ExplainView};

use crate::app::PlanContext;

/// Everything the shell's screens can render, gathered once by whatever assembles real data.
/// Wiring a live scan into `main.rs` is deferred (see that file's own doc); `Default` (every
/// field empty/`None`) is what a not-yet-loaded shell renders, as an explicit state rather than
/// an empty/zeroed summary standing in for "nothing scanned yet".
#[derive(Debug, Default)]
pub struct EngineData<'a> {
    pub atlas: Option<AtlasSummary<'a>>,
    /// One entry per artifact the Explain screen (E09-S03) can select and show. An empty `Vec`
    /// (the `Default`) renders as "nothing to explain yet", the same explicit-not-loaded
    /// posture `atlas`'s `None` already establishes.
    pub explain: Vec<ExplainView<'a>>,
}

impl EngineData<'_> {
    /// The Plan screen's confirmation context (E09-S04) for whichever artifact `selected_index`
    /// resolves to - reduced modulo `explain`'s real length exactly like
    /// `ui::draw_explain_content` already does, so "the same artifact" always means the same
    /// thing on both screens. A pure function of [`ExplainView::policy_outcome`]/
    /// `reversibility` alone (this is the "TUI-to-engine semantic equivalence" the story's
    /// verification plan asks for: no separate classification is invented here).
    pub fn plan_context(&self, selected_index: usize) -> PlanContext {
        let Some(view) = self.explain.get(selected_index % self.explain.len().max(1)) else {
            return PlanContext::default();
        };
        let can_confirm = matches!(view.policy_outcome, PolicyOutcome::Recommended { .. });
        PlanContext {
            can_confirm,
            requires_strong_confirmation: can_confirm
                && view.reversibility == Reversibility::Irreversible,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_model::ArtifactId;

    fn view(
        reversibility: Reversibility,
        policy_outcome: PolicyOutcome<'static>,
    ) -> ExplainView<'static> {
        let artifact_id: &'static ArtifactId = Box::leak(Box::new(ArtifactId::new("a")));
        ExplainView {
            artifact_id,
            provider_id: "claude-code",
            artifact_type: "session",
            project: None,
            relationships: &[],
            risk_class: cancellai_model::RiskClass::R3Resumable,
            reversibility,
            knowledge_confidence: cancellai_policy::KnowledgeConfidence::Verified,
            evidence_ids: &[],
            reachable_authority: cancellai_model::AuthorityLevel::Govern,
            binding_constraints: &[],
            policy_outcome,
        }
    }

    #[test]
    fn an_empty_explain_list_gives_the_default_context() {
        let data = EngineData::default();
        assert_eq!(data.plan_context(0), PlanContext::default());
    }

    #[test]
    fn a_recommended_irreversible_action_requires_strong_confirmation() {
        let data = EngineData {
            explain: vec![view(
                Reversibility::Irreversible,
                PolicyOutcome::Recommended {
                    action_class: cancellai_model::ActionClass::Delete,
                    reason: "past the retention cutoff",
                },
            )],
            ..Default::default()
        };
        let context = data.plan_context(0);
        assert!(context.can_confirm);
        assert!(context.requires_strong_confirmation);
    }

    #[test]
    fn a_recommended_quarantinable_action_does_not_require_strong_confirmation() {
        let data = EngineData {
            explain: vec![view(
                Reversibility::Quarantinable,
                PolicyOutcome::Recommended {
                    action_class: cancellai_model::ActionClass::Delete,
                    reason: "past the retention cutoff",
                },
            )],
            ..Default::default()
        };
        let context = data.plan_context(0);
        assert!(context.can_confirm);
        assert!(
            !context.requires_strong_confirmation,
            "AC2: only Irreversible needs the stronger, two-press path"
        );
    }

    #[test]
    fn an_observation_only_outcome_can_never_be_confirmed_regardless_of_reversibility() {
        let data = EngineData {
            explain: vec![view(
                Reversibility::Irreversible,
                PolicyOutcome::ObservationOnly {
                    reason: "inside the retention window",
                },
            )],
            ..Default::default()
        };
        let context = data.plan_context(0);
        assert!(!context.can_confirm);
        assert!(!context.requires_strong_confirmation);
    }

    #[test]
    fn a_not_evaluated_outcome_can_never_be_confirmed() {
        let data = EngineData {
            explain: vec![view(
                Reversibility::Irreversible,
                PolicyOutcome::NotEvaluated,
            )],
            ..Default::default()
        };
        assert!(!data.plan_context(0).can_confirm);
    }

    #[test]
    fn the_index_wraps_via_modulo_over_the_real_list_length() {
        let data = EngineData {
            explain: vec![
                view(
                    Reversibility::Quarantinable,
                    PolicyOutcome::Recommended {
                        action_class: cancellai_model::ActionClass::Delete,
                        reason: "r1",
                    },
                ),
                view(
                    Reversibility::Irreversible,
                    PolicyOutcome::Recommended {
                        action_class: cancellai_model::ActionClass::Delete,
                        reason: "r2",
                    },
                ),
            ],
            ..Default::default()
        };
        // index 2 % 2 == 0 -> the first (Quarantinable) artifact, not a panic.
        assert!(!data.plan_context(2).requires_strong_confirmation);
        // index 3 % 2 == 1 -> the second (Irreversible) artifact.
        assert!(data.plan_context(3).requires_strong_confirmation);
    }
}

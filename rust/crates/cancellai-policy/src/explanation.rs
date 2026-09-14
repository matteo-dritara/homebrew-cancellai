//! The policy explanation graph (E11-S03, `docs/architecture/POLICY_MODEL.md`'s "Explanation
//! contract": "Every result can be explained in ordered steps... The engine exposes the same
//! explanation graph to CLI, TUI, Guardian, and later fleet UI").
//!
//! This module invents no new authority logic and reads no new fact. [`explain_policy`] wraps
//! [`crate::resolver::resolve_effective_authority`]'s own output -
//! `cancellai_safety::EffectiveAuthority::trace` (already an ordered, deterministic list of
//! every named constraint `effective_authority` evaluated, E03-S04's own AC3) and
//! [`crate::resolver::ResolvedRequest::source`] (which scope supplied the request, E11-S02) -
//! into one [`PolicyExplanation`] a caller can render without re-deriving anything. "Return a
//! trace showing every rule/evidence item that contributed to the final result" (this story's
//! outcome) is exactly `EffectiveAuthority::trace` restated with the one fact it does not
//! itself carry: which scope in the policy document produced the `user_authority` entry.
//!
//! AC "Explanation order is deterministic": [`PolicyExplanation::steps`] is `trace` in the exact
//! order `cancellai_safety::authority::base_constraints` builds it (a fixed `Vec` literal, not a
//! `HashMap` or any other unordered structure) - the same order for every call with the same
//! inputs, proven directly below rather than assumed from the upstream crate's own guarantee.
//!
//! AC "Suppressed higher-authority requests explain exactly which ceiling won":
//! [`PolicyExplanation::suppressed`] is `true` exactly when the resolved request's own level
//! does not equal the final level, and [`PolicyExplanation::binding_constraints`] names every
//! constraint tied at that final minimum (there can be more than one, E03-S04's own AC3) - never
//! just the fact that *something* capped it.

use cancellai_model::AuthorityLevel;
use cancellai_safety::AuthorityInputs;

use crate::resolver::{PolicyContext, PolicyScopeSource, resolve_effective_authority};
use crate::schema::PolicyDocument;

/// One named constraint `effective_authority` evaluated, and whether it is among the one(s)
/// that actually bound the final result. The first step is always `"user_authority"` - the
/// resolved policy request itself - because `cancellai_safety::authority::base_constraints`
/// always includes it first; every later step is one of the independent safety constraints
/// (artifact ceiling, confidence, lifecycle, provider trust, constitutional safety floor).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ExplanationStep {
    pub label: &'static str,
    pub level: AuthorityLevel,
    pub bound: bool,
}

/// The ordered explanation graph for one policy resolution: what was requested (and by which
/// scope), every constraint considered, and the final result. See the module doc for why this
/// invents nothing - it is a reshape of [`crate::resolver::resolve_effective_authority`]'s own
/// return value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PolicyExplanation {
    pub steps: Vec<ExplanationStep>,
    pub requested_source: PolicyScopeSource,
    pub requested_level: AuthorityLevel,
    pub final_level: AuthorityLevel,
    /// Every step tied at the final minimum - the same
    /// `cancellai_safety::EffectiveAuthority::binding_constraints`, carried through verbatim
    /// rather than re-derived from `steps` a second, potentially diverging way.
    pub binding_constraints: Vec<&'static str>,
    /// `true` when the resolved request's own level was not itself the final result - i.e. some
    /// *other* constraint bound it lower. `false` when the request was granted in full (which
    /// still may coincide with other constraints also being tied at that same level - a request
    /// being satisfied does not mean it was the *only* binding constraint, just that it was not
    /// suppressed below its own ask).
    pub suppressed: bool,
}

/// Resolves policy for `context` against `document`, then reshapes the result into an ordered
/// [`PolicyExplanation`]. `inputs.user_requested` is the pre-policy baseline, exactly as
/// [`resolve_effective_authority`] itself documents.
pub fn explain_policy(
    document: &PolicyDocument,
    context: &PolicyContext<'_>,
    inputs: AuthorityInputs,
) -> PolicyExplanation {
    let (effective, resolved) = resolve_effective_authority(document, context, inputs);

    let steps: Vec<ExplanationStep> = effective
        .trace
        .iter()
        .map(|constraint| ExplanationStep {
            label: constraint.name,
            level: constraint.ceiling,
            bound: effective.binding_constraints.contains(&constraint.name),
        })
        .collect();

    PolicyExplanation {
        steps,
        requested_source: resolved.source,
        requested_level: resolved.authority,
        final_level: effective.level,
        binding_constraints: effective.binding_constraints,
        suppressed: resolved.authority != effective.level,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::PolicyContext;
    use crate::schema::ScopePolicy;
    use cancellai_model::{ActivityState, IntegrityState, KnowledgeConfidence, ProtectionState};
    use std::collections::BTreeMap;

    fn scope(authority: AuthorityLevel) -> ScopePolicy {
        ScopePolicy {
            authority: Some(authority),
            retention: None,
            budget: None,
        }
    }

    fn document_with_global(authority: AuthorityLevel) -> PolicyDocument {
        PolicyDocument {
            schema_version: crate::schema::CURRENT_SCHEMA_VERSION,
            global: Some(scope(authority)),
            machine: BTreeMap::new(),
            providers: BTreeMap::new(),
            projects: BTreeMap::new(),
            artifact_types: BTreeMap::new(),
            pins: Vec::new(),
        }
    }

    fn permissive_inputs(user_requested: AuthorityLevel) -> AuthorityInputs {
        AuthorityInputs {
            user_requested,
            artifact_ceiling: AuthorityLevel::Autopilot,
            confidence: KnowledgeConfidence::Verified,
            activity: ActivityState::Idle,
            protection: ProtectionState::Normal,
            integrity: IntegrityState::Healthy,
            provider_trust: crate::trust::builtin_provider_trust(),
        }
    }

    // --- AC "Explanation order is deterministic" ----------------------------------------------

    #[test]
    fn ac_the_same_inputs_produce_byte_for_byte_identical_explanations_across_repeated_calls() {
        let document = document_with_global(AuthorityLevel::Govern);
        let context = PolicyContext::default();
        let inputs = permissive_inputs(AuthorityLevel::Observe);

        let first = explain_policy(&document, &context, inputs);
        let second = explain_policy(&document, &context, inputs);
        assert_eq!(first, second);
    }

    #[test]
    fn ac_step_order_matches_the_fixed_base_constraints_order_every_time() {
        let document = document_with_global(AuthorityLevel::Govern);
        let context = PolicyContext::default();
        let inputs = permissive_inputs(AuthorityLevel::Observe);
        let explanation = explain_policy(&document, &context, inputs);

        let labels: Vec<&str> = explanation.steps.iter().map(|s| s.label).collect();
        assert_eq!(
            labels,
            vec![
                "user_authority",
                "artifact_authority_ceiling",
                "confidence_authority",
                "lifecycle_authority",
                "provider_trust_authority",
                "constitutional_safety_floor",
            ]
        );
    }

    #[test]
    fn the_first_step_is_always_the_resolved_policy_request() {
        let mut document = document_with_global(AuthorityLevel::Quarantine);
        document
            .projects
            .insert("cancellai".to_string(), scope(AuthorityLevel::Autopilot));
        let context = PolicyContext {
            project_ref: Some("cancellai"),
            ..PolicyContext::default()
        };
        let inputs = permissive_inputs(AuthorityLevel::Observe);
        let explanation = explain_policy(&document, &context, inputs);

        assert_eq!(explanation.steps[0].label, "user_authority");
        assert_eq!(explanation.steps[0].level, AuthorityLevel::Autopilot);
        assert_eq!(explanation.requested_source, PolicyScopeSource::Project);
        assert_eq!(explanation.requested_level, AuthorityLevel::Autopilot);
    }

    // --- AC "Suppressed higher-authority requests explain exactly which ceiling won" ----------

    #[test]
    fn a_suppressed_request_names_exactly_the_one_ceiling_that_won() {
        let mut document = document_with_global(AuthorityLevel::Observe);
        document
            .artifact_types
            .insert("session".to_string(), scope(AuthorityLevel::Autopilot));
        let context = PolicyContext {
            artifact_type: Some("session"),
            ..PolicyContext::default()
        };
        let mut inputs = permissive_inputs(AuthorityLevel::Observe);
        inputs.artifact_ceiling = AuthorityLevel::Quarantine;

        let explanation = explain_policy(&document, &context, inputs);
        assert!(
            explanation.suppressed,
            "Autopilot request was not granted in full"
        );
        assert_eq!(explanation.requested_level, AuthorityLevel::Autopilot);
        assert_eq!(explanation.final_level, AuthorityLevel::Quarantine);
        assert_eq!(
            explanation.binding_constraints,
            vec!["artifact_authority_ceiling"]
        );
        // The bound flag on the winning step must agree with the top-level field - no second,
        // potentially diverging way to ask "what won."
        let winning_step = explanation
            .steps
            .iter()
            .find(|s| s.label == "artifact_authority_ceiling")
            .unwrap();
        assert!(winning_step.bound);
        assert!(
            !explanation.steps[0].bound,
            "the request itself lost, so it is not bound"
        );
    }

    #[test]
    fn a_suppressed_request_names_every_tied_ceiling_not_just_the_first() {
        let document = document_with_global(AuthorityLevel::Autopilot);
        let context = PolicyContext::default();
        let mut inputs = permissive_inputs(AuthorityLevel::Observe);
        inputs.protection = ProtectionState::Protected;

        let explanation = explain_policy(&document, &context, inputs);
        assert!(explanation.suppressed);
        assert_eq!(explanation.final_level, AuthorityLevel::Recommend);
        // lifecycle_authority and constitutional_safety_floor both collapse to Recommend when
        // Protected - both must be named (mirrors cancellai-safety::authority's own equivalent
        // test for the underlying mechanism).
        assert_eq!(
            explanation.binding_constraints,
            vec!["lifecycle_authority", "constitutional_safety_floor"]
        );
        for label in ["lifecycle_authority", "constitutional_safety_floor"] {
            assert!(
                explanation
                    .steps
                    .iter()
                    .any(|s| s.label == label && s.bound),
                "{label} must be marked bound"
            );
        }
    }

    #[test]
    fn a_request_granted_in_full_is_not_reported_as_suppressed() {
        let document = document_with_global(AuthorityLevel::Autopilot);
        let context = PolicyContext::default();
        let inputs = permissive_inputs(AuthorityLevel::Observe);

        let explanation = explain_policy(&document, &context, inputs);
        assert_eq!(explanation.final_level, AuthorityLevel::Autopilot);
        assert_eq!(explanation.requested_level, AuthorityLevel::Autopilot);
        assert!(!explanation.suppressed);
    }

    // --- boundary: no scope matched at all -----------------------------------------------------

    #[test]
    fn an_empty_document_still_produces_a_full_deterministic_explanation() {
        let document = PolicyDocument {
            schema_version: crate::schema::CURRENT_SCHEMA_VERSION,
            global: None,
            machine: BTreeMap::new(),
            providers: BTreeMap::new(),
            projects: BTreeMap::new(),
            artifact_types: BTreeMap::new(),
            pins: Vec::new(),
        };
        let context = PolicyContext::default();
        let inputs = permissive_inputs(AuthorityLevel::Recommend);

        let explanation = explain_policy(&document, &context, inputs);
        assert_eq!(explanation.requested_source, PolicyScopeSource::NoMatch);
        assert_eq!(explanation.requested_level, AuthorityLevel::Recommend);
        assert_eq!(explanation.steps.len(), 6);
        assert!(!explanation.suppressed);
    }
}

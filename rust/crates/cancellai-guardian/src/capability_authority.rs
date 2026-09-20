//! Wires [`crate::structural::assess_layout`]'s recommendation into `cancellai-safety`'s one
//! Effective Authority computation (E14-S04, `SI-004` "Unknown provider layout/version reduces
//! capability").
//!
//! Round 1 independent review of E14 found `assess_layout` correctly computed
//! `recommended_authority_ceiling`, but nothing consumed it: it reached no policy, provider
//! capability, or `cancellai_safety::authority::compute_effective_authority` call anywhere in
//! the workspace, so a drifted layout never actually reduced any caller's real authority - an
//! advisory recommendation, not the automatic downgrade AC1 requires.
//!
//! This module is the fix, and it is deliberately thin: `crate::structural` itself still holds
//! no reference to `cancellai-safety` (that module's own doc, "this module holds no reference to
//! cancellai-safety and never will") - the bridge lives here, one level up, in a module built
//! for exactly this purpose. It makes no classification or authority decision of its own and
//! introduces no second path to one: [`effective_authority_after_layout_assessment`] is a thin
//! wrapper over [`cancellai_safety::effective_authority_for_provider_capability`], which is
//! itself the same [`cancellai_safety::compute_effective_authority`] monotonic-minimum
//! computation every other constraint in the system already goes through, extended by one named
//! input. Guardian detection stays advisory in the sense that matters: it observes and reports,
//! `cancellai-safety` still is the one place a mutation's authority is ever decided.

use cancellai_safety::{
    AuthorityInputs, EffectiveAuthority, effective_authority_for_provider_capability,
};

use crate::structural::LayoutDriftFinding;

/// Combines `inputs` with `finding`'s recommended ceiling into one Effective Authority result -
/// the actual, automatic downgrade AC1 requires, not merely an unconsumed recommendation.
/// `finding.recommended_authority_ceiling` is `None` for a recognized layout (no additional
/// cap - the six base constraints alone still apply) and `Some(AuthorityLevel::Observe)` for a
/// drifted one.
pub fn effective_authority_after_layout_assessment(
    inputs: AuthorityInputs,
    finding: &LayoutDriftFinding,
) -> EffectiveAuthority {
    effective_authority_for_provider_capability(inputs, finding.recommended_authority_ceiling)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structural::{LayoutSignature, assess_layout};
    use cancellai_model::{
        ActivityState, AuthorityLevel, IntegrityState, KnowledgeConfidence, ProtectionState,
        ProviderTrust,
    };
    use cancellai_safety::{TrustPromotionEvidence, TrustedTier};

    /// An `AuthorityInputs` that is destructive-capable on every one of the six base
    /// constraints, built from real, legitimately-obtained values - not a permissive fixture
    /// that could hide a bug in `AuthorityInputs` construction itself. `TrustedTier` cannot be
    /// forged at an arbitrary level from outside `cancellai-safety` (that crate's own
    /// `compile_fail` doctest on `TrustedTier` proves it): this promotes a real
    /// `TrustedTier::untrusted()` with real (if synthetic-for-a-test) verifier/fixture evidence,
    /// the same public API a real caller would use.
    fn destructive_capable_inputs() -> AuthorityInputs {
        let evidence = TrustPromotionEvidence {
            verified_by: "e14-s04-end-to-end-test".to_string(),
            fixture_references: vec!["synthetic-fixture".to_string()],
        };
        let provider_trust = TrustedTier::untrusted()
            .promote(ProviderTrust::BuiltinVerified, &evidence)
            .expect("promotion with non-empty verifier/fixture evidence must succeed");
        AuthorityInputs {
            user_requested: AuthorityLevel::Autopilot,
            artifact_ceiling: AuthorityLevel::Autopilot,
            confidence: KnowledgeConfidence::Verified,
            activity: ActivityState::Idle,
            protection: ProtectionState::Normal,
            integrity: IntegrityState::Healthy,
            provider_trust,
        }
    }

    #[test]
    fn e14s04_end_to_end_a_destructive_capable_input_ends_at_observe_under_real_layout_drift() {
        // The exact chain round 1 independent review found broken: assess_layout's own real
        // output, fed into the real effective-authority computation, starting from an input that
        // - absent the layout constraint - would resolve to the highest authority level this
        // system has.
        let known = vec![LayoutSignature::new([
            "sessions/".to_string(),
            "config.json".to_string(),
        ])];
        let observed = LayoutSignature::new(["totally_different_shape/".to_string()]);
        let finding = assess_layout("claude", &known, &observed);
        assert_eq!(
            finding.support,
            crate::structural::LayoutSupport::Drifted,
            "test setup: this observed layout must actually be drift"
        );

        let baseline_without_drift = effective_authority_after_layout_assessment(
            destructive_capable_inputs(),
            &LayoutDriftFinding {
                support: crate::structural::LayoutSupport::Recognized,
                evidence: "recognized (baseline for comparison)".to_string(),
                recommended_authority_ceiling: None,
            },
        );
        assert_eq!(
            baseline_without_drift.level,
            AuthorityLevel::Autopilot,
            "test setup: the destructive-capable input must actually reach the top level absent \
             any layout constraint, or this test would not be proving the drift path matters"
        );

        let under_drift =
            effective_authority_after_layout_assessment(destructive_capable_inputs(), &finding);
        assert_eq!(under_drift.level, AuthorityLevel::Observe);
        assert!(
            under_drift
                .binding_constraints
                .contains(&"provider_capability_authority"),
            "Observe must be attributed to the layout-drift constraint, not merely reached: got \
             {:?}",
            under_drift.binding_constraints
        );
    }

    #[test]
    fn e14s04_a_recognized_provider_name_does_not_bypass_the_drift_verdict() {
        // Mirrors structural.rs's own provider_name_never_changes_the_drift_verdict, one layer
        // up: a well-known provider name reaching this bridge with a genuinely drifted layout
        // must still end at Observe, exactly as an unknown provider name would.
        let known = vec![LayoutSignature::new(["sessions/".to_string()])];
        let observed = LayoutSignature::new(["unrecognized_shape/".to_string()]);

        for provider_id in ["claude", "some-unheard-of-provider-xyz"] {
            let finding = assess_layout(provider_id, &known, &observed);
            let result =
                effective_authority_after_layout_assessment(destructive_capable_inputs(), &finding);
            assert_eq!(
                result.level,
                AuthorityLevel::Observe,
                "provider_id={provider_id} must not bypass the drift-recommended ceiling"
            );
        }
    }
}

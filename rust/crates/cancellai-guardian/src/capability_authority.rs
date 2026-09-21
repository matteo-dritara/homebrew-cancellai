//! Converts this crate's own [`crate::structural::LayoutSignature`] observations into
//! `cancellai-safety`'s [`cancellai_safety::ProviderLayoutAssessment`] (E14-S04, `SI-004`
//! "Unknown provider layout/version reduces capability", ADR-0034, ADR-0035).
//!
//! Three independent review rounds, across two designs, found successively narrower ways a
//! Guardian-computed authority ceiling failed to actually, unavoidably reduce authority:
//!
//! - **Round 1**: `assess_layout`'s recommendation reached no authority computation anywhere.
//! - **Round 2**: the fix (a second, opt-in `effective_authority_for_provider_capability`
//!   function) was ignorable via the pre-existing, still-public plain `effective_authority`.
//! - **Round 3** (against ADR-0034's `Option<AuthorityLevel>` mandatory field): the *ceiling*
//!   was the mandatory input, not the observation it was derived from - a caller could compute
//!   the right ceiling once, then separately construct an otherwise-identical `AuthorityInputs`
//!   with `None`, discarding the finding it still held.
//!
//! ADR-0035 removes the ceiling as an independent value passed between crates at all. This
//! module no longer computes or carries one: [`authority_inputs_with_layout_observation`] takes
//! the same raw `known_signatures`/`observed` facts [`crate::structural::assess_layout`] itself
//! takes, converts them to `cancellai_safety`'s own [`cancellai_safety::LayoutSignature`] (a
//! structurally identical, deliberately separate type - `cancellai-safety` may not depend on
//! `cancellai-guardian`, and `structural.rs` may not depend on `cancellai-safety`), and sets
//! [`cancellai_safety::AuthorityInputs::provider_layout`] to the resulting
//! [`cancellai_safety::ProviderLayoutAssessment::Observed`]. `cancellai_safety::authority::
//! base_constraints` derives the ceiling from those facts *itself* - there is no ceiling value
//! left in this module, or anywhere between the two crates, for a caller to assert, discard, or
//! disagree with once it supplies the observation.

use cancellai_safety::{AuthorityInputs, ProviderLayoutAssessment};

use crate::structural::LayoutSignature;

fn to_safety_signature(signature: &LayoutSignature) -> cancellai_safety::LayoutSignature {
    cancellai_safety::LayoutSignature::new(signature.markers().iter().cloned())
}

/// Sets `inputs.provider_layout` to a real observation of `known_signatures`/`observed` - the
/// same raw facts [`crate::structural::assess_layout`] itself compares, converted to
/// `cancellai-safety`'s own vocabulary. The caller then calls
/// [`cancellai_safety::effective_authority`] itself; this function computes no ceiling and makes
/// no classification decision - `provider_layout` is overwritten, not merged, since this call's
/// observation is the one, authoritative fact for that field going forward.
pub fn authority_inputs_with_layout_observation(
    inputs: AuthorityInputs,
    known_signatures: &[LayoutSignature],
    observed: &LayoutSignature,
) -> AuthorityInputs {
    AuthorityInputs {
        provider_layout: ProviderLayoutAssessment::Observed {
            known_signatures: known_signatures.iter().map(to_safety_signature).collect(),
            observed: to_safety_signature(observed),
        },
        ..inputs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structural::assess_layout;
    use cancellai_model::{
        ActivityState, AuthorityLevel, IntegrityState, KnowledgeConfidence, ProtectionState,
        ProviderTrust,
    };
    use cancellai_safety::{TrustPromotionEvidence, TrustedTier, effective_authority};

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
            provider_layout: ProviderLayoutAssessment::NotAssessed,
        }
    }

    #[test]
    fn e14s04_end_to_end_a_destructive_capable_input_ends_at_observe_under_real_layout_drift() {
        // The exact chain round 1 independent review found broken, still proven end-to-end
        // after two further redesigns: assess_layout's own real detection output agrees with
        // what feeding the identical raw signatures into a real effective-authority computation
        // produces, starting from an input that - absent the layout constraint - would resolve
        // to the highest authority level this system has.
        let known = vec![LayoutSignature::new([
            "sessions/".to_string(),
            "config.json".to_string(),
        ])];
        let observed = LayoutSignature::new(["totally_different_shape/".to_string()]);

        let finding = assess_layout("claude", &known, &observed);
        assert_eq!(
            finding.support(),
            crate::structural::LayoutSupport::Drifted,
            "test setup: this observed layout must actually be drift"
        );

        let baseline_without_drift = effective_authority(destructive_capable_inputs());
        assert_eq!(
            baseline_without_drift.level,
            AuthorityLevel::Autopilot,
            "test setup: the destructive-capable input must actually reach the top level absent \
             any layout observation, or this test would not be proving the drift path matters"
        );

        let under_drift = effective_authority(authority_inputs_with_layout_observation(
            destructive_capable_inputs(),
            &known,
            &observed,
        ));
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
        // up: the provider_id assess_layout takes plays no part in the raw signatures fed into
        // AuthorityInputs, so a well-known provider name reaching a genuinely drifted layout
        // must still end at Observe, exactly as an unknown provider name would.
        let known = vec![LayoutSignature::new(["sessions/".to_string()])];
        let observed = LayoutSignature::new(["unrecognized_shape/".to_string()]);

        for provider_id in ["claude", "some-unheard-of-provider-xyz"] {
            let finding = assess_layout(provider_id, &known, &observed);
            assert_eq!(finding.support(), crate::structural::LayoutSupport::Drifted);
            let result = effective_authority(authority_inputs_with_layout_observation(
                destructive_capable_inputs(),
                &known,
                &observed,
            ));
            assert_eq!(
                result.level,
                AuthorityLevel::Observe,
                "provider_id={provider_id} must not bypass the drift-recommended ceiling"
            );
        }
    }

    #[test]
    fn e14s04_adr0035_a_real_observation_cannot_be_supplied_and_then_separately_discarded() {
        // The exact counterexample E14 round 3 independent review used against ADR-0034: it held
        // a real drifted finding, confirmed Observe, then separately constructed an otherwise-
        // identical AuthorityInputs asserting `provider_capability_ceiling: None` while still
        // holding that finding, and reached Autopilot. There is no such field to assert here
        // anymore: `authority_inputs_with_layout_observation` sets the raw observation itself,
        // and `effective_authority` derives the ceiling from it directly - proven by comparing
        // its result against a hand-built `AuthorityInputs` carrying the identical observation.
        let known = vec![LayoutSignature::new(["sessions/".to_string()])];
        let observed = LayoutSignature::new(["unrecognized_shape/".to_string()]);

        let via_helper = effective_authority(authority_inputs_with_layout_observation(
            destructive_capable_inputs(),
            &known,
            &observed,
        ));
        let via_hand_built = effective_authority(AuthorityInputs {
            provider_layout: ProviderLayoutAssessment::Observed {
                known_signatures: known.iter().map(to_safety_signature).collect(),
                observed: to_safety_signature(&observed),
            },
            ..destructive_capable_inputs()
        });
        assert_eq!(via_helper, via_hand_built);
        assert_eq!(via_helper.level, AuthorityLevel::Observe);
    }
}

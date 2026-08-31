//! Provider trust promotion (E05-S02, SI-021, SI-022, `docs/architecture/PROVIDER_MODEL.md`
//! "Trust chain": "A community manifest cannot declare itself Built-in Verified. Promotion
//! requires maintainer-owned fixtures, compatibility evidence, threat review, and code
//! ownership approval.").
//!
//! [`TrustedTier`] is the only type [`crate::authority::AuthorityInputs::provider_trust`]
//! accepts, and [`TrustedTier::promote`] is the only way anywhere in this workspace to raise
//! one. Its field is private and its only public constructors are [`TrustedTier::untrusted`]
//! (the safe, evidence-free default) and [`TrustedTier::promote`] itself - there is no
//! `From<ProviderTrust>` or other conversion, so an external caller cannot manufacture a
//! `TrustedTier` at an arbitrary level, however trivially they can construct a bare
//! [`ProviderTrust`] (`cancellai-model` keeps that enum public, freely-constructible pure
//! vocabulary - it is `TrustedTier`, not `ProviderTrust`, that guards authority).
//!
//! **E05 verifier review round 1 (FAIL, SI-021/SI-022) found this crate's first version of
//! this gate did not actually gate anything**: `AuthorityInputs::provider_trust` was typed as
//! bare `ProviderTrust`, so any external caller could write
//! `AuthorityInputs { provider_trust: ProviderTrust::BuiltinVerified, .. }` directly and reach
//! `AuthorityLevel::Autopilot` through `effective_authority` with no call to `promote` and no
//! verifier/fixture evidence at all - the exact SI-021 self-assignment case this story's own
//! AC2 was supposed to close. The `promote` free function existed and worked correctly in
//! isolation, but nothing forced a caller to go through it. `TrustedTier` closes that gap the
//! same way `cancellai-safety::SealedPlan` already closed an analogous one (E03 round 1): by
//! making the *type* impossible to construct except through the gate, not by hoping every
//! caller remembers to call it.

use cancellai_model::ProviderTrust;

/// Provenance a trust promotion must present. `verified_by` names who/what performed the
/// verification (a person, CI job, or review record - this crate does not further validate
/// its format, only that it is non-empty); `fixture_references` names the compatibility
/// evidence backing the claim (`docs/development/VERIFICATION_STRATEGY.md`-style fixture
/// identifiers). Both are required - see [`promote`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustPromotionEvidence {
    pub verified_by: String,
    pub fixture_references: Vec<String>,
}

/// Why a promotion attempt was refused. Every variant is a fail-closed outcome: the caller's
/// trust tier is left unchanged, never guessed or partially applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustPromotionError {
    /// `to` is not strictly greater than `from` - a same-level or downward request is not
    /// something `promote` does (a caller that wants to lower trust does not need a gate;
    /// SI-021 concerns only *raising* trust).
    NotAnUpgrade,
    /// `verified_by` was empty or all-whitespace - an anonymous or self-attested claim
    /// ("trust me") is exactly SI-021's "cannot self-assign a trust level" case.
    MissingVerifier,
    /// `fixture_references` was empty - a claim with a named verifier but zero compatibility
    /// evidence is still not the "maintainer-owned fixtures, compatibility evidence" this
    /// promotion requires.
    MissingFixtureEvidence,
}

/// The raw upgrade rule: returns `to` on success, or the reason a caller's trust tier stays at
/// `from`. Not `pub` - `TrustedTier::promote` is the only public entry point (see the module
/// doc for why the previous, `pub` version of this function was the actual defect the E05
/// verifier round 1 review found: existing and working correctly is not the same as being the
/// *only* path).
fn promote(
    from: ProviderTrust,
    to: ProviderTrust,
    evidence: &TrustPromotionEvidence,
) -> Result<ProviderTrust, TrustPromotionError> {
    if to <= from {
        return Err(TrustPromotionError::NotAnUpgrade);
    }
    if evidence.verified_by.trim().is_empty() {
        return Err(TrustPromotionError::MissingVerifier);
    }
    if evidence.fixture_references.is_empty() {
        return Err(TrustPromotionError::MissingFixtureEvidence);
    }
    Ok(to)
}

/// The only [`ProviderTrust`] value [`crate::authority::effective_authority`] accepts as a
/// provider's trust tier. See the module doc for why this exists and what it closes.
///
/// This doctest is the regression proving the E05 verifier round 1 exploit no longer compiles:
/// an external crate cannot construct a `TrustedTier` at an arbitrary level, so it cannot reach
/// [`crate::authority::AuthorityInputs::provider_trust`] with anything but
/// [`TrustedTier::untrusted`] or the result of a real [`TrustedTier::promote`] call.
///
/// ```compile_fail
/// # use cancellai_model::ProviderTrust;
/// # use cancellai_safety::TrustedTier;
/// // TrustedTier's field is private: no tuple-struct construction from outside this crate,
/// // and there is no `From<ProviderTrust>` to reach for instead.
/// let forged = TrustedTier(ProviderTrust::BuiltinVerified);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustedTier(ProviderTrust);

impl TrustedTier {
    /// The safe, zero-evidence default - always constructible, since `Untrusted` is the floor
    /// SI-021 requires anyway, not a claim that needs provenance to make.
    pub fn untrusted() -> Self {
        TrustedTier(ProviderTrust::Untrusted)
    }

    /// The tier this value currently carries.
    pub fn level(self) -> ProviderTrust {
        self.0
    }

    /// Attempts to raise this tier to `to`. On success, returns a *new* `TrustedTier` at `to` -
    /// `self` is unaffected (this type has no interior mutability); on failure, returns
    /// [`TrustPromotionError`] and the caller still holds whatever `TrustedTier` it started
    /// with, never a partially-applied one.
    pub fn promote(
        self,
        to: ProviderTrust,
        evidence: &TrustPromotionEvidence,
    ) -> Result<TrustedTier, TrustPromotionError> {
        promote(self.0, to, evidence).map(TrustedTier)
    }

    /// Constructs a `TrustedTier` at an arbitrary level with no promotion check at all - visible
    /// only inside this crate (`pub(crate)`) and only compiled for tests, so it cannot be used
    /// to bypass the gate from outside `cancellai-safety`. Exists purely so this crate's own
    /// tests can set up a starting fixture state (e.g. "assume a fully-trusted provider, then
    /// assert some *other* input still collapses authority") without threading a real
    /// `TrustPromotionEvidence` through every unrelated test - mirrors
    /// `SealedPlan`'s own `pub(crate)` field constructor (`sealed_plan.rs`) for the identical
    /// reason.
    #[cfg(test)]
    pub(crate) fn for_tests(level: ProviderTrust) -> Self {
        TrustedTier(level)
    }
}

impl Default for TrustedTier {
    /// `Untrusted` - the same fail-closed default [`TrustedTier::untrusted`] returns, so a
    /// `TrustedTier` obtained via `Default::default()` (e.g. `..Default::default()` in a
    /// struct-update fixture) can never accidentally start out more trusted than SI-021 allows.
    fn default() -> Self {
        TrustedTier::untrusted()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_evidence() -> TrustPromotionEvidence {
        TrustPromotionEvidence {
            verified_by: "maintainer-review-2026-08-31".to_string(),
            fixture_references: vec!["tests/fixtures/claude/v1.2.0-layout".to_string()],
        }
    }

    #[test]
    fn ac2_promotion_succeeds_with_a_named_verifier_and_at_least_one_fixture() {
        let result = promote(
            ProviderTrust::LocalCustom,
            ProviderTrust::CommunityVerified,
            &valid_evidence(),
        );
        assert_eq!(result, Ok(ProviderTrust::CommunityVerified));
    }

    #[test]
    fn ac2_promotion_across_more_than_one_tier_is_allowed_when_evidenced() {
        let result = promote(
            ProviderTrust::Untrusted,
            ProviderTrust::BuiltinVerified,
            &valid_evidence(),
        );
        assert_eq!(result, Ok(ProviderTrust::BuiltinVerified));
    }

    #[test]
    fn si021_a_self_attested_claim_with_no_named_verifier_is_rejected() {
        // The exact SI-021 scenario: "trust me, I'm verified" with fixtures attached but no
        // named verifier is still a self-assignment attempt.
        let evidence = TrustPromotionEvidence {
            verified_by: String::new(),
            fixture_references: vec!["some/fixture".to_string()],
        };
        let result = promote(
            ProviderTrust::Untrusted,
            ProviderTrust::BuiltinVerified,
            &evidence,
        );
        assert_eq!(result, Err(TrustPromotionError::MissingVerifier));
    }

    #[test]
    fn si021_whitespace_only_verifier_is_treated_as_missing() {
        let evidence = TrustPromotionEvidence {
            verified_by: "   ".to_string(),
            fixture_references: vec!["some/fixture".to_string()],
        };
        let result = promote(
            ProviderTrust::Untrusted,
            ProviderTrust::LocalCustom,
            &evidence,
        );
        assert_eq!(result, Err(TrustPromotionError::MissingVerifier));
    }

    #[test]
    fn ac2_a_named_verifier_with_zero_fixtures_is_rejected() {
        // A verifier name alone ("I attest this is fine") is not "compatibility evidence" -
        // PROVIDER_MODEL.md requires maintainer-owned fixtures specifically.
        let evidence = TrustPromotionEvidence {
            verified_by: "someone".to_string(),
            fixture_references: Vec::new(),
        };
        let result = promote(
            ProviderTrust::LocalCustom,
            ProviderTrust::CommunityVerified,
            &evidence,
        );
        assert_eq!(result, Err(TrustPromotionError::MissingFixtureEvidence));
    }

    #[test]
    fn a_downward_or_same_level_request_is_refused_not_silently_a_no_op() {
        for (from, to) in [
            (
                ProviderTrust::BuiltinVerified,
                ProviderTrust::BuiltinVerified,
            ),
            (
                ProviderTrust::BuiltinVerified,
                ProviderTrust::CommunityVerified,
            ),
            (ProviderTrust::CommunityVerified, ProviderTrust::Untrusted),
        ] {
            let result = promote(from, to, &valid_evidence());
            assert_eq!(result, Err(TrustPromotionError::NotAnUpgrade));
        }
    }

    #[test]
    fn every_tier_can_be_promoted_to_every_strictly_higher_tier_given_valid_evidence() {
        let tiers = [
            ProviderTrust::Untrusted,
            ProviderTrust::LocalCustom,
            ProviderTrust::CommunityVerified,
            ProviderTrust::BuiltinVerified,
        ];
        for &from in &tiers {
            for &to in &tiers {
                let result = promote(from, to, &valid_evidence());
                if to > from {
                    assert_eq!(result, Ok(to), "from={from:?} to={to:?}");
                } else {
                    assert_eq!(
                        result,
                        Err(TrustPromotionError::NotAnUpgrade),
                        "from={from:?} to={to:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn si021_a_fresh_trusted_tier_defaults_to_untrusted() {
        assert_eq!(TrustedTier::untrusted().level(), ProviderTrust::Untrusted);
        assert_eq!(TrustedTier::default().level(), ProviderTrust::Untrusted);
    }

    #[test]
    fn trusted_tier_promote_mirrors_the_free_function_gate() {
        let tier = TrustedTier::untrusted();
        let promoted = tier
            .promote(ProviderTrust::CommunityVerified, &valid_evidence())
            .expect("valid evidence promotes");
        assert_eq!(promoted.level(), ProviderTrust::CommunityVerified);
        // The original value is unaffected - promotion returns a new TrustedTier.
        assert_eq!(tier.level(), ProviderTrust::Untrusted);
    }

    #[test]
    fn trusted_tier_promote_rejects_missing_evidence_exactly_like_the_free_function() {
        let tier = TrustedTier::untrusted();
        let evidence = TrustPromotionEvidence {
            verified_by: String::new(),
            fixture_references: vec!["some/fixture".to_string()],
        };
        let result = tier.promote(ProviderTrust::BuiltinVerified, &evidence);
        assert_eq!(result, Err(TrustPromotionError::MissingVerifier));
    }

    #[test]
    fn si021_the_verifier_round1_reproduction_is_no_longer_reachable_through_trusted_tier() {
        // The exact adversarial reproduction from project/evidence/E05-VERIFIER-REVIEW.md,
        // restated in terms of what is now the only public API: there is no way to obtain a
        // fully-trusted TrustedTier from outside this crate except through a real, evidenced
        // promote() call. This documents the property the compile_fail doctest on
        // `TrustedTier` proves at compile time; it exists here too so a plain `cargo test`
        // run still carries a visible regression for it.
        let external_style_tier = TrustedTier::untrusted(); // the only zero-evidence starting point
        assert_eq!(external_style_tier.level(), ProviderTrust::Untrusted);
        let forged_attempt = external_style_tier.promote(
            ProviderTrust::BuiltinVerified,
            &TrustPromotionEvidence {
                verified_by: String::new(),
                fixture_references: Vec::new(),
            },
        );
        assert!(forged_attempt.is_err());
    }
}

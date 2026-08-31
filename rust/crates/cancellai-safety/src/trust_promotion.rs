//! Provider trust promotion (E05-S02, SI-021, SI-022, `docs/architecture/PROVIDER_MODEL.md`
//! "Trust chain": "A community manifest cannot declare itself Built-in Verified. Promotion
//! requires maintainer-owned fixtures, compatibility evidence, threat review, and code
//! ownership approval.").
//!
//! [`promote`] is the sole function anywhere in this workspace that can raise a
//! [`ProviderTrust`] tier. It is deliberately not a method on `ProviderTrust` itself
//! (`cancellai-model` stays pure vocabulary with no decision logic) and it is fail-closed:
//! raising a tier requires both a named verifier and at least one fixture reference, and
//! asking for anything that is not a strict upgrade is refused outright rather than silently
//! treated as a no-op. A caller that skips this function - for instance reading a trust field
//! directly out of a manifest a provider supplied about itself - has not actually promoted
//! anything; nothing in `cancellai-safety`'s authority computation
//! ([`crate::authority::effective_authority`]) accepts a `ProviderTrust` value from anywhere
//! but this gate or the conservative `ProviderTrust::Untrusted` default (SI-021: "cannot
//! self-assign a trust level ... above locally verified policy").

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

/// The only way a [`ProviderTrust`] tier may move upward. Returns `to` on success; on any
/// failure returns [`TrustPromotionError`] and the caller's trust tier stays at `from` - there
/// is no partial or best-effort promotion.
pub fn promote(
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
}

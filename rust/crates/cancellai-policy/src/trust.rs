//! Trust tier for cancellAI's own built-in provider adapters (E06-S01).
//!
//! `cancellai-provider-claude`/`cancellai-provider-codex` are compiled into this binary, not
//! loaded as external community manifests - but `cancellai_safety::TrustedTier`'s only public,
//! non-test constructors are [`cancellai_safety::TrustedTier::untrusted`] and a checked
//! `promote` requiring [`cancellai_safety::TrustPromotionEvidence`] (SI-021, SI-022: "a
//! community manifest cannot declare itself Built-in Verified"). This module is the one place
//! that promotion is performed for the two adapters this workspace ships and maintains itself,
//! citing the concrete evidence backing that claim (E05-S03/E05-S04's differential fixture
//! parity suites) rather than skipping the gate.

use cancellai_model::ProviderTrust;
use cancellai_safety::{TrustPromotionEvidence, TrustedTier};

/// The trust tier cancellAI's own maintained Claude Code / Codex CLI adapters carry: `Built-in
/// Verified`, promoted through the one real gate rather than self-assigned. Panics only if
/// `TrustedTier::promote`'s own invariants are violated by this call site itself (an internal
/// programming error, not a runtime/user condition) - the evidence below is a compile-time
/// constant that always satisfies `promote`'s non-empty checks.
pub fn builtin_provider_trust() -> TrustedTier {
    TrustedTier::untrusted()
        .promote(
            ProviderTrust::BuiltinVerified,
            &TrustPromotionEvidence {
                verified_by: "cancellai maintainers (E05-S03/E05-S04 provider adapter stories)"
                    .to_string(),
                fixture_references: vec![
                    "rust/crates/cancellai-provider-claude/tests/claude_fixture_parity.rs"
                        .to_string(),
                    "rust/crates/cancellai-provider-codex/tests/codex_fixture_parity.rs"
                        .to_string(),
                ],
            },
        )
        .expect("built-in provider trust promotion evidence is a compile-time constant that always satisfies TrustedTier::promote")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_provider_trust_reaches_the_top_tier() {
        assert_eq!(
            builtin_provider_trust().level(),
            ProviderTrust::BuiltinVerified
        );
    }
}

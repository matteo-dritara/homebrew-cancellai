//! Canonical domain vocabulary `docs/architecture/DOMAIN_MODEL.md` defines in prose, given
//! typed form here so `SealedPlan` (E03-S02, `cancellai-safety`) and later stories have one
//! shared, reused definition instead of each inventing their own strings/booleans. This
//! crate is "the bottom of the dependency graph" (see `lib.rs`); these are plain, comparable
//! data - nothing here observes the OS or performs I/O.

/// Effective Authority ordering (`docs/architecture/DOMAIN_MODEL.md` "Effective Authority").
/// Capability ordering, not a risk score: `AUTOPILOT` is not "worse," it can simply do more.
/// Declaration order is deliberately `Observe < Recommend < Quarantine < Govern < Autopilot`,
/// matching the documented ordering exactly, so `derive(Ord)` needs no hand-written
/// comparison to get this right (or wrong).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityLevel {
    Observe,
    Recommend,
    Quarantine,
    Govern,
    Autopilot,
}

/// What kind of work an `Action` represents (`docs/architecture/DOMAIN_MODEL.md` "Action").
/// DOMAIN_MODEL.md's own list ends in `...`, leaving room for a future, more specific class;
/// adding one is a deliberate, reviewed vocabulary change, not implied by this comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionClass {
    Observe,
    Quarantine,
    Archive,
    Delete,
}

/// How recoverable an action's effect is (`docs/architecture/DOMAIN_MODEL.md`
/// "Reversibility").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reversibility {
    Rebuildable,
    Quarantinable,
    Archivable,
    VendorConditional,
    Irreversible,
    Unknown,
}

/// How well-evidenced a claim is (`docs/architecture/DOMAIN_MODEL.md` "Knowledge
/// confidence"). Inferred/unknown confidence cannot silently raise destructive authority -
/// enforcing that is a future authority-lattice concern (E03-S04); this type only carries
/// the fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeConfidence {
    Verified,
    Observed,
    Inferred,
    LowUnknown,
}

/// A minimal stand-in for `docs/architecture/DOMAIN_MODEL.md`'s full `ProviderRoot`
/// (`RootId`, `ProviderId`, `Origin`, `FingerprintEvidence[]`, `KnowledgeConfidence`,
/// `MutationEligible`, `CapabilitySnapshot`). Real provider-root fingerprinting is a provider
/// epic (E05) concern that does not exist yet; this carries just enough - a stable root
/// identity, which provider it belongs to, and the confidence backing that claim - for
/// `SealedPlan` (E03-S02) to record "which capability this plan is bound to" without
/// inventing throwaway ad hoc fields that would need to be redefined once the real
/// `ProviderRoot` lands. Deliberately not `MutationEligible`-bearing itself (SI-002): nothing
/// in E03-S02 grants mutation eligibility from a fingerprint alone.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RootFingerprint {
    pub root_id: String,
    pub provider_id: String,
    pub confidence: KnowledgeConfidence,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authority_level_ordering_matches_the_documented_capability_ordering() {
        assert!(AuthorityLevel::Observe < AuthorityLevel::Recommend);
        assert!(AuthorityLevel::Recommend < AuthorityLevel::Quarantine);
        assert!(AuthorityLevel::Quarantine < AuthorityLevel::Govern);
        assert!(AuthorityLevel::Govern < AuthorityLevel::Autopilot);
    }

    #[test]
    fn authority_level_minimum_is_the_lower_capability_not_the_lower_enum_discriminant_by_accident()
    {
        // A future refactor that reorders these variants "for readability" would silently
        // invert every authority comparison in the system - this pins the ordering to the
        // documented meaning, not merely to whatever `derive(Ord)` currently produces.
        let levels = [
            AuthorityLevel::Autopilot,
            AuthorityLevel::Observe,
            AuthorityLevel::Govern,
        ];
        assert_eq!(levels.iter().min().copied(), Some(AuthorityLevel::Observe));
    }
}

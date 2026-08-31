//! `AgentArtifact`: the provider-neutral unit of state cancellAI reasons about
//! (`docs/architecture/DOMAIN_MODEL.md` "AgentArtifact").
//!
//! This carries exactly the wire-format minimum `docs/architecture/JSON_CONTRACTS.md`'s
//! inventory document requires per entry (`schema check_schemas.py` enforces the shape
//! against `tests/fixtures/schemas/golden/inventory.golden.json`), not the full "minimum
//! conceptual fields" list DOMAIN_MODEL.md sketches (`MachineId`, `ProjectRef`, `SessionRef`,
//! `LogicalSize`, `AllocatedSize`, capabilities, ...) - those are classification/policy
//! working data a caller (E06's `cancellai-policy`) keeps alongside an `AgentArtifact`, not
//! part of this type itself, matching `file_facts.rs`'s own precedent of deliberately not
//! widening a wire-adjacent type beyond what a real producer today can back with evidence.
//!
//! `identity_token` here is the wire-format's *stable, content-derived* identity (a string
//! `JSON_CONTRACTS.md` says two conformant engines must agree on for the same artifact, e.g.
//! `"codex:sessions/2026/05/01/rollout-....jsonl"`) - a different concept from
//! `cancellai_platform::IdentityToken` (device/inode, the execution-time TOCTOU check
//! `cancellai-safety` revalidates immediately before mutation). The two share a name in the
//! docs because they answer the same question ("is this still the same object?") at two
//! different layers - wire-level cross-run/cross-engine matching versus execution-time
//! replacement detection - not because either is defined in terms of the other.

use crate::evidence::EvidenceId;
use crate::vocabulary::{
    ActivityState, AuthorityLevel, IntegrityState, KnowledgeConfidence, ProtectionState,
    ResidencyState, Reversibility, RiskClass,
};

/// An opaque, engine-assigned artifact reference (`docs/architecture/JSON_CONTRACTS.md`:
/// "two conformant engines observing the same fixture are never required to assign the same
/// one"). Never used as a differential-comparison matching key - `identity_token` is.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct ArtifactId(pub String);

impl ArtifactId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One observed unit of provider state, classified along every lifecycle axis
/// (`docs/architecture/DOMAIN_MODEL.md` "AgentArtifact").
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AgentArtifact {
    pub artifact_id: ArtifactId,
    pub identity_token: String,
    pub provider_id: String,
    pub artifact_type: String,
    pub risk_class: RiskClass,
    pub reversibility: Reversibility,
    pub knowledge_confidence: KnowledgeConfidence,
    pub activity_state: ActivityState,
    pub residency_state: ResidencyState,
    pub protection_state: ProtectionState,
    pub integrity_state: IntegrityState,
    pub authority_ceiling: AuthorityLevel,
    pub evidence_ids: Vec<EvidenceId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AgentArtifact {
        AgentArtifact {
            artifact_id: ArtifactId::new("artifact-0001"),
            identity_token: "codex:sessions/2026/05/01/rollout-x.jsonl".to_string(),
            provider_id: "codex".to_string(),
            artifact_type: "session".to_string(),
            risk_class: RiskClass::R3Resumable,
            reversibility: Reversibility::Irreversible,
            knowledge_confidence: KnowledgeConfidence::Verified,
            activity_state: ActivityState::Stale,
            residency_state: ResidencyState::Hot,
            protection_state: ProtectionState::Normal,
            integrity_state: IntegrityState::Healthy,
            authority_ceiling: AuthorityLevel::Govern,
            evidence_ids: vec![EvidenceId::new("evidence-0001")],
        }
    }

    #[test]
    fn serializes_every_json_contracts_inventory_field_by_the_documented_snake_case_name() {
        let json = serde_json::to_value(sample()).expect("serializable");
        for key in [
            "artifact_id",
            "identity_token",
            "provider_id",
            "artifact_type",
            "risk_class",
            "reversibility",
            "knowledge_confidence",
            "activity_state",
            "residency_state",
            "protection_state",
            "integrity_state",
            "authority_ceiling",
            "evidence_ids",
        ] {
            assert!(json.get(key).is_some(), "missing field {key} in {json}");
        }
    }
}

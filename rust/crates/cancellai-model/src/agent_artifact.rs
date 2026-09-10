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
//!
//! ## `relationships` (E08-S01)
//!
//! `docs/architecture/PERSISTENCE_MODEL.md`'s Layer 1 lists "artifact identity and
//! relationships" as a bullet distinct from "provider/project/session references" - the latter
//! is project/session *attribution*, deferred to E08-S02 ("Attribute artifacts to projects only
//! through explicit provider metadata, known paths, or strong observed evidence"), which this
//! story does not implement. `relationships` here is the narrower, already-evidenced thing:
//! artifact-to-artifact structural links. Today exactly one such link is real and observed -
//! `cancellai_provider_codex::CodexSession::parent_session_id`, read from a rollout's own
//! `session_meta` record - and it used to reach `cancellai-policy::retention::classify` as a
//! `group_key` argument that was immediately discarded (`let _ = group_key;`). `RelationshipKind`
//! carries only `ChildOf` because that is the one direction this build can name without
//! recomputing the other: a parent knowing all its children requires scanning every sibling,
//! which is derivable from the child links already present rather than independent information,
//! so it is not duplicated here (same reasoning `ErrorCategory::code()` uses for its own
//! single-source-of-truth split, `diagnostic.rs`).
//!
//! ## `provenance`
//!
//! The epic's outcome text also names "provenance" as an axis. `Evidence`/`evidence_ids`
//! (E06-S01) already are that axis - `docs/DECISION_REGISTER.md`: "Every classified artifact has
//! evidence provenance" - so this story does not add a second, parallel field that would only
//! duplicate `evidence_ids`/`knowledge_confidence` without a new fact to carry.
//!
//! ## `project_attribution` (E08-S02)
//!
//! DOMAIN_MODEL.md's own "minimum conceptual fields" sketch lists `ProjectRef? / Unattributed`.
//! `project_attribution: Option<ProjectAttribution>` is that field, `None` meaning `Unattributed`
//! (SI-023: attribution uncertainty must stay explicit, never silently promoted to a stronger
//! claim). Every `Some` records not just *which* project but *how sure* and *from what evidence
//! category* (this story's AC2), reusing the story outcome's own three-source vocabulary
//! (`AttributionSource`).
//!
//! Only `ExplicitProviderMetadata` has a real producer today: Claude's `projects/<name>/`
//! directory name (`cancellai_provider_claude::session::ClaudeSession::project`) is the
//! provider's own structural grouping, taken verbatim - not decoded into a claimed real
//! filesystem path. Claude Code's actual encoding (`/` folded into `-`) is lossy and ambiguous
//! for path segments that themselves contain `-`, so guessing a real path back out of it would
//! risk exactly the overclaim SI-023 forbids; DOMAIN_MODEL.md's own "known paths" category
//! (`KnownPath` below) is reserved for a *real, currently-observed* path, which no adapter
//! resolves today, so it stays unpopulated (the same "field exists for a future real producer,
//! not invented now" precedent `FileFacts::provider_hint`/`category_hint` already set).
//! `ObservedEvidence` (a heuristic match weaker than either) is likewise unpopulated. A blank/
//! whitespace-only project name - degenerate metadata that names no real grouping - resolves to
//! `Unattributed` rather than a hollow `ProjectRef`. Codex sessions have no project concept this
//! adapter observes at all (`cancellai-provider-codex` groups only by subagent tree, never by
//! project), so every Codex artifact is `Unattributed` - a true absence of evidence, not a
//! placeholder guess.
//!
//! `project_attribution.confidence` starts equal to the artifact's own `knowledge_confidence` -
//! not an independent computation, since the same scan evidence backs both today - and
//! `cancellai-policy::retention`'s existing partial-scan downgrade (SI-008/SI-009) lowers it
//! alongside `knowledge_confidence` rather than leaving it stale at a higher value (SI-023's
//! concrete, testable form: a degraded scan cannot leave attribution looking more certain than
//! the artifact it is attached to).

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

/// What kind of structural link one [`AgentArtifact`] has to another (this module's own doc,
/// "`relationships` (E08-S01)"). Not project/session attribution - see [`AgentArtifact::relationships`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKind {
    ChildOf,
}

/// One directed relationship from an [`AgentArtifact`] to another, by [`ArtifactId`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ArtifactRelationship {
    pub kind: RelationshipKind,
    pub related_artifact_id: ArtifactId,
}

/// An opaque, provider-assigned project grouping identity (this module's own doc,
/// "`project_attribution` (E08-S02)"). Like [`ArtifactId`]/`provider_id`, its value is
/// whatever the provider's own metadata names the project - not a claim that it is, or decodes
/// to, a real filesystem path. See [`AttributionSource::KnownPath`] for that distinct, stronger
/// claim.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct ProjectRef(pub String);

impl ProjectRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Which evidence category justified a [`ProjectAttribution`] (E08-S02's own outcome:
/// "explicit provider metadata, known paths, or strong observed evidence").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionSource {
    /// The provider's own structural grouping (e.g. Claude's `projects/<name>/` directory) -
    /// today's only populated source; see this module's own doc for why.
    ExplicitProviderMetadata,
    /// A real, currently-observed filesystem path backs the attribution - not merely a decoded
    /// guess at one. No adapter resolves this today (this module's own doc).
    KnownPath,
    /// A heuristic match strong enough to attribute but weaker than either source above. Not
    /// produced by any adapter today.
    ObservedEvidence,
}

/// One artifact's project attribution: which project, from what evidence, held with how much
/// confidence (E08-S02 AC2). `AgentArtifact::project_attribution` is `None` - `Unattributed` -
/// rather than this type wrapping an optional/empty `ProjectRef`, so "we don't know" can never
/// be represented as a hollow attribution record (SI-023).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectAttribution {
    pub project_ref: ProjectRef,
    pub source: AttributionSource,
    pub confidence: KnowledgeConfidence,
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
    /// Artifact-to-artifact structural links (this module's own doc, "`relationships`
    /// (E08-S01)"). Empty, not omitted, when none are observed - a caller must never infer
    /// absence-of-evidence as absence-of-relationship from a missing key.
    pub relationships: Vec<ArtifactRelationship>,
    /// Which project this artifact belongs to, or `None` for `Unattributed` (this module's own
    /// doc, "`project_attribution` (E08-S02)", SI-023). Present as an explicit `null`, not an
    /// omitted key, for the same reason `relationships` is never omitted empty.
    pub project_attribution: Option<ProjectAttribution>,
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
            relationships: Vec::new(),
            project_attribution: None,
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

    #[test]
    fn empty_relationships_serialize_as_an_empty_array_not_an_omitted_key() {
        let json = serde_json::to_value(sample()).expect("serializable");
        assert_eq!(json.get("relationships"), Some(&serde_json::json!([])));
    }

    #[test]
    fn a_child_of_relationship_serializes_the_kind_and_the_related_artifact_id() {
        let mut artifact = sample();
        artifact.relationships.push(ArtifactRelationship {
            kind: RelationshipKind::ChildOf,
            related_artifact_id: ArtifactId::new("artifact-root"),
        });
        let json = serde_json::to_value(artifact).expect("serializable");
        assert_eq!(
            json.get("relationships"),
            Some(&serde_json::json!([
                { "kind": "child_of", "related_artifact_id": "artifact-root" }
            ]))
        );
    }

    #[test]
    fn unattributed_serializes_as_an_explicit_null_not_an_omitted_key() {
        let json = serde_json::to_value(sample()).expect("serializable");
        assert_eq!(
            json.get("project_attribution"),
            Some(&serde_json::Value::Null)
        );
    }

    #[test]
    fn a_project_attribution_serializes_the_ref_source_and_confidence() {
        let mut artifact = sample();
        artifact.project_attribution = Some(ProjectAttribution {
            project_ref: ProjectRef::new("-Users-example-project"),
            source: AttributionSource::ExplicitProviderMetadata,
            confidence: KnowledgeConfidence::Verified,
        });
        let json = serde_json::to_value(artifact).expect("serializable");
        assert_eq!(
            json.get("project_attribution"),
            Some(&serde_json::json!({
                "project_ref": "-Users-example-project",
                "source": "explicit_provider_metadata",
                "confidence": "verified"
            }))
        );
    }
}

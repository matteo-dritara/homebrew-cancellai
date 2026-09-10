//! Multi-dimensional query views over one classified inventory (E08-S04).
//!
//! `docs/architecture/TARGET.md`'s target diagram places an "Engine / Query API" layer above
//! "Inventory + Providers" and "Policy + Explanation." No dedicated crate backs that layer yet
//! (`cancellai-cli` currently assembles its `status`/`inspect` output ad hoc, per command, with
//! no shared grouping logic between them) - this module is the first real occupant, placed in
//! `cancellai-policy` because that is where the `ClassifiedArtifact` collections it reads
//! already live, one layer below the query API box in the diagram. A dedicated crate split is a
//! future, separately-reviewed architectural step if the query surface grows enough to need its
//! own dependency boundary; this story does not invent one ahead of that need.
//!
//! Every view below borrows from the caller's own `&[ClassifiedArtifact]` - a bucket is a
//! `Vec<&ClassifiedArtifact>`, never a clone of the artifact itself - the same `'a`-borrowing
//! pattern `retention::ProviderPlanningView` already established, for the same reason (AC1:
//! "Views do not duplicate source data").
//!
//! Every grouping function partitions its input: each artifact appears in exactly one bucket of
//! a given dimension, and the buckets' combined artifact count always equals the input length
//! (AC2: "Totals reconcile across dimensions with explicit Unattributed bucket") -
//! `tests::every_dimension_reconciles_to_the_same_total` proves this generically across all five
//! views against one mixed fixture, rather than trusting each grouping function's own bookkeeping
//! by inspection.
//!
//! Two dimensions are honestly degenerate today, not filled in with an invented mechanism ahead
//! of the evidence that would back it (the same restraint E08-S01/E08-S02 already applied to
//! `provider_hint`/`KnownPath`):
//!
//! - **machine**: `docs/architecture/DOMAIN_MODEL.md`'s `MachineId` field does not exist on
//!   `AgentArtifact` (E18/E19's remote-target work has not landed), and every artifact this
//!   single-machine build observes is, definitionally, on the one machine running it - so
//!   [`by_machine`] returns exactly one bucket.
//! - **session**: Claude sessions are already 1:1 with artifacts (flat, per `cancellai_provider_
//!   claude::session`'s own module doc), so its session view coincides with [`by_artifact`].
//!   Codex subagent trees are not: [`by_session`] groups a tree's members under their resolved
//!   `RelationshipKind::ChildOf` root (E08-S01), falling back to the artifact's own id when it
//!   has no such relationship - including an `Orphaned` (E08-S03) artifact, which by definition
//!   never carries one, and so is correctly reported as its own single-member session.

use crate::retention::ClassifiedArtifact;
use cancellai_model::{ArtifactId, RelationshipKind};
use std::collections::BTreeMap;

/// One provider's bucket in [`by_provider`].
#[derive(Debug)]
pub struct ProviderBucket<'a> {
    pub provider_id: &'a str,
    pub artifacts: Vec<&'a ClassifiedArtifact>,
}

/// Groups `artifacts` by `provider_id`. `provider_id` is always populated (never `Unattributed`
/// - only [`by_project`] has that bucket), so ordinary partitioning is enough.
pub fn by_provider(artifacts: &[ClassifiedArtifact]) -> Vec<ProviderBucket<'_>> {
    let mut groups: BTreeMap<&str, Vec<&ClassifiedArtifact>> = BTreeMap::new();
    for artifact in artifacts {
        groups
            .entry(artifact.artifact.provider_id.as_str())
            .or_default()
            .push(artifact);
    }
    groups
        .into_iter()
        .map(|(provider_id, artifacts)| ProviderBucket {
            provider_id,
            artifacts,
        })
        .collect()
}

/// Which project bucket an artifact falls into in [`by_project`] (E08-S02's own
/// `ProjectAttribution`/`Unattributed` split, carried through to this dimension).
#[derive(Debug, PartialEq, Eq)]
pub enum ProjectBucketKey<'a> {
    Attributed(&'a str),
    Unattributed,
}

/// One project's bucket in [`by_project`].
#[derive(Debug)]
pub struct ProjectBucket<'a> {
    pub key: ProjectBucketKey<'a>,
    pub artifacts: Vec<&'a ClassifiedArtifact>,
}

/// Groups `artifacts` by `project_attribution`, with an explicit `Unattributed` bucket for every
/// artifact whose attribution is `None` (AC2) - never silently dropped or merged into an
/// attributed bucket by an empty/default key.
pub fn by_project(artifacts: &[ClassifiedArtifact]) -> Vec<ProjectBucket<'_>> {
    let mut groups: BTreeMap<Option<&str>, Vec<&ClassifiedArtifact>> = BTreeMap::new();
    for artifact in artifacts {
        let key = artifact
            .artifact
            .project_attribution
            .as_ref()
            .map(|attribution| attribution.project_ref.0.as_str());
        groups.entry(key).or_default().push(artifact);
    }
    groups
        .into_iter()
        .map(|(key, artifacts)| ProjectBucket {
            key: match key {
                Some(name) => ProjectBucketKey::Attributed(name),
                None => ProjectBucketKey::Unattributed,
            },
            artifacts,
        })
        .collect()
}

/// One session's bucket in [`by_session`] - `session_root` is the [`ArtifactId`] every member
/// resolves to (this module's own doc for the resolution rule).
#[derive(Debug)]
pub struct SessionBucket<'a> {
    pub session_root: &'a ArtifactId,
    pub artifacts: Vec<&'a ClassifiedArtifact>,
}

/// Groups `artifacts` by session: a `ChildOf` relationship's target when the artifact has one,
/// otherwise the artifact's own id (this module's own doc).
pub fn by_session(artifacts: &[ClassifiedArtifact]) -> Vec<SessionBucket<'_>> {
    let mut groups: BTreeMap<&ArtifactId, Vec<&ClassifiedArtifact>> = BTreeMap::new();
    for artifact in artifacts {
        let root = artifact
            .artifact
            .relationships
            .iter()
            .find(|relationship| relationship.kind == RelationshipKind::ChildOf)
            .map_or(&artifact.artifact.artifact_id, |relationship| {
                &relationship.related_artifact_id
            });
        groups.entry(root).or_default().push(artifact);
    }
    groups
        .into_iter()
        .map(|(session_root, artifacts)| SessionBucket {
            session_root,
            artifacts,
        })
        .collect()
}

/// One machine's bucket in [`by_machine`] - always exactly one today (this module's own doc).
#[derive(Debug)]
pub struct MachineBucket<'a> {
    pub machine: &'static str,
    pub artifacts: Vec<&'a ClassifiedArtifact>,
}

/// The single-machine view every artifact this build observes belongs to (this module's own
/// doc). Still a real partition of one bucket, not a special case a reconciliation check has to
/// carve out - `tests::every_dimension_reconciles_to_the_same_total` treats it identically to
/// every other dimension.
pub fn by_machine(artifacts: &[ClassifiedArtifact]) -> Vec<MachineBucket<'_>> {
    vec![MachineBucket {
        machine: "local",
        artifacts: artifacts.iter().collect(),
    }]
}

/// The degenerate, one-artifact-per-bucket view (this module's own doc: coincides with
/// [`by_session`] for Claude, distinct from it for a Codex subagent tree).
pub fn by_artifact(artifacts: &[ClassifiedArtifact]) -> Vec<&ClassifiedArtifact> {
    artifacts.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_model::{
        ActivityState, AgentArtifact, ArtifactRelationship, AttributionSource, AuthorityLevel,
        EvidenceId, IntegrityState, KnowledgeConfidence, ProjectAttribution, ProjectRef,
        ProtectionState, ResidencyState, Reversibility, RiskClass,
    };
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    #[allow(clippy::too_many_arguments)]
    fn artifact(
        id: &str,
        provider_id: &str,
        project: Option<&str>,
        relationships: Vec<ArtifactRelationship>,
    ) -> ClassifiedArtifact {
        ClassifiedArtifact {
            artifact: AgentArtifact {
                artifact_id: ArtifactId::new(id),
                identity_token: format!("{provider_id}:{id}"),
                provider_id: provider_id.to_string(),
                artifact_type: "session".to_string(),
                risk_class: RiskClass::R3Resumable,
                reversibility: Reversibility::Irreversible,
                knowledge_confidence: KnowledgeConfidence::Verified,
                activity_state: ActivityState::Idle,
                residency_state: ResidencyState::Hot,
                protection_state: ProtectionState::Normal,
                integrity_state: IntegrityState::Healthy,
                authority_ceiling: AuthorityLevel::Govern,
                evidence_ids: vec![EvidenceId::new(format!("evidence-{id}"))],
                relationships,
                project_attribution: project.map(|name| ProjectAttribution {
                    project_ref: ProjectRef::new(name),
                    source: AttributionSource::ExplicitProviderMetadata,
                    confidence: KnowledgeConfidence::Verified,
                }),
                activity_signal: None,
            },
            path: PathBuf::from(format!("/synthetic/{id}")),
            size_bytes: 0,
            reachable_authority: AuthorityLevel::Govern,
            binding_constraints: Vec::new(),
        }
    }

    fn child_of(related_artifact_id: &str) -> ArtifactRelationship {
        ArtifactRelationship {
            kind: RelationshipKind::ChildOf,
            related_artifact_id: ArtifactId::new(related_artifact_id),
        }
    }

    /// Two providers, one with an attributed project and one Unattributed artifact, one Codex
    /// subagent tree (root + child) - exercises every bucket kind at once.
    fn mixed_fixture() -> Vec<ClassifiedArtifact> {
        vec![
            artifact("claude-a", "claude-code", Some("proj-x"), Vec::new()),
            artifact("claude-b", "claude-code", Some("proj-x"), Vec::new()),
            artifact("claude-c", "claude-code", None, Vec::new()),
            artifact("codex-root", "codex-cli", None, Vec::new()),
            artifact(
                "codex-child",
                "codex-cli",
                None,
                vec![child_of("codex-root")],
            ),
        ]
    }

    fn all_ids(artifacts: &[ClassifiedArtifact]) -> BTreeSet<ArtifactId> {
        artifacts
            .iter()
            .map(|a| a.artifact.artifact_id.clone())
            .collect()
    }

    /// AC2: every dimension's buckets partition the input exactly - same total count, same set
    /// of artifact ids, no artifact counted twice or dropped, in any of the five views.
    #[test]
    fn every_dimension_reconciles_to_the_same_total() {
        let fixture = mixed_fixture();
        let expected_ids = all_ids(&fixture);
        let expected_count = fixture.len();

        let provider_ids: BTreeSet<ArtifactId> = by_provider(&fixture)
            .into_iter()
            .flat_map(|bucket| bucket.artifacts)
            .map(|a| a.artifact.artifact_id.clone())
            .collect();
        assert_eq!(provider_ids, expected_ids, "by_provider must reconcile");

        let project_buckets = by_project(&fixture);
        let project_ids: BTreeSet<ArtifactId> = project_buckets
            .iter()
            .flat_map(|bucket| &bucket.artifacts)
            .map(|a| a.artifact.artifact_id.clone())
            .collect();
        assert_eq!(project_ids, expected_ids, "by_project must reconcile");
        assert!(
            project_buckets
                .iter()
                .any(|bucket| bucket.key == ProjectBucketKey::Unattributed),
            "AC2: an explicit Unattributed bucket must be present"
        );

        let session_ids: BTreeSet<ArtifactId> = by_session(&fixture)
            .into_iter()
            .flat_map(|bucket| bucket.artifacts)
            .map(|a| a.artifact.artifact_id.clone())
            .collect();
        assert_eq!(session_ids, expected_ids, "by_session must reconcile");

        let machine_ids: BTreeSet<ArtifactId> = by_machine(&fixture)
            .into_iter()
            .flat_map(|bucket| bucket.artifacts)
            .map(|a| a.artifact.artifact_id.clone())
            .collect();
        assert_eq!(machine_ids, expected_ids, "by_machine must reconcile");

        let artifact_ids: BTreeSet<ArtifactId> = by_artifact(&fixture)
            .into_iter()
            .map(|a| a.artifact.artifact_id.clone())
            .collect();
        assert_eq!(artifact_ids, expected_ids, "by_artifact must reconcile");
        assert_eq!(
            by_artifact(&fixture).len(),
            expected_count,
            "by_artifact is one bucket per input row"
        );
    }

    #[test]
    fn by_project_groups_the_same_project_name_into_one_bucket() {
        let fixture = mixed_fixture();
        let buckets = by_project(&fixture);
        let proj_x = buckets
            .iter()
            .find(|b| b.key == ProjectBucketKey::Attributed("proj-x"))
            .expect("proj-x bucket must exist");
        assert_eq!(proj_x.artifacts.len(), 2);
    }

    #[test]
    fn by_session_groups_a_codex_tree_under_its_root_but_leaves_claude_sessions_apart() {
        let fixture = mixed_fixture();
        let buckets = by_session(&fixture);
        let root_id = ArtifactId::new("codex-root");
        let tree_bucket = buckets
            .iter()
            .find(|b| *b.session_root == root_id)
            .expect("the codex tree's root bucket must exist");
        assert_eq!(
            tree_bucket.artifacts.len(),
            2,
            "the root and its child must share one session bucket"
        );

        let claude_sessions = buckets
            .iter()
            .filter(|b| {
                b.artifacts
                    .iter()
                    .all(|a| a.artifact.provider_id == "claude-code")
            })
            .count();
        assert_eq!(
            claude_sessions, 3,
            "Claude's flat sessions must each be their own bucket"
        );
    }

    #[test]
    fn by_machine_is_exactly_one_bucket_containing_everything() {
        let fixture = mixed_fixture();
        let buckets = by_machine(&fixture);
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].artifacts.len(), fixture.len());
    }

    #[test]
    fn an_empty_inventory_produces_no_buckets_in_every_grouping_dimension() {
        let fixture: Vec<ClassifiedArtifact> = Vec::new();
        assert!(by_provider(&fixture).is_empty());
        assert!(by_project(&fixture).is_empty());
        assert!(by_session(&fixture).is_empty());
        assert!(by_artifact(&fixture).is_empty());
        // `by_machine` is the one dimension that is a fixed, non-input-derived partition (this
        // module's own doc) - it still reports the single machine bucket, now holding zero
        // artifacts, rather than omitting the machine that ran an empty scan.
        assert_eq!(by_machine(&fixture).len(), 1);
        assert!(by_machine(&fixture)[0].artifacts.is_empty());
    }
}

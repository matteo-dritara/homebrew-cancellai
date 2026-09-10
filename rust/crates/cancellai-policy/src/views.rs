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
//!   Codex subagent trees are not: [`by_session`] walks each artifact's `RelationshipKind::
//!   ChildOf` chain to its ultimate root (E08-S01), not merely its immediate parent, falling
//!   back to the furthest verifiable id - the artifact's own, when it has no such relationship
//!   at all, including an `Orphaned` (E08-S03) artifact, which by definition never carries one -
//!   see [`by_session`]'s own doc for the full fail-safe rule set.

use crate::retention::ClassifiedArtifact;
use cancellai_model::{ArtifactId, RelationshipKind};
use std::collections::{BTreeMap, HashMap, HashSet};

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

/// Groups `artifacts` by session: the ultimate root of the artifact's `ChildOf` chain (this
/// module's own doc) - not merely its immediate parent, so a three-level Codex tree (grandparent
/// <- parent <- child) lands in one bucket rather than splitting at the middle member.
///
/// Round-1 independent verifier review (`project/evidence/E08-VERIFIER-REVIEW.md`) found the
/// original version followed exactly one `ChildOf` edge - the *direct* parent - which is what
/// `cancellai-policy::retention::classify` actually records per artifact (E08-S01: a session's
/// relationship names its immediate parent, not the tree root). A transitive chain therefore
/// produced one bucket per generation instead of one per tree. The same review found the target
/// of that one edge trusted blindly: a `ChildOf` pointing at an id absent from the given slice
/// (a real, foreseeable case - a caller may pass a filtered view, e.g. "delete candidates only",
/// that excludes a tree's own root) became the bucket key verbatim, an id nothing in the output
/// could ever back up. [`session_root`] fixes both by walking the full chain against an index of
/// *this slice's own* artifacts, mirroring `cancellai_provider_codex::graph::root_id_for`'s own
/// fail-safe rules exactly: a target not in the index, an artifact with no `ChildOf`
/// relationship, or a cycle (malformed/adversarial metadata) all isolate the walk at its current
/// position rather than looping or propagating an unbacked id.
pub fn by_session(artifacts: &[ClassifiedArtifact]) -> Vec<SessionBucket<'_>> {
    let by_id: HashMap<&ArtifactId, &ClassifiedArtifact> = artifacts
        .iter()
        .map(|classified| (&classified.artifact.artifact_id, classified))
        .collect();

    let mut groups: BTreeMap<&ArtifactId, Vec<&ClassifiedArtifact>> = BTreeMap::new();
    for artifact in artifacts {
        let root = session_root(&artifact.artifact.artifact_id, &by_id);
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

/// Walks `start`'s `ChildOf` chain to its ultimate root against `by_id`, an index built from the
/// exact slice [`by_session`] was called with. See [`by_session`]'s own doc for the fail-safe
/// rules this mirrors from `cancellai_provider_codex::graph::root_id_for`.
fn session_root<'a>(
    start: &'a ArtifactId,
    by_id: &HashMap<&'a ArtifactId, &'a ClassifiedArtifact>,
) -> &'a ArtifactId {
    let mut current = start;
    let mut seen: HashSet<&ArtifactId> = HashSet::new();
    loop {
        if !seen.insert(current) {
            // Cycle: isolate the id this walk actually started from, not wherever the cycle was
            // detected - matching `root_id_for`'s own choice for the identical situation.
            return start;
        }
        let Some(classified) = by_id.get(current) else {
            // `current` is not itself present in this slice - stop here rather than trusting a
            // relationship this view cannot verify against anything.
            return current;
        };
        let Some(parent_relationship) = classified
            .artifact
            .relationships
            .iter()
            .find(|relationship| relationship.kind == RelationshipKind::ChildOf)
        else {
            return current;
        };
        let parent = &parent_relationship.related_artifact_id;
        if !by_id.contains_key(parent) {
            // The declared parent is absent from this slice - `current` is the furthest
            // verifiable point, not the unbacked `parent` id.
            return current;
        }
        current = parent;
    }
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

    /// Counts each artifact id's *occurrences*, not merely whether it is present - a
    /// `BTreeSet`-based comparison collapses a duplicate source row onto a single element,
    /// which is exactly the blind spot round-1 independent verifier review found: the previous
    /// version of this test suite could not have observed a view that silently dropped or
    /// duplicated a row as long as the surviving *set* of ids matched.
    fn counted_ids<'a>(
        artifacts: impl IntoIterator<Item = &'a ClassifiedArtifact>,
    ) -> BTreeMap<ArtifactId, usize> {
        let mut counts = BTreeMap::new();
        for artifact in artifacts {
            *counts
                .entry(artifact.artifact.artifact_id.clone())
                .or_insert(0) += 1;
        }
        counts
    }

    /// AC2: every dimension's buckets partition the input exactly - same total count, same
    /// multiset of artifact ids (each occurring exactly as many times as in the input, so a
    /// duplicate source row is preserved rather than collapsed), in all five views.
    #[test]
    fn every_dimension_reconciles_to_the_same_total() {
        let fixture = mixed_fixture();
        let expected = counted_ids(&fixture);
        let expected_count = fixture.len();

        let provider_counts = counted_ids(
            by_provider(&fixture)
                .into_iter()
                .flat_map(|bucket| bucket.artifacts),
        );
        assert_eq!(provider_counts, expected, "by_provider must reconcile");

        let project_buckets = by_project(&fixture);
        let project_counts = counted_ids(
            project_buckets
                .iter()
                .flat_map(|bucket| bucket.artifacts.iter().copied()),
        );
        assert_eq!(project_counts, expected, "by_project must reconcile");
        assert!(
            project_buckets
                .iter()
                .any(|bucket| bucket.key == ProjectBucketKey::Unattributed),
            "AC2: an explicit Unattributed bucket must be present"
        );

        let session_counts = counted_ids(
            by_session(&fixture)
                .into_iter()
                .flat_map(|bucket| bucket.artifacts),
        );
        assert_eq!(session_counts, expected, "by_session must reconcile");

        let machine_counts = counted_ids(
            by_machine(&fixture)
                .into_iter()
                .flat_map(|bucket| bucket.artifacts),
        );
        assert_eq!(machine_counts, expected, "by_machine must reconcile");

        let artifact_counts = counted_ids(by_artifact(&fixture));
        assert_eq!(artifact_counts, expected, "by_artifact must reconcile");
        assert_eq!(
            by_artifact(&fixture).len(),
            expected_count,
            "by_artifact is one bucket per input row"
        );
    }

    /// The exact blind spot round-1 review found: a duplicate source row (two `ClassifiedArtifact`
    /// entries sharing one `ArtifactId`, e.g. a rollout counted from both `sessions/` and
    /// `archived_sessions/`, per `cancellai_provider_codex::graph`'s own documented duplicate-
    /// handling case) must appear *twice* in a view's reconciliation count, not once - proving
    /// `counted_ids` (and therefore every dimension using it) actually distinguishes multiplicity
    /// from mere presence, which a `BTreeSet` could not.
    #[test]
    fn a_duplicate_source_row_is_preserved_not_collapsed_by_reconciliation() {
        let fixture = vec![
            artifact("dup", "codex-cli", None, Vec::new()),
            artifact("dup", "codex-cli", None, Vec::new()),
        ];
        let expected = counted_ids(&fixture);
        assert_eq!(expected[&ArtifactId::new("dup")], 2);

        let provider_counts = counted_ids(
            by_provider(&fixture)
                .into_iter()
                .flat_map(|bucket| bucket.artifacts),
        );
        assert_eq!(
            provider_counts, expected,
            "both copies of a duplicate id must survive by_provider"
        );

        let session_counts = counted_ids(
            by_session(&fixture)
                .into_iter()
                .flat_map(|bucket| bucket.artifacts),
        );
        assert_eq!(
            session_counts, expected,
            "both copies of a duplicate id must survive by_session"
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

    /// Round-1 independent verifier review's exact reproduction: a three-level chain (grandparent
    /// <- parent <- child) must land in one bucket keyed by the tree's ultimate root, not split
    /// at the middle member because `retention::classify` only ever records an artifact's
    /// *immediate* parent as its `ChildOf` target.
    #[test]
    fn by_session_groups_a_transitive_child_chain_under_the_tree_root() {
        let fixture = vec![
            artifact("grandparent", "codex-cli", None, Vec::new()),
            artifact("parent", "codex-cli", None, vec![child_of("grandparent")]),
            artifact("child", "codex-cli", None, vec![child_of("parent")]),
        ];

        let buckets = by_session(&fixture);
        assert_eq!(
            buckets.len(),
            1,
            "a three-level chain must collapse to exactly one session bucket: {buckets:?}"
        );
        assert_eq!(*buckets[0].session_root, ArtifactId::new("grandparent"));
        assert_eq!(buckets[0].artifacts.len(), 3);
    }

    /// Round-1 independent verifier review's second reproduction: a `ChildOf` target absent from
    /// the given slice (e.g. a caller passed a filtered view that excludes the tree's own root)
    /// must not become the bucket key verbatim - the artifact falls back to its own id, the
    /// furthest point this slice can actually verify.
    #[test]
    fn by_session_falls_back_to_own_id_when_parent_is_absent_from_the_input_slice() {
        let fixture = vec![artifact(
            "child",
            "codex-cli",
            None,
            vec![child_of("not-present")],
        )];

        let buckets = by_session(&fixture);
        assert_eq!(buckets.len(), 1);
        assert_eq!(
            *buckets[0].session_root,
            ArtifactId::new("child"),
            "an unresolvable target must never become the bucket key"
        );
    }

    /// A cyclic `ChildOf` chain (malformed/adversarial metadata: A -> B -> A) must isolate each
    /// member at its own starting point rather than looping forever or merging A and B into one
    /// bucket keyed by whichever id the cycle happened to be entered from - mirroring
    /// `cancellai_provider_codex::graph::root_id_for`'s own documented cycle handling.
    #[test]
    fn by_session_isolates_a_cycle_at_its_starting_point() {
        let fixture = vec![
            artifact("a", "codex-cli", None, vec![child_of("b")]),
            artifact("b", "codex-cli", None, vec![child_of("a")]),
        ];

        let buckets = by_session(&fixture);
        assert_eq!(
            buckets.len(),
            2,
            "a two-cycle must not merge into one bucket: {buckets:?}"
        );
        for bucket in &buckets {
            assert_eq!(
                bucket.artifacts.len(),
                1,
                "each cyclic member isolates as its own single-member session"
            );
        }
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

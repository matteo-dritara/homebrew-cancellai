//! Atlas summary aggregation (E09-S02) - the second occupant of `docs/architecture/TARGET.md`'s
//! "Engine / Query API" layer, after [`crate::views`] (E08-S04). Turns already-classified
//! [`ProviderResolution`]s into the totals `cancellai-tui`'s Atlas screen renders: total
//! footprint, an estimated-reclaimable subset kept as a *separate* field rather than blended
//! into it, per-provider and per-project breakdowns (with the same explicit `Unattributed`
//! bucket [`crate::views::by_project`] established), and the individually biggest contributors.
//! The UI crate never touches a [`ClassifiedArtifact`] or a safety-kernel authority computation
//! directly (`cancellai-tui`'s own AC1: no direct filesystem/provider access, and by extension
//! no direct authority computation either) - both stay behind this module.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use cancellai_model::{ActionClass, ArtifactId};
use cancellai_safety::authority::minimum_authority_for;

use crate::retention::{ClassifiedArtifact, ProviderResolution};
use crate::views::ProjectBucketKey;

/// One provider's contribution to the atlas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTotals {
    pub provider_id: &'static str,
    pub logical_bytes: u64,
    pub reclaimable_bytes: u64,
    pub artifact_count: usize,
    pub scan_complete: bool,
    pub scan_incomplete_reason: Option<String>,
}

/// One project bucket's totals, including the explicit `Unattributed` case.
#[derive(Debug, PartialEq, Eq)]
pub struct ProjectTotals<'a> {
    pub key: ProjectBucketKey<'a>,
    pub logical_bytes: u64,
    pub reclaimable_bytes: u64,
    pub artifact_count: usize,
}

/// One entry in the top-contributors list - the individually largest artifacts observed, across
/// every provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopContributor<'a> {
    pub artifact_id: &'a ArtifactId,
    pub provider_id: &'a str,
    pub logical_bytes: u64,
}

/// The whole Atlas screen's data (E09-S02's outcome: "total footprint, reclaimable estimates,
/// providers, projects, unattributed state, and top contributors").
#[derive(Debug, PartialEq, Eq)]
pub struct AtlasSummary<'a> {
    pub total_logical_bytes: u64,
    /// AC1: kept as its own field, never merged into `total_logical_bytes` - a caller (the UI)
    /// cannot render these as one blended number even by accident.
    pub total_reclaimable_bytes: u64,
    pub total_artifact_count: usize,
    /// AC2: true the moment *any* provider's scan is not `Complete`. `total_logical_bytes`/
    /// `total_reclaimable_bytes` above still sum whatever *was* observed - incompleteness flags
    /// the totals as a possible undercount, it never hides or zeroes them.
    pub any_incomplete: bool,
    pub providers: Vec<ProviderTotals>,
    pub projects: Vec<ProjectTotals<'a>>,
    pub top_contributors: Vec<TopContributor<'a>>,
}

/// An artifact is "reclaimable" precisely when it would be a real deletion candidate today -
/// the identical test `plan`/`clean` already apply via [`ClassifiedArtifact::reachable_authority`]
/// (see that field's own doc in `retention.rs`). Deliberately not a new predicate invented for
/// this screen: a byte counted here as reclaimable is a byte `cancellai-cli plan` would also
/// propose deleting today.
fn is_reclaimable(classified: &ClassifiedArtifact) -> bool {
    classified.reachable_authority >= minimum_authority_for(ActionClass::Delete)
}

/// Build the Atlas summary from every provider's resolution. `top_n` bounds
/// [`AtlasSummary::top_contributors`] (the UI passes a small constant; a parameter here keeps
/// this function's own tests independent of that choice).
pub fn summarize<'a>(resolutions: &'a [ProviderResolution], top_n: usize) -> AtlasSummary<'a> {
    let mut providers = Vec::with_capacity(resolutions.len());
    let mut project_totals: BTreeMap<Option<&'a str>, (u64, u64, usize)> = BTreeMap::new();
    let mut all_artifacts: Vec<&'a ClassifiedArtifact> = Vec::new();

    let mut total_logical_bytes: u64 = 0;
    let mut total_reclaimable_bytes: u64 = 0;
    let mut any_incomplete = false;

    for resolution in resolutions {
        let mut logical: u64 = 0;
        let mut reclaimable: u64 = 0;
        for classified in resolution.observed() {
            logical += classified.size_bytes;
            let reclaimable_now = is_reclaimable(classified);
            if reclaimable_now {
                reclaimable += classified.size_bytes;
            }

            let key = classified
                .artifact
                .project_attribution
                .as_ref()
                .map(|attribution| attribution.project_ref.0.as_str());
            let entry = project_totals.entry(key).or_insert((0, 0, 0));
            entry.0 += classified.size_bytes;
            if reclaimable_now {
                entry.1 += classified.size_bytes;
            }
            entry.2 += 1;

            all_artifacts.push(classified);
        }

        total_logical_bytes += logical;
        total_reclaimable_bytes += reclaimable;
        any_incomplete |= !resolution.scan_complete();

        providers.push(ProviderTotals {
            provider_id: resolution.provider_id,
            logical_bytes: logical,
            reclaimable_bytes: reclaimable,
            artifact_count: resolution.observed().len(),
            scan_complete: resolution.scan_complete(),
            scan_incomplete_reason: resolution.scan_incomplete_reason(),
        });
    }

    let total_artifact_count = all_artifacts.len();

    let projects = project_totals
        .into_iter()
        .map(
            |(key, (logical_bytes, reclaimable_bytes, artifact_count))| ProjectTotals {
                key: match key {
                    Some(name) => ProjectBucketKey::Attributed(name),
                    None => ProjectBucketKey::Unattributed,
                },
                logical_bytes,
                reclaimable_bytes,
                artifact_count,
            },
        )
        .collect();

    let mut top_contributors: Vec<TopContributor<'a>> = all_artifacts
        .iter()
        .map(|classified| TopContributor {
            artifact_id: &classified.artifact.artifact_id,
            provider_id: classified.artifact.provider_id.as_str(),
            logical_bytes: classified.size_bytes,
        })
        .collect();
    // Largest first; a tie keeps the stable input order (`sort_by_key`, not `sort_unstable_by_key`)
    // so this function's own output is deterministic across otherwise-equal-size artifacts.
    // `Reverse` rather than a comparator: current clippy denies the hand-written descending
    // `sort_by`, and the toolchain this was written against did not.
    top_contributors.sort_by_key(|contributor| Reverse(contributor.logical_bytes));
    top_contributors.truncate(top_n);

    AtlasSummary {
        total_logical_bytes,
        total_reclaimable_bytes,
        total_artifact_count,
        any_incomplete,
        providers,
        projects,
        top_contributors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_inventory::completeness::{CompletenessReason, ReasonLog};
    use cancellai_model::{
        ActivityState, AgentArtifact, AttributionSource, AuthorityLevel, EvidenceId,
        IntegrityState, KnowledgeConfidence, ProjectAttribution, ProjectRef, ProtectionState,
        ResidencyState, Reversibility, RiskClass,
    };
    use std::path::PathBuf;

    #[allow(clippy::too_many_arguments)]
    fn artifact(
        id: &str,
        provider_id: &'static str,
        project: Option<&str>,
        size_bytes: u64,
        reachable_authority: AuthorityLevel,
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
                relationships: Vec::new(),
                project_attribution: project.map(|name| ProjectAttribution {
                    project_ref: ProjectRef::new(name),
                    source: AttributionSource::ExplicitProviderMetadata,
                    confidence: KnowledgeConfidence::Verified,
                }),
                activity_signal: None,
            },
            path: PathBuf::from(format!("/synthetic/{id}")),
            size_bytes,
            reachable_authority,
            binding_constraints: Vec::new(),
        }
    }

    fn delete_minimum() -> AuthorityLevel {
        minimum_authority_for(ActionClass::Delete)
    }

    fn below_delete_minimum() -> AuthorityLevel {
        // `AuthorityLevel::Observe` is the lowest variant (declaration-order `Ord`) and is
        // below every action class's minimum, including `Quarantine`'s.
        AuthorityLevel::Observe
    }

    /// Two providers - one fully scanned with a mix of reclaimable/non-reclaimable artifacts
    /// across two projects plus an unattributed one, the other scanned with a permission error.
    fn mixed_fixture() -> Vec<ProviderResolution> {
        let claude = ProviderResolution::for_test(
            "claude-code",
            vec![
                artifact(
                    "claude-a",
                    "claude-code",
                    Some("proj-x"),
                    1_000,
                    delete_minimum(),
                ),
                artifact(
                    "claude-b",
                    "claude-code",
                    Some("proj-x"),
                    500,
                    below_delete_minimum(),
                ),
                artifact("claude-c", "claude-code", None, 200, delete_minimum()),
            ],
            cancellai_inventory::completeness::ScopeObservation::complete(),
        );

        let mut log = ReasonLog::new();
        log.record(CompletenessReason::PermissionDenied {
            path: PathBuf::from("/synthetic/codex/locked"),
        });
        let codex = ProviderResolution::for_test(
            "codex-cli",
            vec![artifact(
                "codex-a",
                "codex-cli",
                Some("proj-y"),
                10_000,
                delete_minimum(),
            )],
            log.into_observation(),
        );

        vec![claude, codex]
    }

    #[test]
    fn totals_sum_every_artifact_and_never_hide_an_incomplete_scan() {
        let fixture = mixed_fixture();
        let summary = summarize(&fixture, 5);

        assert_eq!(summary.total_artifact_count, 4);
        assert_eq!(summary.total_logical_bytes, 1_000 + 500 + 200 + 10_000);
        // AC2: the incomplete codex scan's own 10_000 bytes still counts toward the total -
        // incompleteness is flagged, not hidden.
        assert!(summary.any_incomplete);
    }

    #[test]
    fn logical_and_reclaimable_totals_are_visually_distinct_fields() {
        let fixture = mixed_fixture();
        let summary = summarize(&fixture, 5);

        // claude-a (1000) + claude-c (200) + codex-a (10000) clear the delete-authority floor;
        // claude-b (500) does not.
        assert_eq!(summary.total_reclaimable_bytes, 1_000 + 200 + 10_000);
        assert_ne!(summary.total_reclaimable_bytes, summary.total_logical_bytes);
    }

    #[test]
    fn per_provider_totals_carry_their_own_completeness() {
        let fixture = mixed_fixture();
        let summary = summarize(&fixture, 5);

        let claude = summary
            .providers
            .iter()
            .find(|p| p.provider_id == "claude-code")
            .expect("claude provider totals must exist");
        assert!(claude.scan_complete);
        assert_eq!(claude.logical_bytes, 1_000 + 500 + 200);

        let codex = summary
            .providers
            .iter()
            .find(|p| p.provider_id == "codex-cli")
            .expect("codex provider totals must exist");
        assert!(!codex.scan_complete);
        assert!(codex.scan_incomplete_reason.is_some());
        assert_eq!(codex.logical_bytes, 10_000);
    }

    #[test]
    fn project_totals_include_an_explicit_unattributed_bucket() {
        let fixture = mixed_fixture();
        let summary = summarize(&fixture, 5);

        let proj_x = summary
            .projects
            .iter()
            .find(|p| p.key == ProjectBucketKey::Attributed("proj-x"))
            .expect("proj-x bucket must exist");
        assert_eq!(proj_x.logical_bytes, 1_000 + 500);
        assert_eq!(proj_x.artifact_count, 2);

        let unattributed = summary
            .projects
            .iter()
            .find(|p| p.key == ProjectBucketKey::Unattributed)
            .expect("AC2: an explicit Unattributed bucket must be present, not dropped");
        assert_eq!(unattributed.logical_bytes, 200);
    }

    #[test]
    fn top_contributors_are_sorted_largest_first_and_bounded_by_top_n() {
        let fixture = mixed_fixture();
        let summary = summarize(&fixture, 2);

        assert_eq!(summary.top_contributors.len(), 2);
        assert_eq!(
            summary.top_contributors[0].artifact_id,
            &ArtifactId::new("codex-a")
        );
        assert_eq!(
            summary.top_contributors[1].artifact_id,
            &ArtifactId::new("claude-a")
        );
    }

    #[test]
    fn a_root_unavailable_scope_is_unknown_not_merely_partial_and_still_flags_incomplete() {
        let mut log = ReasonLog::new();
        log.record_root_unavailable(CompletenessReason::ScopeRootUnavailable {
            path: PathBuf::from("/synthetic/gone"),
            detail: "root does not exist".to_string(),
        });
        let resolution =
            ProviderResolution::for_test("codex-cli", Vec::new(), log.into_observation());
        let summary = summarize(std::slice::from_ref(&resolution), 5);

        assert!(summary.any_incomplete);
        assert_eq!(summary.total_artifact_count, 0);
        assert_eq!(summary.total_logical_bytes, 0);
    }

    #[test]
    fn an_empty_resolution_set_produces_empty_zeroed_totals() {
        let summary = summarize(&[], 5);
        assert_eq!(summary.total_logical_bytes, 0);
        assert_eq!(summary.total_reclaimable_bytes, 0);
        assert_eq!(summary.total_artifact_count, 0);
        assert!(!summary.any_incomplete);
        assert!(summary.providers.is_empty());
        assert!(summary.projects.is_empty());
        assert!(summary.top_contributors.is_empty());
    }
}

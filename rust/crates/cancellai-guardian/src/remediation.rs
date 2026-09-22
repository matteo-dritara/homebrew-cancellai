//! Bounded remediation planner (E15-S03, `docs/architecture/GUARDIAN_MODEL.md` "Decision"/
//! "Authority"; SI-027, SI-028).
//!
//! Guardian never computes its own Effective Authority. [`plan_remediation`] reuses
//! [`RemediationCandidate::reachable_authority`] - the exact value
//! `cancellai_policy::retention::build_actions` (the same pipeline `cancellai-cli` already calls)
//! derived from `cancellai_safety::authority::effective_authority` - and narrows it further with
//! Guardian's own pressure-derived intent ceiling via a plain `std::cmp::min`. `min` over two
//! [`AuthorityLevel`] values can never exceed either input, so **AC1** ("Guardian action
//! authority exactly equals or is below Effective Policy") and **SI-028** ("Guardian actions are
//! equal to or weaker than the Effective Policy computed by the shared engine") hold by
//! construction, not by a second, independently-derived comparison this module could get wrong.
//!
//! **AC2**/**SI-027** ("RED pressure cannot bypass artifact ceiling or unknown-state
//! protections") holds the same way: [`pressure_authority_ceiling`] never returns above
//! [`AuthorityLevel::Quarantine`] for *any* pressure state, `Red` included - `Delete` needs
//! [`AuthorityLevel::Govern`], a ceiling this function never produces, so this planner can never
//! emit a `Delete`-eligible plan regardless of pressure. RED pressure changes how urgently
//! Guardian *wants* to act (reflected only in [`plan_remediation`]'s output ordering, per SI-027's
//! "influences urgency and recommendation ordering"), never what it is *allowed* to do. An
//! artifact whose `reachable_authority` is already capped low by an unknown/protected lifecycle
//! state (`cancellai_safety::authority::lifecycle_ceiling`, folded into `reachable_authority`
//! before this module ever sees it) stays capped there under `min`, no matter how high pressure
//! climbs - this module never re-derives or overrides that computation.

use cancellai_model::{ActionClass, ArtifactId, AuthorityLevel};

use crate::pressure::PressureState;

/// Guardian's own maximum intent for a given pressure state - never a substitute for
/// [`RemediationCandidate::reachable_authority`], only ever combined with it via `min` in
/// [`plan_remediation`]. Deliberately capped at [`AuthorityLevel::Quarantine`] for every pressure
/// state, `Red` included: `Quarantine`/`Restore` are this codebase's actions at that authority
/// floor (`cancellai_safety::authority::minimum_authority_for`), and `Delete` needs
/// [`AuthorityLevel::Govern`] - a ceiling this function never returns.
fn pressure_authority_ceiling(pressure: PressureState) -> AuthorityLevel {
    match pressure {
        PressureState::Green => AuthorityLevel::Observe,
        PressureState::Yellow => AuthorityLevel::Recommend,
        PressureState::Orange | PressureState::Red => AuthorityLevel::Quarantine,
    }
}

/// One artifact Guardian may consider, already classified by the shared policy engine
/// (`cancellai_policy::retention`) - never a path, size, or any other field the normal CLI/TUI
/// classification pipeline carries that this planner has no need for, matching this crate's
/// other detection modules' "never a path or content" convention (`baseline`/`structural`).
///
/// **Fields are deliberately private, with `From<&ClassifiedArtifact>` as the only
/// constructor.** Round-1 independent review found the original `pub` fields let any external
/// crate fabricate `RemediationCandidate { reachable_authority: AuthorityLevel::Autopilot, .. }`
/// directly, with no relationship to any real classification at all - `plan_remediation`'s
/// entire AC1/SI-028 argument ("`min` cannot exceed either input") is only as strong as the
/// trustworthiness of `reachable_authority` itself, and a bare public field asserted nothing
/// about where that value came from. This mirrors the exact failure class
/// `docs/architecture/GUARDIAN_MODEL.md`'s own history describes for provider-layout authority
/// (ADR-0034/ADR-0035/ADR-0036: a plain, publicly constructible value asserting an
/// authority-relevant fact is discardable/forgeable) - closing it here the same way: requiring a
/// real `cancellai_policy::ClassifiedArtifact` (the shared engine's own output type, the same
/// trust boundary `cancellai-cli` already operates under) rather than a bare `AuthorityLevel` a
/// caller could assert unchecked. **Disclosed, not fully closed**: `ClassifiedArtifact` itself
/// remains a plain, publicly constructible struct in `cancellai-policy` - fully sealing it the
/// way ADR-0036 sealed provider-layout observations (a non-forgeable type bound to real I/O) is
/// a larger, `cancellai-policy`-wide decision, out of this story's scope. This fix removes the
/// *additional*, weaker forgery surface this crate's own boundary introduced; it does not (and
/// cannot, from this crate alone) remove the pre-existing one shared with `cancellai-cli`.
///
/// This doctest is the regression proving construction from outside this crate still does not
/// compile (the exact fabrication round-1 independent review demonstrated):
///
/// ```compile_fail
/// # use cancellai_guardian::remediation::RemediationCandidate;
/// # use cancellai_model::{ArtifactId, AuthorityLevel};
/// // RemediationCandidate's fields are private: no struct-literal construction from outside
/// // this crate - `From<&ClassifiedArtifact>` is the only way to produce a value of this type.
/// let forged = RemediationCandidate {
///     artifact_id: ArtifactId::new("fabricated"),
///     reachable_authority: AuthorityLevel::Autopilot,
///     binding_constraints: Vec::new(),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct RemediationCandidate {
    artifact_id: ArtifactId,
    reachable_authority: AuthorityLevel,
    binding_constraints: Vec<&'static str>,
}

impl From<&cancellai_policy::ClassifiedArtifact> for RemediationCandidate {
    fn from(classified: &cancellai_policy::ClassifiedArtifact) -> Self {
        Self {
            artifact_id: classified.artifact.artifact_id.clone(),
            reachable_authority: classified.reachable_authority,
            binding_constraints: classified.binding_constraints.clone(),
        }
    }
}

/// One Guardian plan entry: what Guardian would recommend, or is pre-authorized to do, for one
/// candidate - at the authority this call actually granted it, never bare
/// `reachable_authority` when pressure capped it lower.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardianPlanItem {
    pub artifact_id: ArtifactId,
    /// [`ActionClass::Quarantine`] when `granted_authority` reaches the real minimum that action
    /// needs; [`ActionClass::Observe`] otherwise - a recommendation only, never a plan. This
    /// planner never produces [`ActionClass::Delete`], [`ActionClass::Archive`], or
    /// [`ActionClass::Restore`] - see the module docs for why `Delete` in particular is
    /// structurally unreachable.
    pub action_class: ActionClass,
    pub granted_authority: AuthorityLevel,
    pub pressure_state: PressureState,
    pub binding_constraints: Vec<&'static str>,
}

/// Turn pressure plus already-classified candidates into an ordered plan (SI-027: pressure
/// orders and prioritizes; SI-028: it never grants). Output is sorted so pre-authorized
/// (`Quarantine`) items sort before observation-only ones, and within each group, by descending
/// `granted_authority` - the most-actionable, highest-confidence candidates first. The sort is
/// stable, so candidates tied on both keys keep their relative input order - a caller supplying
/// its own priority pre-order (e.g. by anomaly severity, `cancellai_guardian::baseline`) is not
/// silently reshuffled beyond what authority grouping requires.
pub fn plan_remediation(
    pressure: PressureState,
    candidates: &[RemediationCandidate],
) -> Vec<GuardianPlanItem> {
    let ceiling = pressure_authority_ceiling(pressure);
    let quarantine_minimum =
        cancellai_safety::authority::minimum_authority_for(ActionClass::Quarantine);
    let mut plan: Vec<GuardianPlanItem> = candidates
        .iter()
        .map(|candidate| {
            let granted_authority = std::cmp::min(ceiling, candidate.reachable_authority);
            let action_class = if granted_authority >= quarantine_minimum {
                ActionClass::Quarantine
            } else {
                ActionClass::Observe
            };
            GuardianPlanItem {
                artifact_id: candidate.artifact_id.clone(),
                action_class,
                granted_authority,
                pressure_state: pressure,
                binding_constraints: candidate.binding_constraints.clone(),
            }
        })
        .collect();
    plan.sort_by(|a, b| {
        let a_actionable = a.action_class == ActionClass::Quarantine;
        let b_actionable = b.action_class == ActionClass::Quarantine;
        b_actionable
            .cmp(&a_actionable)
            .then(b.granted_authority.cmp(&a.granted_authority))
    });
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_PRESSURE_STATES: [PressureState; 4] = [
        PressureState::Green,
        PressureState::Yellow,
        PressureState::Orange,
        PressureState::Red,
    ];

    const ALL_AUTHORITY_LEVELS: [AuthorityLevel; 5] = [
        AuthorityLevel::Observe,
        AuthorityLevel::Recommend,
        AuthorityLevel::Quarantine,
        AuthorityLevel::Govern,
        AuthorityLevel::Autopilot,
    ];

    fn candidate(reachable_authority: AuthorityLevel) -> RemediationCandidate {
        RemediationCandidate {
            artifact_id: ArtifactId::new("artifact-0001"),
            reachable_authority,
            binding_constraints: vec!["test_constraint"],
        }
    }

    /// The exhaustive pressure x policy (reachable_authority) matrix the verification contract
    /// names: every one of the 4 pressure states against every one of the 5 `AuthorityLevel`
    /// values a real `reachable_authority` can hold.
    #[test]
    fn exhaustive_pressure_by_reachable_authority_matrix_never_exceeds_either_input() {
        for pressure in ALL_PRESSURE_STATES {
            for reachable in ALL_AUTHORITY_LEVELS {
                let plan = plan_remediation(pressure, &[candidate(reachable)]);
                let item = &plan[0];
                assert!(
                    item.granted_authority <= reachable,
                    "granted authority exceeded reachable_authority at pressure={pressure:?} reachable={reachable:?}"
                );
                assert!(
                    item.granted_authority <= pressure_authority_ceiling(pressure),
                    "granted authority exceeded the pressure ceiling at pressure={pressure:?} reachable={reachable:?}"
                );
                assert_eq!(
                    item.granted_authority,
                    std::cmp::min(pressure_authority_ceiling(pressure), reachable),
                    "granted authority was not the minimum at pressure={pressure:?} reachable={reachable:?}"
                );
            }
        }
    }

    /// AC2, the specific case: RED pressure against every possible `reachable_authority`,
    /// including `Govern`/`Autopilot` (an artifact policy would otherwise let go all the way to
    /// deletion) - granted authority must never cross `Quarantine`.
    #[test]
    fn red_pressure_never_exceeds_quarantine_regardless_of_artifact_ceiling() {
        for reachable in ALL_AUTHORITY_LEVELS {
            let plan = plan_remediation(PressureState::Red, &[candidate(reachable)]);
            assert!(
                plan[0].granted_authority <= AuthorityLevel::Quarantine,
                "RED pressure exceeded Quarantine for reachable={reachable:?}: got {:?}",
                plan[0].granted_authority
            );
        }
    }

    /// AC2, the "unknown-state protections" half: an artifact whose `reachable_authority` is
    /// already capped at `Recommend` (simulating `cancellai_safety::authority::lifecycle_ceiling`
    /// having collapsed it there for `Unknown` activity/protection/integrity) stays at
    /// `Recommend` even at maximum pressure - RED cannot promote it to an actionable
    /// `Quarantine` plan.
    #[test]
    fn red_pressure_cannot_promote_an_unknown_state_artifact_past_its_own_ceiling() {
        let unknown_state_candidate = candidate(AuthorityLevel::Recommend);
        let plan = plan_remediation(PressureState::Red, &[unknown_state_candidate]);
        assert_eq!(plan[0].granted_authority, AuthorityLevel::Recommend);
        assert_eq!(plan[0].action_class, ActionClass::Observe);
    }

    #[test]
    fn action_class_is_quarantine_only_at_or_above_the_real_quarantine_minimum() {
        let quarantine_minimum =
            cancellai_safety::authority::minimum_authority_for(ActionClass::Quarantine);
        for pressure in ALL_PRESSURE_STATES {
            for reachable in ALL_AUTHORITY_LEVELS {
                let plan = plan_remediation(pressure, &[candidate(reachable)]);
                let item = &plan[0];
                if item.granted_authority >= quarantine_minimum {
                    assert_eq!(item.action_class, ActionClass::Quarantine);
                } else {
                    assert_eq!(item.action_class, ActionClass::Observe);
                }
            }
        }
    }

    #[test]
    fn never_produces_delete_archive_or_restore_regardless_of_input() {
        for pressure in ALL_PRESSURE_STATES {
            for reachable in ALL_AUTHORITY_LEVELS {
                let plan = plan_remediation(pressure, &[candidate(reachable)]);
                assert!(matches!(
                    plan[0].action_class,
                    ActionClass::Observe | ActionClass::Quarantine
                ));
            }
        }
    }

    #[test]
    fn green_pressure_never_recommends_or_acts() {
        for reachable in ALL_AUTHORITY_LEVELS {
            let plan = plan_remediation(PressureState::Green, &[candidate(reachable)]);
            assert_eq!(plan[0].granted_authority, AuthorityLevel::Observe);
            assert_eq!(plan[0].action_class, ActionClass::Observe);
        }
    }

    #[test]
    fn plan_is_ordered_actionable_first_then_by_descending_granted_authority() {
        let candidates = vec![
            candidate(AuthorityLevel::Observe),    // stays Observe
            candidate(AuthorityLevel::Autopilot),  // capped to Quarantine at Orange
            candidate(AuthorityLevel::Recommend),  // stays Recommend, Observe class
            candidate(AuthorityLevel::Quarantine), // exactly Quarantine
        ];
        let plan = plan_remediation(PressureState::Orange, &candidates);
        let classes: Vec<ActionClass> = plan.iter().map(|item| item.action_class).collect();
        // The two Quarantine-authority candidates sort first (both actionable), then the two
        // Observe-authority ones - actionable items never sort after non-actionable ones.
        assert_eq!(
            classes,
            vec![
                ActionClass::Quarantine,
                ActionClass::Quarantine,
                ActionClass::Observe,
                ActionClass::Observe,
            ]
        );
    }

    #[test]
    fn empty_candidate_list_produces_an_empty_plan() {
        assert!(plan_remediation(PressureState::Red, &[]).is_empty());
    }

    #[test]
    fn from_classified_artifact_carries_the_real_engine_authority_unmodified() {
        use cancellai_model::{
            ActivityState, AgentArtifact, IntegrityState, KnowledgeConfidence, ProtectionState,
            ResidencyState, Reversibility, RiskClass,
        };
        use cancellai_policy::ClassifiedArtifact;
        use std::path::PathBuf;

        let artifact = AgentArtifact {
            artifact_id: ArtifactId::new("artifact-real-0001"),
            identity_token: "codex:sessions/2026/05/01/rollout-x.jsonl".to_string(),
            provider_id: "codex".to_string(),
            artifact_type: "session".to_string(),
            risk_class: RiskClass::R3Resumable,
            reversibility: Reversibility::Quarantinable,
            knowledge_confidence: KnowledgeConfidence::Verified,
            activity_state: ActivityState::Stale,
            residency_state: ResidencyState::Hot,
            protection_state: ProtectionState::Normal,
            integrity_state: IntegrityState::Healthy,
            authority_ceiling: AuthorityLevel::Govern,
            evidence_ids: Vec::new(),
            relationships: Vec::new(),
            project_attribution: None,
            activity_signal: None,
        };
        let classified = ClassifiedArtifact {
            artifact,
            path: PathBuf::from("/tmp/does-not-need-to-exist"),
            size_bytes: 4096,
            reachable_authority: AuthorityLevel::Govern,
            binding_constraints: vec!["provider_trust_authority"],
        };

        let candidate = RemediationCandidate::from(&classified);
        assert_eq!(candidate.artifact_id, ArtifactId::new("artifact-real-0001"));
        assert_eq!(candidate.reachable_authority, AuthorityLevel::Govern);

        // Even a real, Govern-ceilinged artifact stays capped at Quarantine under Guardian's
        // own RED intent - the end-to-end proof of this module's central claim, exercised
        // through the real `cancellai_policy::ClassifiedArtifact` type, not a hand-built stub.
        let plan = plan_remediation(PressureState::Red, &[candidate]);
        assert_eq!(plan[0].granted_authority, AuthorityLevel::Quarantine);
        assert_eq!(plan[0].action_class, ActionClass::Quarantine);
    }
}

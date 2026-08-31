//! Effective Authority as a monotonic minimum over named constraints (E03-S04,
//! `docs/architecture/DOMAIN_MODEL.md` "Effective Authority", SI-001, SI-007, SI-008,
//! SI-009).
//!
//! ```text
//! EffectiveAuthority = minimum(
//!   UserAuthority, ArtifactAuthorityCeiling, ConfidenceAuthority, ReversibilityAuthority,
//!   LifecycleAuthority, ProviderCapabilityAuthority, ProviderTrustAuthority,
//!   ReleaseChannelAuthority, ConstitutionalSafetyFloor
//! )
//! ```
//!
//! [`compute_effective_authority`] is that formula, generically: a monotonic minimum over
//! whatever [`AuthorityConstraint`]s a caller supplies, plus a deterministic explanation
//! trace naming which constraint(s) bound the result (AC3) - raising any one input can never
//! raise the output past whatever the *other* inputs already cap it at (AC1), because a
//! minimum over a fixed set can only go down or stay the same as any single input rises.
//!
//! [`effective_authority`] wires this up for the constraints this story can build for real
//! today: `UserAuthority`, `ArtifactAuthorityCeiling` (supplied by the caller - deriving a
//! ceiling from `RiskClass` is a classification decision this story does not invent),
//! `ConfidenceAuthority` (from `KnowledgeConfidence`), `LifecycleAuthority` (from
//! `ActivityState`/`ProtectionState`/`IntegrityState`), and (E05-S02)
//! `ProviderTrustAuthority` (from `ProviderTrust`, `docs/PROVIDERS.md` "Trust levels",
//! SI-021) plus an explicit `ConstitutionalSafetyFloor` restating SI-001's own rule as its own
//! always-present constraint (SI-006: known protection is checked in more than one place, on
//! purpose). `ProviderCapabilityAuthority` and `ReleaseChannelAuthority` are not wired in - no
//! capability-classification or release-channel subsystem exists yet to supply them -
//! `compute_effective_authority` needing no redesign to add them is exactly the point of
//! keeping it generic over named constraints rather than a fixed nine-argument function.

use cancellai_model::{
    ActionClass, ActivityState, AuthorityLevel, IntegrityState, KnowledgeConfidence,
    ProtectionState, ProviderTrust, Reversibility,
};

/// One named input to an Effective Authority computation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AuthorityConstraint {
    pub name: &'static str,
    pub ceiling: AuthorityLevel,
}

/// The result of an Effective Authority computation: the level itself, which named
/// constraint(s) actually bound it (there can be more than one, tied at the same minimum -
/// none is hidden), and the full trace that produced it, in the order the caller supplied
/// (deterministic; AC3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EffectiveAuthority {
    pub level: AuthorityLevel,
    pub binding_constraints: Vec<&'static str>,
    pub trace: Vec<AuthorityConstraint>,
}

/// The monotonic minimum over `constraints`, with an explanation trace. Empty input has no
/// constraint to derive a ceiling from and fails closed to [`AuthorityLevel::Observe`] - the
/// weakest level, never treated as "no limit."
pub fn compute_effective_authority(constraints: &[AuthorityConstraint]) -> EffectiveAuthority {
    let level = constraints
        .iter()
        .map(|c| c.ceiling)
        .min()
        .unwrap_or(AuthorityLevel::Observe);
    let binding_constraints = constraints
        .iter()
        .filter(|c| c.ceiling == level)
        .map(|c| c.name)
        .collect();
    EffectiveAuthority {
        level,
        binding_constraints,
        trace: constraints.to_vec(),
    }
}

/// SI-001: confidence below `Verified`/`Observed` is not enough to authorize an autonomous
/// destructive action; `LowUnknown` specifically ("insufficient safety confidence") caps at
/// `Recommend` - the system may say what it would do, but performs nothing itself.
fn confidence_ceiling(confidence: KnowledgeConfidence) -> AuthorityLevel {
    match confidence {
        KnowledgeConfidence::Verified => AuthorityLevel::Autopilot,
        KnowledgeConfidence::Observed => AuthorityLevel::Govern,
        KnowledgeConfidence::Inferred => AuthorityLevel::Quarantine,
        KnowledgeConfidence::LowUnknown => AuthorityLevel::Recommend,
    }
}

/// AC2: `Active`/`Unknown` activity, `Pinned`/`Protected` protection, and `Partial`/
/// `Unknown` integrity each collapse to non-destructive authority (`Recommend`) on their
/// own - this function ORs across all three axes rather than requiring all three to be
/// "bad" at once, since any one of them alone is reason enough (SI-008, SI-009). `Corrupted`
/// integrity is treated the same as `Partial`/`Unknown`: a fact this codebase distrusts
/// enough to call it `Corrupted` is not evidence to act on more confidently than a merely
/// incomplete one.
fn lifecycle_ceiling(
    activity: ActivityState,
    protection: ProtectionState,
    integrity: IntegrityState,
) -> AuthorityLevel {
    let non_destructive = matches!(activity, ActivityState::Active | ActivityState::Unknown)
        || matches!(
            protection,
            ProtectionState::Pinned | ProtectionState::Protected
        )
        || matches!(
            integrity,
            IntegrityState::Partial | IntegrityState::Corrupted | IntegrityState::Unknown
        );
    if non_destructive {
        AuthorityLevel::Recommend
    } else {
        AuthorityLevel::Autopilot
    }
}

/// E05-S02, SI-021 ("Provider manifest trust bounds authority"): the maximum default
/// authority a provider's trust tier alone permits, matching `docs/PROVIDERS.md`'s "Trust
/// levels" table exactly - `Untrusted` caps at `Observe` (so an untrusted manifest cannot
/// even reach `Quarantine`, `minimum_authority_for`'s floor for any mutating action class, let
/// alone `Delete`'s `Govern`), `LocalCustom` at `Quarantine`, `CommunityVerified` at `Govern`
/// (irreversible authority stays opt-in/evidence-gated beyond this default), and
/// `BuiltinVerified` at `Autopilot` (no additional cap from trust alone; other constraints
/// still apply independently).
fn provider_trust_ceiling(trust: ProviderTrust) -> AuthorityLevel {
    match trust {
        ProviderTrust::Untrusted => AuthorityLevel::Observe,
        ProviderTrust::LocalCustom => AuthorityLevel::Quarantine,
        ProviderTrust::CommunityVerified => AuthorityLevel::Govern,
        ProviderTrust::BuiltinVerified => AuthorityLevel::Autopilot,
    }
}

/// SI-001's own rule, restated as its own always-present constraint rather than folded only
/// into `lifecycle_ceiling`/`confidence_ceiling` (SI-006: defense in depth - a future change
/// to either of those must not silently remove this floor along with it).
fn constitutional_safety_floor(
    protection: ProtectionState,
    confidence: KnowledgeConfidence,
) -> AuthorityLevel {
    let protected_or_unverified =
        protection == ProtectionState::Protected || confidence == KnowledgeConfidence::LowUnknown;
    if protected_or_unverified {
        AuthorityLevel::Recommend
    } else {
        AuthorityLevel::Autopilot
    }
}

/// The raw facts `effective_authority` needs. Deliberately not `AgentArtifact` itself, which
/// does not exist yet (E02-S01's skeleton note) - this is exactly the subset of it this
/// story's constraints consume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorityInputs {
    pub user_requested: AuthorityLevel,
    pub artifact_ceiling: AuthorityLevel,
    pub confidence: KnowledgeConfidence,
    pub activity: ActivityState,
    pub protection: ProtectionState,
    pub integrity: IntegrityState,
    pub provider_trust: ProviderTrust,
}

/// Compute Effective Authority from the constraints this story wires up for real (module
/// docs list which of the documented nine inputs these are, and which are not yet wired).
pub fn effective_authority(inputs: AuthorityInputs) -> EffectiveAuthority {
    let constraints = vec![
        AuthorityConstraint {
            name: "user_authority",
            ceiling: inputs.user_requested,
        },
        AuthorityConstraint {
            name: "artifact_authority_ceiling",
            ceiling: inputs.artifact_ceiling,
        },
        AuthorityConstraint {
            name: "confidence_authority",
            ceiling: confidence_ceiling(inputs.confidence),
        },
        AuthorityConstraint {
            name: "lifecycle_authority",
            ceiling: lifecycle_ceiling(inputs.activity, inputs.protection, inputs.integrity),
        },
        AuthorityConstraint {
            name: "provider_trust_authority",
            ceiling: provider_trust_ceiling(inputs.provider_trust),
        },
        AuthorityConstraint {
            name: "constitutional_safety_floor",
            ceiling: constitutional_safety_floor(inputs.protection, inputs.confidence),
        },
    ];
    compute_effective_authority(&constraints)
}

/// The minimum [`AuthorityLevel`] required to perform an [`ActionClass`] at all (SI-020:
/// irreversible actions are stronger-gated). E03 verifier review round 1 found `execute`
/// (E03-S05) performed `Delete` regardless of the plan's recorded authority, including at
/// `AuthorityLevel::Observe` - this is the executor-side check that closes that gap.
/// `Delete` sits above `Quarantine` deliberately: an action this codebase cannot undo
/// requires more than the authority that merely reversible action requires.
pub fn minimum_authority_for(action_class: ActionClass) -> AuthorityLevel {
    match action_class {
        ActionClass::Observe => AuthorityLevel::Observe,
        ActionClass::Quarantine | ActionClass::Archive => AuthorityLevel::Quarantine,
        ActionClass::Delete => AuthorityLevel::Govern,
    }
}

/// Whether a plan's recorded [`Reversibility`] is internally consistent with its
/// [`ActionClass`] (SI-020: irreversible actions cannot be disguised as cleanup metadata).
/// E03 verifier review round 1 found a plan claiming `Reversibility::Quarantinable` while
/// carrying `ActionClass::Delete` was executed as a real, irreversible deletion anyway - the
/// recorded reversibility was never checked against what the action actually does.
pub fn reversibility_allowed(action_class: ActionClass, reversibility: Reversibility) -> bool {
    match action_class {
        ActionClass::Observe => true,
        ActionClass::Quarantine => reversibility == Reversibility::Quarantinable,
        ActionClass::Archive => reversibility == Reversibility::Archivable,
        ActionClass::Delete => reversibility == Reversibility::Irreversible,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_LEVELS: [AuthorityLevel; 5] = [
        AuthorityLevel::Observe,
        AuthorityLevel::Recommend,
        AuthorityLevel::Quarantine,
        AuthorityLevel::Govern,
        AuthorityLevel::Autopilot,
    ];

    fn permissive_inputs(
        user_requested: AuthorityLevel,
        artifact_ceiling: AuthorityLevel,
    ) -> AuthorityInputs {
        AuthorityInputs {
            user_requested,
            artifact_ceiling,
            confidence: KnowledgeConfidence::Verified,
            activity: ActivityState::Idle,
            protection: ProtectionState::Normal,
            integrity: IntegrityState::Healthy,
            provider_trust: ProviderTrust::BuiltinVerified,
        }
    }

    #[test]
    fn ac1_effective_authority_is_exhaustively_the_minimum_of_user_and_ceiling_when_all_else_is_permissive()
     {
        // Table-driven, exhaustive over the 5x5 = 25 combinations of the two AC1 inputs.
        for &user in &ALL_LEVELS {
            for &ceiling in &ALL_LEVELS {
                let result = effective_authority(permissive_inputs(user, ceiling));
                assert_eq!(
                    result.level,
                    user.min(ceiling),
                    "user={user:?} ceiling={ceiling:?} produced {:?}",
                    result.level
                );
            }
        }
    }

    #[test]
    fn ac1_raising_user_authority_never_raises_the_result_above_the_artifact_ceiling() {
        let low_ceiling = AuthorityLevel::Quarantine;
        for &user in &ALL_LEVELS {
            let result = effective_authority(permissive_inputs(user, low_ceiling));
            assert!(
                result.level <= low_ceiling,
                "user={user:?} must never exceed ceiling={low_ceiling:?}, got {:?}",
                result.level
            );
        }
        // And the ceiling is actually reachable when user authority is high enough - this is
        // not vacuously true of a function that always returns the ceiling regardless of
        // user authority.
        let reached =
            effective_authority(permissive_inputs(AuthorityLevel::Autopilot, low_ceiling));
        assert_eq!(reached.level, low_ceiling);
    }

    #[test]
    fn ac2_unknown_activity_collapses_to_non_destructive_even_at_maximum_everything_else() {
        let inputs = AuthorityInputs {
            activity: ActivityState::Unknown,
            ..permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot)
        };
        assert_eq!(effective_authority(inputs).level, AuthorityLevel::Recommend);
    }

    #[test]
    fn ac2_active_activity_collapses_to_non_destructive() {
        let inputs = AuthorityInputs {
            activity: ActivityState::Active,
            ..permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot)
        };
        assert_eq!(effective_authority(inputs).level, AuthorityLevel::Recommend);
    }

    #[test]
    fn ac2_protected_state_collapses_to_non_destructive() {
        let inputs = AuthorityInputs {
            protection: ProtectionState::Protected,
            ..permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot)
        };
        assert_eq!(effective_authority(inputs).level, AuthorityLevel::Recommend);
    }

    #[test]
    fn ac2_pinned_state_collapses_to_non_destructive() {
        let inputs = AuthorityInputs {
            protection: ProtectionState::Pinned,
            ..permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot)
        };
        assert_eq!(effective_authority(inputs).level, AuthorityLevel::Recommend);
    }

    #[test]
    fn ac2_partial_integrity_collapses_to_non_destructive() {
        let inputs = AuthorityInputs {
            integrity: IntegrityState::Partial,
            ..permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot)
        };
        assert_eq!(effective_authority(inputs).level, AuthorityLevel::Recommend);
    }

    #[test]
    fn ac2_unknown_integrity_collapses_to_non_destructive() {
        let inputs = AuthorityInputs {
            integrity: IntegrityState::Unknown,
            ..permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot)
        };
        assert_eq!(effective_authority(inputs).level, AuthorityLevel::Recommend);
    }

    #[test]
    fn ac2_low_unknown_confidence_collapses_to_non_destructive() {
        let inputs = AuthorityInputs {
            confidence: KnowledgeConfidence::LowUnknown,
            ..permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot)
        };
        assert_eq!(effective_authority(inputs).level, AuthorityLevel::Recommend);
    }

    #[test]
    fn ac2_a_fully_permissive_artifact_is_not_vacuously_capped() {
        // Without this, every "collapses to Recommend" test above could be explained by a
        // bug that always returns Recommend - prove the ceiling is actually reachable.
        let inputs = permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot);
        assert_eq!(effective_authority(inputs).level, AuthorityLevel::Autopilot);
    }

    #[test]
    fn ac3_the_trace_is_deterministic_across_repeated_calls_with_the_same_inputs() {
        let inputs = permissive_inputs(AuthorityLevel::Govern, AuthorityLevel::Quarantine);
        let first = effective_authority(inputs);
        let second = effective_authority(inputs);
        assert_eq!(first, second);
    }

    #[test]
    fn ac3_binding_constraints_names_the_single_actual_bottleneck() {
        // artifact_ceiling is the unique minimum here (Quarantine < everything else derived
        // from a fully-permissive artifact and Govern user authority).
        let inputs = permissive_inputs(AuthorityLevel::Govern, AuthorityLevel::Quarantine);
        let result = effective_authority(inputs);
        assert_eq!(
            result.binding_constraints,
            vec!["artifact_authority_ceiling"]
        );
    }

    #[test]
    fn ac3_binding_constraints_names_every_tied_bottleneck_not_just_the_first() {
        // lifecycle_authority and constitutional_safety_floor both collapse to Recommend
        // when protection is Protected - both must be named, not just whichever the
        // computation happens to encounter first.
        let inputs = AuthorityInputs {
            protection: ProtectionState::Protected,
            ..permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot)
        };
        let result = effective_authority(inputs);
        assert_eq!(result.level, AuthorityLevel::Recommend);
        assert_eq!(
            result.binding_constraints,
            vec!["lifecycle_authority", "constitutional_safety_floor"]
        );
    }

    #[test]
    fn e05s02_ac1_untrusted_provider_trust_collapses_to_observe_even_at_maximum_everything_else() {
        // SI-021: an untrusted provider must not be able to produce even a reversible
        // mutating action, let alone an irreversible one - Observe is strictly below
        // `minimum_authority_for(ActionClass::Quarantine)`.
        let inputs = AuthorityInputs {
            provider_trust: ProviderTrust::Untrusted,
            ..permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot)
        };
        let result = effective_authority(inputs);
        assert_eq!(result.level, AuthorityLevel::Observe);
        assert!(result.level < minimum_authority_for(ActionClass::Quarantine));
        assert!(result.level < minimum_authority_for(ActionClass::Delete));
    }

    #[test]
    fn e05s02_ac1_local_custom_trust_can_quarantine_but_not_delete() {
        let inputs = AuthorityInputs {
            provider_trust: ProviderTrust::LocalCustom,
            ..permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot)
        };
        let result = effective_authority(inputs);
        assert_eq!(result.level, AuthorityLevel::Quarantine);
        assert!(result.level >= minimum_authority_for(ActionClass::Quarantine));
        assert!(result.level < minimum_authority_for(ActionClass::Delete));
    }

    #[test]
    fn e05s02_ac1_community_verified_trust_can_delete_but_is_not_unbounded() {
        // PROVIDERS.md: Community Verified defaults to Govern; Autopilot stays out of reach
        // from trust alone even when every other input is maximally permissive.
        let inputs = AuthorityInputs {
            provider_trust: ProviderTrust::CommunityVerified,
            ..permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot)
        };
        let result = effective_authority(inputs);
        assert_eq!(result.level, AuthorityLevel::Govern);
        assert!(result.level >= minimum_authority_for(ActionClass::Delete));
        assert!(result.level < AuthorityLevel::Autopilot);
    }

    #[test]
    fn e05s02_builtin_verified_trust_does_not_cap_below_a_fully_permissive_result() {
        // Not vacuously true: proves BuiltinVerified's ceiling is actually Autopilot, not a
        // bug that happens to also produce Observe/Quarantine/Govern here.
        let inputs = permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot);
        assert_eq!(effective_authority(inputs).level, AuthorityLevel::Autopilot);
    }

    #[test]
    fn e05s02_ac3_provider_trust_authority_is_named_when_it_is_the_unique_bottleneck() {
        let inputs = AuthorityInputs {
            provider_trust: ProviderTrust::LocalCustom,
            ..permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot)
        };
        let result = effective_authority(inputs);
        assert_eq!(result.binding_constraints, vec!["provider_trust_authority"]);
    }

    #[test]
    fn compute_effective_authority_fails_closed_on_empty_input() {
        assert_eq!(
            compute_effective_authority(&[]).level,
            AuthorityLevel::Observe
        );
    }

    #[test]
    fn delete_requires_more_authority_than_quarantine() {
        // SI-020: irreversible actions are stronger-gated than merely reversible ones.
        assert!(
            minimum_authority_for(ActionClass::Delete)
                > minimum_authority_for(ActionClass::Quarantine)
        );
    }

    #[test]
    fn e03_verifier_round1_observe_authority_cannot_satisfy_delete() {
        // The exact counterexample the round-1 review used: AuthorityLevel::Observe with
        // ActionClass::Delete must never meet the required minimum.
        assert!(AuthorityLevel::Observe < minimum_authority_for(ActionClass::Delete));
    }

    #[test]
    fn reversibility_allowed_rejects_delete_claimed_as_quarantinable() {
        // The exact counterexample the round-1 review used.
        assert!(!reversibility_allowed(
            ActionClass::Delete,
            Reversibility::Quarantinable
        ));
    }

    #[test]
    fn reversibility_allowed_accepts_the_matching_pair_for_each_mutating_class() {
        assert!(reversibility_allowed(
            ActionClass::Delete,
            Reversibility::Irreversible
        ));
        assert!(reversibility_allowed(
            ActionClass::Quarantine,
            Reversibility::Quarantinable
        ));
        assert!(reversibility_allowed(
            ActionClass::Archive,
            Reversibility::Archivable
        ));
    }

    #[test]
    fn reversibility_allowed_rejects_every_mismatched_pair() {
        let classes = [
            ActionClass::Quarantine,
            ActionClass::Archive,
            ActionClass::Delete,
        ];
        let reversibilities = [
            Reversibility::Rebuildable,
            Reversibility::Quarantinable,
            Reversibility::Archivable,
            Reversibility::VendorConditional,
            Reversibility::Irreversible,
            Reversibility::Unknown,
        ];
        for &class in &classes {
            for &reversibility in &reversibilities {
                let expected_match = match class {
                    ActionClass::Quarantine => reversibility == Reversibility::Quarantinable,
                    ActionClass::Archive => reversibility == Reversibility::Archivable,
                    ActionClass::Delete => reversibility == Reversibility::Irreversible,
                    ActionClass::Observe => true,
                };
                assert_eq!(
                    reversibility_allowed(class, reversibility),
                    expected_match,
                    "class={class:?} reversibility={reversibility:?}"
                );
            }
        }
    }
}

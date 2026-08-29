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
//! `ConfidenceAuthority` (from `KnowledgeConfidence`), and `LifecycleAuthority` (from
//! `ActivityState`/`ProtectionState`/`IntegrityState`) plus an explicit
//! `ConstitutionalSafetyFloor` restating SI-001's own rule as its own always-present
//! constraint (SI-006: known protection is checked in more than one place, on purpose).
//! `ProviderCapabilityAuthority`, `ProviderTrustAuthority`, and `ReleaseChannelAuthority` are
//! not wired in - no provider-adapter or release-channel subsystem exists yet to supply them
//! (E05, a later release story) - `compute_effective_authority` needing no redesign to add
//! them is exactly the point of keeping it generic over named constraints rather than a fixed
//! nine-argument function.

use cancellai_model::{
    ActivityState, AuthorityLevel, IntegrityState, KnowledgeConfidence, ProtectionState,
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
            name: "constitutional_safety_floor",
            ceiling: constitutional_safety_floor(inputs.protection, inputs.confidence),
        },
    ];
    compute_effective_authority(&constraints)
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
    fn compute_effective_authority_fails_closed_on_empty_input() {
        assert_eq!(
            compute_effective_authority(&[]).level,
            AuthorityLevel::Observe
        );
    }
}

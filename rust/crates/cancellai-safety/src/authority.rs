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
//! `ProviderTrustAuthority` (from [`crate::TrustedTier`], `docs/PROVIDERS.md` "Trust levels",
//! SI-021 - `AuthorityInputs::provider_trust` deliberately takes the opaque `TrustedTier`, not
//! a bare `cancellai_model::ProviderTrust`, so this constraint cannot be supplied by an
//! external caller constructing an arbitrary trust value directly; see `trust_promotion.rs`'s
//! module doc for the E05 verifier round 1 defect this closes) plus an explicit
//! `ConstitutionalSafetyFloor` restating SI-001's own rule as its own always-present constraint
//! (SI-006: known protection is checked in more than one place, on purpose).
//!
//! `ProviderCapabilityAuthority` (E14-S04, SI-004) does **not** live on [`AuthorityInputs`] at
//! all, unlike the eight constraints above - it is the one constraint [`effective_authority`]
//! never computes. Four independent review rounds across three designs (ADR-0034, ADR-0035,
//! `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND3.md`,
//! `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND4.md`) each found a way a caller holding a
//! real drift observation could still reach an authority level the drift was supposed to cap,
//! because in every prior design the layout fact - first a bare recommendation, then a
//! caller-asserted ceiling, then the raw observation itself - was carried on `AuthorityInputs`,
//! a plain, publicly constructible value. Nothing stopped a caller from supplying that fact
//! honestly once and then, in a second, independently constructed `AuthorityInputs`, omitting or
//! contradicting it while still holding the real finding: no field shape closes that, because
//! the type doing the asserting has no binding to the real-world object it claims to describe.
//!
//! ADR-0036 (round 5) removes the layout fact from `AuthorityInputs` entirely rather than
//! attempting a fourth field shape. [`effective_authority`] is now purely an *analysis*
//! computation over the eight constraints above - honest for the two production call sites that
//! have never had a live layout observation to supply, and structurally incapable of expressing
//! one at all, so there is nothing left for a caller to discard. A real layout fact instead
//! reaches authority only through [`resolve_provider_execution_authority`], which requires a
//! [`cancellai_platform::BoundLayoutObservation`] - built exclusively from real directory I/O
//! against a real path, never from caller-asserted marker strings - and returns an opaque
//! [`ProviderExecutionPermit`] with no public constructor a caller could fabricate to stand in
//! for one. See `provider_layout.rs`'s own module doc for the full account, and ADR-0036 for the
//! disclosed residuals this round leaves open (no trusted source of "known-recognized" layouts
//! yet, and no consuming call site at the mutation boundary yet - this round establishes the
//! non-forgeable primitive, not the live wiring).
//!
//! [`effective_authority_for_channel`] (E17-S05, SI-030) adds the ninth: `ReleaseChannelAuthority`
//! (from [`crate::BuildChannel`], `docs/security/SUPPLY_CHAIN.md` "Release channels" -
//! deliberately the opaque `BuildChannel`, not a bare `cancellai_model::ReleaseChannel`,
//! mirroring `provider_trust`'s split for the identical reason: a caller must not be able to
//! claim "stable" from a nightly binary by constructing the vocabulary type directly). It is a
//! separate function from [`effective_authority`], not a new required field on
//! [`AuthorityInputs`], deliberately: `AuthorityInputs` already has real production callers
//! (`cancellai-policy::retention::reachable_authority`) that predate release-channel awareness
//! and are not yet wired to supply one - `cancellai-cli` remains a beta, source-built artifact
//! with no packaged release yet (`docs/RELEASING.md`'s "Beta side-by-side" section), so binding
//! its classification pipeline to a real per-build channel is deferred to whichever story wires
//! `cancellai-cli` into the actual release/cutover path (E06-S04). Adding a *required* field to
//! `AuthorityInputs` today would force every existing caller to supply a value that has no
//! honest answer yet, and the only "safe" placeholder (`BuildChannel::default()`, i.e.
//! `Nightly`) would silently cap every one of those callers' existing, already-verified
//! Delete-reaching test scenarios at `Recommend` - a large, misleading blast radius for a
//! constraint nothing yet asks those callers to enforce. `effective_authority_for_channel`
//! keeps the constraint fully implemented and tested here, ready for that future caller to
//! adopt by construction (it cannot forget to combine it correctly, since it calls
//! `effective_authority` itself and only ever narrows the result), without disturbing anyone
//! who has not opted in.

use cancellai_model::{
    ActionClass, ActivityState, AuthorityLevel, IntegrityState, KnowledgeConfidence,
    ProtectionState, ProviderTrust, ReleaseChannel, Reversibility,
};

use crate::build_channel::BuildChannel;
use crate::provider_layout::LayoutSignature;
use crate::trust_promotion::TrustedTier;

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

/// E17-S05, SI-030: the maximum default authority a release channel alone permits, matching
/// `docs/security/SUPPLY_CHAIN.md`'s "Release channels" table - `Stable` carries no additional
/// cap from channel alone (the highest verified default authority a build can claim; other
/// constraints still apply independently), `Beta` caps at `Govern` (reduced autonomous
/// defaults - it can still reach a real, confirmed `Delete`, but not unattended `Autopilot`),
/// and `Nightly` caps at `Recommend` - strictly below `minimum_authority_for(ActionClass::
/// Quarantine)`, let alone `Delete`'s `Govern`, so a nightly build can never reach even
/// `Quarantine` by channel default alone, matching "Observe/Recommend oriented by default" and
/// this story's own AC: "Nightly defaults cannot execute stable-equivalent irreversible
/// autonomy."
fn release_channel_ceiling(channel: ReleaseChannel) -> AuthorityLevel {
    match channel {
        ReleaseChannel::Stable => AuthorityLevel::Autopilot,
        ReleaseChannel::Beta => AuthorityLevel::Govern,
        ReleaseChannel::Nightly => AuthorityLevel::Recommend,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityInputs {
    pub user_requested: AuthorityLevel,
    pub artifact_ceiling: AuthorityLevel,
    pub confidence: KnowledgeConfidence,
    pub activity: ActivityState,
    pub protection: ProtectionState,
    pub integrity: IntegrityState,
    pub provider_trust: TrustedTier,
}

/// The constraint list [`effective_authority`] and [`effective_authority_for_channel`] share -
/// factored out so the latter cannot drift from the former by re-deriving these six by hand.
fn base_constraints(inputs: &AuthorityInputs) -> Vec<AuthorityConstraint> {
    vec![
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
            ceiling: provider_trust_ceiling(inputs.provider_trust.level()),
        },
        AuthorityConstraint {
            name: "constitutional_safety_floor",
            ceiling: constitutional_safety_floor(inputs.protection, inputs.confidence),
        },
    ]
}

/// The authority ceiling a real layout observation implies, computed identically for every
/// caller: `None` for a layout matching one of `known_signatures` (no additional constraint -
/// the other base constraints alone still apply), otherwise `Some(AuthorityLevel::Observe)`, the
/// lowest ceiling this vocabulary expresses, including when `known_signatures` is empty or
/// `observed` carries no markers at all, so an absent or inconclusive comparison is drift, never
/// a default pass (E14-S04, SI-004).
///
/// Not `pub`: [`resolve_provider_execution_authority`] is the only caller. There is no path
/// from outside this crate to a pre-computed ceiling - a caller supplies the raw observation
/// (via [`cancellai_platform::BoundLayoutObservation`]) and `known_signatures`, never a ceiling
/// value to assert, discard, or disagree with.
fn layout_ceiling(
    known_signatures: &[LayoutSignature],
    observed: &LayoutSignature,
) -> Option<AuthorityLevel> {
    if known_signatures.iter().any(|known| known == observed) {
        None
    } else {
        Some(AuthorityLevel::Observe)
    }
}

/// An authority result bound to one real, non-forgeable observation of one provider root -
/// never constructible from caller-supplied facts, and never equal to, or convertible from, a
/// plain [`EffectiveAuthority`]. This is what closes E14-S04 round 4's finding: the *type*
/// [`effective_authority`] returns is now structurally incapable of authorizing a
/// layout-governed action at all, so there is no longer a plain, publicly constructible value a
/// caller could substitute for a real permit while claiming to hold one.
///
/// `root_identity` is carried so a future consumer at the mutation boundary can refuse a permit
/// whose root has since changed underneath it - the same binding
/// `cancellai-platform::mutation::confirmed_delete_file_inner` already applies to a single file.
/// No such consumer exists yet (ADR-0036's disclosed residual): this round establishes the
/// non-forgeable primitive, not the live wiring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderExecutionPermit {
    root_identity: cancellai_platform::IdentityToken,
    level: AuthorityLevel,
    binding_constraints: Vec<&'static str>,
}

impl ProviderExecutionPermit {
    pub fn root_identity(&self) -> &cancellai_platform::IdentityToken {
        &self.root_identity
    }

    pub fn level(&self) -> AuthorityLevel {
        self.level
    }

    pub fn binding_constraints(&self) -> &[&'static str] {
        &self.binding_constraints
    }
}

/// The only production path from a real [`cancellai_platform::BoundLayoutObservation`] to an
/// authority decision (E14-S04 round 5, SI-004, ADR-0036). `base` carries the same eight
/// constraints [`effective_authority`] computes - there is no separate "execution" input shape
/// to keep in sync, since `AuthorityInputs` no longer carries a layout field for one to differ
/// on. `known_signatures` is the closed set of layouts this caller currently recognizes;
/// `observation` is a real, unforgeable reading of the root actually being authorized.
///
/// Disclosed residual (ADR-0036, matching ADR-0034/ADR-0035's own precedent): `known_signatures`
/// is still caller-supplied data, not yet drawn from a trust-bounded provider manifest - wiring
/// a real "what does a recognized layout for this provider look like" source is future
/// orchestrator work, the same residual ADR-0034 disclosed for live layout wiring generally. No
/// production caller supplies a non-empty `known_signatures` today, so every current call
/// resolves to `AuthorityLevel::Observe` for the layout constraint - a safe, honest default, not
/// a silently invented "recognized" answer.
pub fn resolve_provider_execution_authority(
    base: AuthorityInputs,
    observation: &cancellai_platform::BoundLayoutObservation,
    known_signatures: &[LayoutSignature],
) -> ProviderExecutionPermit {
    let mut constraints = base_constraints(&base);
    let observed = LayoutSignature::new(observation.markers().iter().cloned());
    if let Some(ceiling) = layout_ceiling(known_signatures, &observed) {
        constraints.push(AuthorityConstraint {
            name: "provider_capability_authority",
            ceiling,
        });
    }
    let result = compute_effective_authority(&constraints);
    ProviderExecutionPermit {
        root_identity: observation.root_identity().clone(),
        level: result.level,
        binding_constraints: result.binding_constraints,
    }
}

/// Compute Effective Authority from the constraints this story wires up for real (module
/// docs list which of the documented nine inputs these are, and which are not yet wired).
pub fn effective_authority(inputs: AuthorityInputs) -> EffectiveAuthority {
    compute_effective_authority(&base_constraints(&inputs))
}

/// [`effective_authority`], plus the release-channel ceiling (E17-S05, SI-030) - see the
/// module doc for why this is a separate function rather than a new required field on
/// [`AuthorityInputs`]. A caller ready to bind its authority computation to the build's real
/// release channel calls this instead of [`effective_authority`]; nothing else changes.
pub fn effective_authority_for_channel(
    inputs: AuthorityInputs,
    channel: BuildChannel,
) -> EffectiveAuthority {
    let mut constraints = base_constraints(&inputs);
    constraints.push(AuthorityConstraint {
        name: "release_channel_authority",
        ceiling: release_channel_ceiling(channel.level()),
    });
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
        // Restore is the reverse of Quarantine (E12-S02) - undoing a quarantine is no more
        // dangerous than performing one, so it sits at the same floor.
        ActionClass::Quarantine | ActionClass::Archive | ActionClass::Restore => {
            AuthorityLevel::Quarantine
        }
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
        // A restored artifact can itself be quarantined again if the restore turns out to be
        // wrong (E12-S02) - the same recoverability `Quarantine` itself claims.
        ActionClass::Quarantine | ActionClass::Restore => {
            reversibility == Reversibility::Quarantinable
        }
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
            provider_trust: TrustedTier::for_tests(ProviderTrust::BuiltinVerified),
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
        let first = effective_authority(inputs.clone());
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
            provider_trust: TrustedTier::for_tests(ProviderTrust::Untrusted),
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
            provider_trust: TrustedTier::for_tests(ProviderTrust::LocalCustom),
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
            provider_trust: TrustedTier::for_tests(ProviderTrust::CommunityVerified),
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
            provider_trust: TrustedTier::for_tests(ProviderTrust::LocalCustom),
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
        assert!(reversibility_allowed(
            ActionClass::Restore,
            Reversibility::Quarantinable
        ));
    }

    #[test]
    fn reversibility_allowed_rejects_every_mismatched_pair() {
        let classes = [
            ActionClass::Quarantine,
            ActionClass::Archive,
            ActionClass::Delete,
            ActionClass::Restore,
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
                    ActionClass::Quarantine | ActionClass::Restore => {
                        reversibility == Reversibility::Quarantinable
                    }
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

    // --- E17-S05, SI-030: release channel bounds default authority -----------------------
    // Uses effective_authority_for_channel, not effective_authority - see the module doc for
    // why release_channel is a separate opt-in function rather than a required AuthorityInputs
    // field.

    #[test]
    fn e17s05_nightly_channel_collapses_to_non_destructive_even_at_maximum_everything_else() {
        let inputs = permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot);
        let result = effective_authority_for_channel(
            inputs,
            BuildChannel::for_tests(ReleaseChannel::Nightly),
        );
        assert_eq!(result.level, AuthorityLevel::Recommend);
    }

    #[test]
    fn e17s05_ac_nightly_can_never_reach_the_authority_delete_or_even_quarantine_requires() {
        // The story's own AC, stated as a direct proof rather than a single example: a nightly
        // build's channel ceiling alone is strictly below what ActionClass::Quarantine needs,
        // let alone Delete - so no other permissive input can ever let a nightly build reach
        // either, exactly "cannot execute stable-equivalent irreversible autonomy."
        let inputs = permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot);
        let result = effective_authority_for_channel(
            inputs,
            BuildChannel::for_tests(ReleaseChannel::Nightly),
        );
        assert!(result.level < minimum_authority_for(ActionClass::Quarantine));
        assert!(result.level < minimum_authority_for(ActionClass::Delete));
    }

    #[test]
    fn e17s05_beta_channel_can_reach_delete_but_not_unattended_autopilot() {
        let inputs = permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot);
        let result =
            effective_authority_for_channel(inputs, BuildChannel::for_tests(ReleaseChannel::Beta));
        assert_eq!(result.level, AuthorityLevel::Govern);
        assert!(result.level >= minimum_authority_for(ActionClass::Delete));
        assert!(result.level < AuthorityLevel::Autopilot);
    }

    #[test]
    fn e17s05_stable_channel_does_not_cap_below_a_fully_permissive_result() {
        // Not vacuously true: proves Stable's ceiling is actually Autopilot (no additional cap
        // from channel alone), not a bug that happens to also produce a lower level here.
        let inputs = permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot);
        let result = effective_authority_for_channel(
            inputs,
            BuildChannel::for_tests(ReleaseChannel::Stable),
        );
        assert_eq!(result.level, AuthorityLevel::Autopilot);
    }

    #[test]
    fn e17s05_a_fresh_default_build_channel_is_nightly_and_collapses_the_same_way() {
        // BuildChannel::default() must be exactly as restrictive as an explicit Nightly -
        // proves the fail-closed default has teeth in a real computation, not only in
        // build_channel.rs's own unit tests.
        let inputs = permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot);
        let result = effective_authority_for_channel(inputs, BuildChannel::default());
        assert_eq!(result.level, AuthorityLevel::Recommend);
    }

    #[test]
    fn e17s05_raising_user_authority_never_raises_a_nightly_build_past_its_channel_ceiling() {
        // Mirrors ac1_raising_user_authority_never_raises_the_result_above_the_artifact_ceiling
        // above, for the new constraint: SI-030's whole point is that user-side configuration
        // cannot buy back authority a nightly build's channel does not grant by default.
        for &user in &ALL_LEVELS {
            let inputs = permissive_inputs(user, AuthorityLevel::Autopilot);
            let result = effective_authority_for_channel(
                inputs,
                BuildChannel::for_tests(ReleaseChannel::Nightly),
            );
            assert!(
                result.level <= AuthorityLevel::Recommend,
                "user={user:?} must never exceed the Nightly channel ceiling, got {:?}",
                result.level
            );
        }
    }

    #[test]
    fn e17s05_ac3_release_channel_authority_is_named_when_it_is_the_unique_bottleneck() {
        let inputs = permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot);
        let result = effective_authority_for_channel(
            inputs,
            BuildChannel::for_tests(ReleaseChannel::Nightly),
        );
        assert_eq!(
            result.binding_constraints,
            vec!["release_channel_authority"]
        );
    }

    #[test]
    fn e17s05_effective_authority_for_channel_agrees_with_effective_authority_when_channel_is_not_the_bottleneck()
     {
        // The two functions must never silently diverge on the six shared constraints - proven
        // directly rather than assumed from both calling base_constraints internally.
        let inputs = permissive_inputs(AuthorityLevel::Quarantine, AuthorityLevel::Quarantine);
        let plain = effective_authority(inputs.clone());
        let with_channel = effective_authority_for_channel(
            inputs,
            BuildChannel::for_tests(ReleaseChannel::Stable),
        );
        assert_eq!(plain.level, with_channel.level);
        assert_eq!(plain.binding_constraints, with_channel.binding_constraints);
    }

    #[test]
    fn e17s05_release_channel_ceiling_matches_the_documented_table_exactly() {
        assert_eq!(
            release_channel_ceiling(ReleaseChannel::Stable),
            AuthorityLevel::Autopilot
        );
        assert_eq!(
            release_channel_ceiling(ReleaseChannel::Beta),
            AuthorityLevel::Govern
        );
        assert_eq!(
            release_channel_ceiling(ReleaseChannel::Nightly),
            AuthorityLevel::Recommend
        );
    }

    // --- E14-S04, SI-004, ADR-0036 (round 5): provider-capability/layout drift bounds
    // authority only through a real, non-forgeable `cancellai_platform::BoundLayoutObservation`
    // and the opaque `ProviderExecutionPermit` `resolve_provider_execution_authority` mints from
    // it - `AuthorityInputs`/`effective_authority` no longer carry or compute a layout fact at
    // all. Four independent review rounds across three prior designs (ADR-0034, ADR-0035, round
    // 3, round 4) each found a way a caller-supplied layout fact - a bare recommendation, then a
    // ceiling, then the raw observation itself - was discardable in a second, independently
    // constructed value. This round closes it by removing the field, not by reshaping it again.

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let base = std::fs::canonicalize(std::env::temp_dir())
                .unwrap_or_else(|_| std::env::temp_dir());
            let dir = base.join(format!(
                "cancellai-safety-authority-test-{label}-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    /// A real observation of `dir` - the only way this crate's own tests can obtain a
    /// [`cancellai_platform::BoundLayoutObservation`], for the same reason no other crate can:
    /// its only constructor performs real directory I/O.
    fn observe(dir: &TempDir) -> cancellai_platform::BoundLayoutObservation {
        cancellai_platform::BoundLayoutObservation::observe(&dir.0)
            .expect("a real, existing temp dir must observe cleanly")
    }

    #[test]
    fn e14s04_a_real_drifted_observation_caps_the_permit_at_observe_even_at_maximum_everything_else()
     {
        let dir = TempDir::new("drifted");
        std::fs::create_dir_all(dir.0.join("totally_different_shape")).unwrap();
        let observation = observe(&dir);
        let known = vec![LayoutSignature::new(["sessions/".to_string()])];

        let permit = resolve_provider_execution_authority(
            permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot),
            &observation,
            &known,
        );
        assert_eq!(permit.level(), AuthorityLevel::Observe);
        assert_eq!(
            permit.binding_constraints().to_vec(),
            vec!["provider_capability_authority"]
        );
    }

    #[test]
    fn e14s04_a_recognized_observation_does_not_cap_below_a_fully_permissive_result() {
        // Not vacuously true: proves a recognized layout adds no constraint at all, not a bug
        // that happens to also produce a high level here.
        let dir = TempDir::new("recognized");
        std::fs::create_dir_all(dir.0.join("sessions")).unwrap();
        let observation = observe(&dir);
        let known = vec![LayoutSignature::new(["sessions/".to_string()])];

        let permit = resolve_provider_execution_authority(
            permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot),
            &observation,
            &known,
        );
        assert_eq!(permit.level(), AuthorityLevel::Autopilot);
    }

    #[test]
    fn e14s04_raising_user_authority_never_raises_past_the_drifted_layout_ceiling() {
        let dir = TempDir::new("drifted-all-users");
        std::fs::create_dir_all(dir.0.join("totally_different_shape")).unwrap();
        let observation = observe(&dir);
        let known = vec![LayoutSignature::new(["sessions/".to_string()])];

        for &user in &ALL_LEVELS {
            let permit = resolve_provider_execution_authority(
                permissive_inputs(user, AuthorityLevel::Autopilot),
                &observation,
                &known,
            );
            assert_eq!(
                permit.level(),
                AuthorityLevel::Observe,
                "user={user:?} must never exceed the drifted-layout ceiling, got {:?}",
                permit.level()
            );
        }
    }

    #[test]
    fn e14s04_round4_a_permissive_analysis_result_is_never_a_permit() {
        // The exact counterexample round 4 independent review used: a caller retains a real
        // drifted finding, then separately builds an `AuthorityInputs`/`effective_authority`
        // computation over the same base inputs and reaches `Autopilot`. That computation still
        // exists - "what would this be if layout were not in question?" is still a legitimate,
        // honest analysis question, and still legitimately returns `Autopilot` here. What round
        // 4 found missing was a way to stop that analysis result from being usable as if it
        // authorized a layout-governed action. It now cannot be: `EffectiveAuthority` and
        // `ProviderExecutionPermit` are different, unrelated types with no conversion between
        // them, and the only way to obtain a `ProviderExecutionPermit` at all is
        // `resolve_provider_execution_authority`, which requires a real
        // `BoundLayoutObservation` by value - there is no `Option`, no `NotAssessed`, no second
        // call that omits it.
        let dir = TempDir::new("round4-drifted");
        std::fs::create_dir_all(dir.0.join("totally_different_shape")).unwrap();
        let observation = observe(&dir);
        let known = vec![LayoutSignature::new(["sessions/".to_string()])];
        let base = permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot);

        let analysis = effective_authority(base.clone());
        assert_eq!(
            analysis.level,
            AuthorityLevel::Autopilot,
            "test setup: the analysis-only computation, which never mentions layout, must reach \
             the top level here, or this test would not be proving anything about the permit \
             path"
        );

        let permit = resolve_provider_execution_authority(base, &observation, &known);
        assert_eq!(
            permit.level(),
            AuthorityLevel::Observe,
            "a real drifted observation must cap the permit even though the analysis-only \
             computation over the identical base inputs reaches Autopilot"
        );
    }

    #[test]
    fn e14s04_empty_known_signatures_never_recognizes_anything() {
        let dir = TempDir::new("empty-known");
        std::fs::write(dir.0.join("config.json"), b"{}").unwrap();
        let observation = observe(&dir);

        let permit = resolve_provider_execution_authority(
            permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot),
            &observation,
            &[],
        );
        assert_eq!(permit.level(), AuthorityLevel::Observe);
    }

    #[test]
    fn e14s04_the_permit_carries_the_observed_roots_own_identity() {
        let dir = TempDir::new("identity");
        let observation = observe(&dir);
        let permit = resolve_provider_execution_authority(
            permissive_inputs(AuthorityLevel::Autopilot, AuthorityLevel::Autopilot),
            &observation,
            &[],
        );
        assert_eq!(permit.root_identity(), observation.root_identity());
    }
}

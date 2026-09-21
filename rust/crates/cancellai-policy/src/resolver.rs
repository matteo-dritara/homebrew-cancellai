//! The constraint resolver (E11-S02, `docs/architecture/POLICY_MODEL.md`'s "Constitutional
//! precedence" and "Scope hierarchy"): turns a parsed [`crate::schema::PolicyDocument`] plus
//! one artifact/action's identity into the single [`AuthorityLevel`] the *user* is asking for,
//! deterministically.
//!
//! **This module computes no ceiling of its own.** [`resolve_effective_authority`] folds its
//! result into `cancellai_safety::authority::AuthorityInputs::user_requested` and calls the
//! already-verified [`cancellai_safety::effective_authority`] (E03-S04) - the same monotonic
//! minimum over named constraints every other caller in this workspace uses. This is what
//! discharges SI-025 ("Policy cannot override constitutional ceilings") and this story's AC1/
//! AC2 *by construction* rather than by new logic: whatever [`resolve_requested_authority`]
//! returns is just one more input to a minimum, and a minimum over a fixed set of *other*
//! independent constraints (artifact ceiling, confidence, lifecycle, provider trust, the
//! constitutional safety floor) can never be raised past whatever those already cap it at, no
//! matter how permissive the policy-supplied input is. Nothing in this module reaches those
//! other constraints, reads them, or has any way to influence them - the "second path to a
//! safety decision" `docs/adrs/0019-dependency-rings-per-crate.md` warns against does not exist
//! here because there is nothing here that *could* decide permission, only what was *asked for*.
//!
//! ## Scope ladder
//!
//! [`resolve_requested_authority`] walks `docs/architecture/POLICY_MODEL.md`'s scope hierarchy
//! most-specific-first - `ARTIFACT_TYPE -> PROJECT -> PROVIDER -> MACHINE -> GLOBAL` - and
//! returns the first scope that actually sets an `authority` ([`ScopePolicy::authority`]
//! `Some`, not merely present: an empty narrowing defers to the next rung down, matching that
//! type's own doc). Because [`PolicyDocument`]'s scope maps are keyed uniquely (JSON object
//! keys) and the ladder order is fixed, exactly one scope can ever win for a given
//! [`PolicyContext`] - the same document and context always resolve to the same
//! [`ResolvedRequest`] (AC "Conflicts resolve deterministically").
//!
//! **`SESSION`/`EXPLICIT PIN`, the most specific rung in the documented hierarchy, is
//! deliberately not part of this ladder.** [`crate::schema::PinEntry`] has no `authority`
//! field - a pin is a protection fact (mirrors `cancellai_model::ProtectionState::Pinned`, an
//! independent lifecycle axis `effective_authority` already collapses to non-destructive on its
//! own), not a requested authority level. Wiring an explicit pin into the safety inputs'
//! `protection` field is E11-S04's own outcome ("pin/protect semantics"), not this resolver's.

use cancellai_model::AuthorityLevel;
use cancellai_safety::{AuthorityInputs, EffectiveAuthority, effective_authority};

use crate::schema::PolicyDocument;

/// The identity facts one artifact/action resolves policy against. Every field is optional -
/// no command surface calls this resolver yet (E11-S01's own residual risk), so a caller that
/// has not wired a given axis passes `None`, and that axis's scope map is simply never
/// consulted (equivalent to it being absent from the document for this resolution).
#[derive(Debug, Clone, Copy, Default)]
pub struct PolicyContext<'a> {
    pub machine_id: Option<&'a str>,
    pub provider_id: Option<&'a str>,
    pub project_ref: Option<&'a str>,
    pub artifact_type: Option<&'a str>,
}

/// Which rung of the scope ladder supplied a [`ResolvedRequest`], or that none did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyScopeSource {
    ArtifactType,
    Project,
    Provider,
    Machine,
    Global,
    /// No scope in the document set an authority for this context (including a completely
    /// empty document). [`ResolvedRequest::authority`] is then the caller-supplied `fallback` -
    /// `docs/architecture/POLICY_MODEL.md`'s "Product defaults", the weakest link in the
    /// documented precedence list. This module does not hardcode what that default is: the
    /// integration that has not been built yet (no CLI/TUI wiring exists, per E11-S01's
    /// residual risk) is the one place that can honestly say what "no policy at all" should
    /// mean for a given command.
    NoMatch,
}

/// The user's requested authority after policy resolution, and which scope produced it -
/// deliberately separate from `cancellai_safety::EffectiveAuthority` (which this is not): a
/// request is not yet bounded by any safety constraint. [`resolve_effective_authority`] is what
/// actually bounds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedRequest {
    pub authority: AuthorityLevel,
    pub source: PolicyScopeSource,
}

/// Walks the scope ladder for `context` against `document`, most specific first, and returns
/// the first scope that sets an authority - `fallback` (and [`PolicyScopeSource::NoMatch`]) if
/// none do. See the module doc for the ladder order and why `SESSION`/pin is excluded from it.
pub fn resolve_requested_authority(
    document: &PolicyDocument,
    context: &PolicyContext<'_>,
    fallback: AuthorityLevel,
) -> ResolvedRequest {
    let rungs: [(Option<AuthorityLevel>, PolicyScopeSource); 5] = [
        (
            context
                .artifact_type
                .and_then(|key| document.artifact_types.get(key))
                .and_then(|scope| scope.authority),
            PolicyScopeSource::ArtifactType,
        ),
        (
            context
                .project_ref
                .and_then(|key| document.projects.get(key))
                .and_then(|scope| scope.authority),
            PolicyScopeSource::Project,
        ),
        (
            context
                .provider_id
                .and_then(|key| document.providers.get(key))
                .and_then(|scope| scope.authority),
            PolicyScopeSource::Provider,
        ),
        (
            context
                .machine_id
                .and_then(|key| document.machine.get(key))
                .and_then(|scope| scope.authority),
            PolicyScopeSource::Machine,
        ),
        (
            document.global.as_ref().and_then(|scope| scope.authority),
            PolicyScopeSource::Global,
        ),
    ];

    for (authority, source) in rungs {
        if let Some(authority) = authority {
            return ResolvedRequest { authority, source };
        }
    }

    ResolvedRequest {
        authority: fallback,
        source: PolicyScopeSource::NoMatch,
    }
}

/// [`resolve_requested_authority`], folded into `inputs.user_requested` and passed to
/// `cancellai_safety::effective_authority` - the seam that actually discharges SI-025/AC1/AC2.
/// `inputs.user_requested` on the way in is the pre-policy baseline (what `fallback` becomes if
/// no scope matches, e.g. a bare CLI flag with no policy document involved at all); every other
/// field of `inputs` is untouched, so every other constraint `effective_authority` already
/// enforces (artifact ceiling, confidence, lifecycle, provider trust, the constitutional safety
/// floor) applies exactly as it would without this function existing.
pub fn resolve_effective_authority(
    document: &PolicyDocument,
    context: &PolicyContext<'_>,
    mut inputs: AuthorityInputs,
) -> (EffectiveAuthority, ResolvedRequest) {
    let resolved = resolve_requested_authority(document, context, inputs.user_requested);
    inputs.user_requested = resolved.authority;
    (effective_authority(inputs), resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::ScopePolicy;
    use cancellai_model::{ActivityState, IntegrityState, KnowledgeConfidence, ProtectionState};
    use cancellai_safety::TrustedTier;
    use std::collections::BTreeMap;

    fn scope(authority: AuthorityLevel) -> ScopePolicy {
        ScopePolicy {
            authority: Some(authority),
            retention: None,
            keep_latest: None,
            budget: None,
        }
    }

    fn empty_document() -> PolicyDocument {
        PolicyDocument {
            schema_version: crate::schema::CURRENT_SCHEMA_VERSION,
            global: None,
            machine: BTreeMap::new(),
            providers: BTreeMap::new(),
            projects: BTreeMap::new(),
            artifact_types: BTreeMap::new(),
            pins: Vec::new(),
        }
    }

    fn permissive_authority_inputs(user_requested: AuthorityLevel) -> AuthorityInputs {
        AuthorityInputs {
            user_requested,
            artifact_ceiling: AuthorityLevel::Autopilot,
            confidence: KnowledgeConfidence::Verified,
            activity: ActivityState::Idle,
            protection: ProtectionState::Normal,
            integrity: IntegrityState::Healthy,
            provider_trust: crate::trust::builtin_provider_trust(),
            provider_capability_ceiling: None,
        }
    }

    // --- AC "Conflicts resolve deterministically": exhaustive precedence matrix --------------
    // Each rung, if present, is assigned a distinct AuthorityLevel so the winner is identifiable
    // from the resolved value alone. Every one of the 2^5 = 32 subsets of {artifact_type,
    // project, provider, machine, global} being present is exercised; the expected winner is
    // always the most-specific rung that is present.

    #[test]
    fn ac_conflicts_resolve_deterministically_exhaustive_precedence_matrix() {
        const RUNGS: [(&str, AuthorityLevel, PolicyScopeSource); 5] = [
            (
                "artifact_type",
                AuthorityLevel::Autopilot,
                PolicyScopeSource::ArtifactType,
            ),
            (
                "project",
                AuthorityLevel::Govern,
                PolicyScopeSource::Project,
            ),
            (
                "provider",
                AuthorityLevel::Quarantine,
                PolicyScopeSource::Provider,
            ),
            (
                "machine",
                AuthorityLevel::Recommend,
                PolicyScopeSource::Machine,
            ),
            ("global", AuthorityLevel::Observe, PolicyScopeSource::Global),
        ];
        let context = PolicyContext {
            machine_id: Some("machine"),
            provider_id: Some("provider"),
            project_ref: Some("project"),
            artifact_type: Some("artifact_type"),
        };
        let fallback = AuthorityLevel::Autopilot;

        for mask in 0u8..32 {
            let mut document = empty_document();
            let mut present: Vec<usize> = Vec::new();
            for (i, (_, level, _)) in RUNGS.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    present.push(i);
                    match i {
                        0 => {
                            document
                                .artifact_types
                                .insert("artifact_type".to_string(), scope(*level));
                        }
                        1 => {
                            document
                                .projects
                                .insert("project".to_string(), scope(*level));
                        }
                        2 => {
                            document
                                .providers
                                .insert("provider".to_string(), scope(*level));
                        }
                        3 => {
                            document
                                .machine
                                .insert("machine".to_string(), scope(*level));
                        }
                        4 => {
                            document.global = Some(scope(*level));
                        }
                        _ => unreachable!(),
                    }
                }
            }

            let result = resolve_requested_authority(&document, &context, fallback);

            match present.first() {
                Some(&winner) => {
                    let (_, expected_level, expected_source) = RUNGS[winner];
                    assert_eq!(
                        result.authority, expected_level,
                        "mask={mask:#07b} expected rung {winner} to win"
                    );
                    assert_eq!(
                        result.source, expected_source,
                        "mask={mask:#07b} expected rung {winner} to win"
                    );
                }
                None => {
                    assert_eq!(result.authority, fallback, "mask={mask:#07b}");
                    assert_eq!(
                        result.source,
                        PolicyScopeSource::NoMatch,
                        "mask={mask:#07b}"
                    );
                }
            }
        }
    }

    #[test]
    fn resolving_the_same_document_and_context_twice_gives_identical_results() {
        let mut document = empty_document();
        document
            .projects
            .insert("cancellai".to_string(), scope(AuthorityLevel::Quarantine));
        let context = PolicyContext {
            project_ref: Some("cancellai"),
            ..PolicyContext::default()
        };
        let first = resolve_requested_authority(&document, &context, AuthorityLevel::Observe);
        let second = resolve_requested_authority(&document, &context, AuthorityLevel::Observe);
        assert_eq!(first, second);
    }

    #[test]
    fn a_scope_present_but_with_no_authority_set_defers_to_the_next_rung_down() {
        let mut document = empty_document();
        // artifact_type is present (matches context) but sets no authority - must be skipped,
        // not treated as a match with some default value.
        document.artifact_types.insert(
            "session".to_string(),
            ScopePolicy {
                authority: None,
                retention: Some("30d".to_string()),
                keep_latest: None,
                budget: None,
            },
        );
        document
            .projects
            .insert("cancellai".to_string(), scope(AuthorityLevel::Govern));
        let context = PolicyContext {
            artifact_type: Some("session"),
            project_ref: Some("cancellai"),
            ..PolicyContext::default()
        };
        let result = resolve_requested_authority(&document, &context, AuthorityLevel::Observe);
        assert_eq!(result.authority, AuthorityLevel::Govern);
        assert_eq!(result.source, PolicyScopeSource::Project);
    }

    #[test]
    fn an_empty_document_falls_back_to_the_caller_supplied_default() {
        let document = empty_document();
        let context = PolicyContext {
            machine_id: Some("m"),
            provider_id: Some("codex"),
            project_ref: Some("cancellai"),
            artifact_type: Some("session"),
        };
        let result = resolve_requested_authority(&document, &context, AuthorityLevel::Recommend);
        assert_eq!(result.authority, AuthorityLevel::Recommend);
        assert_eq!(result.source, PolicyScopeSource::NoMatch);
    }

    #[test]
    fn a_context_with_no_identity_at_all_only_ever_reaches_global() {
        let mut document = empty_document();
        document
            .projects
            .insert("cancellai".to_string(), scope(AuthorityLevel::Autopilot));
        document.global = Some(scope(AuthorityLevel::Recommend));
        let context = PolicyContext::default();
        let result = resolve_requested_authority(&document, &context, AuthorityLevel::Observe);
        assert_eq!(result.authority, AuthorityLevel::Recommend);
        assert_eq!(result.source, PolicyScopeSource::Global);
    }

    // --- AC1 "Safety invariants cannot be overridden by configuration" -----------------------

    #[test]
    fn a_protected_artifact_stays_non_destructive_no_matter_what_policy_requests() {
        let mut document = empty_document();
        document
            .artifact_types
            .insert("session".to_string(), scope(AuthorityLevel::Autopilot));
        let context = PolicyContext {
            artifact_type: Some("session"),
            ..PolicyContext::default()
        };
        let mut inputs = permissive_authority_inputs(AuthorityLevel::Observe);
        inputs.protection = ProtectionState::Protected;

        let (effective, resolved) = resolve_effective_authority(&document, &context, inputs);
        assert_eq!(
            resolved.authority,
            AuthorityLevel::Autopilot,
            "policy did ask for Autopilot"
        );
        assert_eq!(
            effective.level,
            AuthorityLevel::Recommend,
            "SI-001/SI-025: a Protected artifact must stay non-destructive regardless of what \
             policy requested"
        );
    }

    #[test]
    fn low_unknown_confidence_stays_non_destructive_no_matter_what_policy_requests() {
        let mut document = empty_document();
        document.global = Some(scope(AuthorityLevel::Autopilot));
        let context = PolicyContext::default();
        let mut inputs = permissive_authority_inputs(AuthorityLevel::Observe);
        inputs.confidence = KnowledgeConfidence::LowUnknown;

        let (effective, _) = resolve_effective_authority(&document, &context, inputs);
        assert_eq!(effective.level, AuthorityLevel::Recommend);
    }

    // --- AC2 "More specific user policy cannot exceed artifact/provider/trust ceilings" ------

    #[test]
    fn policy_requesting_autopilot_cannot_exceed_a_lower_artifact_ceiling() {
        let mut document = empty_document();
        document
            .artifact_types
            .insert("session".to_string(), scope(AuthorityLevel::Autopilot));
        let context = PolicyContext {
            artifact_type: Some("session"),
            ..PolicyContext::default()
        };
        let mut inputs = permissive_authority_inputs(AuthorityLevel::Observe);
        inputs.artifact_ceiling = AuthorityLevel::Quarantine;

        let (effective, resolved) = resolve_effective_authority(&document, &context, inputs);
        assert_eq!(resolved.authority, AuthorityLevel::Autopilot);
        assert_eq!(
            effective.level,
            AuthorityLevel::Quarantine,
            "the artifact's own ceiling must still bind the result"
        );
        assert!(
            effective
                .binding_constraints
                .contains(&"artifact_authority_ceiling")
        );
    }

    #[test]
    fn policy_requesting_autopilot_cannot_exceed_an_untrusted_provider_ceiling() {
        let mut document = empty_document();
        document
            .providers
            .insert("shady-cli".to_string(), scope(AuthorityLevel::Autopilot));
        let context = PolicyContext {
            provider_id: Some("shady-cli"),
            ..PolicyContext::default()
        };
        let mut inputs = permissive_authority_inputs(AuthorityLevel::Observe);
        inputs.provider_trust = TrustedTier::untrusted();

        let (effective, resolved) = resolve_effective_authority(&document, &context, inputs);
        assert_eq!(resolved.authority, AuthorityLevel::Autopilot);
        assert_eq!(effective.level, AuthorityLevel::Observe);
    }

    #[test]
    fn a_fully_permissive_case_actually_reaches_autopilot_not_vacuously_capped() {
        // Without this, every "stays capped" test above could be explained by a bug that
        // always returns something low - prove policy CAN actually raise the result when
        // nothing else caps it.
        let mut document = empty_document();
        document
            .artifact_types
            .insert("session".to_string(), scope(AuthorityLevel::Autopilot));
        let context = PolicyContext {
            artifact_type: Some("session"),
            ..PolicyContext::default()
        };
        let inputs = permissive_authority_inputs(AuthorityLevel::Observe);
        let (effective, _) = resolve_effective_authority(&document, &context, inputs);
        assert_eq!(effective.level, AuthorityLevel::Autopilot);
    }

    #[test]
    fn a_narrower_policy_request_is_honored_when_everything_else_would_allow_more() {
        // Policy can also ask for LESS than everything else would allow - "narrowing," the
        // other documented direction, not just "refining upward."
        let mut document = empty_document();
        document.global.replace(scope(AuthorityLevel::Recommend));
        let context = PolicyContext::default();
        let inputs = permissive_authority_inputs(AuthorityLevel::Autopilot);
        let (effective, resolved) = resolve_effective_authority(&document, &context, inputs);
        assert_eq!(resolved.authority, AuthorityLevel::Recommend);
        assert_eq!(effective.level, AuthorityLevel::Recommend);
    }

    #[test]
    fn no_matching_scope_falls_back_to_the_pre_policy_baseline_and_is_still_safety_bounded() {
        let document = empty_document();
        let context = PolicyContext::default();
        let mut inputs = permissive_authority_inputs(AuthorityLevel::Autopilot);
        inputs.protection = ProtectionState::Pinned;
        let (effective, resolved) = resolve_effective_authority(&document, &context, inputs);
        assert_eq!(resolved.source, PolicyScopeSource::NoMatch);
        assert_eq!(resolved.authority, AuthorityLevel::Autopilot);
        assert_eq!(effective.level, AuthorityLevel::Recommend);
    }
}

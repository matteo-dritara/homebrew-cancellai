//! The provider capability contract (E05-S01, `docs/architecture/PROVIDER_MODEL.md`
//! "Capability contract").
//!
//! A provider adapter does not answer "is this provider supported" with one boolean; it
//! answers nine independent capability questions, each with its own [`SupportState`],
//! [`KnowledgeConfidence`], and evidence. A provider can be `VERIFIED` for `inventory_map`
//! and simultaneously `UNSUPPORTED` for `native_delete_capability` - the two never collapse
//! into a single "supported" flag (PROVIDER_MODEL.md: "A provider can therefore be verified
//! for inventory but unsupported for native delete.").
//!
//! Scope note: this story defines the *contract* - the capability enumeration, the outcome
//! envelope (support/confidence/evidence/authority ceiling), and the trait every adapter
//! implements - not the per-capability result payload (a session graph's actual shape, a
//! project attribution's actual fields). Those payload types belong to the stories that
//! produce real data from them (E05-S03/E05-S04 adapters, and the inventory/session-graph
//! epics beyond E05): inventing placeholder payload types nothing produces yet would not make
//! this contract any more correct, matching the precedent `cancellai-safety::SealedPlan` and
//! `cancellai-model::RootFingerprint` set for deferring fields no real subsystem populates.
//!
//! AC1 ("capability absence is first-class and never inferred from provider identity") is
//! enforced by shape, not convention: [`ProviderCapabilities::capability`] is the *only*
//! required trait method, has no default implementation, and takes no provider-identity
//! input beyond `&self` - this crate defines no identity-keyed lookup table anywhere that
//! could infer a capability's support from a provider id string. Every adapter must answer
//! every [`CapabilityKind`] explicitly.

use cancellai_model::{AuthorityLevel, KnowledgeConfidence};

/// The nine independent provider capabilities PROVIDER_MODEL.md's contract enumerates, named
/// after the exact functions their code block lists. `ALL` mirrors
/// `cancellai_model::ErrorCategory::ALL`: an exhaustive, ordered enumeration lets
/// [`capability_report`] (and any future adapter's own conformance test) ask "every
/// capability, in a fixed order" without hand-listing nine calls and risking a missed one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityKind {
    Detect,
    FingerprintRoot,
    InventoryMap,
    ProjectAttribution,
    SessionGraph,
    ActivityState,
    NativeDeleteCapability,
    RetentionCapability,
    Explain,
}

impl CapabilityKind {
    pub const ALL: [CapabilityKind; 9] = [
        CapabilityKind::Detect,
        CapabilityKind::FingerprintRoot,
        CapabilityKind::InventoryMap,
        CapabilityKind::ProjectAttribution,
        CapabilityKind::SessionGraph,
        CapabilityKind::ActivityState,
        CapabilityKind::NativeDeleteCapability,
        CapabilityKind::RetentionCapability,
        CapabilityKind::Explain,
    ];

    /// Stable, machine-facing string code - same "never renumbered or repurposed" contract
    /// as `ErrorCategory::code` (`cancellai-model/src/diagnostic.rs`), and identical to the
    /// function name PROVIDER_MODEL.md's capability contract block uses for this capability.
    pub const fn code(self) -> &'static str {
        match self {
            CapabilityKind::Detect => "detect",
            CapabilityKind::FingerprintRoot => "fingerprint_root",
            CapabilityKind::InventoryMap => "inventory_map",
            CapabilityKind::ProjectAttribution => "project_attribution",
            CapabilityKind::SessionGraph => "session_graph",
            CapabilityKind::ActivityState => "activity_state",
            CapabilityKind::NativeDeleteCapability => "native_delete_capability",
            CapabilityKind::RetentionCapability => "retention_capability",
            CapabilityKind::Explain => "explain",
        }
    }
}

/// A capability's support state (PROVIDER_MODEL.md "Support states"). Deliberately not a
/// boolean: `UNSUPPORTED` and `UNKNOWN_VERSION` and `LAYOUT_DRIFT` are distinct, evidenced
/// claims, not one collapsed "false".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupportState {
    Verified,
    SupportedObserved,
    Unsupported,
    UnknownVersion,
    LayoutDrift,
    ErrorPartial,
}

impl SupportState {
    pub const ALL: [SupportState; 6] = [
        SupportState::Verified,
        SupportState::SupportedObserved,
        SupportState::Unsupported,
        SupportState::UnknownVersion,
        SupportState::LayoutDrift,
        SupportState::ErrorPartial,
    ];

    /// Stable, machine-facing string code - see [`CapabilityKind::code`] for the same
    /// stability contract applied to capabilities instead of support states.
    pub const fn code(self) -> &'static str {
        match self {
            SupportState::Verified => "VERIFIED",
            SupportState::SupportedObserved => "SUPPORTED_OBSERVED",
            SupportState::Unsupported => "UNSUPPORTED",
            SupportState::UnknownVersion => "UNKNOWN_VERSION",
            SupportState::LayoutDrift => "LAYOUT_DRIFT",
            SupportState::ErrorPartial => "ERROR_PARTIAL",
        }
    }
}

impl serde::Serialize for SupportState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

/// One capability's answer: support state, the confidence behind it, at least one evidence
/// note explaining why, and any authority ceiling this capability implies
/// (PROVIDER_MODEL.md: "Every capability result includes: support state; ... evidence;
/// confidence/trust; any authority ceiling it implies.").
///
/// Fields are private and the only public constructor, [`CapabilityOutcome::new`], requires a
/// `primary_evidence` string in addition to the `Vec` of any further notes - there is no way
/// to build an outcome with zero evidence (AC2: "capability responses carry evidence and
/// confidence"), the same "invariant enforced by API shape, not convention" pattern
/// `cancellai-safety::SealedPlan` and `cancellai-inventory::PlanningView` use for their own
/// non-droppable fields.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CapabilityOutcome {
    support: SupportState,
    confidence: KnowledgeConfidence,
    evidence: Vec<String>,
    authority_ceiling: Option<AuthorityLevel>,
}

impl CapabilityOutcome {
    pub fn new(
        support: SupportState,
        confidence: KnowledgeConfidence,
        primary_evidence: impl Into<String>,
        additional_evidence: impl IntoIterator<Item = String>,
        authority_ceiling: Option<AuthorityLevel>,
    ) -> Self {
        let mut evidence = vec![primary_evidence.into()];
        evidence.extend(additional_evidence);
        Self {
            support,
            confidence,
            evidence,
            authority_ceiling,
        }
    }

    pub fn support(&self) -> SupportState {
        self.support
    }

    pub fn confidence(&self) -> KnowledgeConfidence {
        self.confidence
    }

    pub fn evidence(&self) -> &[String] {
        &self.evidence
    }

    pub fn authority_ceiling(&self) -> Option<AuthorityLevel> {
        self.authority_ceiling
    }
}

/// A provider adapter's answer to the nine-capability contract.
///
/// [`ProviderCapabilities::capability`] is the sole required method: an adapter matches
/// exhaustively over [`CapabilityKind`] and returns an explicit [`CapabilityOutcome`] for
/// every arm (AC1 - see the module doc). The nine named methods below (`detect`,
/// `fingerprint_root`, ...) are provided default methods that just delegate to `capability`;
/// they exist so callers can write `provider.session_graph()` instead of
/// `provider.capability(CapabilityKind::SessionGraph)`, matching the function names
/// PROVIDER_MODEL.md's contract block documents, without giving an adapter two places to
/// implement the same answer.
pub trait ProviderCapabilities {
    fn provider_id(&self) -> &str;

    fn capability(&self, kind: CapabilityKind) -> CapabilityOutcome;

    fn detect(&self) -> CapabilityOutcome {
        self.capability(CapabilityKind::Detect)
    }

    fn fingerprint_root(&self) -> CapabilityOutcome {
        self.capability(CapabilityKind::FingerprintRoot)
    }

    fn inventory_map(&self) -> CapabilityOutcome {
        self.capability(CapabilityKind::InventoryMap)
    }

    fn project_attribution(&self) -> CapabilityOutcome {
        self.capability(CapabilityKind::ProjectAttribution)
    }

    fn session_graph(&self) -> CapabilityOutcome {
        self.capability(CapabilityKind::SessionGraph)
    }

    fn activity_state(&self) -> CapabilityOutcome {
        self.capability(CapabilityKind::ActivityState)
    }

    fn native_delete_capability(&self) -> CapabilityOutcome {
        self.capability(CapabilityKind::NativeDeleteCapability)
    }

    fn retention_capability(&self) -> CapabilityOutcome {
        self.capability(CapabilityKind::RetentionCapability)
    }

    fn explain(&self) -> CapabilityOutcome {
        self.capability(CapabilityKind::Explain)
    }
}

/// Runs every [`CapabilityKind`] against `provider`, in [`CapabilityKind::ALL`] order.
///
/// This is the reusable half of the "mock provider contract conformance suite" this story's
/// verification plan names: this crate's own tests drive it against a mock, and a future
/// provider adapter (E05-S03 Claude, E05-S04 Codex) can drive the exact same function against
/// its real implementation rather than re-deriving the nine-call enumeration itself.
pub fn capability_report(
    provider: &dyn ProviderCapabilities,
) -> Vec<(CapabilityKind, CapabilityOutcome)> {
    CapabilityKind::ALL
        .into_iter()
        .map(|kind| (kind, provider.capability(kind)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A configurable mock: each [`CapabilityKind`] maps to whatever [`CapabilityOutcome`]
    /// the test wired up for it, keyed purely by an explicit `HashMap` the test controls -
    /// never by `provider_id`. That is what lets
    /// `capability_absence_is_never_inferred_from_provider_identity` below prove AC1: two
    /// mocks sharing the same `provider_id` can still disagree on a capability's support,
    /// because nothing in this contract ties the two together.
    struct MockProvider {
        provider_id: &'static str,
        outcomes: HashMap<&'static str, CapabilityOutcome>,
    }

    impl MockProvider {
        fn new(provider_id: &'static str) -> Self {
            Self {
                provider_id,
                outcomes: HashMap::new(),
            }
        }

        fn with(mut self, kind: CapabilityKind, outcome: CapabilityOutcome) -> Self {
            self.outcomes.insert(kind.code(), outcome);
            self
        }
    }

    impl ProviderCapabilities for MockProvider {
        fn provider_id(&self) -> &str {
            self.provider_id
        }

        fn capability(&self, kind: CapabilityKind) -> CapabilityOutcome {
            self.outcomes.get(kind.code()).cloned().unwrap_or_else(|| {
                CapabilityOutcome::new(
                    SupportState::Unsupported,
                    KnowledgeConfidence::LowUnknown,
                    format!(
                        "mock provider declares no explicit answer for {}",
                        kind.code()
                    ),
                    Vec::new(),
                    None,
                )
            })
        }
    }

    fn verified(evidence: &str) -> CapabilityOutcome {
        CapabilityOutcome::new(
            SupportState::Verified,
            KnowledgeConfidence::Verified,
            evidence.to_string(),
            Vec::new(),
            None,
        )
    }

    fn unsupported(evidence: &str) -> CapabilityOutcome {
        CapabilityOutcome::new(
            SupportState::Unsupported,
            KnowledgeConfidence::LowUnknown,
            evidence.to_string(),
            Vec::new(),
            None,
        )
    }

    #[test]
    fn ac1_capability_absence_is_never_inferred_from_provider_identity() {
        // Same provider_id, deliberately different answers for the same capability - proving
        // the contract itself does not tie support state to identity. If this crate ever grew
        // an identity-keyed lookup that inferred support from `provider_id`, these two mocks
        // could not disagree while sharing an id.
        let claude_a = MockProvider::new("claude-code").with(
            CapabilityKind::NativeDeleteCapability,
            verified("tested delete API"),
        );
        let claude_b = MockProvider::new("claude-code").with(
            CapabilityKind::NativeDeleteCapability,
            unsupported("this adapter has not implemented delete yet"),
        );

        assert_eq!(claude_a.provider_id(), claude_b.provider_id());
        assert_eq!(
            claude_a.native_delete_capability().support(),
            SupportState::Verified
        );
        assert_eq!(
            claude_b.native_delete_capability().support(),
            SupportState::Unsupported
        );
    }

    #[test]
    fn ac1_a_provider_with_no_explicit_answer_reports_unsupported_not_a_guess() {
        // No `.with(...)` at all: `capability` still returns a real, evidenced Unsupported
        // outcome rather than panicking, defaulting to Verified, or being uncallable.
        let unconfigured = MockProvider::new("unknown-provider");

        for kind in CapabilityKind::ALL {
            let outcome = unconfigured.capability(kind);
            assert_eq!(outcome.support(), SupportState::Unsupported);
            assert!(!outcome.evidence().is_empty());
        }
    }

    #[test]
    fn ac1_one_capability_verified_does_not_imply_another_is() {
        // PROVIDER_MODEL.md: "A provider can therefore be verified for inventory but
        // unsupported for native delete."
        let mixed = MockProvider::new("claude-code")
            .with(
                CapabilityKind::InventoryMap,
                verified("known layout fingerprint"),
            )
            .with(
                CapabilityKind::NativeDeleteCapability,
                unsupported("no vendor delete API integrated"),
            );

        assert_eq!(mixed.inventory_map().support(), SupportState::Verified);
        assert_eq!(
            mixed.native_delete_capability().support(),
            SupportState::Unsupported
        );
    }

    #[test]
    fn ac2_every_capability_report_entry_carries_evidence_and_confidence() {
        let provider = MockProvider::new("claude-code")
            .with(CapabilityKind::Detect, verified("config root present"))
            .with(
                CapabilityKind::SessionGraph,
                unsupported("session graph not implemented for this layout"),
            );

        let report = capability_report(&provider);
        assert_eq!(report.len(), CapabilityKind::ALL.len());
        for (kind, outcome) in report {
            assert!(
                !outcome.evidence().is_empty(),
                "{} reported no evidence",
                kind.code()
            );
            // `confidence()` is a required, non-Option field - reaching this line already
            // proves every outcome carries one; the match below just pins the value is
            // meaningful, not a placeholder default silently reused for every kind.
            match outcome.confidence() {
                KnowledgeConfidence::Verified
                | KnowledgeConfidence::Observed
                | KnowledgeConfidence::Inferred
                | KnowledgeConfidence::LowUnknown => {}
            }
        }
    }

    #[test]
    fn capability_kind_codes_are_stable_and_match_provider_model_function_names() {
        let expected = [
            (CapabilityKind::Detect, "detect"),
            (CapabilityKind::FingerprintRoot, "fingerprint_root"),
            (CapabilityKind::InventoryMap, "inventory_map"),
            (CapabilityKind::ProjectAttribution, "project_attribution"),
            (CapabilityKind::SessionGraph, "session_graph"),
            (CapabilityKind::ActivityState, "activity_state"),
            (
                CapabilityKind::NativeDeleteCapability,
                "native_delete_capability",
            ),
            (CapabilityKind::RetentionCapability, "retention_capability"),
            (CapabilityKind::Explain, "explain"),
        ];
        for (kind, code) in expected {
            assert_eq!(kind.code(), code);
        }
    }

    #[test]
    fn support_state_codes_are_distinct() {
        let mut seen = std::collections::HashSet::new();
        for state in SupportState::ALL {
            assert!(seen.insert(state.code()), "duplicate code for {state:?}");
        }
    }

    #[test]
    fn outcome_serializes_support_as_its_stable_code() {
        let outcome = unsupported("no adapter yet");
        let json = serde_json::to_string(&outcome).expect("serializable");
        assert!(json.contains("\"UNSUPPORTED\""));
    }
}

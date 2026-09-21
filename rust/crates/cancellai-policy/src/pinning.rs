//! Explicit pin/protect semantics (E11-S04, `docs/architecture/POLICY_MODEL.md`'s `SESSION` /
//! `EXPLICIT PIN` scope - the one rung `crate::resolver`'s scope ladder deliberately excludes,
//! see that module's own doc for why).
//!
//! A pin is a protection fact, not a requested authority: matching
//! [`crate::schema::PinEntry::session`] against an artifact's session/identity resolves to
//! [`cancellai_model::ProtectionState::Pinned`], the same lifecycle axis
//! `cancellai_safety::authority::effective_authority` already collapses to non-destructive
//! authority on its own (E03-S04's `lifecycle_ceiling`) - this module computes no ceiling,
//! exactly like [`crate::resolver`] and [`crate::explanation`] before it. AC "Pinned/protected
//! state outranks cleanup policy" is therefore discharged by the same pre-existing, independently
//! verified constraint every other `ProtectionState::Pinned`/`Protected` artifact already relies
//! on, not by new logic here.

use cancellai_model::ProtectionState;

use crate::schema::PolicyDocument;

/// Whether `session_id` matches an explicit pin in `document`.
pub fn is_pinned(document: &PolicyDocument, session_id: &str) -> bool {
    document.pins.iter().any(|pin| pin.session == session_id)
}

/// Combines an artifact's already-determined [`ProtectionState`] with `document`'s pins for
/// `session_id`. **`Protected` always outranks a pin match** - protection from a barrier this
/// codebase already trusts (SI-006: protected-name/category barriers are defense in depth) is
/// never *weakened* to a mere pin by this function; a pin can only ever raise `Normal` to
/// `Pinned`, never lower `Protected`. This is "pinned/protected state outranks cleanup policy"
/// stated the other way round: policy (a pin) cannot outrank protection, only add to it.
pub fn resolve_protection(
    document: &PolicyDocument,
    session_id: &str,
    existing: ProtectionState,
) -> ProtectionState {
    if existing == ProtectionState::Protected {
        return existing;
    }
    if is_pinned(document, session_id) {
        ProtectionState::Pinned
    } else {
        existing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::PinEntry;
    use std::collections::BTreeMap;

    fn document_with_pin(session: &str) -> PolicyDocument {
        PolicyDocument {
            schema_version: crate::schema::CURRENT_SCHEMA_VERSION,
            global: None,
            machine: BTreeMap::new(),
            providers: BTreeMap::new(),
            projects: BTreeMap::new(),
            artifact_types: BTreeMap::new(),
            pins: vec![PinEntry {
                session: session.to_string(),
            }],
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

    #[test]
    fn a_matching_session_is_pinned() {
        let document = document_with_pin("abc123");
        assert!(is_pinned(&document, "abc123"));
    }

    #[test]
    fn a_non_matching_session_is_not_pinned() {
        let document = document_with_pin("abc123");
        assert!(!is_pinned(&document, "def456"));
    }

    #[test]
    fn an_empty_pin_list_pins_nothing() {
        let document = empty_document();
        assert!(!is_pinned(&document, "anything"));
    }

    // --- AC "Pinned/protected state outranks cleanup policy" ----------------------------------

    #[test]
    fn a_pin_match_raises_normal_protection_to_pinned() {
        let document = document_with_pin("abc123");
        assert_eq!(
            resolve_protection(&document, "abc123", ProtectionState::Normal),
            ProtectionState::Pinned
        );
    }

    #[test]
    fn a_pin_match_never_downgrades_an_already_protected_artifact() {
        let document = document_with_pin("abc123");
        assert_eq!(
            resolve_protection(&document, "abc123", ProtectionState::Protected),
            ProtectionState::Protected,
            "Protected must never be weakened to Pinned by a policy pin"
        );
    }

    #[test]
    fn no_pin_match_leaves_existing_protection_unchanged() {
        let document = document_with_pin("some-other-session");
        assert_eq!(
            resolve_protection(&document, "abc123", ProtectionState::Normal),
            ProtectionState::Normal
        );
        assert_eq!(
            resolve_protection(&document, "abc123", ProtectionState::Protected),
            ProtectionState::Protected
        );
    }

    // --- composition: a pin match actually reaches non-destructive authority ------------------

    #[test]
    fn a_pinned_artifact_reaches_non_destructive_authority_through_the_existing_lifecycle_ceiling()
    {
        use cancellai_model::{ActivityState, AuthorityLevel, IntegrityState, KnowledgeConfidence};
        use cancellai_safety::{AuthorityInputs, effective_authority};

        let document = document_with_pin("abc123");
        let protection = resolve_protection(&document, "abc123", ProtectionState::Normal);
        assert_eq!(protection, ProtectionState::Pinned);

        let result = effective_authority(AuthorityInputs {
            user_requested: AuthorityLevel::Autopilot,
            artifact_ceiling: AuthorityLevel::Autopilot,
            confidence: KnowledgeConfidence::Verified,
            activity: ActivityState::Idle,
            protection,
            integrity: IntegrityState::Healthy,
            provider_trust: crate::trust::builtin_provider_trust(),
            provider_layout: cancellai_safety::ProviderLayoutAssessment::NotAssessed,
        });
        assert_eq!(
            result.level,
            AuthorityLevel::Recommend,
            "a pinned artifact must stay non-destructive even when every other input is maximally permissive"
        );
        assert!(result.binding_constraints.contains(&"lifecycle_authority"));
    }
}

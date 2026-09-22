//! `LayoutSignature`: the closed vocabulary `crate::authority::resolve_provider_execution_
//! authority` (E14-S04 round 5, ADR-0036) compares against a real
//! [`cancellai_platform::BoundLayoutObservation`]'s markers.
//!
//! Four independent review rounds (ADR-0034, ADR-0035,
//! `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND3.md`,
//! `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND4.md`) found successively narrower ways a
//! caller-supplied layout fact - first a ceiling, then the raw `known_signatures`/`observed`
//! values themselves - could be asserted honestly once and then omitted or contradicted in a
//! second, independently constructed value, because the fact itself was a plain, publicly
//! constructible type with no binding to the real object it claimed to describe.
//!
//! ADR-0036 removes the caller-supplied *observation* half of that vocabulary entirely:
//! `AuthorityInputs` no longer carries a layout field at all (it is now purely the eight-
//! constraint analysis this crate always computed for the two honest call sites that have never
//! had a live observation to supply), and the only way a real observation of a real provider
//! root reaches an authority decision is `cancellai_platform::BoundLayoutObservation` - built
//! exclusively from real directory I/O, never from caller-asserted marker strings - fed
//! directly into `crate::authority::resolve_provider_execution_authority`, which mints an opaque
//! `ProviderExecutionPermit` a caller cannot fabricate, discard, or contradict with a sibling
//! value. `LayoutSignature` here is unchanged from ADR-0035: a normalized, order-independent set
//! of marker strings, still deliberately not shared with `cancellai_guardian::structural`'s
//! structurally identical type of the same name (this crate may not depend on
//! `cancellai-guardian`, and that module may not depend on this crate).

/// An opaque, closed set of structural marker tokens describing an observed provider-root
/// shape (directory names, a version tag) - never a real path or file content. Equality is
/// order-independent and deduplicated: [`LayoutSignature::new`] normalizes both, so two
/// signatures naming the same markers in a different order or with accidental duplicates
/// compare equal. Identical in shape and behavior to
/// `cancellai_guardian::structural::LayoutSignature`, which independently performs the same
/// comparison for its own detection report - deliberately not reused directly, since
/// `cancellai-safety` may not depend on `cancellai-guardian` (`docs/architecture/TARGET.md`'s
/// forbidden dependency direction; this crate's own module doc) and `structural.rs` may not
/// depend on `cancellai-safety` (that module's own "holds no reference to cancellai-safety and
/// never will").
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub struct LayoutSignature(Vec<String>);

impl LayoutSignature {
    pub fn new(markers: impl IntoIterator<Item = String>) -> Self {
        let mut markers: Vec<String> = markers.into_iter().collect();
        markers.sort();
        markers.dedup();
        Self(markers)
    }

    pub fn markers(&self) -> &[String] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_equality_is_order_independent_and_deduplicated() {
        let a = LayoutSignature::new(["b".to_string(), "a".to_string(), "a".to_string()]);
        let b = LayoutSignature::new(["a".to_string(), "b".to_string()]);
        assert_eq!(a, b);
    }
}

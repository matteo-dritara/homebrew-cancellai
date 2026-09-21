//! Provider layout/capability authority binding (E14-S04, `SI-004`: "Provider/version/layout
//! drift cannot preserve destructive capabilities merely because the provider name is
//! recognized", ADR-0034, ADR-0035).
//!
//! Two independent review rounds and one prior owner-authorized redesign (ADR-0034) each found a
//! way a caller could hold a real, drifted layout observation and still reach an authority level
//! the drift is supposed to cap:
//!
//! - **Round 1**: the observation reached no authority computation at all.
//! - **Round 2**: the fix was a second, opt-in `effective_authority_for_provider_capability`
//!   function beside the pre-existing, still-public plain `effective_authority`, which ignored it.
//! - **Round 3 (against ADR-0034's `Option<AuthorityLevel>` mandatory field)**: the *ceiling* was
//!   the mandatory input, not the observation it was derived from - a caller could compute the
//!   right ceiling once, then separately construct an otherwise-identical `AuthorityInputs` with
//!   `provider_capability_ceiling: None`, discarding the finding it still held. `Option<
//!   AuthorityLevel>` is caller-asserted data with no binding to what was actually observed.
//!
//! ADR-0035 closes this by removing the ceiling as an independent input entirely: crate::
//! authority::AuthorityInputs::provider_layout takes [`ProviderLayoutAssessment`] - either
//! [`ProviderLayoutAssessment::NotAssessed`] (the honest, always-safe "no observation reached
//! this construction" default, adding no constraint) or [`ProviderLayoutAssessment::Observed`]
//! (the raw `known_signatures`/`observed` [`LayoutSignature`] facts). `crate::authority::
//! base_constraints` derives the ceiling from those facts *itself*, with [`layout_ceiling`], the
//! same comparison [`cancellai_guardian::structural::assess_layout`] independently performs for
//! its own detection report - there is no ceiling value left for a caller to assert, discard, or
//! disagree with once it supplies the observation: supplying the facts and getting a different
//! result than the one derived from them is not an option a caller has.
//!
//! This closes exactly the round-3 counterexample: a caller that supplies a real `Observed`
//! layout cannot also construct an `AuthorityInputs` that ignores it, because there is no second
//! field carrying an independently-assertable ceiling to omit. The residual ADR-0034 disclosed
//! and ADR-0035 narrows further, not closes, is unchanged in kind: a caller that never supplies an
//! observation at all (`NotAssessed`) still adds no constraint - this module cannot compel a
//! caller that never observed a layout to go observe one, only guarantee that a caller who did
//! cannot then discard what it found. Wiring a live provider-root probe into every real
//! authority-resolution call site remains future orchestrator work (ADR-0034's own disclosed
//! residual, restated by ADR-0035).

use cancellai_model::AuthorityLevel;

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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

/// [`crate::authority::AuthorityInputs::provider_layout`]'s value: either no observation reached
/// this construction ([`ProviderLayoutAssessment::NotAssessed`], the honest, always-safe default
/// that adds no constraint), or the raw facts of one ([`ProviderLayoutAssessment::Observed`]).
/// There is deliberately no third, caller-asserted "ceiling" variant - see the module doc for
/// why: that shape is exactly what round 3 independent review found discardable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderLayoutAssessment {
    /// No layout observation reached this `AuthorityInputs` construction. Adds no constraint -
    /// identical in effect to a recognized layout, since this crate cannot distinguish "checked,
    /// fine" from "never checked," and both are equally safe to leave uncapped.
    NotAssessed,
    /// A real observation: `known_signatures` is the closed set of layouts this caller currently
    /// recognizes; `observed` is the layout actually seen. [`layout_ceiling`] derives the
    /// resulting constraint from these two values alone - never from a value supplied alongside
    /// them.
    Observed {
        known_signatures: Vec<LayoutSignature>,
        observed: LayoutSignature,
    },
}

impl Default for ProviderLayoutAssessment {
    /// [`ProviderLayoutAssessment::NotAssessed`] - the same safe, constraint-free default
    /// `NotAssessed` always is, so a value obtained via `Default::default()` (e.g.
    /// `..Default::default()` in a struct-update fixture) never accidentally carries a
    /// fabricated observation.
    fn default() -> Self {
        ProviderLayoutAssessment::NotAssessed
    }
}

/// The authority ceiling `assessment` implies, computed the same way for every caller: `None`
/// for [`ProviderLayoutAssessment::NotAssessed`] or a recognized layout (no additional
/// constraint - the other base constraints alone still apply). Otherwise, for a layout matching
/// none of `known_signatures`, the result is `Some(AuthorityLevel::Observe)`, the lowest ceiling
/// this vocabulary expresses, including when `known_signatures` is empty or `observed` carries
/// no markers at all, so an absent or inconclusive comparison is drift, never a default pass.
///
/// Not `pub`: `crate::authority::base_constraints` is the only caller. A caller outside this
/// crate supplies the raw [`ProviderLayoutAssessment`], never a pre-computed ceiling - that
/// distinction is the whole point (module doc).
pub(crate) fn layout_ceiling(assessment: &ProviderLayoutAssessment) -> Option<AuthorityLevel> {
    match assessment {
        ProviderLayoutAssessment::NotAssessed => None,
        ProviderLayoutAssessment::Observed {
            known_signatures,
            observed,
        } => {
            if known_signatures.iter().any(|known| known == observed) {
                None
            } else {
                Some(AuthorityLevel::Observe)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_assessment_defaults_to_not_assessed() {
        assert_eq!(
            ProviderLayoutAssessment::default(),
            ProviderLayoutAssessment::NotAssessed
        );
    }

    #[test]
    fn signature_equality_is_order_independent_and_deduplicated() {
        let a = LayoutSignature::new(["b".to_string(), "a".to_string(), "a".to_string()]);
        let b = LayoutSignature::new(["a".to_string(), "b".to_string()]);
        assert_eq!(a, b);
    }

    #[test]
    fn not_assessed_adds_no_constraint() {
        assert_eq!(layout_ceiling(&ProviderLayoutAssessment::NotAssessed), None);
    }

    #[test]
    fn a_recognized_layout_adds_no_constraint() {
        let known = vec![LayoutSignature::new(["sessions/".to_string()])];
        let observed = LayoutSignature::new(["sessions/".to_string()]);
        assert_eq!(
            layout_ceiling(&ProviderLayoutAssessment::Observed {
                known_signatures: known,
                observed,
            }),
            None
        );
    }

    #[test]
    fn a_drifted_layout_caps_at_observe() {
        let known = vec![LayoutSignature::new(["sessions/".to_string()])];
        let observed = LayoutSignature::new(["totally_different_shape/".to_string()]);
        assert_eq!(
            layout_ceiling(&ProviderLayoutAssessment::Observed {
                known_signatures: known,
                observed,
            }),
            Some(AuthorityLevel::Observe)
        );
    }

    #[test]
    fn empty_known_signatures_never_recognizes_anything() {
        let observed = LayoutSignature::new(["config.json".to_string()]);
        assert_eq!(
            layout_ceiling(&ProviderLayoutAssessment::Observed {
                known_signatures: vec![],
                observed,
            }),
            Some(AuthorityLevel::Observe)
        );
    }

    #[test]
    fn empty_observed_markers_is_drift_not_a_vacuous_match() {
        let known = vec![LayoutSignature::new(Vec::<String>::new())];
        let observed = LayoutSignature::new(["config.json".to_string()]);
        assert_eq!(
            layout_ceiling(&ProviderLayoutAssessment::Observed {
                known_signatures: known,
                observed,
            }),
            Some(AuthorityLevel::Observe)
        );
    }
}

//! Structural anomaly detection (E14-S04) -
//! `docs/architecture/GUARDIAN_MODEL.md`'s "Detection": "session-count explosion; unexpected
//! giant artifacts; provider layout drift; orphan-state growth."
//!
//! Same isolation as [`crate::pressure`]/[`crate::forecast`]/[`crate::baseline`] (`SI-027`,
//! "Detection severity does not create authority"): this module holds no reference to
//! `cancellai-safety` and never will. The three counting/size signals below are thin, named
//! wrappers over [`crate::baseline::Baseline`] - they exist so "session explosion", "giant
//! artifact" and "orphan growth" are concrete, testable capabilities this crate exposes by name
//! (matching the vocabulary `GUARDIAN_MODEL.md` uses), not because the underlying comparison
//! differs from `Baseline::assess`'s own robust median/MAD judgment. Input stays plain numeric
//! metadata (a count, a byte size) - never a path, prompt, or file content, matching "without
//! content inspection".
//!
//! Layout drift is different in kind from the other three: it is a discrete match/mismatch
//! against a closed set of previously recognized structural signatures, not a continuous
//! numeric deviation, so it gets its own comparison in [`assess_layout`] rather than reusing
//! [`crate::baseline::Baseline`]. `SI-004` ("Provider/version/layout drift cannot preserve
//! destructive capabilities merely because the provider name is recognized") is discharged by
//! construction: [`assess_layout`]'s decision is a pure function of `known_signatures` and
//! `observed` alone - `provider_id` is carried only as an opaque evidence label threaded into
//! the returned finding's `evidence` string, and no branch anywhere in this module reads it.
//! `recommended_authority_ceiling` is exactly that - a recommendation carried as
//! `cancellai_model::AuthorityLevel`, this crate's existing kernel-vocabulary dependency,
//! evaluated automatically from the observed signature (AC1's "automatically") but never
//! applied or executed by this module. `cancellai-safety` remains the sole executor
//! (`docs/CONSTITUTION.md`: "All ... route mutation through one safety boundary"); nothing here
//! is a second path to that decision, only an unapplied observation feeding it.

use cancellai_model::AuthorityLevel;

use crate::baseline::{AnomalyAssessment, Baseline};

/// Which of the three counting/size signals `GUARDIAN_MODEL.md`'s "Detection" section names
/// produced a [`StructuralFinding`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralSignal {
    SessionCount,
    ArtifactSize,
    OrphanCount,
}

/// A named structural signal paired with the [`AnomalyAssessment`] that judged it - AC2
/// ("structural signals reference concrete observed evidence") is inherited directly from
/// [`AnomalyAssessment`], which always carries the observed value and the baseline it was
/// compared against, never a bare severity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StructuralFinding {
    pub signal: StructuralSignal,
    pub assessment: AnomalyAssessment,
}

/// Session-count explosion: compares `observed_session_count` against `baseline`, the caller's
/// own bounded history of session counts for this scope (provider/project). A count far above
/// the baseline's usual spread reads as [`crate::baseline::AnomalySeverity::Anomalous`].
pub fn assess_session_explosion(
    baseline: &Baseline,
    observed_session_count: f64,
) -> StructuralFinding {
    StructuralFinding {
        signal: StructuralSignal::SessionCount,
        assessment: baseline.assess(observed_session_count),
    }
}

/// Unexpected giant artifact: compares `observed_size_bytes` against `baseline`, the caller's
/// own bounded history of artifact sizes for this scope. A size far above what this scope has
/// typically produced reads as anomalous, regardless of whether it would be unremarkable in
/// absolute terms for a different provider/category.
pub fn assess_giant_artifact(baseline: &Baseline, observed_size_bytes: f64) -> StructuralFinding {
    StructuralFinding {
        signal: StructuralSignal::ArtifactSize,
        assessment: baseline.assess(observed_size_bytes),
    }
}

/// Orphan-state growth: compares `observed_orphan_count` against `baseline`, the caller's own
/// bounded history of orphan counts for this scope. A runaway climb reads as anomalous; a
/// stable or shrinking orphan count does not.
pub fn assess_orphan_growth(baseline: &Baseline, observed_orphan_count: f64) -> StructuralFinding {
    StructuralFinding {
        signal: StructuralSignal::OrphanCount,
        assessment: baseline.assess(observed_orphan_count),
    }
}

/// An opaque, closed set of structural marker tokens a caller already computed by inspecting a
/// provider root's shape (directory names, a version tag it read) - never a real path or file
/// content, the same "without content inspection" boundary the rest of this module keeps.
/// Equality is order-independent and deduplicated: [`LayoutSignature::new`] normalizes both, so
/// two signatures naming the same markers in a different order or with accidental duplicates
/// compare equal.
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

/// Whether an observed layout was recognized against the caller's known signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutSupport {
    /// `observed` matched one of `known_signatures` exactly.
    Recognized,
    /// `observed` matched none of `known_signatures` - an unrecognized shape, a partial match,
    /// or (when `known_signatures` is empty) nothing yet established as known at all. Absence of
    /// a positive match is drift, never a default pass (the same "malformed/absent input must
    /// never read as safe" principle [`crate::pressure`]/[`crate::forecast`]/[`crate::baseline`]
    /// already apply to their own inputs).
    Drifted,
}

/// The result of [`assess_layout`]: the support determination, human-readable evidence naming
/// the concrete markers compared (AC2), and the authority ceiling this drift recommends
/// (`SI-004`) - `None` only when the layout was [`LayoutSupport::Recognized`].
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutDriftFinding {
    pub support: LayoutSupport,
    pub evidence: String,
    pub recommended_authority_ceiling: Option<AuthorityLevel>,
}

/// A recognized layout never reduces the caller's own authority ceiling - this function returns
/// `None` rather than a "no change" sentinel that a future edit could mistake for a real value.
const RECOGNIZED_CEILING: Option<AuthorityLevel> = None;

/// A drifted or unrecognized layout is capped at `Observe` - the lowest ceiling this crate's
/// vocabulary can express, matching `SI-004`'s "cannot preserve destructive capabilities":
/// whatever the caller's authority was computed to be from provider identity alone, a structural
/// mismatch recommends refusing it down to observation-only, never a partial reduction that
/// still permits mutation.
const DRIFTED_CEILING: Option<AuthorityLevel> = Some(AuthorityLevel::Observe);

/// Compares `observed` against `known_signatures`, the closed set of layouts this caller
/// currently recognizes for the provider named by `provider_id`.
///
/// `provider_id` is carried only as an opaque evidence label baked into the returned finding's
/// `evidence` string - it never participates in the comparison itself. This is `SI-004`
/// discharged by construction: two calls with different `provider_id` values and identical
/// `known_signatures`/`observed` always return the identical [`LayoutSupport`] and
/// `recommended_authority_ceiling` (see the `provider_name_never_changes_the_drift_verdict`
/// test) - a recognized provider name cannot rescue a structurally drifted layout, because
/// nothing here ever looks at the name to decide that.
pub fn assess_layout(
    provider_id: &str,
    known_signatures: &[LayoutSignature],
    observed: &LayoutSignature,
) -> LayoutDriftFinding {
    if known_signatures.iter().any(|known| known == observed) {
        LayoutDriftFinding {
            support: LayoutSupport::Recognized,
            evidence: format!(
                "provider_id={provider_id}: observed layout markers {:?} match a known signature",
                observed.markers()
            ),
            recommended_authority_ceiling: RECOGNIZED_CEILING,
        }
    } else {
        LayoutDriftFinding {
            support: LayoutSupport::Drifted,
            evidence: format!(
                "provider_id={provider_id}: observed layout markers {:?} match none of {} known signature(s)",
                observed.markers(),
                known_signatures.len()
            ),
            recommended_authority_ceiling: DRIFTED_CEILING,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baseline::{AnomalySeverity, InsufficientBaselineReason};

    fn stable_baseline(value: f64, count: usize) -> Baseline {
        let mut baseline = Baseline::new(count.max(6));
        for _ in 0..count {
            baseline.observe(value);
        }
        baseline
    }

    // --- session explosion ---

    #[test]
    fn normal_session_count_is_not_flagged() {
        let baseline = stable_baseline(5.0, 12);
        let finding = assess_session_explosion(&baseline, 5.0);
        assert_eq!(finding.signal, StructuralSignal::SessionCount);
        match finding.assessment {
            AnomalyAssessment::Assessed { severity, .. } => {
                assert_eq!(severity, AnomalySeverity::Normal)
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    #[test]
    fn session_explosion_is_flagged_with_observed_count_in_evidence() {
        let baseline = stable_baseline(5.0, 12);
        let finding = assess_session_explosion(&baseline, 5_000.0);
        assert_eq!(finding.signal, StructuralSignal::SessionCount);
        match finding.assessment {
            AnomalyAssessment::Assessed {
                severity,
                observed_value,
                ..
            } => {
                assert_eq!(severity, AnomalySeverity::Anomalous);
                assert_eq!(observed_value, 5_000.0);
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    #[test]
    fn session_explosion_with_insufficient_history_is_insufficient_not_normal() {
        let baseline = Baseline::new(20);
        let finding = assess_session_explosion(&baseline, 5_000.0);
        assert_eq!(
            finding.assessment,
            AnomalyAssessment::InsufficientBaseline(InsufficientBaselineReason::TooFewObservations)
        );
    }

    // --- giant artifact ---

    #[test]
    fn normal_artifact_size_is_not_flagged() {
        let baseline = stable_baseline(2_000_000.0, 12); // ~2 MB typical
        let finding = assess_giant_artifact(&baseline, 2_050_000.0);
        assert_eq!(finding.signal, StructuralSignal::ArtifactSize);
        match finding.assessment {
            AnomalyAssessment::Assessed { severity, .. } => {
                assert_eq!(severity, AnomalySeverity::Normal)
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    #[test]
    fn giant_artifact_is_flagged_with_observed_size_in_evidence() {
        let baseline = stable_baseline(2_000_000.0, 12); // ~2 MB typical
        let finding = assess_giant_artifact(&baseline, 20_000_000_000.0); // 20 GB
        assert_eq!(finding.signal, StructuralSignal::ArtifactSize);
        match finding.assessment {
            AnomalyAssessment::Assessed {
                severity,
                observed_value,
                ..
            } => {
                assert_eq!(severity, AnomalySeverity::Anomalous);
                assert_eq!(observed_value, 20_000_000_000.0);
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    // --- orphan growth ---

    #[test]
    fn stable_orphan_count_is_not_flagged() {
        let baseline = stable_baseline(3.0, 12);
        let finding = assess_orphan_growth(&baseline, 3.0);
        assert_eq!(finding.signal, StructuralSignal::OrphanCount);
        match finding.assessment {
            AnomalyAssessment::Assessed { severity, .. } => {
                assert_eq!(severity, AnomalySeverity::Normal)
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    #[test]
    fn runaway_orphan_growth_is_flagged() {
        let baseline = stable_baseline(3.0, 12);
        let finding = assess_orphan_growth(&baseline, 900.0);
        assert_eq!(finding.signal, StructuralSignal::OrphanCount);
        match finding.assessment {
            AnomalyAssessment::Assessed { severity, .. } => {
                assert_eq!(severity, AnomalySeverity::Anomalous)
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    // --- layout drift: recognized ---

    #[test]
    fn recognized_layout_does_not_reduce_ceiling() {
        let known = vec![LayoutSignature::new([
            "sessions/".to_string(),
            "config.json".to_string(),
        ])];
        let observed = LayoutSignature::new(["config.json".to_string(), "sessions/".to_string()]);
        let finding = assess_layout("claude", &known, &observed);
        assert_eq!(finding.support, LayoutSupport::Recognized);
        assert_eq!(finding.recommended_authority_ceiling, None);
        assert!(finding.evidence.contains("claude"));
    }

    #[test]
    fn signature_equality_is_order_independent_and_deduplicated() {
        let a = LayoutSignature::new(["b".to_string(), "a".to_string(), "a".to_string()]);
        let b = LayoutSignature::new(["a".to_string(), "b".to_string()]);
        assert_eq!(a, b);
    }

    // --- layout drift: drifted ---

    #[test]
    fn unrecognized_layout_reduces_ceiling_to_observe() {
        let known = vec![LayoutSignature::new([
            "sessions/".to_string(),
            "config.json".to_string(),
        ])];
        let observed = LayoutSignature::new(["totally_different_shape/".to_string()]);
        let finding = assess_layout("claude", &known, &observed);
        assert_eq!(finding.support, LayoutSupport::Drifted);
        assert_eq!(
            finding.recommended_authority_ceiling,
            Some(AuthorityLevel::Observe)
        );
        assert!(finding.evidence.contains("totally_different_shape/"));
    }

    #[test]
    fn empty_known_signatures_never_recognizes_anything() {
        let observed = LayoutSignature::new(["config.json".to_string()]);
        let finding = assess_layout("claude", &[], &observed);
        assert_eq!(finding.support, LayoutSupport::Drifted);
        assert_eq!(
            finding.recommended_authority_ceiling,
            Some(AuthorityLevel::Observe)
        );
    }

    #[test]
    fn empty_observed_markers_is_drift_not_a_vacuous_match() {
        let known = vec![LayoutSignature::new(Vec::<String>::new())];
        let observed = LayoutSignature::new(["config.json".to_string()]);
        let finding = assess_layout("claude", &known, &observed);
        assert_eq!(finding.support, LayoutSupport::Drifted);
    }

    /// SI-004: a recognized provider name never rescues a drifted layout. Two calls that differ
    /// only in `provider_id` must reach the identical verdict and ceiling.
    #[test]
    fn provider_name_never_changes_the_drift_verdict() {
        let known = vec![LayoutSignature::new(["sessions/".to_string()])];
        let observed = LayoutSignature::new(["unrecognized_shape/".to_string()]);

        let well_known = assess_layout("claude", &known, &observed);
        let unknown_name = assess_layout("some-unheard-of-provider-xyz", &known, &observed);

        assert_eq!(well_known.support, unknown_name.support);
        assert_eq!(
            well_known.recommended_authority_ceiling,
            unknown_name.recommended_authority_ceiling
        );
        assert_eq!(well_known.support, LayoutSupport::Drifted);
        assert_eq!(
            well_known.recommended_authority_ceiling,
            Some(AuthorityLevel::Observe)
        );
    }

    /// Same proof for the recognized branch: a well-known provider name gets no special
    /// treatment when its layout genuinely matches, either.
    #[test]
    fn provider_name_never_changes_the_recognized_verdict() {
        let known = vec![LayoutSignature::new(["sessions/".to_string()])];
        let observed = LayoutSignature::new(["sessions/".to_string()]);

        let well_known = assess_layout("claude", &known, &observed);
        let unknown_name = assess_layout("some-unheard-of-provider-xyz", &known, &observed);

        assert_eq!(well_known.support, unknown_name.support);
        assert_eq!(
            well_known.recommended_authority_ceiling,
            unknown_name.recommended_authority_ceiling
        );
        assert_eq!(well_known.support, LayoutSupport::Recognized);
    }
}

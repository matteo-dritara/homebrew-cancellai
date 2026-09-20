//! Behavioral baseline anomaly detection (E14-S03) -
//! `docs/architecture/GUARDIAN_MODEL.md`'s "Baselines": "Baselines are local and metadata-only.
//! The first implementation should prefer transparent robust statistics/heuristics over opaque
//! ML." and "Detection": "baseline deviation; session-count explosion; unexpected giant
//! artifacts; provider layout drift; orphan-state growth."
//!
//! Same isolation as [`crate::pressure`] and [`crate::forecast`] (`SI-027`, "Detection severity
//! does not create authority"): this module holds no reference to `cancellai-safety` and never
//! will, and returns no type an execution path could consume - it observes and explains, it
//! never decides or acts (AC2's "never directly destructive" is discharged by this absence, not
//! by a runtime check). [`Baseline`] and [`Baseline::assess`] are pure/deterministic over their
//! own state and a caller-supplied value: no I/O, no clock, no shared state - a caller owns
//! feeding it observations (this crate's `cancellai-store` sibling's bounded rollups are the
//! intended upstream, same "primitive delivered, no orchestrator yet" precedent as
//! `crate::pressure`/`crate::forecast`). Input is a plain `f64` metadata reading (a count, a
//! byte size, a rate) - never a path, prompt, or file content, matching "without content
//! inspection".
//!
//! AC1 ("baseline uses bounded analytical memory") is satisfied by [`Baseline`] holding at most
//! [`Baseline::capacity`] observations at any time - `observe` evicts the oldest reading before
//! admitting a new one past that bound, so memory never grows with the number of observations
//! ever seen, only with the configured window. AC2 ("anomaly score is explanatory and never
//! directly destructive") is satisfied by [`AnomalyAssessment`] always carrying the observed
//! value, the baseline median and spread it was compared against, and the deviation expressed in
//! median-absolute-deviations - a caller (or a test) can always reconstruct *why* a severity was
//! assigned, never just read an opaque score.

use std::collections::VecDeque;

/// How far a single observation sits from the current baseline, in order from least to most
/// unusual so callers can compare/threshold by position like [`crate::pressure::PressureState`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnomalySeverity {
    Normal,
    Elevated,
    Anomalous,
}

/// A deviation of at least this many median-absolute-deviations from the baseline median is
/// [`AnomalySeverity::Elevated`]; a robust analogue of "about two standard deviations" that does
/// not assume a normal distribution (calibrated conservatively, like `pressure`'s and
/// `forecast`'s own thresholds - `docs/architecture/GUARDIAN_MODEL.md`: "the exact function is
/// calibrated later").
const ELEVATED_DEVIATION_MADS: f64 = 3.0;

/// A deviation at or beyond this many median-absolute-deviations is [`AnomalySeverity::Anomalous`].
const ANOMALOUS_DEVIATION_MADS: f64 = 6.0;

/// Fewer than this many observations in the window cannot support a baseline at all -
/// [`Baseline::assess`] returns [`AnomalyAssessment::InsufficientBaseline`] rather than silently
/// comparing against zero or one point (an "unknown baseline" read as "normal" would be exactly
/// the unknown-to-safe promotion `docs/development/AGENT_PROTOCOL.md`'s falsification axes warn
/// against, here applied to detection rather than mutation authority).
const MIN_OBSERVATIONS_FOR_BASELINE: usize = 3;

/// A baseline whose observations are all identical (MAD of `0.0`) would divide-by-zero-style
/// blow up any nonzero deviation into an infinite (or, after this floor, a tiny-denominator-huge)
/// MAD count - a degenerate baseline that has simply never yet seen its own natural noise would
/// then read ordinary future noise as maximally anomalous. Flooring the MAD at a fraction of the
/// baseline's own magnitude gives a flat baseline some proportional tolerance instead (a signal
/// that has held steady at `100` tolerates a `98`-`102` wobble the same way a signal steady at
/// `100_000` tolerates a proportionally larger absolute wobble), while [`MIN_MAD_ABSOLUTE_FLOOR`]
/// still catches the degenerate case of a baseline sitting at (or near) zero itself.
const MIN_MAD_RELATIVE_FLOOR_FRACTION: f64 = 0.01;

/// Absolute floor under [`MIN_MAD_RELATIVE_FLOOR_FRACTION`], for a baseline whose median is
/// itself at or near `0.0` (where a relative floor would itself collapse to zero).
const MIN_MAD_ABSOLUTE_FLOOR: f64 = 1e-9;

/// Why [`Baseline::assess`] could not produce a severity. Named concretely, like
/// [`crate::forecast::InsufficientDataReason`], rather than a bare refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsufficientBaselineReason {
    /// Fewer than [`MIN_OBSERVATIONS_FOR_BASELINE`] observations have been recorded yet.
    TooFewObservations,
}

/// The result of comparing one observation against the current baseline. Every non-insufficient
/// variant carries the numbers a reader needs to reconstruct the judgment (AC2): the value
/// observed, the baseline's own median and (floored) MAD, and the deviation expressed in units of
/// that MAD - never a bare severity with no supporting evidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnomalyAssessment {
    InsufficientBaseline(InsufficientBaselineReason),
    Assessed {
        severity: AnomalySeverity,
        observed_value: f64,
        baseline_median: f64,
        baseline_mad: f64,
        deviation_mads: f64,
    },
}

/// A local, bounded, metadata-only baseline over one numeric signal (a session count, a byte
/// size, a rate - never a path or content). Holds at most `capacity` of the most recent
/// observations; robust statistics (median/MAD) rather than mean/standard-deviation, so a single
/// outlier already in the window cannot itself dominate the comparison the way an unbounded mean
/// would let it.
#[derive(Debug, Clone)]
pub struct Baseline {
    capacity: usize,
    window: VecDeque<f64>,
}

impl Baseline {
    /// `capacity` is clamped to at least [`MIN_OBSERVATIONS_FOR_BASELINE`]: a smaller window
    /// could never produce anything but [`AnomalyAssessment::InsufficientBaseline`], which is a
    /// caller misconfiguration this constructor refuses to manufacture silently.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(MIN_OBSERVATIONS_FOR_BASELINE),
            window: VecDeque::with_capacity(capacity.max(MIN_OBSERVATIONS_FOR_BASELINE)),
        }
    }

    /// The configured bound on how many observations this baseline ever holds at once (AC1).
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// How many observations are currently held (`<= capacity()` always).
    pub fn len(&self) -> usize {
        self.window.len()
    }

    pub fn is_empty(&self) -> bool {
        self.window.is_empty()
    }

    /// Admits `value` into the window, evicting the oldest observation first if already at
    /// `capacity` - the window's size never exceeds `capacity`, regardless of how many
    /// observations have been admitted over the baseline's lifetime (AC1). A non-finite `value`
    /// (NaN or infinite) is refused rather than admitted: a malformed reading must never silently
    /// widen the baseline's own spread and make future genuine anomalies look normal by
    /// comparison - the unknown-to-safe promotion this module's own [`Baseline::assess`] refuses
    /// on the read side would otherwise be reintroduced on the write side.
    pub fn observe(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        if self.window.len() == self.capacity {
            self.window.pop_front();
        }
        self.window.push_back(value);
    }

    /// Compares `value` against the current window without modifying it - a caller decides
    /// separately (via [`Baseline::observe`]) whether an assessed value itself joins the
    /// baseline, so a single anomalous reading is judged against what came *before* it, never
    /// against a baseline that already includes it.
    ///
    /// A non-finite `value` is always [`AnomalySeverity::Anomalous`] with an infinite deviation,
    /// never silently read as calm (same principle as [`crate::pressure`]'s NaN handling):
    /// malformed input on a detection axis must never read as safe.
    pub fn assess(&self, value: f64) -> AnomalyAssessment {
        if self.window.len() < MIN_OBSERVATIONS_FOR_BASELINE {
            return AnomalyAssessment::InsufficientBaseline(
                InsufficientBaselineReason::TooFewObservations,
            );
        }
        let values: Vec<f64> = self.window.iter().copied().collect();
        let baseline_median = median(&values);
        let raw_mad = median(
            &values
                .iter()
                .map(|v| (v - baseline_median).abs())
                .collect::<Vec<f64>>(),
        );
        let baseline_mad = raw_mad
            .max(baseline_median.abs() * MIN_MAD_RELATIVE_FLOOR_FRACTION)
            .max(MIN_MAD_ABSOLUTE_FLOOR);

        if !value.is_finite() {
            return AnomalyAssessment::Assessed {
                severity: AnomalySeverity::Anomalous,
                observed_value: value,
                baseline_median,
                baseline_mad,
                deviation_mads: f64::INFINITY,
            };
        }

        let deviation_mads = (value - baseline_median).abs() / baseline_mad;

        let severity = if deviation_mads >= ANOMALOUS_DEVIATION_MADS {
            AnomalySeverity::Anomalous
        } else if deviation_mads >= ELEVATED_DEVIATION_MADS {
            AnomalySeverity::Elevated
        } else {
            AnomalySeverity::Normal
        };

        AnomalyAssessment::Assessed {
            severity,
            observed_value: value,
            baseline_median,
            baseline_mad,
            deviation_mads,
        }
    }

    /// Convenience for the common caller loop: assess `value` against the baseline as it stood
    /// before this call, then admit `value` into the baseline. Equivalent to calling
    /// [`Baseline::assess`] followed by [`Baseline::observe`] in that order - provided so a
    /// caller cannot accidentally reverse the order and compare a value against a baseline that
    /// already contains it.
    pub fn observe_and_assess(&mut self, value: f64) -> AnomalyAssessment {
        let assessment = self.assess(value);
        self.observe(value);
        assessment
    }
}

/// Same simplified median as [`crate::forecast`]'s own helper: for an even-length input this
/// takes the upper-middle element rather than averaging the two middle values. Kept consistent
/// with that sibling module rather than reintroducing a second median convention in this crate.
fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("caller filters non-finite values"));
    let mid = sorted.len() / 2;
    *sorted
        .get(mid)
        .expect("mid is in bounds when sorted is non-empty")
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- AC1: bounded memory ---

    #[test]
    fn window_never_exceeds_capacity_across_many_observations() {
        let mut baseline = Baseline::new(20);
        for i in 0..10_000 {
            baseline.observe(10.0 + (i % 3) as f64);
        }
        assert_eq!(baseline.len(), 20);
        assert_eq!(baseline.capacity(), 20);
    }

    #[test]
    fn capacity_is_clamped_to_the_minimum_usable_window() {
        let baseline = Baseline::new(1);
        assert_eq!(baseline.capacity(), MIN_OBSERVATIONS_FOR_BASELINE);
    }

    #[test]
    fn oldest_observation_is_evicted_first() {
        let mut baseline = Baseline::new(3);
        baseline.observe(1.0);
        baseline.observe(2.0);
        baseline.observe(3.0);
        baseline.observe(4.0); // evicts 1.0
        let values: Vec<f64> = baseline.window.iter().copied().collect();
        assert_eq!(values, vec![2.0, 3.0, 4.0]);
    }

    // --- AC1/AC2: insufficient baseline is refused, never read as "normal" ---

    #[test]
    fn empty_baseline_is_insufficient_not_normal() {
        let baseline = Baseline::new(10);
        assert_eq!(
            baseline.assess(1_000_000.0),
            AnomalyAssessment::InsufficientBaseline(InsufficientBaselineReason::TooFewObservations)
        );
    }

    #[test]
    fn below_minimum_observation_count_is_insufficient() {
        let mut baseline = Baseline::new(10);
        baseline.observe(1.0);
        baseline.observe(2.0);
        assert!(matches!(
            baseline.assess(2.0),
            AnomalyAssessment::InsufficientBaseline(_)
        ));
    }

    #[test]
    fn exactly_minimum_observation_count_is_sufficient() {
        let mut baseline = Baseline::new(10);
        baseline.observe(1.0);
        baseline.observe(1.0);
        baseline.observe(1.0);
        assert!(matches!(
            baseline.assess(1.0),
            AnomalyAssessment::Assessed { .. }
        ));
    }

    // --- known-normal corpus: stable signal never flags ---

    #[test]
    fn stable_repeating_signal_is_always_normal() {
        let mut baseline = Baseline::new(30);
        for _ in 0..30 {
            baseline.observe(100.0);
        }
        for v in [98.0, 99.0, 100.0, 101.0, 102.0] {
            let assessment = baseline.assess(v);
            match assessment {
                AnomalyAssessment::Assessed { severity, .. } => {
                    assert_eq!(
                        severity,
                        AnomalySeverity::Normal,
                        "value {v} flagged unexpectedly"
                    )
                }
                other => panic!("expected Assessed, got {other:?}"),
            }
        }
    }

    #[test]
    fn mild_natural_variation_stays_normal() {
        let mut baseline = Baseline::new(30);
        let mut toggle = false;
        for _ in 0..30 {
            baseline.observe(if toggle { 98.0 } else { 102.0 });
            toggle = !toggle;
        }
        let assessment = baseline.assess(100.0);
        match assessment {
            AnomalyAssessment::Assessed { severity, .. } => {
                assert_eq!(severity, AnomalySeverity::Normal)
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    // --- runaway corpus: clear divergence is detected and explained ---

    #[test]
    fn session_count_explosion_is_anomalous_and_explained() {
        let mut baseline = Baseline::new(30);
        for _ in 0..30 {
            baseline.observe(5.0); // typical session count
        }
        let assessment = baseline.assess(500.0); // runaway explosion
        match assessment {
            AnomalyAssessment::Assessed {
                severity,
                observed_value,
                baseline_median,
                deviation_mads,
                ..
            } => {
                assert_eq!(severity, AnomalySeverity::Anomalous);
                assert_eq!(observed_value, 500.0);
                assert_eq!(baseline_median, 5.0);
                assert!(deviation_mads >= ANOMALOUS_DEVIATION_MADS);
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    #[test]
    fn moderate_deviation_is_elevated_not_yet_anomalous() {
        let mut baseline = Baseline::new(30);
        for i in 0..30 {
            baseline.observe(100.0 + (i % 2) as f64); // MAD of 0.5 or 1.0 depending on parity
        }
        // Pick a value whose deviation lands in the elevated band by construction: derive it
        // from the same baseline's own median/MAD rather than a hand-guessed constant.
        let values: Vec<f64> = (0..30).map(|i| 100.0 + (i % 2) as f64).collect();
        let baseline_median = median(&values);
        let mad = median(
            &values
                .iter()
                .map(|v| (v - baseline_median).abs())
                .collect::<Vec<f64>>(),
        )
        .max(baseline_median.abs() * MIN_MAD_RELATIVE_FLOOR_FRACTION)
        .max(MIN_MAD_ABSOLUTE_FLOOR);
        let elevated_value = baseline_median + mad * (ELEVATED_DEVIATION_MADS + 0.5);
        let assessment = baseline.assess(elevated_value);
        match assessment {
            AnomalyAssessment::Assessed { severity, .. } => {
                assert_eq!(severity, AnomalySeverity::Elevated)
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    #[test]
    fn gradual_ramp_within_band_is_never_flagged_as_a_single_jump() {
        // A slow, steady increase should not itself look like a runaway spike once each new
        // value has become part of a baseline that has also shifted with it.
        let mut baseline = Baseline::new(20);
        for i in 0..20 {
            baseline.observe(100.0 + i as f64);
        }
        let assessment = baseline.observe_and_assess(120.0);
        match assessment {
            AnomalyAssessment::Assessed { severity, .. } => {
                assert_ne!(severity, AnomalySeverity::Anomalous)
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    // --- robustness to a single outlier already in the window ---

    #[test]
    fn single_past_outlier_does_not_permanently_poison_the_baseline() {
        let mut baseline = Baseline::new(21);
        for _ in 0..20 {
            baseline.observe(10.0);
        }
        baseline.observe(10_000.0); // one isolated spike enters the window
        // The median is robust to one outlier in a 21-wide window: a subsequent normal reading
        // must still read as Normal, not be dragged toward "anomalous" by the spike's mean-like
        // pull (a plain mean/stddev baseline would be skewed hard by this single point).
        let assessment = baseline.assess(10.0);
        match assessment {
            AnomalyAssessment::Assessed {
                severity,
                baseline_median,
                ..
            } => {
                assert_eq!(severity, AnomalySeverity::Normal);
                assert_eq!(baseline_median, 10.0);
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    #[test]
    fn outlier_naturally_ages_out_of_a_bounded_window() {
        let mut baseline = Baseline::new(5);
        baseline.observe(10.0);
        baseline.observe(10_000.0); // spike, second-oldest once the window fills
        baseline.observe(10.0);
        baseline.observe(10.0);
        baseline.observe(10.0);
        // Window is now exactly [10.0, 10000.0, 10.0, 10.0, 10.0] (capacity 5, FIFO). Eviction
        // removes the oldest entry first, so the spike needs two more admissions to age out: one
        // to evict the very first 10.0 ahead of it, one more to evict the spike itself.
        baseline.observe(10.0);
        baseline.observe(10.0);
        let values: Vec<f64> = baseline.window.iter().copied().collect();
        assert!(!values.contains(&10_000.0));
    }

    // --- malformed input never reads as calm ---

    #[test]
    fn nan_observation_is_never_admitted_to_the_baseline() {
        let mut baseline = Baseline::new(5);
        baseline.observe(1.0);
        baseline.observe(f64::NAN);
        baseline.observe(1.0);
        assert_eq!(
            baseline.len(),
            2,
            "NaN must be refused, not silently admitted"
        );
    }

    #[test]
    fn infinite_observation_is_never_admitted_to_the_baseline() {
        let mut baseline = Baseline::new(5);
        baseline.observe(1.0);
        baseline.observe(f64::INFINITY);
        baseline.observe(f64::NEG_INFINITY);
        assert_eq!(baseline.len(), 1);
    }

    #[test]
    fn nan_assessment_is_always_anomalous_never_normal() {
        let mut baseline = Baseline::new(10);
        for _ in 0..10 {
            baseline.observe(50.0);
        }
        let assessment = baseline.assess(f64::NAN);
        match assessment {
            AnomalyAssessment::Assessed {
                severity,
                deviation_mads,
                ..
            } => {
                assert_eq!(severity, AnomalySeverity::Anomalous);
                assert!(deviation_mads.is_infinite());
            }
            other => panic!("expected Assessed(Anomalous), got {other:?}"),
        }
    }

    // --- flat baseline (MAD == 0) never divides by zero into a spurious result ---

    #[test]
    fn flat_baseline_with_zero_spread_does_not_panic_and_floors_mad() {
        let mut baseline = Baseline::new(10);
        for _ in 0..10 {
            baseline.observe(42.0);
        }
        let assessment = baseline.assess(42.0);
        match assessment {
            AnomalyAssessment::Assessed {
                severity,
                baseline_mad,
                deviation_mads,
                ..
            } => {
                assert_eq!(severity, AnomalySeverity::Normal);
                assert!(baseline_mad > 0.0 && baseline_mad.is_finite());
                assert_eq!(deviation_mads, 0.0);
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    #[test]
    fn flat_baseline_small_relative_deviation_is_tolerated() {
        // The relative MAD floor gives a flat baseline proportional tolerance instead of
        // treating every future wobble as maximally anomalous.
        let mut baseline = Baseline::new(10);
        for _ in 0..10 {
            baseline.observe(42.0);
        }
        let assessment = baseline.assess(43.0);
        match assessment {
            AnomalyAssessment::Assessed { severity, .. } => {
                assert_eq!(severity, AnomalySeverity::Normal)
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    #[test]
    fn flat_baseline_large_deviation_is_still_anomalous() {
        let mut baseline = Baseline::new(10);
        for _ in 0..10 {
            baseline.observe(42.0);
        }
        let assessment = baseline.assess(50.0);
        match assessment {
            AnomalyAssessment::Assessed { severity, .. } => {
                assert_eq!(severity, AnomalySeverity::Anomalous)
            }
            other => panic!("expected Assessed, got {other:?}"),
        }
    }

    // --- ordering, matching PressureState's own pattern ---

    #[test]
    fn anomaly_severity_orders_low_to_high() {
        assert!(AnomalySeverity::Normal < AnomalySeverity::Elevated);
        assert!(AnomalySeverity::Elevated < AnomalySeverity::Anomalous);
    }

    // --- assess() never mutates the baseline it reads ---

    #[test]
    fn assess_does_not_mutate_the_baseline() {
        let mut baseline = Baseline::new(10);
        for _ in 0..5 {
            baseline.observe(1.0);
        }
        let before = baseline.len();
        let _ = baseline.assess(999.0);
        assert_eq!(baseline.len(), before);
    }
}

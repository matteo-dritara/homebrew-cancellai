//! Growth velocity and time-to-pressure forecasting (E14-S02) -
//! `docs/architecture/GUARDIAN_MODEL.md`'s "Forecasting": "Forecasts can include estimated time
//! to disk pressure or budget exhaustion. They must surface insufficient-data/uncertainty states
//! and are never authorization inputs by themselves."
//!
//! Same isolation as [`crate::pressure`] (SI-027's spirit, though this story carries no formal
//! safety obligation): this module holds no reference to `cancellai-safety` and never will - it
//! estimates a trend from a caller-supplied time series, it does not decide what cancellAI may
//! do. It is also a pure function of its input, like [`crate::pressure::classify`]: no I/O, no
//! clock, no shared state - a caller owns fetching the series (this crate's `cancellai-store`
//! sibling exposes `AnalyticalMemory::raw_samples`/`hourly_rollups`/`daily_rollups` for that) and
//! owns converting a caller-scale `value` into whatever unit it wants back; this module never
//! reads a database itself (same "primitive delivered, no orchestrator yet" precedent as
//! `cancellai-store`'s own E13/E12 stories).
//!
//! AC1 ("forecast uncertainty is surfaced") is satisfied by making [`GrowthEstimate`] and
//! [`PressureForecast`] carry an explicit [`Confidence`] alongside every numeric answer, and an
//! explicit [`InsufficientDataReason`] variant instead of a numeric answer at all when the input
//! cannot support one. AC2 ("sparse/noisy history produces insufficient-data rather than false
//! precision") is satisfied by [`fit_growth`] refusing to return a fit at all - not merely a
//! low-confidence one - when there are too few points, too short a time span, or too poor a
//! linear fit (`MIN_R_SQUARED`) to support a trend claim; a single isolated burst is additionally
//! bounded by one outlier trim ([`maybe_trim_outlier_and_refit`]) before that judgment is made, so
//! one spike cannot itself manufacture or hide a trend.

/// One growth measurement: how much was used/consumed at a point in time, in whatever unit the
/// caller's series uses (bytes, a `[0.0, 1.0]` capacity fraction, item count, ...) - this module
/// never interprets the unit, only the trend across it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrowthObservation {
    pub recorded_at_secs: u64,
    pub value: f64,
}

/// Why a series could not support a trend estimate. Each reason names a concrete, testable
/// insufficiency rather than a generic refusal, so a caller (or a test) can tell "too little
/// history" from "history present but too noisy to trust."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsufficientDataReason {
    /// Fewer than [`MIN_OBSERVATIONS`] usable (finite-valued) points.
    TooFewObservations,
    /// The usable points span less than [`MIN_SPAN_SECS`], or all share one timestamp.
    TimeSpanTooShort,
    /// A trend line could be fit, but it explains too little of the variance
    /// (`r_squared < MIN_R_SQUARED`) to be reported as a trend rather than noise.
    NoDiscernibleTrend,
}

/// Graded confidence in a returned estimate. Kept as a first-class, ordered field on every
/// non-insufficient answer (rather than folded into a single confident/unconfident bool) so a
/// caller can distinguish "reported and near-certain" from "reported but only just cleared the
/// bar" without those two being indistinguishable (AC1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// Growth rate estimated over the caller's own reporting `window_secs`, compatible with
/// [`crate::pressure::PressureInputs::growth_velocity_fraction_per_window`] (that field's own doc
/// names this module as the intended producer) - this module returns the raw estimate and lets
/// `pressure`'s own axis clamp/interpret range, rather than duplicating that clamping here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GrowthEstimate {
    InsufficientData(InsufficientDataReason),
    Available {
        /// Never negative: a declining series (net reclaiming) floors at `0.0` rather than
        /// reporting a negative velocity, matching `PressureInputs`'s own documented floor for
        /// this axis - a shrinking footprint contributes no growth pressure, it does not offset
        /// other axes as "negative pressure."
        velocity_per_window: f64,
        confidence: Confidence,
    },
}

/// A forecast of time until a series reaches `threshold_value`, given its observed trend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PressureForecast {
    InsufficientData(InsufficientDataReason),
    /// The series is flat, declining, or already at/past `threshold_value`: there is no future
    /// moment at which continuing the observed trend reaches the threshold, so no time estimate
    /// is offered rather than a spurious negative or infinite one.
    NotTrendingTowardThreshold,
    Forecast {
        estimated_seconds_to_threshold: f64,
        confidence: Confidence,
    },
}

/// Fewer usable points than this can fit *a* line but cannot support trusting it as a trend
/// distinct from noise. Calibrated conservatively, like `pressure`'s own thresholds
/// (`docs/architecture/GUARDIAN_MODEL.md`: "the exact function is calibrated later").
const MIN_OBSERVATIONS: usize = 3;

/// Usable points spanning less than one hour are treated as sparse regardless of count: a
/// handful of samples seconds apart describes an instant, not a trend.
const MIN_SPAN_SECS: u64 = 3_600;

/// A fit explaining less than half the variance in the series is noise, not a trend
/// (AC2: "noisy history produces insufficient-data rather than false precision").
const MIN_R_SQUARED: f64 = 0.5;

/// `r_squared` at or above this is reported as [`Confidence::High`].
const HIGH_CONFIDENCE_R_SQUARED: f64 = 0.85;
/// `r_squared` at or above this (and below the `High` cutoff) is reported as
/// [`Confidence::Medium`]; anything from [`MIN_R_SQUARED`] up to here is [`Confidence::Low`] -
/// still a reportable trend, just a weaker one, never itself a reason to withhold the estimate.
const MEDIUM_CONFIDENCE_R_SQUARED: f64 = 0.65;

/// A single point's fit residual beyond this many median-absolute-deviations is trimmed as one
/// outlier before judging the trend, so one isolated burst cannot itself manufacture a trend the
/// rest of the series does not show, nor hide a real one under an inflated residual sum.
const OUTLIER_MAD_MULTIPLIER: f64 = 3.0;

/// Trimming one point from a series this small would leave too few points to trust; below this,
/// an outlier (if any) is left in the fit and instead handled by [`MIN_R_SQUARED`] rejecting the
/// resulting poor fit outright.
const MIN_FOR_OUTLIER_TRIM: usize = 5;

#[derive(Debug, Clone, Copy)]
struct LinearFit {
    slope_per_sec: f64,
    intercept: f64,
    r_squared: f64,
}

/// Ordinary least squares over `(seconds_since_first, value)` points. `None` only when the
/// points do not span more than one distinct time (a vertical fit is undefined) - callers filter
/// for enough points and span before calling this, so this is the last-resort degenerate case,
/// not the primary rejection path.
fn ols_fit(points: &[(f64, f64)]) -> Option<LinearFit> {
    let n = points.len() as f64;
    if n < 2.0 {
        return None;
    }
    let mean_t = points.iter().map(|(t, _)| t).sum::<f64>() / n;
    let mean_v = points.iter().map(|(_, v)| v).sum::<f64>() / n;

    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for (t, v) in points {
        numerator += (t - mean_t) * (v - mean_v);
        denominator += (t - mean_t) * (t - mean_t);
    }
    if denominator <= 0.0 {
        return None;
    }
    let slope_per_sec = numerator / denominator;
    let intercept = mean_v - slope_per_sec * mean_t;

    let mut ss_res = 0.0;
    let mut ss_tot = 0.0;
    for (t, v) in points {
        let predicted = slope_per_sec * t + intercept;
        ss_res += (v - predicted).powi(2);
        ss_tot += (v - mean_v).powi(2);
    }
    let r_squared = if ss_tot <= 0.0 {
        // Every value identical: a flat line explains all of it (zero variance to explain).
        1.0
    } else {
        (1.0 - ss_res / ss_tot).max(0.0)
    };

    Some(LinearFit {
        slope_per_sec,
        intercept,
        r_squared,
    })
}

/// Median of a slice, via a sorted copy. Only ever called on small (rollup-scale) slices - see
/// this module's own doc comment on expected input size.
fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("caller filters non-finite values"));
    let mid = sorted.len() / 2;
    *sorted
        .get(mid)
        .expect("mid is in bounds: sorted is non-empty")
}

/// Trims at most one point - the single worst residual, and only if it is a statistical outlier
/// against the rest - then refits. Never trims more than one point per call and never trims below
/// [`MIN_FOR_OUTLIER_TRIM`], so a genuinely noisy (not merely one-spike) series is left to
/// [`MIN_R_SQUARED`] to reject rather than progressively hollowed out here.
fn maybe_trim_outlier_and_refit(points: &[(f64, f64)], initial: LinearFit) -> LinearFit {
    if points.len() < MIN_FOR_OUTLIER_TRIM {
        return initial;
    }
    let residuals: Vec<f64> = points
        .iter()
        .map(|(t, v)| (v - (initial.slope_per_sec * t + initial.intercept)).abs())
        .collect();
    let median_residual = median(&residuals);
    let mad = median(
        &residuals
            .iter()
            .map(|r| (r - median_residual).abs())
            .collect::<Vec<_>>(),
    )
    .max(f64::EPSILON);

    let (worst_index, worst_residual) = residuals
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).expect("residuals are finite"))
        .map(|(i, r)| (i, *r))
        .expect("points is non-empty: length checked above");

    if worst_residual <= OUTLIER_MAD_MULTIPLIER * mad {
        return initial;
    }
    let trimmed: Vec<(f64, f64)> = points
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != worst_index)
        .map(|(_, p)| *p)
        .collect();
    ols_fit(&trimmed).unwrap_or(initial)
}

/// Fraction of the fitted (whole-series) slope the *recent segment*'s own slope must retain for
/// the whole-series fit to be trusted as an *ongoing* trend, not a burst that has already ended.
/// Round 1 independent review of E14-S02: a jump followed by a flat plateau (`(0, 0.0)`,
/// `(3600, 100.0)`, `(7200, 100.0)`) produces a deceptively good OLS fit over the whole series -
/// `r_squared` alone cannot distinguish it from a genuinely continuing trend, because both shapes
/// fit a positive-slope line reasonably well with only a few points. But the *whole series*
/// fitting a line is not the same claim as the trend *still holding at the most recent
/// observations*, which is what a forward-looking velocity/forecast actually promises.
const RECENT_SLOPE_MIN_FRACTION: f64 = 0.5;

/// How much of the series (from the end) counts as "recent" for
/// [`trend_still_holds_at_the_most_recent_observation`] - a fraction rather than a fixed count so
/// this scales with series length, floored at [`MIN_RECENT_SEGMENT_POINTS`] so a short series
/// still gets a meaningful comparison.
const RECENT_SEGMENT_FRACTION: f64 = 0.5;
const MIN_RECENT_SEGMENT_POINTS: usize = 2;

/// Whether the fitted trend still holds at the most recent observations, judged by fitting the
/// most recent segment of the series *on its own* and comparing its slope to the whole series's.
/// A continuing trend keeps growing at roughly the fitted rate right through the recent segment;
/// a burst that has since leveled off or reversed does not, and reporting the whole-series
/// average as the *current* rate in that case is exactly the false precision AC2 exists to
/// refuse. This deliberately mirrors [`fit_growth`]'s own outlier handling
/// ([`maybe_trim_outlier_and_refit`]) rather than comparing two raw points: a bare two-point
/// delta is exactly as vulnerable to one noisy sample as the whole-series fit would be without
/// trimming, and would flag ordinary noise (e.g. `moderate_scatter_around_a_real_trend_is_
/// reported_at_lower_confidence_not_withheld`'s alternating wobble) as a false stall.
fn trend_still_holds_at_the_most_recent_observation(
    points: &[(f64, f64)],
    fit: &LinearFit,
) -> bool {
    if fit.slope_per_sec <= 0.0 {
        // A flat or declining fit makes no forward-looking growth claim that could go stale -
        // NotTrendingTowardThreshold and the velocity floor already handle this case.
        return true;
    }
    let recent_count = ((points.len() as f64 * RECENT_SEGMENT_FRACTION).ceil() as usize)
        .clamp(MIN_RECENT_SEGMENT_POINTS, points.len());
    let Some(recent) = points.get(points.len() - recent_count..) else {
        return true;
    };
    let Some(recent_initial_fit) = ols_fit(recent) else {
        // Degenerate recent segment (e.g. every point at the same timestamp); do not
        // additionally reject on this axis - fit_growth's own span/count gates already cover it.
        return true;
    };
    let recent_fit = maybe_trim_outlier_and_refit(recent, recent_initial_fit);
    recent_fit.slope_per_sec >= fit.slope_per_sec * RECENT_SLOPE_MIN_FRACTION
}

fn confidence_from_r_squared(r_squared: f64) -> Confidence {
    if r_squared >= HIGH_CONFIDENCE_R_SQUARED {
        Confidence::High
    } else if r_squared >= MEDIUM_CONFIDENCE_R_SQUARED {
        Confidence::Medium
    } else {
        Confidence::Low
    }
}

/// The shared core of [`estimate_growth`] and [`forecast_time_to_threshold`]: sorts, filters
/// non-finite values (malformed input never silently propagates into a fitted trend), checks
/// AC2's sufficiency gates, and returns a trimmed, judged fit or the specific reason it refused
/// one.
fn fit_growth(
    observations: &[GrowthObservation],
) -> Result<(LinearFit, Confidence), InsufficientDataReason> {
    let mut usable: Vec<&GrowthObservation> = observations
        .iter()
        .filter(|observation| observation.value.is_finite())
        .collect();
    if usable.len() < MIN_OBSERVATIONS {
        return Err(InsufficientDataReason::TooFewObservations);
    }
    usable.sort_by_key(|observation| observation.recorded_at_secs);

    let min_t = usable
        .first()
        .expect("length checked above")
        .recorded_at_secs;
    let max_t = usable
        .last()
        .expect("length checked above")
        .recorded_at_secs;
    if max_t.saturating_sub(min_t) < MIN_SPAN_SECS {
        return Err(InsufficientDataReason::TimeSpanTooShort);
    }

    let points: Vec<(f64, f64)> = usable
        .iter()
        .map(|observation| {
            (
                (observation.recorded_at_secs - min_t) as f64,
                observation.value,
            )
        })
        .collect();

    let initial_fit = ols_fit(&points).ok_or(InsufficientDataReason::TimeSpanTooShort)?;
    let fit = maybe_trim_outlier_and_refit(&points, initial_fit);

    if fit.r_squared < MIN_R_SQUARED {
        return Err(InsufficientDataReason::NoDiscernibleTrend);
    }
    if !trend_still_holds_at_the_most_recent_observation(&points, &fit) {
        return Err(InsufficientDataReason::NoDiscernibleTrend);
    }
    let confidence = confidence_from_r_squared(fit.r_squared);
    Ok((fit, confidence))
}

/// Estimates growth velocity over `window_secs` from `observations`. See [`GrowthEstimate`] for
/// what a caller receives instead of a number when the series cannot support one.
pub fn estimate_growth(observations: &[GrowthObservation], window_secs: u64) -> GrowthEstimate {
    match fit_growth(observations) {
        Err(reason) => GrowthEstimate::InsufficientData(reason),
        Ok((fit, confidence)) => {
            let velocity_per_window = (fit.slope_per_sec * window_secs as f64).max(0.0);
            GrowthEstimate::Available {
                velocity_per_window,
                confidence,
            }
        }
    }
}

/// Forecasts time until the trend observed in `observations` carries `current_value` to
/// `threshold_value`. See [`PressureForecast`] for the non-numeric outcomes.
pub fn forecast_time_to_threshold(
    observations: &[GrowthObservation],
    current_value: f64,
    threshold_value: f64,
) -> PressureForecast {
    match fit_growth(observations) {
        Err(reason) => PressureForecast::InsufficientData(reason),
        Ok((fit, confidence)) => {
            if threshold_value <= current_value || fit.slope_per_sec <= 0.0 {
                return PressureForecast::NotTrendingTowardThreshold;
            }
            let estimated_seconds_to_threshold =
                (threshold_value - current_value) / fit.slope_per_sec;
            if !estimated_seconds_to_threshold.is_finite() {
                return PressureForecast::NotTrendingTowardThreshold;
            }
            PressureForecast::Forecast {
                estimated_seconds_to_threshold,
                confidence,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: u64 = 3_600;
    const DAY: u64 = 24 * HOUR;

    fn series(points: &[(u64, f64)]) -> Vec<GrowthObservation> {
        points
            .iter()
            .map(|(t, v)| GrowthObservation {
                recorded_at_secs: *t,
                value: *v,
            })
            .collect()
    }

    // --- Clean linear trend ---

    fn clean_trend_hourly(hours: u64, rate_per_hour: f64) -> Vec<GrowthObservation> {
        series(
            &(0..hours)
                .map(|h| (h * HOUR, rate_per_hour * h as f64))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn clean_linear_trend_yields_high_confidence_matching_velocity() {
        let observations = clean_trend_hourly(24, 0.01);
        match estimate_growth(&observations, HOUR) {
            GrowthEstimate::Available {
                velocity_per_window,
                confidence,
            } => {
                assert!(
                    (velocity_per_window - 0.01).abs() < 1e-9,
                    "got {velocity_per_window}"
                );
                assert_eq!(confidence, Confidence::High);
            }
            other => panic!("expected Available, got {other:?}"),
        }
    }

    #[test]
    fn clean_linear_trend_forecasts_reasonable_time_to_threshold() {
        let observations = clean_trend_hourly(24, 0.01);
        // current value at hour 23 is 0.23; threshold 0.30 is 7 more units away at 0.01/hour.
        match forecast_time_to_threshold(&observations, 0.23, 0.30) {
            PressureForecast::Forecast {
                estimated_seconds_to_threshold,
                confidence,
            } => {
                let expected_hours = 7.0;
                assert!(
                    (estimated_seconds_to_threshold / HOUR as f64 - expected_hours).abs() < 0.01,
                    "got {estimated_seconds_to_threshold} seconds"
                );
                assert_eq!(confidence, Confidence::High);
            }
            other => panic!("expected Forecast, got {other:?}"),
        }
    }

    // --- Boundary values (falsification axis #7) ---

    #[test]
    fn two_observations_is_too_few() {
        let observations = series(&[(0, 0.0), (DAY, 1.0)]);
        assert_eq!(
            estimate_growth(&observations, HOUR),
            GrowthEstimate::InsufficientData(InsufficientDataReason::TooFewObservations)
        );
    }

    #[test]
    fn three_observations_spanning_a_full_day_is_sufficient() {
        let observations = series(&[(0, 0.0), (12 * HOUR, 0.5), (DAY, 1.0)]);
        assert!(matches!(
            estimate_growth(&observations, HOUR),
            GrowthEstimate::Available { .. }
        ));
    }

    #[test]
    fn span_just_under_one_hour_is_too_short() {
        let observations = series(&[(0, 0.0), (1_000, 0.1), (MIN_SPAN_SECS - 1, 0.2)]);
        assert_eq!(
            estimate_growth(&observations, HOUR),
            GrowthEstimate::InsufficientData(InsufficientDataReason::TimeSpanTooShort)
        );
    }

    #[test]
    fn span_of_exactly_one_hour_is_sufficient() {
        let observations = series(&[(0, 0.0), (1_800, 0.5), (MIN_SPAN_SECS, 1.0)]);
        assert!(matches!(
            estimate_growth(&observations, HOUR),
            GrowthEstimate::Available { .. }
        ));
    }

    #[test]
    fn all_observations_at_the_same_timestamp_is_too_short_a_span() {
        let observations = series(&[(1_000, 0.1), (1_000, 0.2), (1_000, 0.3)]);
        assert_eq!(
            estimate_growth(&observations, HOUR),
            GrowthEstimate::InsufficientData(InsufficientDataReason::TimeSpanTooShort)
        );
    }

    // --- Malformed / untrusted input (falsification axis #10) ---

    #[test]
    fn nan_values_are_filtered_not_propagated() {
        let mut observations = clean_trend_hourly(24, 0.01);
        observations[10].value = f64::NAN;
        // One NaN among 24 finite points still leaves plenty to fit.
        match estimate_growth(&observations, HOUR) {
            GrowthEstimate::Available {
                velocity_per_window,
                ..
            } => assert!((velocity_per_window - 0.01).abs() < 1e-9),
            other => panic!("expected Available, got {other:?}"),
        }
    }

    #[test]
    fn too_many_nan_values_is_too_few_observations() {
        let observations = series(&[(0, f64::NAN), (HOUR, 0.1), (MIN_SPAN_SECS, f64::NAN)]);
        assert_eq!(
            estimate_growth(&observations, HOUR),
            GrowthEstimate::InsufficientData(InsufficientDataReason::TooFewObservations)
        );
    }

    #[test]
    fn out_of_order_timestamps_give_the_same_result_as_sorted() {
        let sorted = clean_trend_hourly(24, 0.01);
        let mut shuffled = sorted.clone();
        shuffled.reverse();
        assert_eq!(
            estimate_growth(&sorted, HOUR),
            estimate_growth(&shuffled, HOUR)
        );
    }

    // --- Unknown-to-authority-style promotion: noise must never read as a confident answer ---

    #[test]
    fn noisy_history_with_no_trend_is_insufficient_not_a_confident_zero() {
        // Zigzag around a constant mean: no real trend, high residual variance.
        let observations = series(
            &(0..12)
                .map(|i| {
                    let t = i as u64 * HOUR;
                    let v = if i % 2 == 0 { 0.9 } else { 0.1 };
                    (t, v)
                })
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            estimate_growth(&observations, HOUR),
            GrowthEstimate::InsufficientData(InsufficientDataReason::NoDiscernibleTrend)
        );
    }

    #[test]
    fn isolated_burst_does_not_produce_a_wildly_skewed_estimate() {
        let mut observations = clean_trend_hourly(24, 0.01);
        // A single spike far off the trend line at one point.
        observations[12].value += 10.0;
        match estimate_growth(&observations, HOUR) {
            GrowthEstimate::Available {
                velocity_per_window,
                ..
            } => {
                // Without trimming, one +10.0 spike among 24 points still drags OLS slope far
                // above 0.01/hour; the trim should bring it back close to the true rate.
                assert!(
                    (velocity_per_window - 0.01).abs() < 0.005,
                    "burst skewed the estimate too far: got {velocity_per_window}"
                );
            }
            other => panic!(
                "expected the trend to still be reportable after trimming one outlier, got {other:?}"
            ),
        }
    }

    #[test]
    fn isolated_burst_too_small_a_series_to_trim_is_judged_on_fit_quality_alone() {
        // Only 3 points (below MIN_FOR_OUTLIER_TRIM): a burst here is never trimmed, so a severe
        // enough spike must instead fail MIN_R_SQUARED rather than being silently absorbed.
        let observations = series(&[(0, 0.0), (HOUR, 10.0), (2 * HOUR, 0.02)]);
        assert_eq!(
            estimate_growth(&observations, HOUR),
            GrowthEstimate::InsufficientData(InsufficientDataReason::NoDiscernibleTrend)
        );
    }

    // --- Declining series ---

    #[test]
    fn declining_series_floors_velocity_at_zero_never_negative() {
        let observations = clean_trend_hourly(24, -0.01);
        match estimate_growth(&observations, HOUR) {
            GrowthEstimate::Available {
                velocity_per_window,
                ..
            } => assert_eq!(velocity_per_window, 0.0),
            other => panic!("expected Available, got {other:?}"),
        }
    }

    #[test]
    fn declining_series_never_forecasts_exhaustion() {
        let observations = clean_trend_hourly(24, -0.01);
        assert_eq!(
            forecast_time_to_threshold(&observations, 0.5, 0.9),
            PressureForecast::NotTrendingTowardThreshold
        );
    }

    #[test]
    fn already_past_threshold_does_not_trend_toward_it() {
        let observations = clean_trend_hourly(24, 0.01);
        assert_eq!(
            forecast_time_to_threshold(&observations, 0.9, 0.5),
            PressureForecast::NotTrendingTowardThreshold
        );
    }

    #[test]
    fn flat_series_with_perfect_fit_never_forecasts_exhaustion() {
        let observations = series(&[(0, 0.5), (HOUR, 0.5), (MIN_SPAN_SECS, 0.5)]);
        assert_eq!(
            forecast_time_to_threshold(&observations, 0.5, 0.9),
            PressureForecast::NotTrendingTowardThreshold
        );
    }

    // --- Confidence grading is a first-class, ordered signal (AC1) ---

    #[test]
    fn confidence_orders_low_to_high() {
        assert!(Confidence::Low < Confidence::Medium);
        assert!(Confidence::Medium < Confidence::High);
    }

    #[test]
    fn moderate_scatter_around_a_real_trend_is_reported_at_lower_confidence_not_withheld() {
        // A real upward trend with enough scatter to miss the High cutoff but still clear
        // MIN_R_SQUARED - AC1's point that uncertainty is graded, not only binary.
        let observations = series(
            &(0..12)
                .map(|i| {
                    let t = i as u64 * HOUR;
                    let base = 0.02 * i as f64;
                    let wobble = if i % 2 == 0 { 0.03 } else { -0.03 };
                    (t, base + wobble)
                })
                .collect::<Vec<_>>(),
        );
        match estimate_growth(&observations, HOUR) {
            GrowthEstimate::Available { confidence, .. } => {
                assert!(confidence < Confidence::High);
            }
            other => panic!("expected a reportable (if lower-confidence) trend, got {other:?}"),
        }
    }

    // --- Burst followed by a sustained plateau (round 1 independent review, E14-S02) ---

    #[test]
    fn burst_then_flat_plateau_is_insufficient_data_not_a_continuing_trend() {
        // Round 1 independent review's own reproduction: a jump from 0 to 100 in the first hour,
        // then no further change for a second hour. The whole-series OLS fit alone reports this
        // as `Available { velocity_per_window: 50.0, .. }` - a completed burst is not an ongoing
        // trend, and reporting half its magnitude as the *current* growth rate is exactly the
        // false precision AC2 exists to refuse.
        let observations = series(&[(0, 0.0), (HOUR, 100.0), (2 * HOUR, 100.0)]);
        assert_eq!(
            estimate_growth(&observations, HOUR),
            GrowthEstimate::InsufficientData(InsufficientDataReason::NoDiscernibleTrend)
        );
        assert_eq!(
            forecast_time_to_threshold(&observations, 100.0, 200.0),
            PressureForecast::InsufficientData(InsufficientDataReason::NoDiscernibleTrend)
        );
    }

    #[test]
    fn burst_then_a_longer_plateau_never_reports_a_large_velocity() {
        // The same shape, spread over more points so the plateau itself has more than two
        // samples. Here the whole-series fit is already dominated by the flat majority (its own
        // slope is at or near zero), so the pre-existing zero-floor in `estimate_growth` already
        // gives the right answer without needing the recent-segment check to intervene - this
        // confirms that stays true rather than regressing to the round-1 false `50.0`.
        let mut points = vec![(0, 0.0), (HOUR, 100.0)];
        for h in 2..8 {
            points.push((h * HOUR, 100.0));
        }
        let observations = series(&points);
        match estimate_growth(&observations, HOUR) {
            GrowthEstimate::InsufficientData(_) => {}
            GrowthEstimate::Available {
                velocity_per_window,
                ..
            } => assert!(
                velocity_per_window < 5.0,
                "a burst absorbed into a long plateau must not still report a large velocity, \
                 got {velocity_per_window}"
            ),
        }
    }

    // --- Performance (falsification axis #11, bounded to this module's realistic input size) ---

    #[test]
    fn moderately_large_series_still_resolves_without_pathological_cost() {
        // Realistic upper bound for this module: rollup-scale history, not raw-sample-scale
        // (see this module's own doc comment on expected input size), exercised here at a
        // deliberately generous multiple of that to confirm nothing here is quadratic in a way
        // that would matter at rollup scale.
        let observations = clean_trend_hourly(500, 0.001);
        assert!(matches!(
            estimate_growth(&observations, HOUR),
            GrowthEstimate::Available { .. }
        ));
    }
}

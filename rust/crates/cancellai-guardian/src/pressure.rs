//! Pressure state model (E14-S01) - `docs/architecture/GUARDIAN_MODEL.md`'s "Pressure states".
//!
//! `SI-027` ("Detection severity does not create authority"): this module holds no reference to
//! `cancellai-safety`'s `AuthorityLevel`/`ActionClass`/`Reversibility` and never will - it
//! observes and classifies, it does not decide what cancellAI may do. [`PressureState`] is a
//! plain, closed enum with no path to an authorization decision anywhere in this crate graph;
//! satisfying AC2 ("pressure does not change authority by itself") is a property of what this
//! module does *not* import, not a runtime check.
//!
//! [`classify`] is a pure, deterministic function of its inputs and the previously observed
//! state (AC1: "pressure is deterministic from inputs and independently testable") - no I/O, no
//! shared state, no clock. A caller owns persisting `previous` across calls (matching this
//! crate's `cancellai-store` sibling's "primitive delivered, no orchestrator yet" precedent for
//! E13/E12): nothing here reads or writes a database, so there is no crash window internal to
//! this module to reason about.

/// GREEN/YELLOW/ORANGE/RED severity named by `docs/architecture/GUARDIAN_MODEL.md`'s "Pressure
/// states". Ordered low to high pressure so hysteresis can compare states by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PressureState {
    /// Normal.
    Green,
    /// Approaching soft budget/pressure; surface context.
    Yellow,
    /// Material risk; recommend or execute pre-authorized reversible actions.
    Orange,
    /// Critical disk/budget trajectory; prioritize safe remediation.
    Red,
}

/// The five signals `docs/architecture/GUARDIAN_MODEL.md` names for the pressure model. Every
/// field is a plain primitive - no path, no provider content, nothing beyond what a caller
/// already computed as a number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PressureInputs {
    /// Fraction of total disk capacity currently free, in `[0.0, 1.0]`. Lower means less free
    /// space and higher pressure. Out-of-range or NaN input is treated as maximum pressure on
    /// this axis (never silently read as "plenty of free space") - see
    /// [`safe_axis_higher_is_safer`].
    pub free_space_fraction: f64,
    /// Fraction of the caller's own self-budget already consumed, in `[0.0, 1.0]`. Higher means
    /// higher pressure.
    pub budget_usage_fraction: f64,
    /// Growth velocity as a fraction of capacity consumed per the caller's own reporting window
    /// (E14-S02 computes this value from `cancellai-store`'s analytical memory; this module only
    /// consumes it as an opaque number). A negative value (net reclaiming) never raises pressure
    /// on this axis; it floors at zero contribution rather than offsetting the other axes.
    pub growth_velocity_fraction_per_window: f64,
    /// Fraction of currently used space that is reclaimable without provider data loss, in
    /// `[0.0, 1.0]`. Higher reclaimability dampens the computed score, because remediation is
    /// cheap and low-risk when most of what is used can be freed - but see [`RECLAIM_DAMPENING_FACTOR`]:
    /// dampening is bounded and can never fully mask a near-full disk.
    pub reclaimable_fraction: f64,
    /// Whether a provider is actively writing right now. The same free-space/budget/growth
    /// numbers observed while a provider is actively writing describe a trend that is still
    /// worsening at observation time, not a snapshot of a settled state, so an active workload
    /// raises the computed score - it never lowers it, and never bypasses hysteresis.
    pub active_workload: bool,
}

/// Reclaimable space lowers the score by at most this much - even `1.0` (fully reclaimable)
/// leaves a zero-free-space observation at `Orange`, never `Green`/`Yellow`: reclaimability makes
/// remediation cheap, it does not make a full disk not a problem.
const RECLAIM_DAMPENING_FACTOR: f64 = 0.15;

/// An actively writing provider raises the score by this much, on top of whatever the
/// free-space/budget/growth axes already contribute.
const WORKLOAD_BUMP: f64 = 0.10;

/// Hysteresis thresholds on the `[0.0, ...]` pressure score. Each boundary carries a stricter
/// "up" threshold and a more lenient "down" threshold so a score oscillating near a boundary
/// does not flap the classified state back and forth every observation
/// (`docs/architecture/GUARDIAN_MODEL.md`: "Hysteresis prevents notification/action flapping").
const UP_GREEN_TO_YELLOW: f64 = 0.50;
const DOWN_YELLOW_TO_GREEN: f64 = 0.45;
const UP_YELLOW_TO_ORANGE: f64 = 0.75;
const DOWN_ORANGE_TO_YELLOW: f64 = 0.70;
const UP_ORANGE_TO_RED: f64 = 0.90;
const DOWN_RED_TO_ORANGE: f64 = 0.85;

/// Clamps a `[0.0, 1.0]`-documented axis where a *higher* value means *more* pressure (budget
/// usage, growth velocity), treating NaN as the maximum-pressure value `1.0` rather than
/// silently propagating it or reading it as "no pressure" - malformed input on a detection axis
/// must never read as safe.
fn safe_axis_higher_is_worse(value: f64) -> f64 {
    if value.is_nan() {
        1.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

/// Clamps a `[0.0, 1.0]`-documented axis where a *higher* value means *less* pressure (free
/// space, reclaimability). NaN is treated as `0.0` - the least safe reading on this axis - for
/// the same reason `safe_axis_higher_is_worse` treats it as `1.0`: whichever direction is worse
/// is what malformed input must resolve to, never whichever direction is safe.
fn safe_axis_higher_is_safer(value: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

/// Growth's own axis: a negative value (net reclaiming) floors at `0.0` rather than reducing the
/// score, and NaN is treated the same conservative way `safe_axis_higher_is_worse` treats it.
fn safe_growth_axis(value: f64) -> f64 {
    if value.is_nan() {
        1.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

/// Computes the continuous pressure score `classify` thresholds against. Not itself part of the
/// public contract (the enum is); kept private so the exact combination stays free to recalibrate
/// (`docs/architecture/GUARDIAN_MODEL.md`: "The exact function is calibrated later") without
/// disturbing [`PressureState`]'s meaning for callers.
fn pressure_score(inputs: &PressureInputs) -> f64 {
    let free_space_score = 1.0 - safe_axis_higher_is_safer(inputs.free_space_fraction);
    let budget_score = safe_axis_higher_is_worse(inputs.budget_usage_fraction);
    let growth_score = safe_growth_axis(inputs.growth_velocity_fraction_per_window);
    let base = free_space_score.max(budget_score).max(growth_score);

    let reclaim_relief =
        safe_axis_higher_is_safer(inputs.reclaimable_fraction) * RECLAIM_DAMPENING_FACTOR;
    let workload_penalty = if inputs.active_workload {
        WORKLOAD_BUMP
    } else {
        0.0
    };

    (base - reclaim_relief).max(0.0) + workload_penalty
}

/// Classifies `inputs` into a [`PressureState`], applying hysteresis against `previous` so a
/// score oscillating near one boundary does not flap the result every call. Steps at most one
/// level per boundary crossed, repeated until no further threshold is cleared - so a score that
/// jumps several levels in one observation (or falls back several) reaches its correct resting
/// state in one call, while a score that only clears an intermediate boundary stops there rather
/// than skipping past it (AC1: deterministic from `inputs` and `previous` alone; no I/O, no
/// clock, no shared state - see this module's own doc comment).
pub fn classify(inputs: &PressureInputs, previous: PressureState) -> PressureState {
    let score = pressure_score(inputs);
    let mut state = previous;

    loop {
        let next = match state {
            PressureState::Green if score >= UP_GREEN_TO_YELLOW => Some(PressureState::Yellow),
            PressureState::Yellow if score >= UP_YELLOW_TO_ORANGE => Some(PressureState::Orange),
            PressureState::Orange if score >= UP_ORANGE_TO_RED => Some(PressureState::Red),
            _ => None,
        };
        match next {
            Some(next_state) => state = next_state,
            None => break,
        }
    }

    loop {
        let next = match state {
            PressureState::Red if score < DOWN_RED_TO_ORANGE => Some(PressureState::Orange),
            PressureState::Orange if score < DOWN_ORANGE_TO_YELLOW => Some(PressureState::Yellow),
            PressureState::Yellow if score < DOWN_YELLOW_TO_GREEN => Some(PressureState::Green),
            _ => None,
        };
        match next {
            Some(next_state) => state = next_state,
            None => break,
        }
    }

    state
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calm_inputs() -> PressureInputs {
        PressureInputs {
            free_space_fraction: 1.0,
            budget_usage_fraction: 0.0,
            growth_velocity_fraction_per_window: 0.0,
            reclaimable_fraction: 0.0,
            active_workload: false,
        }
    }

    fn inputs_with_free_space(free_space_fraction: f64) -> PressureInputs {
        PressureInputs {
            free_space_fraction,
            ..calm_inputs()
        }
    }

    // --- Boundary values (falsification axis #7) ---

    #[test]
    fn exact_up_threshold_crosses_from_green_to_yellow() {
        // score = 1.0 - free_space_fraction = UP_GREEN_TO_YELLOW exactly.
        let inputs = inputs_with_free_space(1.0 - UP_GREEN_TO_YELLOW);
        assert_eq!(
            classify(&inputs, PressureState::Green),
            PressureState::Yellow
        );
    }

    #[test]
    fn just_below_up_threshold_stays_green() {
        let inputs = inputs_with_free_space(1.0 - UP_GREEN_TO_YELLOW + 0.001);
        assert_eq!(
            classify(&inputs, PressureState::Green),
            PressureState::Green
        );
    }

    #[test]
    fn exact_up_threshold_crosses_from_yellow_to_orange() {
        let inputs = inputs_with_free_space(1.0 - UP_YELLOW_TO_ORANGE);
        assert_eq!(
            classify(&inputs, PressureState::Yellow),
            PressureState::Orange
        );
    }

    #[test]
    fn exact_up_threshold_crosses_from_orange_to_red() {
        let inputs = inputs_with_free_space(1.0 - UP_ORANGE_TO_RED);
        assert_eq!(classify(&inputs, PressureState::Orange), PressureState::Red);
    }

    #[test]
    fn zero_free_space_is_red_from_any_previous_state() {
        let inputs = inputs_with_free_space(0.0);
        for previous in [
            PressureState::Green,
            PressureState::Yellow,
            PressureState::Orange,
            PressureState::Red,
        ] {
            assert_eq!(classify(&inputs, previous), PressureState::Red);
        }
    }

    // --- Hysteresis (verification contract: "boundary and hysteresis tests") ---

    #[test]
    fn hysteresis_prevents_flapping_around_a_boundary() {
        let above_up_threshold = inputs_with_free_space(1.0 - 0.52);
        let inside_hysteresis_band = inputs_with_free_space(1.0 - 0.48); // above 0.45, below 0.50

        let mut state = classify(&above_up_threshold, PressureState::Green);
        assert_eq!(state, PressureState::Yellow);

        // A score that falls back into the band (above the down-threshold, below the
        // up-threshold) must not flap the state back to Green on repeated observations.
        for _ in 0..5 {
            state = classify(&inside_hysteresis_band, state);
            assert_eq!(
                state,
                PressureState::Yellow,
                "score inside the hysteresis band must not flap"
            );
        }
    }

    #[test]
    fn hysteresis_requires_a_genuine_drop_to_step_down() {
        let borderline = inputs_with_free_space(1.0 - 0.48);
        let state = classify(&borderline, PressureState::Yellow);
        assert_eq!(
            state,
            PressureState::Yellow,
            "0.48 is above the 0.45 down-threshold"
        );

        let genuinely_lower = inputs_with_free_space(1.0 - 0.40);
        let state = classify(&genuinely_lower, PressureState::Yellow);
        assert_eq!(
            state,
            PressureState::Green,
            "0.40 is below the 0.45 down-threshold"
        );
    }

    #[test]
    fn multi_level_jump_up_reaches_correct_state_in_one_call() {
        let inputs = inputs_with_free_space(0.02); // score = 0.98, clears every up-threshold
        assert_eq!(classify(&inputs, PressureState::Green), PressureState::Red);
    }

    #[test]
    fn drop_from_red_stops_at_orange_when_only_reds_own_band_is_cleared() {
        // score = 0.80: below DOWN_RED_TO_ORANGE (0.85) but above DOWN_ORANGE_TO_YELLOW (0.70),
        // so a single call from Red must land on Orange, never skip straight to Yellow/Green.
        let inputs = inputs_with_free_space(1.0 - 0.80);
        assert_eq!(classify(&inputs, PressureState::Red), PressureState::Orange);
    }

    // --- Growth velocity floor ---

    #[test]
    fn negative_growth_velocity_never_raises_pressure() {
        let mut inputs = calm_inputs();
        inputs.growth_velocity_fraction_per_window = -50.0;
        assert_eq!(
            classify(&inputs, PressureState::Green),
            PressureState::Green
        );
    }

    // --- Malformed/untrusted input (falsification axis #10) ---

    #[test]
    fn nan_free_space_is_treated_as_maximum_pressure_not_silently_green() {
        let inputs = inputs_with_free_space(f64::NAN);
        assert_eq!(classify(&inputs, PressureState::Green), PressureState::Red);
    }

    #[test]
    fn nan_growth_velocity_is_treated_as_maximum_pressure() {
        let mut inputs = calm_inputs();
        inputs.growth_velocity_fraction_per_window = f64::NAN;
        assert_eq!(classify(&inputs, PressureState::Green), PressureState::Red);
    }

    #[test]
    fn out_of_range_fraction_above_one_is_clamped_not_amplified() {
        let mut inputs = calm_inputs();
        inputs.budget_usage_fraction = 5.0;
        // Clamped to 1.0, same as a legitimate fully-consumed budget - not five times worse.
        assert_eq!(classify(&inputs, PressureState::Green), PressureState::Red);
    }

    // --- Reclaimability dampens but cannot mask a genuine crisis ---

    #[test]
    fn full_reclaimability_dampens_score_but_not_below_orange_at_zero_free_space() {
        let mut inputs = inputs_with_free_space(0.0);
        inputs.reclaimable_fraction = 1.0;
        // score = 1.0 - 0.30 = 0.70, still at/above the Yellow/Orange up-threshold.
        assert_eq!(
            classify(&inputs, PressureState::Green),
            PressureState::Orange
        );
    }

    // --- Active workload raises urgency, never authority ---

    #[test]
    fn active_workload_raises_score_over_the_same_inputs_at_rest() {
        let idle = inputs_with_free_space(1.0 - 0.44);
        let mut busy = idle;
        busy.active_workload = true;
        assert_eq!(classify(&idle, PressureState::Green), PressureState::Green);
        assert_eq!(classify(&busy, PressureState::Green), PressureState::Yellow);
    }

    // --- Determinism (AC1) ---

    #[test]
    fn classify_is_pure_and_deterministic() {
        let inputs = inputs_with_free_space(0.3);
        let first = classify(&inputs, PressureState::Yellow);
        let second = classify(&inputs, PressureState::Yellow);
        assert_eq!(first, second);
    }

    // --- Ordering used internally by the hysteresis loops ---

    #[test]
    fn pressure_state_orders_low_to_high() {
        assert!(PressureState::Green < PressureState::Yellow);
        assert!(PressureState::Yellow < PressureState::Orange);
        assert!(PressureState::Orange < PressureState::Red);
    }
}

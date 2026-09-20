# Evidence Packet - E14-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex, round 1 - **FAIL** (`project/evidence/E14-VERIFIER-REVIEW.md`) -
  a burst followed by a sustained plateau (`(0,0.0)`, `(3600,100.0)`, `(7200,100.0)`) reported a
  false `Available { velocity_per_window: 50.0 }`/`Forecast { .. 7200.0 .. }` instead of
  insufficient data. Repaired below
- Change Risk: CR1 (declared at planning time in `project/epics/E14.json`; the diff adds a new,
  dependency-free, pure crate module with no filesystem, database, or authority surface, so no
  reclassification applies)
- Spec version/commit: `docs/architecture/GUARDIAN_MODEL.md` "Forecasting"

## Outcome

PASS after repair (see "Repair - round 1 independent review finding" below)

## Scope

`cancellai_guardian::forecast` (new module, `rust/crates/cancellai-guardian/src/forecast.rs`,
registered alongside the crate's existing `pressure` module in `src/lib.rs`). `GrowthObservation`
is a plain `(recorded_at_secs, value)` pair; `estimate_growth` and `forecast_time_to_threshold`
both go through one shared fit (`fit_growth`): sort, filter non-finite values, require at least
`MIN_OBSERVATIONS` (3) usable points spanning at least `MIN_SPAN_SECS` (1 hour), fit an ordinary
least-squares line, trim at most one outlier by a median-absolute-deviation check
(`maybe_trim_outlier_and_refit`, only above `MIN_FOR_OUTLIER_TRIM` = 5 points), then require the
resulting fit's `r_squared >= MIN_R_SQUARED` (0.5) before reporting anything. `GrowthEstimate` and
`PressureForecast` each carry an explicit `InsufficientDataReason`
(`TooFewObservations`/`TimeSpanTooShort`/`NoDiscernibleTrend`) instead of a number when that gate
is not cleared, and an explicit, ordered `Confidence` (`Low`/`Medium`/`High`, from `r_squared`)
alongside every number when it is. The crate gains no new dependency; the module references
neither `cancellai-safety` nor `cancellai-store`. No orchestrator wires this to a live
`AnalyticalMemory` scan yet, matching E14-S01/E12/E13's own "primitive delivered, no orchestrator
yet" precedent.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Forecast uncertainty is surfaced." | `GrowthEstimate::Available` and `PressureForecast::Forecast` both carry a `Confidence` field alongside their number; `Confidence` is `Ord` (`confidence_orders_low_to_high`) and is exercised at `High` (`clean_linear_trend_yields_high_confidence_matching_velocity`) and below `High` for a real but scattered trend (`moderate_scatter_around_a_real_trend_is_reported_at_lower_confidence_not_withheld` - a trend still reportable, just at graded lower confidence rather than binary withheld/not). | PASS |
| AC2 - "Sparse/noisy history produces insufficient-data rather than false precision." | `fit_growth` returns `Err(InsufficientDataReason)` - never a number - for too few points (`two_observations_is_too_few`), too short a span (`span_just_under_one_hour_is_too_short`, `all_observations_at_the_same_timestamp_is_too_short_a_span`), and no discernible trend in a noisy/zigzag series (`noisy_history_with_no_trend_is_insufficient_not_a_confident_zero`, `isolated_burst_too_small_a_series_to_trim_is_judged_on_fit_quality_alone`). Round 1 found a further false-precision case: a burst followed by a plateau fits a deceptively good whole-series line despite growth having already stopped. **Fix:** `fit_growth` now also fits the most recent half of the series on its own and requires its slope to retain at least half the whole-series slope, or refuses the same way (`trend_still_holds_at_the_most_recent_observation`); `burst_then_flat_plateau_is_insufficient_data_not_a_continuing_trend` reproduces the exact round-1 finding and confirms both `estimate_growth` and `forecast_time_to_threshold` now refuse it. | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Synthetic trend datasets" | `clean_linear_trend_yields_high_confidence_matching_velocity`, `clean_linear_trend_forecasts_reasonable_time_to_threshold`, `three_observations_spanning_a_full_day_is_sufficient`, `span_of_exactly_one_hour_is_sufficient`, `declining_series_floors_velocity_at_zero_never_negative`, `declining_series_never_forecasts_exhaustion`, `already_past_threshold_does_not_trend_toward_it`, `flat_series_with_perfect_fit_never_forecasts_exhaustion` - synthetic linear, flat, and declining series with known closed-form expected answers. | PASS |
| "Synthetic burst datasets" | `isolated_burst_does_not_produce_a_wildly_skewed_estimate` (a clean 24-point trend with one +10.0 spike still recovers a velocity within 0.005 of the true 0.01/hour rate after the one-outlier trim) and `isolated_burst_too_small_a_series_to_trim_is_judged_on_fit_quality_alone` (below the trim floor, the same kind of spike is instead rejected outright by `MIN_R_SQUARED`, never silently absorbed). | PASS |

## Adversarial-cases pass (CR1, executed at CR2+ rigor for a numeric/statistical domain)

Falsification axes worked before implementation (`adversarial-cases` skill):

| # | Axis | Case | Expected | Test |
| --- | --- | --- | --- | --- |
| 7 | Boundary values | 2 observations (below `MIN_OBSERVATIONS`); exactly 3 spanning a full day; span one second under/at `MIN_SPAN_SECS`; every observation at the identical timestamp | `TooFewObservations` / `Available` / `TimeSpanTooShort` / `Available` / `TimeSpanTooShort` respectively, never a panic or a divide-by-zero | `two_observations_is_too_few`, `three_observations_spanning_a_full_day_is_sufficient`, `span_just_under_one_hour_is_too_short`, `span_of_exactly_one_hour_is_sufficient`, `all_observations_at_the_same_timestamp_is_too_short_a_span` |
| 10 | Malformed/untrusted input | A single NaN among 24 good points; enough NaNs to leave fewer than `MIN_OBSERVATIONS` usable; observations supplied out of timestamp order | Filtered and unaffected; correctly falls back to `TooFewObservations`; identical result to the sorted input | `nan_values_are_filtered_not_propagated`, `too_many_nan_values_is_too_few_observations`, `out_of_order_timestamps_give_the_same_result_as_sorted` |
| 11 | Performance/large datasets | 500 hourly observations (a generous multiple of this module's realistic rollup-scale input - see the module's own doc comment on expected input size) | Resolves without pathological (quadratic-feeling) cost | `moderately_large_series_still_resolves_without_pathological_cost` |
| safety-specific: unknown-to-authority promotion (adapted: unknown-to-confident promotion) | A noisy, trendless zigzag series with a real (if small) span and enough points | Must read as `NoDiscernibleTrend`, never as a confident near-zero velocity - the conservative direction, not the convenient one | `noisy_history_with_no_trend_is_insufficient_not_a_confident_zero` |
| safety-specific: second-path check | Does this module decide anything `cancellai-safety` decides, or read as an authorization input? | No - the module contains no authority type and is never consumed as one; verifiable directly in the diff (no `cancellai-safety` import) | Verified by the diff itself, documented in the module's own doc comment |
| n/a | Path/identity, partial reads, links/mounts, provider drift, concurrency, crash/retry, platform differences | Not applicable - pure function over caller-supplied `(timestamp, value)` pairs, no filesystem/database/network/shared-state surface | - |
| domain-specific (not one of the eleven, added for this change) | A declining series (provider net-reclaiming) | `estimate_growth` floors velocity at `0.0` (never negative); `forecast_time_to_threshold` never reports an exhaustion estimate | `declining_series_floors_velocity_at_zero_never_negative`, `declining_series_never_forecasts_exhaustion` |
| domain-specific | Current value already at or past the forecast threshold | `NotTrendingTowardThreshold`, never a negative or zero time-to-threshold | `already_past_threshold_does_not_trend_toward_it` |
| domain-specific | A perfectly flat series (r_squared = 1.0, slope = 0) forecast toward a higher threshold | `NotTrendingTowardThreshold`, not a divide-by-zero or an infinite/NaN time | `flat_series_with_perfect_fit_never_forecasts_exhaustion` |
| 10 (round 1 repair) | A burst (0 -> 100 in one hour) followed by a flat plateau (no further change) | `InsufficientData(NoDiscernibleTrend)` from both `estimate_growth` and `forecast_time_to_threshold`, not the whole-series average reported as a false current rate | `burst_then_flat_plateau_is_insufficient_data_not_a_continuing_trend` |
| 10 (round 1 repair) | The same shape spread over more points, where the whole-series fit is already near-zero on its own | Never a large false velocity (either insufficient data, or a near-zero `Available`) | `burst_then_a_longer_plateau_never_reports_a_large_velocity` |
| 7 (round 1 repair, regression) | A real trend with alternating ±noise in its last two points (the recent-segment check must not be fooled by ordinary per-point noise) | Still reportable at graded lower confidence, unchanged from before the repair | `moderate_scatter_around_a_real_trend_is_reported_at_lower_confidence_not_withheld` (pre-existing test, re-verified against the new check) |

## Safety Evidence

No safety obligations are declared for this story (`Safety Obligations: none` in the executor
brief). The general Forecasting principle this document states -
"[forecasts] are never authorization inputs by themselves" - is nonetheless upheld the same way
SI-027 is upheld for `pressure`: no `AuthorityLevel`/`ActionClass`/`Reversibility` type is
imported or referenced anywhere in `forecast.rs`, verifiable by absence in the diff.

## Verification Commands

```text
cd rust && cargo fmt --check                                              # PASS (after one fmt pass)
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings   # PASS (after fixing one indexing_slicing finding in `median`)
cd rust && cargo check --workspace --all-targets                          # PASS
cd rust && cargo test --workspace                                         # PASS - 0 failed (22 cancellai-guardian::forecast tests after repair)
cd rust && cargo deny check                                               # PASS - advisories, bans, licenses, sources OK; no new dependency
python3 scripts/check_risk_classification.py check                       # PASS - no new story below its floor
python3 scripts/check_docs.py check                                       # PASS
python3 scripts/project_os.py check                                       # PASS
```

Not run: the Windows-target and Linux-target cross-compiled clippy passes AGENTS.md calls out for
changes to `cancellai-platform` or anything moving the workspace-wide lint surface - this story
touches neither. CI's existing per-platform `cargo check`/full quality matrix (`rust.yml`) still
runs this crate on all three platforms before merge.

## Compatibility

- Platforms/providers/schemas exercised: none - this module has no platform-specific code path
  and no provider/schema surface.

## Performance / operability

- `fit_growth` is O(n log n) in the number of usable observations (dominated by the sort and the
  one median computation in the outlier-trim step), exercised up to 500 synthetic points
  (`moderately_large_series_still_resolves_without_pathological_cost`) - a generous multiple of
  this module's realistic input size (bounded rollup history from `AnalyticalMemory`, not raw
  samples at scale; see the module's own doc comment).

## Repair - round 1 independent review finding

Codex's round-1 review (`project/evidence/E14-VERIFIER-REVIEW.md`) FAILed AC2: `fit_growth`'s
whole-series OLS fit alone cannot distinguish a genuinely continuing trend from a burst that has
already ended, because both shapes fit a positive-slope line reasonably well over only a few
points. The independent reproduction: `(0, 0.0)`, `(3600, 100.0)`, `(7200, 100.0)` - a jump in
the first hour, then no change in the second - produced `Available { velocity_per_window: 50.0,
confidence: Medium }` and `Forecast { estimated_seconds_to_threshold: 7200.0, .. }`, both false:
the series has already stopped growing.

Repair: `fit_growth` now also fits the most recent half of the series (at least 2 points) on its
own, applying the identical one-outlier-trim handling `maybe_trim_outlier_and_refit` already
gives the whole-series fit (so a single noisy recent point cannot itself manufacture a false
stall - this defeated a naive two-point-delta version of the check against
`moderate_scatter_around_a_real_trend_is_reported_at_lower_confidence_not_withheld`'s alternating
±0.03 wobble), and requires that recent-segment slope to retain at least half
(`RECENT_SLOPE_MIN_FRACTION`) of the whole-series slope. A burst that has since leveled off or
reversed fails this and is reported as `NoDiscernibleTrend` - the same insufficiency AC2 already
names for other cases - rather than the whole-series average being reported as the *current*
rate. Two new tests cover the exact round-1 reproduction and a longer-plateau variant; every
pre-existing test still passes unchanged, including the alternating-noise case that a cruder
two-point check would have broken.

## Documentation updated

- `docs/architecture/GUARDIAN_MODEL.md` - "Forecasting" section documents the implementation,
  its fit/trim/gating mechanism, and the AC1/AC2 discharge, matching `pressure`'s own entry style.
- `CHANGELOG.md` - `Unreleased`/`Added` entry.

## Method defects

- none

## Residual risks

- No orchestrator calls `estimate_growth`/`forecast_time_to_threshold` from a live
  `cancellai-store::rollup::AnalyticalMemory` scan yet - a later story's scope, matching every
  other primitive delivered in E12/E13/E14-S01.
- The sufficiency thresholds (`MIN_OBSERVATIONS`, `MIN_SPAN_SECS`, `MIN_R_SQUARED`, the confidence
  bands, the outlier-trim multiplier) are a first, documented-as-provisional calibration
  (`docs/architecture/GUARDIAN_MODEL.md`'s own "the exact function is calibrated later," extended
  here from `pressure`'s thresholds to this module's), not a value derived from real Guardian
  telemetry, which does not exist yet.
- The outlier trim removes at most one point per fit and only above `MIN_FOR_OUTLIER_TRIM` (5)
  points; a series with more than one genuine outlier, or fewer than 5 points, relies on
  `MIN_R_SQUARED` alone to reject a poor fit - covered by
  `isolated_burst_too_small_a_series_to_trim_is_judged_on_fit_quality_alone`, but a second or
  third simultaneous outlier above the trim floor is not separately exercised. The same applies
  to the recent-segment fit added in the round-1 repair (it reuses the identical trim logic, at
  the identical floor, over a smaller slice).
- `RECENT_SLOPE_MIN_FRACTION` (0.5) and `RECENT_SEGMENT_FRACTION` (0.5) are a first, provisional
  calibration, like the rest of this module's thresholds - a real deceleration slower than
  "halves within the most recent half of the series" is not caught by this check and relies on
  `MIN_R_SQUARED`/the other gates alone.

## Verifier verdict

Round 1: **FAIL** (Codex) - `project/evidence/E14-VERIFIER-REVIEW.md`. Repaired above; round 2
pending.

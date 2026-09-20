# Guardian Model

Guardian is a local observer and bounded remediation client of the same cancellAI engine. It is not a second safety implementation.

## Three separated concerns

### Detection

What is happening?

- free-disk capacity;
- provider/project budgets;
- growth velocity and acceleration;
- baseline deviation;
- session-count explosion;
- unexpected giant artifacts;
- provider layout drift;
- orphan-state growth.

### Decision

What would improve the situation?

The engine constructs candidate recommendations/plans using the normal artifact and policy model.

### Authority

What may cancellAI actually do?

Only the Effective Authority permits actions. Pressure or anomaly severity never self-escalates authority.

## Pressure states

The exact function is calibrated later, but the semantic states are:

- `GREEN` - normal.
- `YELLOW` - approaching soft budget/pressure; surface context.
- `ORANGE` - material risk; recommend or execute pre-authorized reversible actions.
- `RED` - critical disk/budget trajectory; prioritize safe remediation but do not bypass constitutional limits.

Hysteresis prevents notification/action flapping.

E14-S01 implements the classification itself: `cancellai_guardian::pressure`, a pure, dependency-free
module (no `cancellai-safety`/`cancellai-store` reference, no I/O, no clock) - `PressureState` is a
closed four-value enum and `classify(inputs, previous) -> PressureState` is deterministic in both
arguments (AC1), independently unit-tested without any live scanner or store. `PressureInputs`
carries the five named signals as plain primitives (free space, self-budget usage, growth velocity
- an opaque number this module consumes but does not compute; E14-S02 is where it comes from -
reclaimability, and whether a provider is actively writing). A continuous internal score combines
them (worst-axis-wins across free space/budget/growth, reclaimability dampens by a bounded amount
that can never mask a genuinely full disk, an active workload raises the score since the same
numbers observed mid-write describe a still-worsening trend); `classify` then walks that score
through explicit per-boundary up/down thresholds, stepping at most one level per boundary crossed
until none remain clear, so hysteresis holds even across a multi-level jump in one observation. A
NaN or out-of-range input on any axis resolves to that axis's worst reading, never its safest one -
malformed detection input must not read as calm. AC2 (SI-027: "pressure does not change authority
by itself") holds by construction: this module imports no `AuthorityLevel`/`ActionClass`/
`Reversibility` type and cannot express one - `PressureState` is not a type any authority decision
in this crate graph reads. No caller wires this to a live `cancellai-store` scan yet, matching
E13/E12's own "primitive delivered, no orchestrator yet" precedent.

## Forecasting

Forecasts can include estimated time to disk pressure or budget exhaustion. They must surface insufficient-data/uncertainty states and are never authorization inputs by themselves.

E14-S02 implements growth-rate estimation and time-to-threshold forecasting:
`cancellai_guardian::forecast`, the same pure, dependency-free shape as `pressure` (no
`cancellai-safety` reference, no I/O, no clock - a caller supplies the time series, this crate's
`cancellai-store` sibling's `AnalyticalMemory::raw_samples`/`hourly_rollups`/`daily_rollups` being
the intended source, wired by a later orchestrator). `estimate_growth` and
`forecast_time_to_threshold` share one fit (`fit_growth`): an ordinary-least-squares line over the
caller's `(timestamp, value)` observations, with at most one outlier trimmed by a
median-absolute-deviation check before judgment, so a single isolated burst cannot itself
manufacture or hide a trend. AC1 ("forecast uncertainty is surfaced") holds because every non-
insufficient answer carries an explicit, ordered `Confidence` (`Low`/`Medium`/`High`, from the
fit's `r_squared`) alongside its number - uncertainty is a field on the result, not an
implementation detail. AC2 ("sparse/noisy history produces insufficient-data rather than false
precision") holds because `fit_growth` refuses to return a fit at all, not merely a low-confidence
one, when there are too few usable points, too short a time span, or too poor a linear fit to
support a trend claim - a named `InsufficientDataReason` distinguishes "too little history" from
"history present but too noisy to trust." A declining or flat series never produces a negative
velocity or a spurious exhaustion forecast: growth velocity floors at zero contribution (matching
`PressureInputs::growth_velocity_fraction_per_window`'s own documented floor) and a non-positive
trend, or a series already at or past the threshold, yields `NotTrendingTowardThreshold` rather
than a nonsensical negative or infinite time. No caller wires this to a live `cancellai-store`
scan yet, matching E14-S01/E13/E12's own "primitive delivered, no orchestrator yet" precedent.

## Baselines

Baselines are local and metadata-only. The first implementation should prefer transparent robust statistics/heuristics over opaque ML. More sophisticated models are allowed only if their output remains advisory evidence and can be explained sufficiently for debugging.

E14-S03 implements the first baseline/anomaly primitive: `cancellai_guardian::baseline`, the same
pure, dependency-free shape as `pressure`/`forecast` (no `cancellai-safety` reference, no I/O, no
clock - a caller feeds it observations, this crate's `cancellai-store` sibling's bounded rollups
being the intended source, wired by a later orchestrator). `Baseline` holds a robust local model -
median and median-absolute-deviation (MAD), not mean/standard-deviation, so one outlier already in
the window cannot itself dominate the comparison - over a plain numeric metadata reading (a count,
a byte size, a rate; never a path or content, matching "without content inspection"). AC1
("baseline uses bounded analytical memory") holds because `Baseline::observe` evicts the oldest
reading before admitting a new one past its configured `capacity`, so its memory never grows with
the number of observations ever seen, only with that fixed window. AC2 ("anomaly score is
explanatory and never directly destructive") holds because `Baseline::assess` returns an
`AnomalyAssessment` that always carries the observed value, the baseline's own median and MAD, and
the deviation expressed in units of that MAD alongside an ordered `AnomalySeverity`
(`Normal`/`Elevated`/`Anomalous`) - never a bare score - and because the module imports no
authority/mutation type and returns none, the same by-construction argument `pressure` already
makes for SI-027. A baseline with fewer than three observations refuses to assess at all
(`AnomalyAssessment::InsufficientBaseline`) rather than reading "no baseline yet" as "normal" - the
same unknown-to-safe promotion hazard the falsification axes name for mutation authority, here
applied to detection. A degenerate, perfectly flat baseline (MAD of `0.0`) floors its MAD at a
fraction of the baseline's own magnitude rather than an absolute epsilon, so a flat signal still
tolerates proportional natural noise instead of reading every future wobble as maximally anomalous
regardless of scale. NaN/infinite input is never admitted into the baseline and always assesses as
`Anomalous` with infinite deviation - malformed detection input must not read as calm, matching
`pressure`'s own NaN handling. No caller wires this to a live `cancellai-store` scan yet, matching
E14-S01/S02's and E13/E12's own "primitive delivered, no orchestrator yet" precedent.

## Runtime

Target user-service mechanisms:

- macOS: `launchd` user agent.
- Linux: `systemd --user` where available, explicit fallback otherwise.
- Windows: user-scoped scheduled task/service design chosen through platform ADR.
- WSL: separate environment behavior; do not silently install a Windows host service from the Linux guest.

## Kill switch

Guardian must have an immediate local disable path. Disabling automation never prevents manual read-only inspection or recovery. Any in-flight destructive action still follows safety executor transaction semantics rather than being killed mid-syscall unsafely.

## Audit

Every Guardian remediation/recommendation records:

- detection evidence;
- pressure/anomaly state;
- policy resolution;
- authority result;
- sealed plan ID if any;
- execution result.

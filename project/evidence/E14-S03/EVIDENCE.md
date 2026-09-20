# Evidence Packet - E14-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E14 epic review
- Change Risk: CR2 (declared at planning time in `project/epics/E14.json`; the diff adds a new,
  dependency-free, pure crate module with no filesystem, database, or authority surface, so no
  reclassification applies - `check_risk_classification.py check` records no new story below its
  floor)
- Spec version/commit: `docs/architecture/GUARDIAN_MODEL.md` "Baselines"

## Outcome

PASS

## Scope

`cancellai_guardian::baseline` (new module, `rust/crates/cancellai-guardian/src/baseline.rs`,
registered alongside the crate's existing `pressure`/`forecast` modules in `src/lib.rs`).
`Baseline` holds a bounded `VecDeque<f64>` window (`capacity`, clamped to at least
`MIN_OBSERVATIONS_FOR_BASELINE` = 3): `observe` evicts the oldest reading before admitting a new
one past that bound and refuses non-finite values outright. `assess` computes the window's median
and median-absolute-deviation (MAD, floored at `max(raw_mad, |median| * 1%, 1e-9)` to give a
degenerate flat baseline proportional rather than zero tolerance) and classifies a caller-supplied
value's deviation, in MAD units, into `AnomalySeverity::{Normal, Elevated, Anomalous}` against two
fixed thresholds (3 and 6 MADs). Fewer than 3 observations in the window returns
`AnomalyAssessment::InsufficientBaseline` instead of comparing against too little data.
`observe_and_assess` composes `assess` then `observe` in that fixed order, so a value is always
judged against the baseline as it stood *before* that value, never one that already includes it.
The crate gains no new dependency; the module references neither `cancellai-safety` nor
`cancellai-store`. No orchestrator wires this to a live `AnalyticalMemory` scan yet, matching
E14-S01/S02/E12/E13's own "primitive delivered, no orchestrator yet" precedent.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Baseline uses bounded analytical memory." | `Baseline::observe` evicts the oldest entry before admitting a new one past `capacity`; `window_never_exceeds_capacity_across_many_observations` ingests 10,000 observations into a capacity-20 baseline and asserts `len() == 20` throughout. `capacity_is_clamped_to_the_minimum_usable_window` and `oldest_observation_is_evicted_first` pin the bound and the FIFO eviction order. | PASS |
| AC2 - "Anomaly score is explanatory and never directly destructive." | `AnomalyAssessment::Assessed` always carries `observed_value`, `baseline_median`, `baseline_mad`, and `deviation_mads` alongside `severity` - never a bare score (`session_count_explosion_is_anomalous_and_explained` asserts all four fields together). "Never directly destructive" holds by construction: the module imports no `cancellai-safety` type and returns none an execution path could consume - verifiable directly in the diff (no such import exists). | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Known-normal ... corpus" | `stable_repeating_signal_is_always_normal` (30 identical observations, then five nearby values all read `Normal`), `mild_natural_variation_stays_normal` (alternating values, midpoint reads `Normal`), `gradual_ramp_within_band_is_never_flagged_as_a_single_jump` (a steady one-per-step ramp does not itself read as a jump). | PASS |
| "... and runaway simulation corpus" | `session_count_explosion_is_anomalous_and_explained` (baseline steady at 5, a 500 spike reads `Anomalous` with `deviation_mads >= 6`), `moderate_deviation_is_elevated_not_yet_anomalous` (a value placed exactly in the `Elevated` band by construction from the baseline's own median/MAD reads `Elevated`, not `Anomalous`). | PASS |

## Adversarial-cases pass (CR2)

Falsification axes worked before implementation (`adversarial-cases` skill):

| # | Axis | Case | Expected | Test |
| --- | --- | --- | --- | --- |
| 7 | Boundary values | Empty baseline; exactly 2 observations (below minimum); exactly 3 (at minimum) | `InsufficientBaseline` / `InsufficientBaseline` / `Assessed` | `empty_baseline_is_insufficient_not_normal`, `below_minimum_observation_count_is_insufficient`, `exactly_minimum_observation_count_is_sufficient` |
| 10 | Malformed/untrusted input | NaN and +/-infinite observations offered to `observe`; NaN offered to `assess` | Refused on write (never admitted to the window); on read, always `Anomalous` with infinite deviation, never read as calm | `nan_observation_is_never_admitted_to_the_baseline`, `infinite_observation_is_never_admitted_to_the_baseline`, `nan_assessment_is_always_anomalous_never_normal` |
| 11 | Performance/large datasets | 10,000 sequential observations into a capacity-20 window | Bounded memory holds throughout, no unbounded growth | `window_never_exceeds_capacity_across_many_observations` |
| safety-specific: unknown-to-authority promotion (adapted: unknown-to-normal promotion) | A baseline with too few observations to judge anything | Must read as `InsufficientBaseline`, never as a confident `Normal` - the conservative direction, not the convenient one | `empty_baseline_is_insufficient_not_normal`, `below_minimum_observation_count_is_insufficient` |
| safety-specific: second-path check | Does this module decide anything `cancellai-safety` decides, or read as an authorization input? | No - the module contains no authority type and is never consumed as one; verifiable directly in the diff (no `cancellai-safety` import) | Verified by the diff itself, documented in the module's own doc comment |
| n/a | Path/identity, partial reads, links/mounts, provider drift, concurrency, crash/retry, platform differences | Not applicable - pure in-memory struct over caller-supplied `f64` readings, no filesystem/database/network/shared-state surface | - |
| domain-specific (not one of the eleven, added for this change) | A single isolated spike already inside the window | Median-based statistics resist the one outlier: a subsequent normal reading still assesses `Normal`, and the spike ages out under continued FIFO eviction without leaving a permanent skew | `single_past_outlier_does_not_permanently_poison_the_baseline`, `outlier_naturally_ages_out_of_a_bounded_window` |
| domain-specific | A perfectly flat baseline (MAD of `0.0`) | Never panics/divides-by-zero; floors MAD proportionally so small relative deviations stay `Normal` while a large deviation is still `Anomalous` | `flat_baseline_with_zero_spread_does_not_panic_and_floors_mad`, `flat_baseline_small_relative_deviation_is_tolerated`, `flat_baseline_large_deviation_is_still_anomalous` |
| domain-specific | Does `assess` mutate the state it reads? | No - `assess` takes `&self`; length is unchanged after a call | `assess_does_not_mutate_the_baseline` |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-027 "Detection severity does not create authority" | No `AuthorityLevel`/`ActionClass`/`Reversibility` type is imported or referenced anywhere in `baseline.rs`; `AnomalyAssessment`/`AnomalySeverity` are the only types this module returns and neither is consumed by any authority-deciding code in this crate graph | Verified by absence in the diff (no such import exists in the new file) | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                              # PASS (after one fmt pass over the new test module)
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings   # PASS, no findings
cd rust && cargo check --workspace --all-targets                          # PASS
cd rust && cargo test --workspace                                         # PASS - 0 failed (23 new cancellai-guardian::baseline tests, 58 total in the crate, no regressions in any other crate)
cd rust && cargo deny check                                               # PASS - advisories, bans, licenses, sources OK; no new dependency
python3 scripts/check_risk_classification.py check                       # PASS - no new story below its floor
python3 scripts/check_schemas.py check                                   # PASS
python3 scripts/check_fixtures.py check                                  # PASS
python3 scripts/check_docs.py check                                      # PASS
python3 scripts/project_os.py check                                      # PASS
```

Not run: the Windows-target and Linux-target cross-compiled clippy passes AGENTS.md calls out for
changes to `cancellai-platform` or anything moving the workspace-wide lint surface - this story
touches neither. CI's existing per-platform `cargo check`/full quality matrix (`rust.yml`) still
runs this crate on all three platforms before merge.

## Compatibility

- Platforms/providers/schemas exercised: none - this module has no platform-specific code path
  and no provider/schema surface.

## Performance / operability

- `Baseline::observe`/`assess` are O(capacity) per call (dominated by the median/MAD sort over the
  window), exercised across 10,000 sequential observations into a bounded window
  (`window_never_exceeds_capacity_across_many_observations`) with no unbounded growth - this
  module's realistic input is bounded rollup-scale history, not raw samples at scale, matching
  `forecast`'s own documented performance scope.

## Documentation updated

- `docs/architecture/GUARDIAN_MODEL.md` - "Baselines" section documents the implementation, the
  robust-statistics/bounded-window mechanism, and the AC1/AC2 discharge, matching
  `pressure`/`forecast`'s own entry style.
- `CHANGELOG.md` - `Unreleased`/`Added` entry.

## Method defects

- none

## Residual risks

- No orchestrator calls `Baseline::observe`/`assess` from a live
  `cancellai-store::rollup::AnalyticalMemory` scan yet - a later story's scope, matching every
  other primitive delivered in E12/E13/E14-S01/E14-S02.
- The thresholds (`ELEVATED_DEVIATION_MADS` = 3, `ANOMALOUS_DEVIATION_MADS` = 6,
  `MIN_MAD_RELATIVE_FLOOR_FRACTION` = 1%) are a first, documented-as-provisional calibration
  (`docs/architecture/GUARDIAN_MODEL.md`'s own "the exact function is calibrated later," extended
  here from `pressure`/`forecast`'s own thresholds to this module's), not a value derived from
  real Guardian telemetry, which does not exist yet.
- This module handles one numeric signal at a time (a caller runs one `Baseline` per signal, e.g.
  session count and byte footprint separately); a combined, cross-signal anomaly explanation
  (`docs/architecture/GUARDIAN_MODEL.md`'s "provider layout drift" as a structural rather than
  purely numeric signal) is out of this story's scope and not attempted here.
- The window's robustness to a single past outlier is exercised (`single_past_outlier_...`,
  `outlier_naturally_ages_out_...`); a window containing two or more simultaneous outliers is not
  separately exercised, matching the same disclosed residual `forecast`'s own outlier-trim
  mechanism already carries for this crate.

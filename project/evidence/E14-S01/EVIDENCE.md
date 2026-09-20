# Evidence Packet - E14-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E14 epic review
- Change Risk: CR2 (declared at planning time in `project/epics/E14.json`; the diff adds a new,
  dependency-free crate module with no filesystem, database, or authority surface, so no
  reclassification applies)
- Spec version/commit: `docs/architecture/GUARDIAN_MODEL.md` "Pressure states"; `docs/security/
  SAFETY_INVARIANTS.md` SI-027 ("Detection severity does not create authority")

## Outcome

PASS

## Scope

`cancellai_guardian::pressure` (new module, `rust/crates/cancellai-guardian/src/pressure.rs`,
exposed through a new `src/lib.rs` alongside the crate's existing skeleton `main.rs` - the same
bin+lib split `cancellai-tui` already uses). `PressureState` is the closed
GREEN/YELLOW/ORANGE/RED enum `docs/architecture/GUARDIAN_MODEL.md` names. `PressureInputs`
carries the five signals the same document names as pressure determinants: free space, self-budget
usage, growth velocity (an opaque number this module consumes but does not compute - E14-S02's
scope), reclaimability, and whether a provider is actively writing. `classify(inputs, previous) ->
PressureState` combines them into a continuous score (worst-of free-space/budget/growth, dampened
but not erased by reclaimability, raised by an active workload) and walks that score through
explicit per-boundary hysteresis thresholds. The crate gains no new dependency; the module
references neither `cancellai-safety` nor `cancellai-store`. No orchestrator wires this to a live
scan yet, matching E12/E13's own "primitive delivered, no orchestrator yet" precedent.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Pressure is deterministic from inputs and independently testable." | `classify` is a pure function (`&PressureInputs`, `PressureState`) -> `PressureState`, no I/O, no clock, no shared/interior mutable state. `classify_is_pure_and_deterministic` calls it twice with identical inputs and asserts equal results. All 17 tests in `pressure::tests` exercise the module directly, with no store, scanner, or filesystem fixture. | PASS |
| AC2 - "Pressure does not change authority by itself." | `pressure.rs` imports nothing from `cancellai-safety` and defines no function that produces or consumes an `AuthorityLevel`/`ActionClass`/`Reversibility` - verifiable directly in the diff (no such import exists) rather than by a runtime check, matching SI-027's "by construction" pattern already used for E12-S04/E13-S06. `cancellai-guardian`'s `Cargo.toml` is unchanged by this story. | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Boundary tests" | `exact_up_threshold_crosses_from_green_to_yellow`, `just_below_up_threshold_stays_green`, `exact_up_threshold_crosses_from_yellow_to_orange`, `exact_up_threshold_crosses_from_orange_to_red`, `zero_free_space_is_red_from_any_previous_state` exercise every up-threshold at its exact value and one epsilon below it. | PASS |
| "Hysteresis tests" | `hysteresis_prevents_flapping_around_a_boundary` (a score inside the up/down band does not flap back after being elevated), `hysteresis_requires_a_genuine_drop_to_step_down` (a score above the down-threshold does not drop; a score below it does), `multi_level_jump_up_reaches_correct_state_in_one_call` and `drop_from_red_stops_at_orange_when_only_reds_own_band_is_cleared` (a multi-level move in one observation resolves to the exact correct level, honoring each intermediate boundary's own threshold rather than skipping past it). | PASS |

## Adversarial-cases pass (CR2)

Falsification axes worked before implementation (`adversarial-cases` skill):

| # | Axis | Case | Expected | Test |
| --- | --- | --- | --- | --- |
| 7 | Boundary values | Score exactly at each up-threshold, and one epsilon below | Exact value crosses; epsilon below does not | `exact_up_threshold_crosses_from_green_to_yellow`, `just_below_up_threshold_stays_green`, and the Yellow->Orange / Orange->Red equivalents |
| 10 | Malformed/untrusted input | NaN on `free_space_fraction` and on `growth_velocity_fraction_per_window`; an out-of-range value (`5.0`) on `budget_usage_fraction` | Each resolves to that axis's worst reading (never silently "safe"/Green), and out-of-range is clamped rather than amplified | `nan_free_space_is_treated_as_maximum_pressure_not_silently_green`, `nan_growth_velocity_is_treated_as_maximum_pressure`, `out_of_range_fraction_above_one_is_clamped_not_amplified` |
| safety-specific: unknown-to-authority promotion (adapted: unknown-to-*calm* promotion) | A NaN/malformed detection axis reads as calm/Green instead of forcing a refusal-equivalent (here, the worst state) | Refused as a design: NaN never reads as safe on any axis | Same three tests above |
| safety-specific: second-path check | Does this module decide anything `cancellai-safety` already decides, or duplicate an authority decision? | No - the module contains no authority type at all, so there is no second path to duplicate | Verified by the diff itself: no `cancellai-safety` import in `pressure.rs`; documented in the module's own doc comment |
| n/a | Path/identity, partial reads, links/mounts, provider drift, concurrency, crash/retry, platform differences, performance/scale | Not applicable - this module has no filesystem, database, network, or shared-state surface; it is a pure function over caller-supplied primitives | - |
| domain-specific (not one of the eleven, added for this change) | Negative `growth_velocity_fraction_per_window` (net reclaiming) | Never raises pressure; floors at zero contribution rather than offsetting other axes | `negative_growth_velocity_never_raises_pressure` |
| domain-specific | Reclaimability at `1.0` combined with zero free space | Dampens the score but cannot mask a genuinely full disk - resolves to `Orange`, never `Green`/`Yellow` | `full_reclaimability_dampens_score_but_not_below_orange_at_zero_free_space` |
| domain-specific | Active workload vs. the same inputs at rest | Raises the score (never lowers it), moving one level higher than the idle case at the same free-space value | `active_workload_raises_score_over_the_same_inputs_at_rest` |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-027 ("Detection severity does not create authority") | Whether any code path in this module can produce, consume, or influence an `AuthorityLevel`/`ActionClass`/`Reversibility` | No such type is imported or referenced anywhere in `pressure.rs`; `PressureState` is a standalone enum with no conversion to or from any `cancellai-safety` type. Discharged by construction (absence in the diff), not by a runtime assertion - the same pattern `docs/architecture/PERSISTENCE_MODEL.md` already documents for E12-S04/E13-S06's own SI-020/SI-026 boundaries. | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                              # PASS
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings   # PASS
cd rust && cargo check --workspace --all-targets                          # PASS
cd rust && cargo test --workspace                                         # PASS - 0 failed (17 new cancellai-guardian::pressure tests)
cd rust && cargo deny check                                               # PASS - advisories, bans, licenses, sources OK; no new dependency
python3 scripts/check_risk_classification.py check                       # PASS
python3 scripts/project_os.py check                                       # PASS
```

Not run: the Windows-target and Linux-target cross-compiled clippy passes AGENTS.md calls out for
changes to `cancellai-platform` or anything moving the workspace-wide lint surface - this story
touches neither. CI's existing per-platform `cargo check`/full quality matrix
(`.github/workflows/rust.yml`) still runs this change on macOS, Linux, and Windows before merge.
`pre-commit run --all-files` was not invoked as one combined command; its constituent hooks
relevant to this diff (listed above, plus the standard whitespace/EOF/large-file/merge-conflict
checks) ran individually with the same result, and the full hook set runs automatically at commit
time regardless.

## Residual Risks

- **No live caller yet.** Nothing in this crate graph currently calls `classify` from a real
  `cancellai-store`/scanner observation - this story delivers the detection primitive only,
  matching E12-S04/E13-S04/E13-S06's own disclosed "primitive delivered, no orchestrator yet"
  pattern. Wiring a real Guardian runtime to call this on a schedule is later epic scope (E14-S02
  onward for the growth-velocity input this module already accepts, E15 for the runtime that
  would actually invoke it).
- **Thresholds and weighting are a first calibration, not a tuned product constant.**
  `docs/architecture/GUARDIAN_MODEL.md` states "the exact function is calibrated later" for this
  reason; the specific score formula, threshold values, `RECLAIM_DAMPENING_FACTOR`, and
  `WORKLOAD_BUMP` in `pressure.rs` are a reasonable first choice consistent with the document's
  qualitative description, not the only legal one, and are free to be recalibrated by a later
  story without changing `PressureState`'s meaning for callers.
- **Active workload's effect on severity is a documented design choice, not dictated by the
  architecture document.** `docs/architecture/GUARDIAN_MODEL.md` names "active workload" as one of
  the five pressure inputs without specifying its direction. This implementation raises severity
  (a provider actively writing describes a still-worsening trend at observation time) rather than
  dampening it; the rationale is recorded in `pressure.rs`'s own doc comment on
  `PressureInputs::active_workload` so a future story revisiting this choice has the reasoning to
  argue against, not just the code.

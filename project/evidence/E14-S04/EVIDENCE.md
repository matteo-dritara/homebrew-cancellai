# Evidence Packet - E14-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex, round 1 - **FAIL** (`project/evidence/E14-VERIFIER-REVIEW.md`) -
  `recommended_authority_ceiling` reached no actual authority computation anywhere in the
  workspace; `authority.rs` explicitly documented `ProviderCapabilityAuthority` as unwired. An
  advisory recommendation nobody consumes is not AC1's "automatic" downgrade. Repaired below
- Change Risk: CR2 (declared at planning time in `project/epics/E14.json`; the diff adds a new,
  dependency-free, pure crate module reusing an existing sibling module and the crate's existing
  `cancellai-model` dependency - no filesystem, database, or execution surface, so no
  reclassification applies - `check_risk_classification.py check` records no new story below its
  floor)
- Spec version/commit: `docs/architecture/GUARDIAN_MODEL.md` "Detection"

## Outcome

PASS after repair (see "Repair - round 1 independent review finding" below)

## Scope

`cancellai_guardian::structural` (new module, `rust/crates/cancellai-guardian/src/structural.rs`,
registered alongside the crate's existing `pressure`/`forecast`/`baseline` modules in
`src/lib.rs`). Three of the four named signals - session-count explosion, giant artifacts, and
orphan-state growth - are thin, named wrapper functions (`assess_session_explosion`,
`assess_giant_artifact`, `assess_orphan_growth`) over `crate::baseline::Baseline::assess`,
returning a `StructuralFinding { signal: StructuralSignal, assessment: AnomalyAssessment }` so the
underlying evidence-bearing comparison stays E14-S03's, only labeled by name. The fourth,
`assess_layout`, is a discrete comparison: `LayoutSignature` is an opaque, order-independent,
deduplicated set of caller-computed marker tokens (never a real path or file content);
`assess_layout(provider_id, known_signatures, observed) -> LayoutDriftFinding` returns
`LayoutSupport::Recognized` when `observed` exactly matches one of `known_signatures`, otherwise
`LayoutSupport::Drifted` - including when `known_signatures` is empty or `observed` carries no
markers - plus a `recommended_authority_ceiling: Option<AuthorityLevel>` (`cancellai-model`'s
existing shared vocabulary type; no new crate dependency was added or considered admissible, see
the `rust-kernel-guard` verdict below) and an `evidence` string naming the concrete markers
compared. `Recognized` always returns `None`; `Drifted` always returns
`Some(AuthorityLevel::Observe)`. `provider_id` is threaded only into the evidence string and
participates in no comparison. `structural.rs` itself remains unchanged by the round-1 repair
below and still holds no reference to `cancellai-safety`; the new
`cancellai_guardian::capability_authority` module (`src/capability_authority.rs`) is where
`recommended_authority_ceiling` actually reaches a real authority computation (see "Repair"
below). No orchestrator wires any of this to a live `cancellai-store` scan or a real provider
adapter yet, matching E14-S01/S02/S03/E12/E13's own "primitive delivered, no orchestrator yet"
precedent - that residual is about who *invokes* the now-real chain in production, not whether the
chain itself functions (which the round-1 repair's end-to-end test now proves it does).

## rust-kernel-guard verdict (dependency review)

Before implementation, evaluated whether `cancellai-guardian` should depend on
`cancellai-provider-api` to reuse its `CapabilityOutcome`/`SupportState` types for the layout-drift
finding (both outer-ring per ADR-0019).

```
Ring:        outer (cancellai-guardian), outer (cancellai-provider-api)
Dependency:  cancellai-provider-api -> refused (no real caller exists yet for this "primitive
             delivered, no orchestrator" story; would add an unusual outer->outer reverse edge -
             a detector depending on an adapter-contract crate's types - with no concrete benefit
             today; a later orchestration story can add this edge when a real caller exists)
Unsafe:      forbid intact (no change)
Second path: none - the module returns only a recommendation (Option<AuthorityLevel>, the
             existing shared cancellai-model vocabulary type), never an execution;
             cancellai-safety remains the sole mutation executor (docs/CONSTITUTION.md: "route
             mutation through one safety boundary")
Checks:      not applicable - no Cargo.toml change made
```

Decision: implemented without adding any new crate dependency.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Layout drift can downgrade provider capabilities automatically." | `assess_layout` computes `recommended_authority_ceiling` deterministically from the signature comparison alone with no manual step: `unrecognized_layout_reduces_ceiling_to_observe` (drifted -> `Some(Observe)`), `empty_known_signatures_never_recognizes_anything` (nothing yet known -> drifted, still reduced), `empty_observed_markers_is_drift_not_a_vacuous_match` (no markers observed -> drifted, not vacuously recognized), `recognized_layout_does_not_reduce_ceiling` (exact match -> `None`). Round 1 found this recommendation reached no actual authority computation anywhere - "automatically" was unmet in practice. **Fix:** new module `cancellai_guardian::capability_authority::effective_authority_after_layout_assessment` routes `recommended_authority_ceiling` into the new `cancellai_safety::effective_authority_for_provider_capability` (the ninth, previously-unwired Effective Authority constraint). `e14s04_end_to_end_a_destructive_capable_input_ends_at_observe_under_real_layout_drift` proves a real `assess_layout` drift finding, combined with `AuthorityInputs` that are destructive-capable on every other constraint, resolves to `AuthorityLevel::Observe` with `provider_capability_authority` as the binding constraint. | PASS |
| AC2 - "Structural signals reference concrete observed evidence." | Every `StructuralFinding` carries the full `AnomalyAssessment` (observed value, baseline median/MAD, deviation) via `session_explosion_is_flagged_with_observed_count_in_evidence`, `giant_artifact_is_flagged_with_observed_size_in_evidence`; every `LayoutDriftFinding.evidence` names the concrete compared markers (asserted via `evidence.contains(...)` in `recognized_layout_does_not_reduce_ceiling` and `unrecognized_layout_reduces_ceiling_to_observe`) - never an opaque score/boolean. | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Provider drift ... fixtures" | `recognized_layout_does_not_reduce_ceiling` (conforming layout, order/duplicate-insensitive match), `unrecognized_layout_reduces_ceiling_to_observe` (drifted shape), `empty_known_signatures_never_recognizes_anything`, `empty_observed_markers_is_drift_not_a_vacuous_match`, `provider_name_never_changes_the_drift_verdict`, `provider_name_never_changes_the_recognized_verdict` (SI-004 direct proof: a well-known vs. an unheard-of `provider_id` reach the identical verdict in both the drifted and the recognized case). | PASS |
| "... and explosion fixtures" | `normal_session_count_is_not_flagged` / `session_explosion_is_flagged_with_observed_count_in_evidence` (stable-5 baseline vs. a 5,000 spike); `normal_artifact_size_is_not_flagged` / `giant_artifact_is_flagged_with_observed_size_in_evidence` (~2 MB baseline vs. 20 GB); `stable_orphan_count_is_not_flagged` / `runaway_orphan_growth_is_flagged` (stable-3 baseline vs. 900); `session_explosion_with_insufficient_history_is_insufficient_not_normal` (too little history refuses to judge, never reads as normal). | PASS |

## Adversarial-cases pass (CR2)

Falsification axes worked before implementation (`adversarial-cases` skill):

| # | Axis | Case | Expected | Test |
| --- | --- | --- | --- | --- |
| 4 | Provider version/layout drift | Unrecognized directory shape | Finds "nothing to clean" for that signature (drift, reduced ceiling), never guesses a match | `unrecognized_layout_reduces_ceiling_to_observe` |
| 7 | Boundary values | No known signatures at all; observed signature with zero markers; a baseline with fewer observations than the minimum | All read as drift/insufficient, never a default pass/normal | `empty_known_signatures_never_recognizes_anything`, `empty_observed_markers_is_drift_not_a_vacuous_match`, `session_explosion_with_insufficient_history_is_insufficient_not_normal` |
| 8 | Policy/trust conflicts (adapted: identity vs. structure) | A well-known provider name paired with a structurally drifted layout | Reduction applies regardless of name - the name carries no trust the structure did not independently earn | `provider_name_never_changes_the_drift_verdict` |
| 10 | Malformed/untrusted input | Duplicate/out-of-order marker tokens fed to `LayoutSignature::new` | Normalized before comparison; two signatures naming the same markers in different order/with duplicates compare equal | `signature_equality_is_order_independent_and_deduplicated` |
| 11 | Performance/large datasets | N/A here directly - `structural.rs` reuses `baseline.rs`'s already-exercised bounded-window performance case (`window_never_exceeds_capacity_across_many_observations`, 10,000 observations); this module adds no new unbounded state | - |
| safety-specific: unknown-to-authority promotion (adapted: absence-to-recognized promotion) | Empty `known_signatures` or empty observed markers | Must read as `Drifted` (reduced), never as a vacuous `Recognized` | `empty_known_signatures_never_recognizes_anything`, `empty_observed_markers_is_drift_not_a_vacuous_match` |
| safety-specific: second-path check | Does this module decide anything `cancellai-safety` decides? Does a provider name change the decision? | No - no `cancellai-safety` import anywhere in the diff; `provider_id` never branches the comparison (proven, not just asserted, by the two `provider_name_never_changes_the_..._verdict` tests) | `provider_name_never_changes_the_drift_verdict`, `provider_name_never_changes_the_recognized_verdict`; verified by the diff itself (no `cancellai-safety` import) |
| n/a | Path/identity, partial reads, links/mounts, concurrency, crash/retry, platform differences | Not applicable - pure in-memory functions over caller-supplied primitives (`f64`, `String` tokens), no filesystem/database/network/shared-state surface | - |
| domain-specific (not one of the eleven) | Recognized-layout case gets a `None` ceiling, never a "no-op" sentinel a future edit could confuse with a real reduced value | `RECOGNIZED_CEILING`/`RECOGNIZED_CEILING` is a named `None` constant, distinct in the diff from `DRIFTED_CEILING`'s `Some(Observe)` | `recognized_layout_does_not_reduce_ceiling` |
| second-path (round 1 repair) | An `AuthorityInputs` destructive-capable on every one of the six base constraints (`Autopilot` user/artifact, verified confidence, idle/normal/healthy lifecycle, real `TrustedTier::promote`d to `BuiltinVerified`), combined with a real drifted `LayoutDriftFinding` | Resolves to `AuthorityLevel::Observe`, attributed to `provider_capability_authority` - and a `None`-ceiling baseline with the identical inputs reaches `Autopilot`, proving the test is not vacuous | `e14s04_end_to_end_a_destructive_capable_input_ends_at_observe_under_real_layout_drift` |
| 8 (round 1 repair) | The same destructive-capable input, with both a well-known and an unheard-of `provider_id` producing the identical drifted layout | Both end at `Observe` - the provider name does not bypass the ceiling at the authority-computation layer either, not only inside `assess_layout` itself | `e14s04_a_recognized_provider_name_does_not_bypass_the_drift_verdict` |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-004 "Unknown provider layout/version reduces capability" | A well-known `provider_id` ("claude") paired with a structurally drifted layout still reduces the ceiling; an unheard-of `provider_id` paired with a genuinely matching layout still reads `Recognized` | `provider_name_never_changes_the_drift_verdict`, `provider_name_never_changes_the_recognized_verdict` - `assess_layout` has no branch on `provider_id` anywhere in the diff (verified by inspection of the function body, not only by test) | PASS |
| SI-027 "Detection severity does not create authority" (module-level isolation, same principle `pressure`/`forecast`/`baseline` already carry) | No `cancellai-safety` type is imported or referenced anywhere in `structural.rs`; `recommended_authority_ceiling` is a recommendation only. After the round-1 repair it *is* consumed - by `capability_authority::effective_authority_after_layout_assessment` - but only through the one existing `compute_effective_authority` monotonic-minimum computation every other constraint already goes through, never by a second, independent authority decision or a direct execution | `structural.rs` unchanged, verified by absence in that file's diff; `capability_authority.rs` makes no classification/execution decision of its own, verified by inspection (it only builds one more `AuthorityConstraint` and calls the existing function) | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                              # PASS (after one fmt pass over the new module)
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings   # PASS, no findings
cd rust && cargo check --workspace --all-targets                          # PASS
cd rust && cargo test --workspace                                         # PASS - 0 failed (15 cancellai-guardian::structural tests + 2 new cancellai-guardian::capability_authority end-to-end tests + 5 new cancellai-safety::authority tests after repair, no regressions in any other crate)
cd rust && cargo deny check                                               # PASS - advisories, bans, licenses, sources OK; no new dependency
python3 scripts/check_mutation_boundary.py check                         # PASS - only the existing platform/safety mutation seam deletes anything
python3 scripts/check_rust_workspace.py check                            # PASS - 13 crates match TARGET.md, acyclic
python3 scripts/check_risk_classification.py check                       # PASS - no new story below its floor
python3 scripts/check_schemas.py check                                   # PASS
python3 scripts/check_fixtures.py check                                  # PASS
python3 scripts/project_os.py check                                      # PASS
```

Not run: the Windows-target and Linux-target cross-compiled clippy passes AGENTS.md calls out for
changes to `cancellai-platform` or anything moving the workspace-wide lint surface - this story
touches neither. CI's existing per-platform `cargo check`/full quality matrix (`rust.yml`) still
runs this crate on all three platforms before merge.

## Compatibility

- Platforms/providers/schemas exercised: none - this module has no platform-specific code path
  and no provider/schema surface; `LayoutSignature`/`provider_id` are caller-supplied opaque
  primitives, not tied to any real provider adapter.

## Performance / operability

- `assess_layout` is O(known_signatures.len()) per call (a linear scan for an exact match); the
  three counting/size wrappers inherit `Baseline::assess`'s O(capacity) cost, already exercised at
  scale by `baseline.rs`'s own performance test. No new unbounded state is introduced by this
  module.

## Repair - round 1 independent review finding

Codex's round-1 review (`project/evidence/E14-VERIFIER-REVIEW.md`) FAILed AC1: `assess_layout`
correctly computed `recommended_authority_ceiling`, and `authority.rs`'s own module doc explicitly
documented `ProviderCapabilityAuthority` (the ninth of the nine constraints
`docs/architecture/DOMAIN_MODEL.md`'s "Effective Authority" formula names) as unwired - "no
capability-classification subsystem exists yet to supply it." A workspace-wide search confirmed
`assess_layout`/`LayoutDriftFinding`/`recommended_authority_ceiling` were referenced only inside
`structural.rs` itself. A correctly-computed recommendation nobody consumes is not AC1's
"automatic" downgrade; it permits TM-05's layout-drift condition to leave destructive authority
intact.

Repair, in two crates:

1. `cancellai-safety::authority::effective_authority_for_provider_capability(inputs,
   provider_capability_ceiling: Option<AuthorityLevel>) -> EffectiveAuthority` is the ninth
   constraint, wired the same way `effective_authority_for_channel` (E17-S05) already wires the
   eighth: a separate function rather than a new required `AuthorityInputs` field, so existing
   callers that predate capability-classification are not forced to supply a value with no
   honest answer. Five new unit tests in that crate prove its own contract directly (a `Some`
   ceiling collapses even a maximally-permissive input; `None` adds no constraint; the trace
   correctly attributes the bottleneck).
2. `cancellai_guardian::capability_authority::effective_authority_after_layout_assessment(inputs,
   finding: &LayoutDriftFinding) -> EffectiveAuthority` is the bridge: it takes
   `finding.recommended_authority_ceiling` and calls the new `cancellai-safety` function. It
   lives one module up from `structural.rs`, specifically so `structural.rs`'s own "no reference
   to `cancellai-safety`" isolation stays true. `cancellai-guardian` already depended on
   `cancellai-safety` (for reasons predating this story), so no new crate dependency was added -
   the `rust-kernel-guard` verdict above, about *not* adding `cancellai-provider-api`, is
   unaffected. Two end-to-end tests here prove the actual chain round 1 found missing: a real
   `assess_layout` drift finding, combined with an `AuthorityInputs` destructive-capable on every
   other constraint (built from real API calls - `TrustedTier::untrusted().promote(..)` with real
   evidence, not a test-only backdoor - since `TrustedTier` cannot be forged from outside
   `cancellai-safety`), resolves to `AuthorityLevel::Observe`; and a well-known provider name does
   not bypass this at the authority-computation layer either.

## Documentation updated

- `docs/architecture/GUARDIAN_MODEL.md` - "Detection" section rewritten for the round-1 repair
  (the `capability_authority` bridge and what AC1's "automatically" now actually means).
- `rust/crates/cancellai-safety/src/authority.rs`'s own module doc updated: `ProviderCapabilityAuthority`
  is no longer documented as unwired.
- `CHANGELOG.md` - `Unreleased`/`Added` entries for E14-S02 and E14-S04 updated for both repairs.

## Method defects

- none

## Residual risks

- No orchestrator calls `capability_authority::effective_authority_after_layout_assessment` from
  a live `cancellai-store` scan or a real provider adapter yet - a later story's scope, matching
  every other primitive delivered in E12/E13/E14-S01/S02/S03. Unlike before the round-1 repair,
  this is now honestly a residual about who *invokes* an already-real, already-end-to-end-tested
  chain, not a stand-in for the chain not existing: round 1 found the latter framing was actually
  masking AC1 being unmet, since the recommendation reached no computation to invoke in the first
  place.
- `assess_layout`'s two-state model (`Recognized`/`Drifted`) does not reproduce
  `cancellai-provider-api::capability::SupportState`'s full six-state vocabulary
  (`Verified`/`SupportedObserved`/`Unsupported`/`UnknownVersion`/`LayoutDrift`/`ErrorPartial`) -
  deliberately, per the `rust-kernel-guard` verdict above, since no real caller exists yet to
  justify the cross-crate dependency that reuse would require. A future orchestration story
  wiring this into a real provider adapter's capability report will need to reconcile the two
  vocabularies (or add the dependency once a concrete caller justifies it).
- The single fixed ceiling value for any drift (`AuthorityLevel::Observe`, never a graduated
  reduction between `Recognized`'s untouched ceiling and full observation-only) is a first,
  documented-as-provisional design choice, not derived from real Guardian telemetry, which does
  not exist yet - consistent with `GUARDIAN_MODEL.md`'s own "the exact function is calibrated
  later."
- The three counting/size wrappers handle one numeric signal at a time via one `Baseline` per
  signal (same scope boundary `baseline.rs` itself already discloses); this story does not add a
  combined, cross-signal structural explanation.

## Verifier verdict

Round 1: **FAIL** (Codex) - `project/evidence/E14-VERIFIER-REVIEW.md`. Repaired above; round 2
pending.

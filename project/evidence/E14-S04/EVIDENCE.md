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
| AC1 - "Layout drift can downgrade provider capabilities automatically." | `assess_layout` computes `LayoutSupport` deterministically from the signature comparison alone with no manual step: `unrecognized_layout_reduces_ceiling_to_observe`, `empty_known_signatures_never_recognizes_anything`, `empty_observed_markers_is_drift_not_a_vacuous_match` (all `Drifted`), `recognized_layout_does_not_reduce_ceiling` (`Recognized`). Round 1 found the recommendation reached no authority computation; round 2 found the fix (a second, opt-in function) bypassable via the still-public plain `effective_authority`, and `LayoutDriftFinding`'s public fields forgeable; round 3 found ADR-0034's mandatory `Option<AuthorityLevel>` ceiling field itself a discardable conclusion, separable from the observation it summarized. **Fix (ADR-0035):** `AuthorityInputs::provider_layout: ProviderLayoutAssessment` carries the raw `known_signatures`/`observed` facts, never a ceiling; `base_constraints` derives the constraint itself via `layout_ceiling`, the identical comparison `assess_layout` performs. `e14s04_end_to_end_a_destructive_capable_input_ends_at_observe_under_real_layout_drift` proves `assess_layout`'s own detection output and feeding the identical raw signatures into a real effective-authority computation agree, ending at `Observe`; `e14s04_adr0035_a_real_observation_cannot_be_supplied_and_then_separately_discarded` (guardian) and its counterpart in `cancellai-safety::authority` prove round 3's own counterexample no longer reproduces - there is no second field left to assert a contradictory conclusion into. | PASS |
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
| domain-specific (not one of the eleven) | `Recognized` and `Drifted` are two named, distinct `LayoutSupport` values, never a `None`/no-op sentinel a future edit could confuse with the recognized case | `LayoutSupport` is an enum with no `None`/default variant; `assess_layout` always returns exactly one of the two | `recognized_layout_does_not_reduce_ceiling`, `unrecognized_layout_reduces_ceiling_to_observe` |
| second-path (round 3 repair, ADR-0035) | A caller holds a real drifted observation, confirms `Observe` through it, then separately constructs an otherwise-identical `AuthorityInputs` claiming no observation was made - round 3's own exploit against ADR-0034's ceiling field | Both constructions built from the *same* raw signatures can only ever agree (there is no second, independently-settable ceiling field); a construction that honestly claims `NotAssessed` is a distinguishable, different observation, not a discard of the real one | `e14s04_adr0035_a_real_observation_cannot_be_supplied_and_then_separately_discarded` (guardian), `e14s04_adr0035_a_real_observation_cannot_be_supplied_and_then_separately_discarded` (safety) |
| second-path (round 1 repair) | An `AuthorityInputs` destructive-capable on every one of the six base constraints (`Autopilot` user/artifact, verified confidence, idle/normal/healthy lifecycle, real `TrustedTier::promote`d to `BuiltinVerified`), combined with a real drifted `LayoutDriftFinding` | Resolves to `AuthorityLevel::Observe`, attributed to `provider_capability_authority` - and a `None`-ceiling baseline with the identical inputs reaches `Autopilot`, proving the test is not vacuous | `e14s04_end_to_end_a_destructive_capable_input_ends_at_observe_under_real_layout_drift` |
| 8 (round 1 repair) | The same destructive-capable input, with both a well-known and an unheard-of `provider_id` producing the identical drifted layout | Both end at `Observe` - the provider name does not bypass the ceiling at the authority-computation layer either, not only inside `assess_layout` itself | `e14s04_a_recognized_provider_name_does_not_bypass_the_drift_verdict` |
| second-path (round 2 repair, ADR-0034) | A caller that reaches for the pre-existing, still-public plain `effective_authority` directly, holding a real drifted ceiling in `AuthorityInputs` - round 2's own exploit | Reaches `Observe`, not `Autopilot`: there is no more permissive computation to reach for once the field is mandatory | `e14s04_adr0034_the_plain_public_effective_authority_cannot_be_used_to_bypass_a_drifted_ceiling` (safety), `e14s04_adr0034_the_bridge_and_a_direct_call_to_effective_authority_can_only_ever_agree` (guardian) |
| 10 (round 2 repair, ADR-0034) | A caller constructs `LayoutDriftFinding { support: Recognized, recommended_authority_ceiling: None, .. }` directly, in place of a real drifted finding - round 2's own exploit | Does not compile from outside `cancellai-guardian`: `LayoutDriftFinding`'s fields are private and `assess_layout` is the only production constructor | `compile_fail` doctest on `LayoutDriftFinding` (`structural.rs`) |

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

## Repair - round 2 independent review finding

Codex's round-2 review (`project/evidence/E14-VERIFIER-REVIEW-ROUND2.md`) FAILed AC1 again, on a
different defect: `effective_authority_for_provider_capability` was correct in isolation, but a
real `assess_layout` drift finding fed through it to `Observe`, while the identical
`AuthorityInputs`, unchanged, still reached `Autopilot` through the pre-existing, equally public
plain `effective_authority` - nothing forced a caller to prefer the capability-aware function.
`LayoutDriftFinding`'s public fields compounded this: the reviewer's own reproduction constructed
`LayoutDriftFinding { support: Recognized, evidence: ..., recommended_authority_ceiling: None }`
directly, in place of a real drifted finding, and the bridge accepted it without complaint. The
review named this the second failure of the same approach (round 1: no consumer; round 2:
optional/bypassable consumer) and, per the owner's two-round limit, blocked the story rather than
authorizing a third patch attempt: an owner-level ADR/scope decision was required first.

The owner authorized developing that alternative solution immediately (not blocking on a future
session), specifically along the lines of "a mandatory, non-forgeable authority-construction
boundary, analogous to `TrustedTier`, with existing public APIs restricted/migrated" - recorded as
[ADR-0034](../../../docs/adrs/0034-provider-capability-authority-is-a-mandatory-authorityinputs-field.md).
Repair, replacing round 1's fix rather than patching it:

1. `cancellai_safety::authority::AuthorityInputs` gained a **mandatory**
   `provider_capability_ceiling: Option<AuthorityLevel>` field, consumed inside `base_constraints`
   - the function `effective_authority` and `effective_authority_for_channel` already share.
   `effective_authority_for_provider_capability` was deleted outright: there is no longer a
   second, more permissive authority computation next to `effective_authority` for a caller to
   reach for. `None` (no assessment performed, or a recognized layout) is a safe, honest default
   that adds no constraint, so no pre-existing caller's already-verified scenario changed; every
   `AuthorityInputs` literal in the workspace (the compiler enforced finding every one of them)
   now states a value for this field.
2. `cancellai_guardian::structural::LayoutDriftFinding`'s fields became private, with
   `assess_layout` as the only production constructor and a `#[cfg(test)] pub(crate) for_tests`
   escape hatch for this crate's own fixtures - mirroring `cancellai-safety::TrustedTier`'s
   identical defect-and-fix shape exactly (that type's own module doc names the same "existing
   and working correctly is not the same as being the *only* path" lesson). A `compile_fail`
   doctest proves an external crate cannot construct a `LayoutDriftFinding` at all, let alone a
   fabricated `Recognized`/`None` one.
3. `capability_authority::effective_authority_after_layout_assessment` now does nothing but read
   `finding`'s ceiling through its accessor and call the one public `effective_authority` - a new
   test (`e14s04_adr0034_the_bridge_and_a_direct_call_to_effective_authority_can_only_ever_agree`)
   asserts the bridge and a direct call to `effective_authority` with the identical resulting
   `AuthorityInputs` produce the exact same `EffectiveAuthority`, since they are now the same
   function; a matching `authority.rs` test
   (`e14s04_adr0034_the_plain_public_effective_authority_cannot_be_used_to_bypass_a_drifted_ceiling`)
   reproduces round 2's own counterexample and proves it is now closed.
4. The two real, non-test production call sites of `AuthorityInputs { .. }` outside
   `cancellai-safety` itself (`cancellai_policy::resolver`'s caller-supplied inputs construction
   site, and `cancellai_policy::retention::reachable_authority`) now state
   `provider_capability_ceiling: None` explicitly, each with a comment pointing at ADR-0034's
   disclosed residual rather than silently omitting the concept.

ADR-0034 discloses, rather than closes, the residual round 2 did not ask this story to close: no
current production caller performs a live layout assessment before computing authority at all
(Guardian's structural detection operates at provider-root scope, not the per-artifact scope
`reachable_authority` runs at). A mandatory field makes discarding a real finding impossible; it
cannot compel a caller that never obtained one to go get it - see the ADR's own "Disclosed
residual" section and Residual risks below.

## Repair - round 3 independent review finding (the repository's 3-round ceiling, continued with owner authorization)

Codex's round-3 review (`project/evidence/E14-S04-VERIFIER-REVIEW-ROUND3.md`) FAILed AC1 a third
time, on ADR-0034's own mandatory field: "An external-crate-style integration probe obtained a
real public `Drifted` `LayoutDriftFinding`... then called public `cancellai_safety::
effective_authority` with separately constructed, otherwise identical destructive-capable
`AuthorityInputs { provider_capability_ceiling: None, .. }`. That call returned `Autopilot` while
the caller still held the real finding." Its diagnosis: "the required fact is an assessed
capability state whose provenance must survive into authority construction... the protected
result is not the authority input type and is therefore freely discardable." A mandatory field
requires spelling a value, but `Option<AuthorityLevel>` is a *conclusion*, and nothing bound that
conclusion to the observation it was drawn from - a caller could compute it correctly once, then
assert a contradictory one the second time. The review also flagged `compute_effective_authority`
itself as a standing public generic primitive callable with hand-assembled constraints; this
story does not attempt to close that (see ADR-0035's own "Context" section for why: it is the
E03-S04 generic primitive `effective_authority_for_channel` already relies on, and restricting it
is a separately-scoped redesign of the whole Effective Authority public surface, not this story's
defect). Given this was a third distinct structural failure and the repository's own 3-round
cost ceiling, the review explicitly declined to authorize a fourth patch-and-repeat cycle,
requiring an owner-decided, structurally different design instead. The owner authorized
developing that redesign immediately, and (separately) authorized the review process to continue
past the 3-round ceiling for it specifically (`scripts/check_process.py`'s
`REVIEW_ROUND_EXCEPTIONS["E14"]`, mirroring the E12/E13 precedent for an owner-authorized round
beyond the default ceiling).

Repair, recorded as [ADR-0035](../../../docs/adrs/0035-provider-layout-authority-is-derived-from-raw-observation-not-a-caller-supplied-ceiling.md),
replacing ADR-0034's shape rather than patching it:

1. `cancellai_safety::authority::AuthorityInputs::provider_capability_ceiling: Option<AuthorityLevel>`
   is removed. In its place, `provider_layout: ProviderLayoutAssessment`
   (`cancellai-safety::provider_layout`, new module) carries the *raw observation* -
   `NotAssessed`, or `Observed { known_signatures: Vec<LayoutSignature>, observed:
   LayoutSignature }` - never a pre-computed ceiling. `base_constraints` derives whatever
   constraint the observation implies itself, via a private `layout_ceiling` function performing
   the identical comparison `assess_layout` performs, from the same raw facts. There is no
   ceiling value anywhere in this path for a caller to assert, discard, or disagree with once it
   supplies the observation.
2. `cancellai_guardian::structural::assess_layout`/`LayoutDriftFinding` are simplified to match:
   they compute no authority ceiling at all now (there is nothing left to hand off) - `evidence`/
   `support` are unchanged, `recommended_authority_ceiling` is gone entirely.
3. `cancellai_guardian::capability_authority` no longer "bridges" a ceiling; it converts this
   crate's own `LayoutSignature` into `cancellai-safety`'s structurally identical, deliberately
   separate type of the same name (the two crates may not depend on each other in the direction
   that would let them share one) via `authority_inputs_with_layout_observation`, and sets
   `provider_layout` to the resulting `Observed` value. A new test
   (`e14s04_adr0035_a_real_observation_cannot_be_supplied_and_then_separately_discarded`)
   reproduces round 3's own counterexample shape and proves there is no longer a second,
   contradictory construction path; a matching test in `cancellai-safety::authority` proves the
   same thing directly against the crate that owns the derivation.
4. `AuthorityInputs` is no longer `Copy` (it now owns a `Vec<LayoutSignature>` inside
   `ProviderLayoutAssessment::Observed`) - a mechanical consequence of carrying real evidence
   rather than a bare enum value. Every test/production call site that reused one value across
   two calls now clones it explicitly (`cancellai-safety::authority`,
   `cancellai-policy::explanation`); behavior is unchanged.

ADR-0035 discloses, narrower than ADR-0034 but not closed: no current production caller
constructs a real `ProviderLayoutAssessment::Observed` at all - the same residual ADR-0034
already disclosed, now stated more precisely: removing the discardable-ceiling defect makes a
*real* observation, once supplied, unavoidable; it still cannot compel a caller that never made
one to go make it.

## Documentation updated

- `docs/architecture/GUARDIAN_MODEL.md` - "Detection" section rewritten three times: round 1 (the
  `capability_authority` bridge and what AC1's "automatically" first meant), round 2/ADR-0034
  (the mandatory field, the private `LayoutDriftFinding`), round 3/ADR-0035 (the raw-observation
  redesign and its narrower disclosed residual).
- `docs/architecture/DOMAIN_MODEL.md` - "Effective Authority" section updated twice: first to say
  `ProviderCapabilityAuthority` is wired in (no longer "not wired in yet"), then to describe the
  raw-observation shape ADR-0035 replaced the ceiling field with.
- `rust/crates/cancellai-safety/src/authority.rs`'s own module doc rewritten for ADR-0035 (the
  three-round history and the raw-observation rationale).
- `docs/adrs/0034-provider-capability-authority-is-a-mandatory-authorityinputs-field.md` and
  `docs/adrs/0035-provider-layout-authority-is-derived-from-raw-observation-not-a-caller-supplied-ceiling.md` -
  both new.
- `CHANGELOG.md` - `Unreleased`/`Added` entry for E14-S04 rewritten for the round-3/ADR-0035
  repair.
- `scripts/check_process.py` - `REVIEW_ROUND_EXCEPTIONS["E14"]` added, recording the owner's
  authorization to continue review past the repository's 3-round ceiling for this specific
  redesign.

## Method defects

- none

## Residual risks

- No orchestrator calls `capability_authority::authority_inputs_with_layout_observation` from a
  live `cancellai-store` scan or a real provider adapter yet - a later story's scope, matching
  every other primitive delivered in E12/E13/E14-S01/S02/S03. This is a residual about who
  *invokes* an already-real, already-end-to-end-tested, now-non-discardable-when-invoked chain,
  not a stand-in for the chain not existing or being overridable once invoked: round 1 found the
  chain didn't reach a computation at all, round 2 found the computation it reached was
  skippable, round 3 found the mandatory field it was reduced to was a discardable conclusion,
  and ADR-0035 closes that third problem by removing the conclusion as an independent value -
  while explicitly disclosing that the first is still not fully closed: no current production
  caller (`resolve_effective_authority`'s caller, `reachable_authority`) constructs a real
  observation, so both state `ProviderLayoutAssessment::NotAssessed` honestly rather than
  fabricating one. A future orchestrator story wiring a live probe into a real
  authority-resolution call site is what actually closes this, per the ADR's own "Disclosed
  residual" section.
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

Round 1: **FAIL** (Codex) - `project/evidence/E14-VERIFIER-REVIEW.md`. Repaired above.
Round 2: **FAIL** (Codex) - `project/evidence/E14-VERIFIER-REVIEW-ROUND2.md`; blocked pending an
owner-level ADR per the two-round limit. Repaired above via ADR-0034.
Round 3: **FAIL** (Codex) - `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND3.md`, the
repository's 3-round ceiling; no fourth patch-and-repeat cycle authorized on the same design.
Repaired above via ADR-0035 (owner-authorized redesign, and owner-authorized continuation of
review past the 3-round ceiling for it specifically); round 4 pending.

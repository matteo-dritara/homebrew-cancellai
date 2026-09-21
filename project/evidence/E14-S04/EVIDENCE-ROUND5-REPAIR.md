# Evidence Packet - E14-S04 (round 5, ADR-0036)

- Commit/PR: repairs the state `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND4.md` reviewed and
  found `FAIL`
- Executor: Claude
- Independent verifier: Codex (pending)
- Change Risk: CR4
- Spec version/commit: `project/epics/E14.json` (E14-S04), `docs/adrs/0036-provider-execution-
  authority-requires-a-non-forgeable-observation-type.md`

## Outcome

PARTIAL (repair complete, awaiting independent review per this repository's executor/verifier
separation - the executor may not close its own CR4 repair, and round 4 explicitly required an
owner-authorized structurally different design rather than a fifth patch)

## Context

Round 4 (`project/evidence/E14-S04-VERIFIER-REVIEW-ROUND4.md`) found ADR-0035's raw-observation
design still discardable: `AuthorityInputs` remained a plain, publicly constructible struct, so a
caller holding a real `Observed{Drifted}` value could build a second, sibling `AuthorityInputs`
asserting `NotAssessed` and reach `Autopilot` through the same public `effective_authority`. The
review explicitly declined a fifth patch on the same field-level approach and required "a
different authority boundary: the real authority resolver must own or require an identity/
scope-bound, non-discardable assessment for the provider root it resolves."

Given the scope and prior 4-round history, the owner was consulted on how to proceed
(scoped type-level fix vs. full mutation-boundary rewiring vs. ADR-only). Decision: implement the
type-level fix that closes the round-4 public-API bypass, explicitly deferring mutation-boundary
wiring and trusted-manifest integration as disclosed residuals - matching this codebase's own
precedent (ADR-0034/ADR-0035's identically-shaped "no live wiring yet" residual).

## Design (see ADR-0036 for the full account)

- `AuthorityInputs::provider_layout` removed entirely. `effective_authority` is now
  analysis-only, over the remaining eight constraints.
- New `cancellai_platform::provider_layout::BoundLayoutObservation`: real directory I/O is its
  only constructor (`observe(root, identity_observer)`); no forgeable construction path exists.
- New `cancellai_safety::authority::ProviderExecutionPermit` (opaque, no public constructor) and
  `resolve_provider_execution_authority(base, observation, known_signatures) ->
  ProviderExecutionPermit` - the only function that can produce a permit, requiring a real
  observation by value.
- `cancellai_guardian::capability_authority` (the round-3/4-era bridge into
  `AuthorityInputs::provider_layout`) removed - orphaned by the field's removal, per this
  repository's diff-discipline rule ("remove only what your own edits orphaned").
  `cancellai_guardian::structural::assess_layout`/`LayoutDriftFinding` unchanged, still useful for
  reporting, no longer on the authority path.
- Four call sites that previously stated `provider_layout: ProviderLayoutAssessment::NotAssessed`
  (`cancellai-policy::pinning`, `resolver`, `explanation`, `retention`) updated to state nothing -
  a mechanical consequence, identical in effect (`NotAssessed` added no constraint; omitting the
  field adds none either).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 (layout drift can downgrade provider capabilities automatically) | `e14s04_a_real_drifted_observation_caps_the_permit_at_observe_even_at_maximum_everything_else`, `e14s04_raising_user_authority_never_raises_past_the_drifted_layout_ceiling` (both `authority.rs`, real `BoundLayoutObservation` via real tempdir I/O) | PASS |
| AC2 (structural signals reference concrete observed evidence) | `BoundLayoutObservation::markers()`/`root_identity()` carry the real observed facts through to the permit; `e14s04_the_permit_carries_the_observed_roots_own_identity` | PASS |
| SI-004 round-4 adversarial reproduction | `e14s04_round4_a_permissive_analysis_result_is_never_a_permit`: retains a real drifted observation, shows the analysis-only computation over identical base inputs still legitimately reaches `Autopilot`, and shows the permit path still caps at `Observe` regardless - the exact round-4 sequence, now safely separated by type | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-004 | Round-4's exact bypass: hold a real drift observation, build a second permissive analysis input | `EffectiveAuthority` and `ProviderExecutionPermit` are distinct types with no conversion; the permissive analysis result cannot be used as, or converted to, a permit | PASS |
| C-02 (ambiguity never escalates privilege) | No known-signatures source wired yet | Every current call resolves the layout constraint to `Observe`, never an invented "recognized" | PASS (residual disclosed, not hidden) |
| C-05 / TM-05 | Fabricated recognized observation | `BoundLayoutObservation`'s only constructor performs real I/O; no enum variant or field lets a caller assert a fabricated one | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                                    -> PASS
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings -> PASS
cd rust && cargo check --workspace --all-targets                                -> PASS
cd rust && cargo test --workspace                                               -> PASS (0 failed, full workspace, including 4 new cancellai-platform::provider_layout tests and 6 new cancellai-safety::authority::tests::e14s04_* tests)
cd rust && cargo deny check                                                     -> PASS (advisories ok, bans ok, licenses ok, sources ok)
python3 scripts/check_rust_workspace.py check                                   -> PASS (13 crates match TARGET.md, acyclic, model/safety isolated)
python3 scripts/check_mutation_boundary.py check                                -> PASS (unaffected - this repair does not touch the mutation boundary)
python3 scripts/project_os.py check                                             -> PASS
python3 scripts/check_docs.py check                                             -> PASS
python3 scripts/check_process.py check                                          -> PASS (E14's review-round exception needs a round-5 entry once the round-5 review record exists)
```

## Compatibility

- No platform-specific behavior. `BoundLayoutObservation::observe` uses `std::fs::read_dir`/
  `IdentityObserver`, already cross-platform primitives this workspace uses elsewhere.

## Documentation updated

- `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-observation-type.md` (new)
- `docs/architecture/GUARDIAN_MODEL.md` ("Detection" section, layout drift subsection rewritten)
- `docs/architecture/DOMAIN_MODEL.md` ("Effective Authority" section, `ProviderCapabilityAuthority`
  paragraph rewritten)
- Module docs in `rust/crates/cancellai-safety/src/authority.rs`,
  `rust/crates/cancellai-safety/src/provider_layout.rs`,
  `rust/crates/cancellai-platform/src/provider_layout.rs`,
  `rust/crates/cancellai-guardian/src/structural.rs`

## Method defects

- none

## Residual risks

- **`known_signatures` has no trusted source yet** (disclosed in ADR-0036): every current call
  resolves the layout constraint to `Observe`; wiring a trust-bounded provider manifest is future,
  separately-scoped work requiring its own dependency-ring ADR (`cancellai-safety` may not depend
  on `cancellai-provider-api` without one, per ADR-0019).
- **No mutation-boundary call site consumes a permit yet** (disclosed in ADR-0036, matching
  ADR-0034/ADR-0035's identically-shaped residual): `cancellai_safety::mutation_executor::execute`
  is unchanged and still authorizes on a plain `AuthorityLevel`. Future orchestrator work.

## Verifier verdict

PENDING - awaiting independent review by Codex against this design, per this repository's
executor/verifier separation.

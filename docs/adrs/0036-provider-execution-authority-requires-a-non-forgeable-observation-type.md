# ADR-0036: Provider execution authority requires a non-forgeable observation type

- Status: Accepted
- Date: 2026-09-21
- Owners: owner (Matteo Pugliese)
- Related: E14-S04, SI-004, C-02, C-05, TM-05, ADR-0034, ADR-0035,
  `docs/architecture/GUARDIAN_MODEL.md`, `docs/architecture/DOMAIN_MODEL.md`,
  `docs/security/SAFETY_INVARIANTS.md`

## Context

E14-S04's AC1 reads "Layout drift can downgrade provider capabilities automatically." Four
independent review rounds across three designs falsified every attempt to make that true against
an adversarial caller of the public API:

- **Round 1**: `cancellai_guardian::structural::assess_layout` computed a
  `recommended_authority_ceiling` that reached no authority computation anywhere.
- **Round 2**: the fix (a second, opt-in `effective_authority_for_provider_capability` function)
  was ignorable via the still-public plain `effective_authority`, and `LayoutDriftFinding`'s
  public fields let a caller fabricate a fake `Recognized` result.
- **Round 3 (ADR-0034)**: a mandatory `AuthorityInputs::provider_capability_ceiling:
  Option<AuthorityLevel>` field was itself a caller-asserted *conclusion*, disconnected from the
  facts it claimed to summarize - a caller could compute the right ceiling once, then separately
  construct an otherwise-identical `AuthorityInputs` with `None`, discarding the real finding it
  still held.
- **Round 4 (ADR-0035)**: replacing the ceiling with the raw observation itself
  (`AuthorityInputs::provider_layout: ProviderLayoutAssessment::{NotAssessed, Observed{..}}`)
  closed that exact reconstruction, but not the general one: `AuthorityInputs` remained a plain,
  publicly constructible struct with all-public fields. A caller holding a real
  `Observed{Drifted}` value could still build a second, sibling `AuthorityInputs` asserting
  `NotAssessed` for the same logical decision, because nothing bound either value to the real
  provider root it claimed to describe. Round 4's own diagnosis: "the actual authority call is
  not bound to an assessment of the provider root it governs... the real authority resolver must
  own or require an identity/scope-bound, non-discardable assessment for the provider root it
  resolves."

Round 4 explicitly declined a fifth patch cycle on the same field-level approach and required an
owner decision on a structurally different design. The owner authorized this session to design
and implement round 5 directly, with an explicit scope limit: **this round establishes the
non-forgeable primitive; it does not wire it into the mutation boundary or into a trusted source
of "known-recognized" layouts.** Both of those are separately scoped, disclosed residuals below,
matching the precedent ADR-0034/ADR-0035 already set for "no live wiring yet."

## Decision

Two changes, together:

**1. `AuthorityInputs` no longer carries a layout fact in any shape.** The
`provider_layout` field is removed outright - not replaced with a fourth shape. `effective_authority`
becomes purely an *analysis* computation over the remaining eight constraints
(`UserAuthority`, `ArtifactAuthorityCeiling`, `ConfidenceAuthority`, `LifecycleAuthority`,
`ProviderTrustAuthority`, `ConstitutionalSafetyFloor`, and the two channel/remote-execution
extensions built the same way). This is honest for the two production callers
(`cancellai_policy::resolver::resolve_effective_authority`,
`cancellai_policy::retention::reachable_authority`) that have never had a live layout observation
to supply, and it is now structurally impossible for any caller to *claim* to discard a layout
fact, because there is no field left to omit or contradict.

**2. A real layout fact reaches authority only through a new, separate path that a caller cannot
substitute a plain value for.** `cancellai_platform::provider_layout::BoundLayoutObservation` is
a new type whose only constructor, `observe(root)`, performs real directory I/O against a real
path and records the root's own `IdentityToken` alongside the marker names it actually found -
both read from the same call, internally, with no parameter through which a caller could supply
either independently (see "Round 5 self-correction" below for why the first version's public
observer parameter did not yet fully achieve this). There is no way to construct one from
caller-asserted marker strings, in any crate. `cancellai_safety::authority::
resolve_provider_execution_authority(base: AuthorityInputs,
observation: &BoundLayoutObservation, known_signatures: &[LayoutSignature]) ->
ProviderExecutionPermit` is the only function that can produce a `ProviderExecutionPermit` - an
opaque type with no public constructor, no `From`/`Into` conversion from `EffectiveAuthority`,
and no field a caller could set directly. It requires the observation *by value*, not as an
`Option`, so there is no way to call it while omitting one.

Why this closes round 4's exact counterexample: the attack required a plain, publicly
constructible `AuthorityInputs` (or its round-4 successor) to be reconstructible in two different,
contradictory ways for what was claimed to be the same decision. After this change, an
`AuthorityInputs`-based computation (`effective_authority`) can still legitimately reach
`Autopilot` while a `BoundLayoutObservation` of the same root sits unused elsewhere - but that
`Autopilot` result is an `EffectiveAuthority`, not a `ProviderExecutionPermit`, and nothing
converts one into the other. The only way to obtain a permit is to supply the real observation to
`resolve_provider_execution_authority`, which then derives the layout constraint from it directly
and folds it into the same monotonic minimum every other constraint already goes through
(`compute_effective_authority`, unchanged - this is still the point of keeping it generic over
named constraints).

`cancellai_guardian::capability_authority`, the round-3/4-era bridge that converted this crate's
own `LayoutSignature` into `cancellai-safety`'s `AuthorityInputs::provider_layout`, is removed
outright rather than adapted: its only reason to exist was populating a field this ADR deletes.
`cancellai_guardian::structural::assess_layout`/`LayoutDriftFinding` are unchanged and remain
useful for reporting/explanation (`GUARDIAN_MODEL.md`'s "Detection" vocabulary), but are no
longer on the path anything authority-typed is derived from - `structural.rs` already declared
(SI-027) that it holds no reference to `cancellai-safety` and never will; this ADR makes that
true one layer further out too, since authority no longer flows through this crate even as a
converted value.

## Disclosed residuals (both narrower than what this ADR closes)

**1. No trusted source of "known-recognized" layouts exists yet.** `known_signatures` is still a
caller-supplied parameter to `resolve_provider_execution_authority`, not drawn from a
trust-bounded provider manifest (`cancellai-provider-api::manifest`, which `cancellai-safety` may
not depend on without its own dedicated, reviewed ADR per the kernel-ring dependency rule,
`docs/adrs/0019-dependency-rings-per-crate.md`). A caller that can choose `known_signatures`
freely could in principle assert its own observed markers as "known," reaching `Autopilot` for a
genuinely drifted layout. No production caller supplies a non-empty `known_signatures` today, so
this is not currently reachable, and the honest, safe answer until a trusted source exists is
what this implementation does: every current call resolves to `AuthorityLevel::Observe` for the
layout constraint, never a silently invented "recognized" answer. Wiring a real trusted-layout
source is future, separately-scoped work requiring its own dependency-ring ADR.

**2. No production call site mints a permit, and the mutation boundary does not require one.**
`cancellai_safety::mutation_executor::execute` and `SealedPlan` are unchanged by this ADR; they
continue to authorize on a plain `AuthorityLevel` exactly as before. This round establishes the
non-forgeable primitive a future story can wire into the real destructive path (binding a
`ProviderExecutionPermit`'s `root_identity()` to the plan's own root identity before mutation,
the same way `cancellai-platform::mutation::confirmed_delete_file_inner` already binds a single
file's identity through open-time and immediately-before-unlink checks) - it does not perform
that wiring itself. This is the identical shape of residual ADR-0034 and ADR-0035 each disclosed
("no current production call site performs a live layout assessment before computing authority")
and is not a new gap this ADR introduces.

## Consequences

- E14-S04 can close against this design once independent review confirms the current diff -
  submitted as round 5 under the owner's explicit authorization to design and implement a
  structurally different approach after round 4 declined a fifth patch cycle on ADR-0035's shape.
- `AuthorityInputs` loses a field; every construction site in the workspace (both production
  policy call sites and every test fixture) was updated to no longer state one. This is a
  mechanical consequence of the field's removal, not a behavior change for either policy call
  site - both already supplied `NotAssessed`, which added no constraint, and now supply nothing,
  which is definitionally identical.
- Any future story adding a new named `AuthorityConstraint` from an external observation should
  read this ADR before choosing a shape: a caller-supplied value of any kind - a ceiling, raw
  facts, an enum with an honest "not assessed" variant - remains freely discardable by a second,
  sibling construction no matter how mandatory its field is, because nothing binds it to the real
  object it describes. Splitting "analysis" (a plain, publicly constructible input/output pair,
  fine for explanation and planning) from "execution" (an opaque, non-forgeable observation type
  feeding an opaque, non-forgeable result type, with no conversion between the two) is what
  actually closes that gap - not a fourth attempt at a cleverer field shape.
- A future orchestrator story wiring a live layout probe into the real mutation boundary, and a
  future story establishing a trusted provider-layout-manifest source, both remain open work this
  ADR does not close and does not claim to.

## Round 5 self-correction (same-day independent review)

The first committed version of `BoundLayoutObservation::observe` took a public
`identity_observer: &dyn cancellai_platform::IdentityObserver` parameter, reasoning that a
capability parameter (matching this codebase's own `IdentityObserver`/`MutationExecutor`
pattern elsewhere) was itself a non-forgeable seam. Independent review
(`project/evidence/E14-S04-VERIFIER-REVIEW-ROUND5.md`) found this insufficient:
`cancellai_platform::SyntheticIdentityObserver` is itself legitimate public API (needed for
testing elsewhere in this workspace), so an external caller could observe a real, favorably-
shaped, *unrelated* directory's markers while pairing them - via a synthetic observer - with a
fabricated `IdentityToken` equal to some *other*, genuinely drifted root's real identity. The
markers were real I/O; the identity binding them to a specific root was, once again, a
caller-supplied fact, just relocated one level down from where round 4 found it.

The repair stays inside this ADR's design - it is not a sixth architecture. `observe` no longer
takes an observer parameter at all: identity is now always read via
`cancellai_platform::SystemIdentityObserver` internally, from the same call that reads the
markers, closing the seam between the two facts entirely rather than trusting a parameter type to
close it. A `compile_fail` doctest on `BoundLayoutObservation::observe` pins this the same way
`TrustedTier`/`LayoutDriftFinding`'s own doctests already pin their non-forgeability elsewhere in
this codebase.

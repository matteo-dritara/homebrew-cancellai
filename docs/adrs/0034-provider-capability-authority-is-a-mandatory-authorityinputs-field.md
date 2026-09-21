# ADR-0034: Provider-capability authority is a mandatory `AuthorityInputs` field, not an opt-in function

- Status: Accepted
- Date: 2026-09-21
- Owners: owner (Matteo Pugliese)
- Related: E14-S04, SI-004, C-02, C-05, TM-05, ADR-0025, `docs/architecture/GUARDIAN_MODEL.md`,
  `docs/security/SAFETY_INVARIANTS.md`

## Context

E14-S04's AC1 reads "Layout drift can downgrade provider capabilities automatically." Two
independent review rounds falsified every attempt to make that automatic within the shape the
story started with:

- **Round 1**: `cancellai_guardian::structural::assess_layout` correctly computed a
  `recommended_authority_ceiling`, but nothing consumed it. It reached no policy, provider
  capability, or `cancellai_safety::authority::compute_effective_authority` call anywhere in the
  workspace - an advisory recommendation, not an automatic downgrade.
- **Round 2**: the repair added `effective_authority_for_provider_capability`, a second function
  beside the pre-existing, still-public `effective_authority`. The reviewer's own counterexample:
  a real drift finding fed through the new bridge correctly reached `Observe`, but the same
  `AuthorityInputs`, unchanged, still reached `Autopilot` through the plain `effective_authority`
  every existing caller already used - nothing forced a caller to prefer the new function.
  `LayoutDriftFinding`'s public fields compounded this: a caller holding a real drifted finding
  could construct a fresh `LayoutDriftFinding { support: Recognized, ..., ceiling: None }` and
  discard it before it ever reached the bridge.

The owner directed (in the same conversation authorizing this ADR) a hard two-round budget per
approach: a third round on the *same* design is not authorized, and reaching it means a
structurally different approach is required instead - the identical process ADR-0033 (E12-S04)
already established for this repository. Round 2's own required-repair section named the two
properties a third attempt would need to hold structurally rather than by convention: a single
mandatory authority-construction boundary, and a non-forgeable capability/layout result.

## Decision

Two changes, together, replace the round-2 design rather than patching it:

1. **`cancellai_safety::authority::AuthorityInputs` gains a mandatory field**,
   `provider_capability_ceiling: Option<AuthorityLevel>`, consumed by `base_constraints` - the
   function both `effective_authority` and `effective_authority_for_channel` already share.
   `effective_authority_for_provider_capability` is deleted outright; there is no longer a second,
   more permissive authority computation to reach for. `None` (no assessment performed, or a
   recognized layout) is a safe, honest, always-correct default: it adds no constraint beyond the
   six pre-existing ones, so no already-verified caller's behavior changes, and every
   `AuthorityInputs` literal in the workspace must now state a value for this field, not merely
   have the option of ignoring it. This mirrors why `effective_authority_for_channel` chose the
   opposite (opt-in) shape for release channel and remains right for that constraint: a nightly
   default (`BuildChannel::default()`) is *not* safe-and-honest the way `None` is here - it would
   silently cap already-verified scenarios that have nothing to do with release channel. Provider
   capability has no such trap: absence of an assessment is precisely what `None` already meant.

2. **`cancellai_guardian::structural::LayoutDriftFinding`'s fields become private**, with
   `assess_layout` as the only production constructor (a `#[cfg(test)] pub(crate)` `for_tests`
   escape hatch exists for this crate's own fixtures, mirroring
   `cancellai_safety::TrustedTier::for_tests`). A `compile_fail` doctest proves an external crate
   cannot construct a `LayoutDriftFinding` value at all, let alone a fabricated `Recognized`/
   `None` one to stand in for a real drifted result. This is the identical defect-and-fix shape
   `cancellai-safety::TrustedTier` already used for the analogous provider-trust problem (that
   type's own module doc: "this crate's first version of this gate did not actually gate
   anything" because the guarded value was a bare, freely-constructible public type).

Together: `cancellai_guardian::capability_authority::effective_authority_after_layout_assessment`
now does nothing but read `finding`'s ceiling through its accessor and call the one public
`cancellai_safety::effective_authority` - proven by a test asserting the bridge and a direct call
to `effective_authority` with the identical resulting `AuthorityInputs` can only ever agree, since
they are now the same function. The two real, non-test production call sites of
`AuthorityInputs { .. }` outside `cancellai-safety` itself
(`cancellai_policy::resolver::resolve_effective_authority`'s caller-supplied `inputs`, and
`cancellai_policy::retention::reachable_authority`) both now state
`provider_capability_ceiling: None` explicitly, with a comment naming this ADR's disclosed
residual below rather than silently omitting the concept.

## Disclosed residual

**No current production call site performs a live `cancellai-guardian` layout assessment before
computing authority.** `cancellai_policy::retention::reachable_authority` runs per-artifact during
a filesystem scan, with no provider-root layout signature available at that point to assess -
Guardian's structural detection operates at the scope of a whole provider root, not a single
artifact. Making the mandatory field impossible to skip closes the exact bypass round 2
demonstrated (a caller with a real finding in hand reaching a more permissive API instead); it
does not - and no purely type-level change can - compel a caller that has never run
`assess_layout` at all to run it before constructing `AuthorityInputs`. That is a live-wiring
integration between a real-time provider-root probe and the CLI's authority-resolution path, which
does not exist yet for any constraint this codebase enforces this way (the identical residual
already exists, undisclosed until now, for `effective_authority_for_channel`'s release-channel
constraint - `cancellai-cli` remains a beta, source-built artifact with no packaged release, per
that function's own module doc).

Closing this residual is a future orchestrator story's job: it wires a real, live
`assess_layout` (or its future scope-level equivalent) result into whichever call site actually
resolves authority for a live session, sourcing `provider_capability_ceiling` from that result
rather than a caller-asserted `None`. Until that story exists, "automatically" in AC1 means what
this ADR closes - a real finding, once obtained, cannot be discarded or bypassed on its way into
the one authority computation - not that every authority computation in the codebase is
necessarily preceded by a live layout probe.

## Consequences

- E14-S04 can close against this reading once independent review confirms the current diff.
- SI-004's "cannot preserve destructive capabilities merely because the provider name is
  recognized" is satisfied for any caller that possesses a real `LayoutDriftFinding`: it can no
  longer be discarded, and no separate, more permissive authority computation exists beside it.
  It is not yet satisfied end-to-end for a caller that never obtains one - that gap is the
  disclosed residual above, not silently accepted.
- A future orchestrator story wiring a live layout probe into a real authority-resolution call
  site must also close this residual, and should consider whether `effective_authority_for_channel`
  has the identical one for release channel worth closing at the same time.
- Any other story adding a new named `AuthorityConstraint` should read this ADR before choosing
  between a mandatory `AuthorityInputs` field and a separate opt-in function: the deciding
  question is whether that constraint's safe default is also an honest one a pre-existing caller
  can state truthfully with no invented information (mandatory field, as here) or one that would
  silently misrepresent a fact those callers have no way to supply yet (opt-in function, as
  `effective_authority_for_channel` remains for release channel).

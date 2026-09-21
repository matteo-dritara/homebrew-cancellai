# ADR-0035: Provider-layout authority is derived from a raw observation, not a caller-supplied ceiling

- Status: Accepted
- Date: 2026-09-21
- Owners: owner (Matteo Pugliese)
- Related: E14-S04, SI-004, C-02, C-05, TM-05, ADR-0034, ADR-0025,
  `docs/architecture/GUARDIAN_MODEL.md`, `docs/architecture/DOMAIN_MODEL.md`,
  `docs/security/SAFETY_INVARIANTS.md`

## Context

ADR-0034 replaced E14-S04's round-2 design (a second, opt-in `effective_authority_for_provider_
capability` function) with a mandatory `AuthorityInputs::provider_capability_ceiling:
Option<AuthorityLevel>` field, on the reasoning that a required field cannot be skipped the way a
second function can. Round 3 independent review - the repository's own three-round cost ceiling,
reached with the owner's explicit authorization to continue past it for this specific redesign -
found a way that reasoning was incomplete:

> An external-crate-style integration probe obtained a real public `Drifted` `LayoutDriftFinding`
> from `assess_layout`, observed `Observe` through the bridge, then called public
> `cancellai_safety::effective_authority` with separately constructed, otherwise identical
> destructive-capable `AuthorityInputs { provider_capability_ceiling: None, .. }`. That call
> returned `Autopilot` while the caller still held the real finding. The mandatory field requires
> spelling `None`; it does not bind the field to the finding or prevent a caller from discarding
> the finding.

The review's own diagnosis: "the required fact is an assessed capability state whose provenance
must survive into authority construction... the protected result is not the authority input type
and is therefore freely discardable." `Option<AuthorityLevel>` is a ceiling - a *conclusion* -
and nothing bound that conclusion to the observation it was supposedly conclude*d from*. A
caller could compute the conclusion correctly once, then assert a different, unrelated
conclusion the second time, and nothing in the type system or the function's logic could tell
the difference.

The review also flagged `compute_effective_authority`'s standing public visibility as "an
additional bypass surface," since any caller can invoke it directly with a hand-assembled
`AuthorityConstraint` list. This ADR does not attempt to close that: it is the generic primitive
[`docs/architecture/DOMAIN_MODEL.md`](../architecture/DOMAIN_MODEL.md) and E03-S04 built
specifically so future constraints (this one included) need no redesign to add - `effective_
authority_for_channel` already relies on the identical generic call. Any caller that wants to
bypass a specific safety property can always decline to call the function that enforces it and
write its own logic instead; that is true of every public function in every language and is not
particular to this constraint. Restricting `compute_effective_authority`'s visibility would be a
separately-scoped redesign of the Effective Authority primitive's entire public surface, breaking
`effective_authority_for_channel` along with it, and is out of this story's scope.

## Decision

`AuthorityInputs::provider_capability_ceiling: Option<AuthorityLevel>` is removed. In its place,
`AuthorityInputs::provider_layout: ProviderLayoutAssessment` (`cancellai-safety::provider_layout`,
new module) carries the *raw observation*, never a pre-computed ceiling:

```rust
pub enum ProviderLayoutAssessment {
    NotAssessed,
    Observed { known_signatures: Vec<LayoutSignature>, observed: LayoutSignature },
}
```

`crate::authority::base_constraints` derives the ceiling itself, via a private `layout_ceiling`
function, from `known_signatures`/`observed` directly - the identical comparison
`cancellai_guardian::structural::assess_layout` independently performs for its own detection
report. There is no ceiling value anywhere in this path for a caller to assert, discard, or
disagree with once it supplies the observation: supplying the facts and receiving a different
result than the one those facts imply is not an operation the type or the function offers.

`cancellai_guardian::structural::assess_layout`/`LayoutDriftFinding` are simplified to match:
they no longer compute or carry an authority ceiling at all (there is nothing left for them to
hand off). `cancellai_guardian::capability_authority` no longer "bridges" a ceiling; it converts
this crate's own `LayoutSignature` into `cancellai-safety`'s structurally identical, deliberately
separate type of the same name (the two crates may not depend on each other in the direction that
would let them share one), and sets `provider_layout` to the resulting `Observed` value. The
caller then calls the one public `effective_authority` itself.

This closes the round-3 counterexample precisely: reconstructing it now requires a caller to
supply the real `known_signatures`/`observed` (getting `Observe`, deterministically) *and* a
second, contradictory `AuthorityInputs` that also claims to have made an observation but asserts
different facts, or claims `NotAssessed` while it has just made one. The latter is not a new kind
of forgery this ADR introduces protection against and declines to close - it is the same "a good-
faith local observer assembles `AuthorityInputs` honestly" trust boundary every other field
(`confidence`, `activity`, `protection`, `integrity`) already relies on without a dedicated
opaque wrapper, and is a fundamentally different property from round 3's actual finding: a
*conclusion* (`Option<AuthorityLevel>`) disconnected from the *facts* it was drawn from, which
this ADR eliminates by removing the conclusion as an independently-existing value at all.

## Disclosed residual (unchanged in kind from ADR-0034, narrower in scope)

No current production caller constructs `ProviderLayoutAssessment::Observed` at all.
`cancellai_policy::resolver`'s caller-supplied inputs and `cancellai_policy::retention::
reachable_authority` both still state `NotAssessed` explicitly, honestly, because neither call
site has a live provider-root layout to observe (Guardian's structural detection operates at
provider-root scope; `reachable_authority` runs at per-artifact scope during a filesystem scan).
This ADR closes "a real observation, once supplied, cannot be discarded or contradicted" (round
3's finding); it does not and cannot compel a caller that never observed a layout to go observe
one - that live-wiring integration, connecting a real provider-root probe to a real authority-
resolution call site, remains future orchestrator work, as ADR-0034 already disclosed and this
ADR narrows rather than closes.

## Consequences

- E14-S04 can close against this design once independent review confirms the current diff.
- `AuthorityInputs` is no longer `Copy` (it now owns `Vec<LayoutSignature>` inside
  `ProviderLayoutAssessment::Observed`); every caller that reused one value across two calls now
  clones it explicitly. This is a mechanical consequence of carrying real evidence rather than a
  bare enum value, not a behavior change.
- A future orchestrator story wiring a live layout probe into a real authority-resolution call
  site closes the disclosed residual by constructing `ProviderLayoutAssessment::Observed` from
  that probe's real output - no further change to `cancellai-safety` is needed for that story to
  do so.
- Any other story adding a new named `AuthorityConstraint` from an external observation should
  read this ADR before choosing a shape: a caller-supplied *conclusion* (a ceiling, a level, a
  verdict) is freely discardable no matter how mandatory its field is, because nothing binds it
  to the facts that produced it. Threading the *raw facts* into the authority-relevant crate and
  deriving the conclusion there, once, is what actually closes that gap - the same reason
  `TrustedTier`/`BuildChannel` are opaque wrappers rather than plain enums, extended here to a
  case where the "wrapper" is the derivation itself rather than an evidence-gated promotion.

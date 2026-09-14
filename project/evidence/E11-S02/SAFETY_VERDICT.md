# Safety Verdict - E11-S02

- Change: Constraint resolver for declarative policy scopes
- Risk: CR4
- Commit/PR: `76a34f0` within review target `d2a4c2d^..b035f83`
- Independent verifier: Codex (`/root`), independent of the Claude executor
- Date: 2026-09-14

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

`resolve_effective_authority` turns the most-specific matching policy request into
`AuthorityInputs::user_requested`, then delegates the entire effective-authority decision to
`cancellai_safety::effective_authority`. The module adds no mutation path and no independent
ceiling.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-001 | Protected or low/unknown state remains non-destructive regardless of policy. | An `autopilot` artifact-type request with `ProtectionState::Protected`, and a global `autopilot` request with `KnowledgeConfidence::LowUnknown`, both return `Recommend`; the trace retains the existing lifecycle and constitutional constraints. | PASS |
| SI-025 | Policy cannot elevate above artifact, confidence, lifecycle, provider-trust, or constitutional ceilings. | Code inspection of `resolver.rs:146-154` shows that only `user_requested` is replaced. The other six `AuthorityInputs` fields are moved unmodified into `effective_authority`; an exhaustive policy requesting `Autopilot` remains capped by an artifact `Quarantine` ceiling and an untrusted provider at `Observe`. | PASS |

## Adversarial cases

- Exhaustive 32-combination scope precedence matrix: fixed most-specific order is deterministic.
- A more-specific `Autopilot` request was compared with direct safety calls under every lower
  non-policy ceiling; it can raise a caller's *request* only where the pre-existing minimum also
  permits it, never a ceiling.
- Unknown/missing policy context reaches only global/fallback; it does not invent an identity.
- `rg -n --glob '*.rs' 'resolve_effective_authority\\(' rust` found only the policy resolver,
  explanation wrapper, and their tests: no current production caller supplies real artifact data.

## Differential / compatibility evidence

- This is a new Rust policy surface; no Python equivalent exists, so a Python/Rust differential
  fixture is not applicable.
- Schema parsing rejects unsupported/missing/null versions and unrecognised authority values;
  policy is declarative data only.
- Workspace format, clippy, check, tests, and dependency-policy gates pass in this session.

## Known residual risks

- The resolver is not wired to a production command yet. A future integration must parse policy,
  supply current trusted lifecycle facts, resolve pins into `ProtectionState`, and select the
  channel-aware safety entry point where a release-channel constraint is available; no policy
  document may be treated as an authority source by itself.
- This verdict would be falsified by any change that modifies an `AuthorityInputs` field other
  than `user_requested`, bypasses `cancellai_safety::effective_authority`, or adds a production
  caller that accepts policy authority without the complete safety inputs.

## Rollback / recovery

The resolver is pure and has no persisted state or mutation. Reverting its commit removes the
unwired policy-resolution API; it cannot require data recovery.

## Owner decision

`ACCEPT`

Owner note: Accepted 2026-09-14 on the record that both round-1 findings (the E11-S01 duplicate
scope-key ambiguity and the E11-S04 budget-selection overflow) were repaired and independently
re-verified with regression tests in round 2 (`project/evidence/E11-VERIFIER-REVIEW-ROUND2.md`),
with zero new findings. The residual named above - no production caller wires this resolver to
real safety facts yet - is accepted as a known, disclosed gap for a future integration story to
close, not a defect in this story's own contract.

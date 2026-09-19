<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E18-S01
Rendered-by: Claude (orchestrating executor)
Rendered-on: 2026-09-19
Brief-Checksum: f500b766be7c4210fb12ed5fcd451504e0665a83b52153d394f5c1779263efb0

<!-- end handoff header -->
# Verifier Brief - E18-S01 - Remote target abstraction

Status: ready_for_review | Change Risk: CR3
Outcome: Model SSH/dev-container/CI-runner targets as explicit machines with independent capabilities and trust.
Dependencies: E16-S02

## Acceptance Criteria
- Remote inventory never masquerades as local state.
- Disconnected targets preserve last-seen state as stale, not current.

## Verification Contract
- Mock remote lifecycle tests.

## Safety Obligations
- none

## Documentation Impact
- docs/architecture/TARGET.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

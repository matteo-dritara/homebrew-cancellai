<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E14-S01
Rendered-by: Claude (orchestrator, E12/E13/E14 review coordination)
Rendered-on: 2026-09-20
Brief-Checksum: f39be6b11563f40d674b85942f630c71ae16f09409c4c1c7cb42314fe6d2caf4

<!-- end handoff header -->
# Verifier Brief - E14-S01 - Pressure state model

Status: ready_for_review | Change Risk: CR2
Outcome: Define GREEN/YELLOW/ORANGE/RED from free space, budgets, growth velocity, reclaimability, and active workload.
Dependencies: E13-S03

## Acceptance Criteria
- Pressure is deterministic from inputs and independently testable.
- Pressure does not change authority by itself.

## Verification Contract
- Boundary and hysteresis tests.

## Safety Obligations

### SI-027 Detection severity does not create authority

Pressure/anomaly/forecast state influences urgency and recommendation ordering but never increases mutation authority by itself.

## Documentation Impact
- docs/architecture/GUARDIAN_MODEL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

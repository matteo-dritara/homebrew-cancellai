<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E14-S04
Rendered-by: Claude (orchestrator, E12/E13/E14 review coordination)
Rendered-on: 2026-09-20
Brief-Checksum: 06830bdaa24f243876d2b4751e2358c21b8ec8ff381d8abee72636d1e7430a4c

<!-- end handoff header -->
# Verifier Brief - E14-S04 - Structural anomaly detection

Status: ready_for_review | Change Risk: CR2
Outcome: Detect session explosion, unexpected giant files, orphan growth, and provider layout drift.
Dependencies: E05-S05, E14-S01

## Acceptance Criteria
- Layout drift can downgrade provider capabilities automatically.
- Structural signals reference concrete observed evidence.

## Verification Contract
- Provider drift and explosion fixtures.

## Safety Obligations

### SI-004 Unknown provider layout/version reduces capability

Provider/version/layout drift cannot preserve destructive capabilities merely because the provider name is recognized.

## Documentation Impact
- docs/architecture/GUARDIAN_MODEL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

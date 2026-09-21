<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E14-S04
Rendered-by: claude-sonnet-5
Rendered-on: 2026-09-21
Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c

<!-- end handoff header -->
# Verifier Brief - E14-S04 - Structural anomaly detection

Status: ready_for_review | Change Risk: CR4
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

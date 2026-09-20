<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E14-S03
Rendered-by: Claude (orchestrator, E12/E13/E14 review coordination)
Rendered-on: 2026-09-20
Brief-Checksum: bed472633028a03e5506c75cb1837c12511b21ed2b3cf472ccfb015e9aa73180

<!-- end handoff header -->
# Verifier Brief - E14-S03 - Behavioral baseline anomaly detection

Status: ready_for_review | Change Risk: CR2
Outcome: Build local baselines and flag statistically/heuristically unusual growth without content inspection.
Dependencies: E13-S03

## Acceptance Criteria
- Baseline uses bounded analytical memory.
- Anomaly score is explanatory and never directly destructive.

## Verification Contract
- Known-normal and runaway simulation corpus.

## Safety Obligations

### SI-027 Detection severity does not create authority

Pressure/anomaly/forecast state influences urgency and recommendation ordering but never increases mutation authority by itself.

## Documentation Impact
- docs/architecture/GUARDIAN_MODEL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

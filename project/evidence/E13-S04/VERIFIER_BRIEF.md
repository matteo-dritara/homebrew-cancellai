<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E13-S04
Rendered-by: Claude (orchestrating executor)
Rendered-on: 2026-09-19
Brief-Checksum: 29d1f9f58400ce38b5ee0394cd6ab5c300c0ef55fa93474a069cc5a440356e73

<!-- end handoff header -->
# Verifier Brief - E13-S04 - Self-budget and local reset

Status: ready_for_review | Change Risk: CR3
Outcome: Enforce cancellAI state/log budgets and provide reset of cancellAI state only.
Dependencies: E13-S02, E13-S03

## Acceptance Criteria
- Budget overrun triggers compaction before growth continues.
- reset --local-state cannot target provider roots.
- Ephemeral inspect performs no persistent writes.

## Verification Contract
- Self-budget stress test and reset boundary tests.

## Safety Obligations

### SI-026 cancellAI reset/self-budget cannot target provider payload

Internal compaction/reset operations are restricted to cancellAI-owned state and cannot reuse provider-root deletion primitives.

## Documentation Impact
- docs/architecture/PERSISTENCE_MODEL.md
- docs/CLI.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

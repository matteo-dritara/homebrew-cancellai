<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E13-S05
Rendered-by: Claude Sonnet 5 (executor)
Rendered-on: 2026-09-17
Brief-Checksum: d78bbc149e2b33b351a7d72fff727d853f1efb6b165ae73c0c0bfa8a1a5d105d

<!-- end handoff header -->
# Verifier Brief - E13-S05 - Incremental inventory reuse

Status: ready_for_review | Change Risk: CR3
Outcome: Reuse persisted facts safely between scans while invalidating on identity, metadata, provider-knowledge, and completeness changes.
Dependencies: E13-S01, E04-S03

## Acceptance Criteria
- Cached facts are performance hints only and never a source of destructive truth.
- Identity, mtime/metadata, provider fingerprint, knowledge version, or completeness uncertainty invalidates the relevant cache scope.
- Fresh execution-time observation remains mandatory for mutation preconditions.

## Verification Contract
- Stale-cache, provider-layout-change, clock/mtime edge, and partial-scan adversarial tests.

## Safety Obligations

### SI-024 Persistent cache is never destructive truth

Cached inventory/current-state data may accelerate reads but mutation preconditions depend on fresh revalidation. Stale cache cannot authorize mutation.

## Policy and Guardian

## Documentation Impact
- docs/architecture/PERSISTENCE_MODEL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

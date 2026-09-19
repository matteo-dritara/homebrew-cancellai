<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E18-S02
Rendered-by: Claude (orchestrating executor)
Rendered-on: 2026-09-19
Brief-Checksum: ababc394cafa0c872500d5cbf99e99abcb7a2786c8e2a89a351740cc55021ed3

<!-- end handoff header -->
# Verifier Brief - E18-S02 - Local-agent remote execution boundary

Status: ready_for_review | Change Risk: CR4
Outcome: Keep mutation execution on the target node under its local safety kernel even when a remote controller requests policy evaluation.
Dependencies: E18-S01

## Acceptance Criteria
- Remote control cannot bypass target safety invariants.
- Every remote request is authenticated, authorized, and audit-linked.

## Verification Contract
- Replay, stale-policy, and unauthorized-controller tests.

## Safety Obligations

### SI-031 Remote controller cannot bypass target-node safety

Remote/fleet requests are intents. The target node independently authenticates, resolves policy, builds/revalidates plans, and retains final mutation authority.

## Safety proof style

For a CR4 story, verification should answer:

1. Which invariant(s) could the change violate?
2. What counterexample would prove failure?
3. Which automated tests reproduce those counterexamples?
4. Which residual risk cannot be eliminated and why?
5. What rollback/recovery exists if the assumption is wrong in production?

## Documentation Impact
- docs/security/THREAT_MODEL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

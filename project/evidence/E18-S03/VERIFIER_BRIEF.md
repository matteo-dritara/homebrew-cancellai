<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E18-S03
Rendered-by: Claude (orchestrating executor)
Rendered-on: 2026-09-19
Brief-Checksum: b1a9bb24614ecf62be2b39170d95ad793cb6ed9fcb967cc3a3e8a7a3c85e061a

<!-- end handoff header -->
# Verifier Brief - E18-S03 - OSS/commercial protocol boundary

Status: ready_for_review | Change Risk: CR4
Outcome: Define an open local node protocol and separate optional fleet-control services without reducing local functionality.
Dependencies: E18-S02

## Acceptance Criteria
- Single-machine workflows remain fully functional without account/cloud.
- Commercial services add coordination, not local destructive capabilities.
- If no commercial or fleet-coordination service is configured or reachable, single-machine functionality is not reduced or refused.

## Verification Contract
- Offline conformance tests.

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
- docs/PRODUCT.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E34-S05
Rendered-by: Claude
Rendered-on: 2026-09-24
Brief-Checksum: 4b9fbf7d73e21b96b246696bccd4e73eeb6b623cd3fec902aee66adc2391387e

<!-- end handoff header -->
# Verifier Brief - E34-S05 - A brief re-rendered after its criteria change keeps the verdicts that answered the old one

Status: ready_for_review | Change Risk: CR1
Outcome: verifier_handoff.py checked every verdict a story ever received against the story's current brief, so narrowing an acceptance criterion after a review round - as the owner did for E34-S02 on 2026-09-24 - made the earlier round's record fail the gate. Re-rendering now keeps the superseded brief beside the new one, and a verdict is valid when it answers either the current brief or a superseded brief the gate rendered.
Dependencies: none

## Acceptance Criteria
- When a brief is re-rendered with a different checksum, the system shall keep the previous brief byte for byte as a superseded brief named by its checksum.
- The system shall accept a verdict whose checksum answers the story's current brief or one of its superseded briefs, and refuse any other checksum.
- If a superseded brief's body does not hash to its declared checksum, or it records no Rendered-by, then the system shall report it and no verdict shall be accepted against it.

## Verification Contract
- Unit tests over a synthetic evidence tree: re-render archives, an old verdict passes against its superseded brief, an unknown checksum fails, a tampered superseded brief fails; check passes on the committed repository.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_PROTOCOL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

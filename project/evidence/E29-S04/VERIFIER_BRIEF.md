<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E29-S04
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: fbccd60d2bc26f28870e263744c9a9f90b5c1c884e956df16827642c15e87c94

<!-- end handoff header -->
# Verifier Brief - E29-S04 - A method defect left proposed is aged, not forgotten

Status: ready_for_review | Change Risk: CR1
Outcome: E28-S03 requires every recorded method defect to carry a disposition, and `proposed` satisfies it with no expiry. That is the right resting state - it means the owner has the finding and has not ruled - and it is also a state an entry can sit in forever without any gate noticing. The toolchain manifest already solves the same problem for decisions: one older than the review cadence is reported as *unexamined* rather than wrong. The same treatment applies here, and nothing more: ageing a proposal must not convert it into a defect.
Dependencies: none

## Acceptance Criteria
- A proposal older than a stated cadence shall be reported as unexamined, naming its age and the story that carries it.
- An aged proposal shall not be declared invalid, and the report shall not fail the gate on age alone.
- An accepted or declined entry shall stay visible after its disposition, so the record of what was considered survives.

## Verification Contract
- A proposal dated past the cadence is shown to be reported, and one inside it is shown not to be.
- An aged proposal is shown not to fail the gate.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_PROTOCOL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

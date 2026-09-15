<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E28-S02
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: b317d36f455274c32d020ebf40e2005394b5207ec9b9e5882e322784d9990869

<!-- end handoff header -->
# Verifier Brief - E28-S02 - The verifier handoff is a mechanism, not a manual step

Status: ready_for_review | Change Risk: CR2
Outcome: Executor/verifier separation is the method this repository is built on, and it is the only part of the method with no mechanism. `project_os.py brief <ID> --role verifier` renders the verifier's input; a human then carries it to the other agent, carries the verdict back, and the evidence packet records a review whose transport nobody can audit. Two things follow that the ledger cannot currently see: whether the verifier was given the brief the gate rendered or a paraphrase of it, and whether the verdict committed is the verdict the verifier produced. The handoff becomes an artifact - a rendered brief with a checksum, and a verdict recorded against it - so `check_evidence.py` can assert the pairing. What must not change is who may write what: the executor's session may render, transport and record, and may never author a verdict or move a story past ready_for_review. Automating a handoff between two roles is the most direct way to collapse them, so the story's main obligation is that it does not.
Dependencies: E28-S01

## Acceptance Criteria
- A verifier brief shall be rendered to a durable artifact with a content checksum, so the input a verdict claims to answer is identifiable rather than asserted.
- A recorded verdict shall name the brief it answers, and if it names a brief that does not exist or whose checksum does not match, then the evidence gate shall refuse.
- The executor's session shall not be able to author a verdict or advance a story beyond ready_for_review, and the gate shall refuse such an attempt rather than rely on the executor declining to make it.
- If the verifier agent is unavailable or returns nothing, then the handoff shall leave the story at ready_for_review with the failure recorded, and shall never fall back to the executor's own judgement.
- A CR4 Safety Verdict shall remain attributable to the verifier that produced it, and the mechanism shall not make an unattributed verdict expressible.
- The mechanism shall be optional: a handoff performed by a human shall remain valid, because the method is the separation and not the automation of it.

## Verification Contract
- An adversarial case attempts to record a verdict from the executor's own session and is shown to be refused by the gate, not merely discouraged by documentation.
- A verdict is recorded against a brief whose checksum has been altered by one byte, and the evidence gate is shown to refuse it.
- The verifier-unavailable path is exercised deliberately, asserting that the story stays at ready_for_review and that no verdict is written.
- The existing manual route is exercised unchanged on a real story, proving the mechanism did not become mandatory.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_PROTOCOL.md
- AGENTS.md
- project/templates/

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E28-S03
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: 1926c4cb947c9c7941ed6ff08e7850df1b235051f5daa9ce7e90672a979fbd07

<!-- end handoff header -->
# Verifier Brief - E28-S03 - The method improves from recorded friction, not from recollection

Status: ready_for_review | Change Risk: CR1
Outcome: Every rule in AGENTS.md that is worth having was written after something went wrong, and the record of what went wrong is nowhere. A session notices that a rule is missing, wrong, or unreachable; the correction happens in chat; the session ends and the observation is gone. Two from the E27 work alone: an executor nearly reverted a correct verifier repair because it read the wrong version of a dependency, and rebuilt a defect class it had fixed days earlier because nothing recorded that the environment, not the code, had been the cause. Both were caught, neither left an artifact, and a third session can make either again. The evidence packet already records residual risks about the product; this adds the same discipline for defects in the method itself, in the packet where the work lives rather than in a new store. The idea comes from rebelytics/one-skill-to-rule-them-all, which was rejected as a package - 14,460 tokens, always-on by its own instruction, and it generates skill edits - and kept as a concept. The concept survives only if the generation does not: a recorded defect produces a proposal the owner decides, never an edit to a skill or a canonical document.
Dependencies: none

## Acceptance Criteria
- A defect in the method shall be recorded against the story where it was observed, with what was expected, what happened, and the correction - not in a free-floating log detached from the work.
- A recorded defect shall name the document, gate or skill that would have prevented it, or state explicitly that none exists, so the record either points somewhere or says it does not.
- A recorded defect shall carry a disposition, and if it carries none then the evidence gate shall refuse the packet, because an observation nobody dispositioned is a note rather than a finding.
- Nothing in this mechanism shall modify a skill, AGENTS.md or any canonical document automatically; a defect produces a proposal the owner decides.
- If the owner declines a proposal, then the decline shall be recorded with its date, so a later session does not re-propose it as though it were new.
- The record shall live inside the evidence packet the story already commits, and shall not create a second store of truth about the method.

## Verification Contract
- The gate is shown refusing a packet whose method-defect record has no disposition, before it is shown accepting one that has.
- A test asserts that no path in the mechanism writes to .claude/skills/ or to a canonical document, so the property survives someone later adding a convenience.
- The format is exercised on the two real defects from E27 named in the outcome - a format that cannot carry the cases that motivated it is the wrong format, and this is the cheapest way to find that out.
- A declined proposal is shown to remain visible rather than disappearing, so the record of what was considered and rejected survives.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_PROTOCOL.md
- project/templates/
- AGENTS.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

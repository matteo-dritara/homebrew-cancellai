<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E30-S01
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: c9972e04c6e0e006af579793120e585bb12b0cf0b48b449a77aa063f52c6ad0f

<!-- end handoff header -->
# Verifier Brief - E30-S01 - A story that is blocked records what blocks it

Status: done | Change Risk: CR2
Outcome: `blocked` is a status with ten sibling fields and none of them says why. The two stories carrying it are the two that gate the entire Rust cutover line, and both declare dependencies that are all satisfied, so the only honest reading of the control plane is that nothing holds them - which is false. The reasons exist, in `docs/development/RELEASE_GATES.md`'s cutover checklist, and that document has already drifted: it names `E16-S05` as an outstanding dependency of E17-S07 and E16-S05 is `done`. A block that only prose can explain is a block nobody can check, and a document nobody checks is a document that is eventually wrong.
Dependencies: none

## Acceptance Criteria
- If a story or epic is blocked, then it shall record what blocks it, and the control plane shall refuse the status without it.
- A recorded blocker shall point at where its argument lives, so the reason can be read rather than inferred.
- If an item is not blocked, then it shall not carry a blocker, because a stale reason is worse than none.
- The two stories already blocked shall record the reasons that actually hold them, taken from the cutover checklist rather than invented.
- A blocker naming a work item that has since closed shall be reported, because that is how the existing prose went wrong.

## Verification Contract
- The gate is shown refusing a blocked item with no recorded blocker before it is shown accepting one with it.
- A non-blocked item carrying a blocker is shown to be refused.
- A blocker naming a closed work item is shown to be reported, using the real E16-S05 reference the prose still carries.
- The committed control plane is shown to pass, with both real blockers recorded.

## Safety Obligations
- none

## Documentation Impact
- docs/development/WORK_ITEM_MODEL.md
- docs/development/RELEASE_GATES.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

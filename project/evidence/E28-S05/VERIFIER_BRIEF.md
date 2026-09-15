<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E28-S05
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: 557d7bb5178ac60f131e9b5cc9ea5a00067826b012a13285b19a2537f91e01f9

<!-- end handoff header -->
# Verifier Brief - E28-S05 - An epic that closed without a release still satisfies a dependency

Status: ready_for_review | Change Risk: CR2
Outcome: ADR-0025 gave an epic a second way to close: `done_no_release`, for an epic whose work is finished and which changed nothing in the shipped artifact. `project_os.py` defines the pair as `CLOSED_EPIC_STATUS`, and a test asserts its contents - and no code consults it. The dependency check compares against the literal string `done`, so an epic closed the second way satisfies nothing that depends on it, permanently. Three epics have closed that way (E11, E24, E26) and three depend on them (E12, E13, E28): the entire quarantine and local-state line of the product backlog is unreachable, and so is this epic. It was found by trying to move E28 to ready_for_review, not by any gate - the constant that names the correct answer has sat beside the wrong one since ADR-0025 landed, protected by a test that checks the definition rather than its use.
Dependencies: none

## Acceptance Criteria
- An epic whose dependency closed as done_no_release shall be allowed to advance, because the dependency's work is finished and that is what a dependency asserts.
- If a dependency is genuinely unfinished, then the advance shall still be refused and the unfinished dependency named.
- The definition of a closed epic shall exist once and shall be consulted wherever closure is decided, rather than restated as a literal.
- A story whose dependency closed that way shall be treated the same as its epic, so the two levels cannot disagree about what finished means.

## Verification Contract
- The case is shown failing before it is shown passing: E28 at ready_for_review behind E26 was refused, which is how the defect was found.
- A test asserts that the constant is used and not merely defined, so the same divergence cannot reappear by restating the literal.
- An unfinished dependency is still refused, proving the fix widened the accepted set rather than removing the check.
- The three epics currently blocked by this - E12, E13 and E28 - are shown to advance afterwards.

## Safety Obligations
- none

## Documentation Impact
- docs/development/WORK_ITEM_MODEL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

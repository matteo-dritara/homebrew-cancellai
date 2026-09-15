<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E29-S07
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: 3b58c93324cfd6a21e7091de984d1ea1911ac1a5a889eae32516ba91c5364a9d

<!-- end handoff header -->
# Verifier Brief - E29-S07 - The set of statuses that close an epic cannot grow without a reviewed decision

Status: ready_for_review | Change Risk: CR2
Outcome: E28-S05 made `CLOSED_EPIC_STATUS` the single definition of a finished epic and the tests assert behaviour against the statuses that exist today. A future edit adding a status to that set would satisfy every dependency in the backlog at once, silently, and no test would object - which is the same shape as the defect E28-S05 repaired, moved one level up. The round-2 review raised this after the round-1 verdict had recorded no residual for the story at all.
Dependencies: none

## Acceptance Criteria
- The exact set of statuses that close an epic shall be asserted, so adding one fails until the assertion is consciously updated.
- If the set is widened, then the change shall require a reviewed policy edit alongside the test, rather than a test update alone.
- The behavioural test that an unfinished dependency is still refused shall remain, because an exact-set assertion alone would not catch a wrong member.

## Verification Contract
- Adding a status to the set is shown to fail the suite until both the policy record and the assertion are updated.
- The existing behavioural refusal test is shown still to pass.

## Safety Obligations
- none

## Documentation Impact
- docs/development/WORK_ITEM_MODEL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

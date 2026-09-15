<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E25-S16
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: 4f9584598ad195248f14756741ce995bf37197f868d50a205e1f2c918dc6de81

<!-- end handoff header -->
# Verifier Brief - E25-S16 - An anchor picked for stability by phase alone went stale within one epic close

Status: ready_for_review | Change Risk: CR2
Outcome: The `story-status-forged` mutant was anchored on E11-S01, chosen - by the comment above the anchor itself - because E11 was 'a future phase' whose stories were 'stable', after an earlier anchor on a story this session was actively moving went stale within the hour. E11 closed as `done_no_release` in this same session, turning that story's status from `planned` to `done` and taking the anchor's match count from one to zero: `tests/test_governance_extras.py::MutantIntegrityTests` failed on main, and `story-status-forged` stopped being demonstrated at all rather than being demonstrated on the wrong thing, which is the same class of silent gate loss E25-S12 found for SI-008 and E25-S06 exists to catch in general. Phase is not a stability signal this backlog can rely on - the project reached a 'future' phase's epic inside one session - so the replacement anchor is chosen by a measurable property instead: E19-S02 sits behind more unmet transitive dependencies (9) than any other planned story in the backlog, computed from `project/epics/*.json`, and carries no evidence packet.
Dependencies: E25-S06, E25-S12

## Acceptance Criteria
- The `story-status-forged` mutant in scripts/gate_sensitivity.py anchors on E19-S02 rather than E11-S01, and the anchor text matches exactly one site in project/epics/E19.json.
- The reasoning for the chosen anchor - the measurable property used, not just the story id - is recorded next to the mutant definition, so a third failure updates the same record instead of re-deriving the pattern from scratch.
- project/generated/GATE_SENSITIVITY.md and the committed sensitivity report are regenerated against the new anchor.
- If the `tests` workflow is re-run on main after this change, then it shall report success, because a fix that leaves the same job red has not fixed the finding it was opened for.

## Verification Contract
- tests/test_governance_extras.py::MutantIntegrityTests passes locally against the current working tree.
- scripts/gate_sensitivity.py check reproduces the committed report byte-for-byte.
- A manual replay of `python3 -m pytest tests -v` no longer shows story-status-forged failing.

## Safety Obligations
- none

## Documentation Impact
- project/generated/GATE_SENSITIVITY.md
- CHANGELOG.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

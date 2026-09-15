# Evidence Packet - E29-S04

- Commit/PR: on `main`, this epic's commit
- Executor: Claude
- Independent verifier: pending - Codex
- Change Risk: CR1
- Spec version/commit: project/epics/E29.json

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | A proposal past the 90-day cadence is reported with its age. `tests/test_method_defects.py::AProposalIsAgedNotForgotten::test_a_proposal_past_the_cadence_is_reported` | PASS |
| AC2 | It is a warning, never an error. `tests/test_method_defects.py::AProposalIsAgedNotForgotten::test_an_aged_proposal_is_not_an_error` | PASS |
| AC3 | Accepted and declined entries stay in the packet and are never aged. `tests/test_method_defects.py::AProposalIsAgedNotForgotten::test_a_disposed_entry_is_never_aged` | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| none declared | This story governs the engineering process and the agent toolchain, not the mutation boundary or any runtime authority. | The mutation-boundary gate (SI-019) is unchanged and still passes. | PASS |

## Verification Commands

```text
python3 -m pytest tests -q
pre-commit run --all-files
python3 scripts/process_metrics.py check
python3 scripts/check_agent_toolchain.py check
python3 scripts/check_skill_content.py check
python3 scripts/check_evidence.py check
```

## Compatibility

- Platforms/providers/schemas exercised: macOS, Python 3.13. No product surface, no schema change.

## Performance / operability

- None measurable. All changes are to governance scripts already in the gate set.

## Documentation updated

- `docs/development/AGENT_PROTOCOL.md`, `docs/development/AGENT_TOOLCHAIN.md`, `docs/development/WORK_ITEM_MODEL.md`, `docs/adrs/0029-a-verdict-s-author-is-declared-not-authenticated-and-that-is-accepted-for-now.md`

## Method defects

- none

## Residual risks

- A proposal can be re-dated to reset its age, and nothing compares the date against when the entry first appeared in git history.
- The 90-day cadence matches the toolchain manifest's by analogy, not by evidence that 90 days is where an unresolved proposal starts costing something.

## Verifier verdict

pending

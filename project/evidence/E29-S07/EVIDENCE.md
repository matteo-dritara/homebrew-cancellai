# Evidence Packet - E29-S07

- Commit/PR: on `main`, this epic's commit
- Executor: Claude
- Independent verifier: pending - Codex
- Change Risk: CR2
- Spec version/commit: project/epics/E29.json

## Outcome

REPAIRED by the independent verifier: the document test only required the existing status words
to occur somewhere in the policy, so code and assertion could widen together without a policy edit.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | `tests/test_project_os.py::ClosedEpicDependencies::test_the_closing_set_is_exactly_these_two` asserts the exact set. | PASS |
| AC2 | The test pins the exact policy sentence defining the two closing statuses, so a widened code assertion cannot pass without its policy edit. `::test_the_policy_document_names_the_same_two` | REPAIRED |
| AC3 | The behavioural refusal test is unchanged and still passes. `::test_an_unfinished_dependency_is_still_refused` | PASS |

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

- The exact-set assertion is a tripwire, not an argument: it forces a conversation, it does not judge whether a proposed new member is correct.
- A test asserting the policy document names the same statuses couples a test to prose, which drifts differently from code.

## Verifier verdict

REPAIRED pending epic-round record

# Evidence Packet - E29-S05

- Commit/PR: on `main`, this epic's commit
- Executor: Claude
- Independent verifier: pending - Codex
- Change Risk: CR2
- Spec version/commit: project/epics/E29.json

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | A `measured` block records tokens bound to a component version and a date. `tests/test_agent_toolchain.py::LocalMeasurementsForWhatCiCannotSee::test_a_measurement_bound_to_the_current_version_counts` | PASS |
| AC2 | A version mismatch reads as stale, not current. `::test_a_measurement_taken_against_another_version_is_stale_not_current` | PASS |
| AC3 | No measurement stays unmeasured rather than zero. `::test_no_measurement_stays_unmeasured_rather_than_zero` | PASS |
| AC4 | CI passes with none recorded at all. `::test_ci_passes_with_no_local_measurements_at_all` | PASS |

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

- No local measurement is recorded yet, so the mechanism is tested and unexercised on real data. The first one recorded is the real test of whether the binding is the right one.
- A recorded measurement is trusted as taken: nothing proves the number came from the component it names.

## Verifier verdict

pending

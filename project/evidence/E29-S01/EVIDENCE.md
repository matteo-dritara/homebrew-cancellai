# Evidence Packet - E29-S01

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
| AC1 | `NOT_COUNTED` lists every recognised-but-uncounted record by name and the report prints it. `tests/test_review_yield.py::RecordsThisToolDoesNotCount::test_a_skipped_record_is_named_rather_than_dropped` | PASS |
| AC2 | `Review-Scope: epic` and `Round: N` in a record's header make it countable whatever its filename. `tests/test_review_yield.py::RecordsThisToolDoesNotCount::test_a_record_declaring_epic_scope_is_counted_whatever_its_filename` | PASS |
| AC3 | `REPAIRED` is a verdict and `found` is separate from `rejected`; the yield column reads `found`. `tests/test_review_yield.py::FindingIsNotRejecting` | PASS |
| AC4 | Every figure is computed from the records on each run, and no committed record uses `REPAIRED`, so historical yields are unchanged. `tests/test_review_yield.py::FindingIsNotRejecting::test_the_new_verdict_changes_no_historical_number` | PASS |

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

- `REPAIRED` is a verdict a reviewer chooses to write. A reviewer who repairs and records `PASS` still measures zero, so the fix removes the tool's blindness and not the reporter's discretion.
- No historical round is reclassified. E28's round 1 remains uncounted and is now listed as uncounted, which is honest and is not the same as measured.
- The 10% threshold is unchanged, and ADR-0025 already says it is a judgement rather than a derivation.

## Verifier verdict

pending

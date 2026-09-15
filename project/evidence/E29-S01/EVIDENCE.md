# Evidence Packet - E29-S01

- Commit/PR: on `main`, this epic's commit
- Executor: Claude
- Independent verifier: pending - Codex
- Change Risk: CR2
- Spec version/commit: project/epics/E29.json

## Outcome

REPAIRED by the independent verifier: ADR-0025 still defined yield as rejections while the
implementation correctly measured findings, and the handoff gate could not bind an epic record's
per-story checksums. The clarification and epic-record parsing now make the rule, report, and
review artifact agree.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | `NOT_COUNTED` lists every recognised-but-uncounted record by name and the report prints it. `tests/test_review_yield.py::RecordsThisToolDoesNotCount::test_a_skipped_record_is_named_rather_than_dropped` | PASS |
| AC2 | `Review-Scope: epic` and `Round: N` in a record's header make it countable whatever its filename; the handoff gate also validates the per-story checksum in an epic record. `tests/test_verifier_handoff.py::TheSeparationSurvivesTheAutomation::test_an_epic_round_answers_each_named_story_brief` | REPAIRED |
| AC3 | `REPAIRED` is a verdict and `found` is separate from `rejected`; the yield column reads `found`. `tests/test_review_yield.py::FindingIsNotRejecting` | PASS |
| AC4 | Every figure is computed from the records on each run, and no pre-E29 committed record uses `REPAIRED`, so historical yields are unchanged. `tests/test_review_yield.py::FindingIsNotRejecting::test_the_new_verdict_changes_no_historical_number` | PASS |

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

- **What happened**: the executor changed what `yield` counts in `process_metrics.py` - from rejections to findings - and left ADR-0025, the decision that defines the stopping rule, saying the old thing. The code and the decision it implements disagreed, and every gate stayed green because no gate compares them. Found by the round-1 verifier and repaired in `44d4bab`, which amended the ADR. **Prevented by**: none exists; nothing here requires a change to a mechanism an ADR specifies to update that ADR in the same commit, and the documentation-impact field of a story is prose nobody checks against the diff. **Disposition**: proposed 2026-09-15

## Residual risks

- `REPAIRED` is a verdict a reviewer chooses to write. A reviewer who repairs and records `PASS` still measures zero, so the fix removes the tool's blindness and not the reporter's discretion.
- No historical round is reclassified. E28's round 1 remains uncounted and is now listed as uncounted, which is honest and is not the same as measured.
- The 10% threshold is unchanged, and ADR-0025 already says it is a judgement rather than a derivation.

## Verifier verdict

REPAIRED pending epic-round record

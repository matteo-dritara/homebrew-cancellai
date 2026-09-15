# Evidence Packet - E29-S02

- Commit/PR: on `main`, this epic's commit
- Executor: Claude
- Independent verifier: pending - Codex
- Change Risk: CR2
- Spec version/commit: project/epics/E29.json

## Outcome

REPAIRED by the independent verifier: matching scanner versions previously accepted a waiver
file with an absent or malformed `revalidated` value, so the claimed dated act was not enforced.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | `project/skill_content_waivers.json` records `scanner` and `revalidated`. `tests/test_skill_content.py::WaiversAreBoundToTheScannerThatProducedThem::test_the_committed_file_is_bound_to_the_pinned_version` | PASS |
| AC2 | `waiver_provenance_errors` refuses before the scan when the two differ, naming both versions. `tests/test_skill_content.py::WaiversAreBoundToTheScannerThatProducedThem::test_a_pin_change_refuses_until_the_waivers_are_revalidated` | PASS |
| AC3 | A matching version now also requires a present, valid, non-future ISO-8601 `revalidated` date; passing once does not set it. `tests/test_skill_content.py::WaiversAreBoundToTheScannerThatProducedThem::test_matching_versions_without_a_revalidation_date_still_refuse` | REPAIRED |

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

- The gate refuses on a version difference; it cannot tell whether a given fingerprint actually drifted. Revalidation is a human reading the report, and the mechanism records that it happened rather than that it was done well.
- Revalidation remains a human reading the report: the date proves a claimed act was recorded, not that the review was performed well.

## Verifier verdict

REPAIRED pending epic-round record

# Evidence Packet - E29-S06

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
| AC1 | Every component records `license_source_revision`, resolved from the real upstreams. `tests/test_agent_toolchain.py::LicenceEvidenceIsBoundToARevision::test_every_committed_component_names_the_revision_it_was_read_from` | PASS |
| AC2 | A licence without a revision is reported, naming what cannot be told. `::test_a_licence_without_a_source_revision_is_reported` | PASS |
| AC3 | The binding is a warning and never an error, so no network is required to pass. `::test_the_report_is_a_warning_and_never_an_error` | PASS |
| AC4 | Not discharged in this story: the comparison itself belongs with E26-S02's network check. Recorded as a residual. | PASS |

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

- The binding is recorded; the comparison is not implemented. Nothing yet re-reads an upstream licence, so this story makes the check possible rather than performing it - which is the shape the story declared and is still a gap between what is recorded and what is verified.
- The revisions were resolved at HEAD of each upstream's default branch, not at the exact revision the plugins were installed from, because the manifest pins those as `unpinned`.

## Verifier verdict

pending

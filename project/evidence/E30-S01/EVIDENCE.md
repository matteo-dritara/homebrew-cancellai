# Evidence Packet - E30-S01

- Commit/PR: on `main`, this epic's commit
- Executor: Claude
- Independent verifier: none - the owner waived independent review for this story
- Change Risk: CR2
- Spec version/commit: project/epics/E30.json

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | `project_os.blocked_by_errors` refuses a blocked item whose dependencies are all satisfied and which records no `blocked_by`. Shown failing on the real shape of E06-S04 before it was recorded. `tests/test_blocked_reasons.py::ABlockNamesWhatHoldsIt::test_blocked_with_every_dependency_satisfied_and_no_reason_is_refused` | PASS |
| AC2 | `blocked_by.argument` is a repository path; both recorded blockers name `docs/development/RELEASE_GATES.md`. `tests/test_blocked_reasons.py::TheCommittedControlPlane::test_the_two_cutover_stories_record_a_real_argument` | PASS |
| AC3 | An item in any non-blocked status carrying `blocked_by` is refused. `tests/test_blocked_reasons.py::AStaleReasonIsWorseThanNone::test_an_unblocked_item_carrying_a_reason_is_refused` | PASS |
| AC4 | E06-S04 and E17-S07 record the reasons the cutover checklist actually gives - G2's missing independent adversarial pass and G4's dependence on E17-S07 for the first; the Guardian chain E16 -> E15 for the second. Neither was invented: both are quoted from `docs/development/RELEASE_GATES.md`. | PASS |
| AC5 | A `waiting_on` naming a closed work item is reported. `tests/test_blocked_reasons.py::AStaleReasonIsWorseThanNone::test_waiting_on_something_that_has_closed_is_reported`, and `::test_no_recorded_blocker_waits_on_something_already_closed` asserts the committed control plane carries none. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| none declared | The change adds a required field to a status in the control plane. It touches no mutation path and no runtime authority. | The mutation-boundary gate (SI-019) is unchanged and still passes. | PASS |

## Verification Commands

```text
python3 scripts/project_os.py check
python3 scripts/check_schemas.py check
python3 scripts/check_docs.py check
python3 -m pytest tests -q
pre-commit run --all-files
```

## Compatibility

- Platforms/providers/schemas exercised: `project/schemas/epic.schema.json` gains an optional `blocked_by` object on both epics and stories. No existing document becomes invalid: the field is additive and `additionalProperties` was already `false`, so it had to be declared rather than smuggled in.

## Performance / operability

- None. One extra pass over the epics and stories already loaded.

## Documentation updated

- `docs/development/WORK_ITEM_MODEL.md`, `docs/development/RELEASE_GATES.md`

## Method defects

- **What happened**: `RELEASE_GATES.md` named `E16-S05` as an outstanding dependency of E17-S07 in two separate paragraphs, and `E16-S05` had closed. The document is the living checklist the cutover gate is evaluated against, it was last updated 2026-09-12, and nothing compared its claims to the control plane - so the only record of why two CR4 stories were blocked was prose that had quietly become wrong. Both references are corrected in this change and the chain now lives in the stories' own `blocked_by`, where a gate reads it. **Prevented by**: none exists; nothing requires a document that names work items to have those references checked, and `check_docs.py` validates links and invariant ids but not work-item ids. **Disposition**: proposed 2026-09-15

## Residual risks

- The rule is narrow by design: an open dependency counts as explanation, so an item blocked for a reason *other* than its open dependency can still hide behind it. That is the price of not duplicating the dependency graph in prose, and it is a real gap rather than a theoretical one.
- `blocked_by.summary` is prose. The gate checks that it exists and that `waiting_on` does not name something closed; it cannot check that the summary is true, and the two recorded summaries are the executor's reading of the checklist rather than an independent one.
- `check_docs.py` still does not validate work-item references in prose, so another document can go stale the same way. Recorded as a method defect rather than fixed here.
- The owner initially waived independent review. A retrospective independent review subsequently
  repaired the implementation; see `E30-S01-VERIFIER-REVIEW.md` (Codex, 2026-09-15).

## Verifier verdict

REPAIRED — the retrospective independent review found that the schema and runtime checker accepted
`blocked_by` without its required argument path, and that `waiting_on` could name an unknown or
cancelled work item. The repair requires a readable repository file and a current, open work item;
see `E30-S01-VERIFIER-REVIEW.md` (Codex, 2026-09-15). The initial waiver remains historical, not
the final review state.

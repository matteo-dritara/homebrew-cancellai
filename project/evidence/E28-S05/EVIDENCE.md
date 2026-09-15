# Evidence Packet - E28-S05

- Commit/PR: on `main`, this epic's commits
- Executor: Claude
- Independent verifier: pending - Codex
- Change Risk: CR2
- Spec version/commit: project/epics/E28.json

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | `project_os.py` now tests epic dependencies against `CLOSED_EPIC_STATUS`. E28 behind `done_no_release` E26 advances; before the change `python3 scripts/project_os.py check` raised `E28: status ready_for_review but epic dependencies are not done: ['E26']`. `tests/test_project_os.py::ClosedEpicDependencies` | PASS |
| AC2 | A dependency in any non-closing status still refuses and is named. `tests/test_project_os.py::ClosedEpicDependencies::test_an_unfinished_dependency_is_still_refused` | PASS |
| AC3 | Both dependency checks consult `CLOSED_EPIC_STATUS`; a test asserts the constant is used rather than only defined, which is what let the divergence live. `tests/test_project_os.py::ClosedEpicDependencies::test_the_definition_is_consulted_not_restated` | PASS |
| AC4 | The story-level check distinguishes the two: an epic dependency is satisfied by either closing status, a story dependency only by `done`, because `done_no_release` is an epic status. `tests/test_project_os.py::ClosedEpicDependencies::test_a_story_dependency_still_requires_done` | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| none declared | The change widens which dependency states are accepted. The counterexample that matters is the opposite one - that it does not accept an unfinished dependency - and it is tested. | `tests/test_project_os.py::ClosedEpicDependencies::test_an_unfinished_dependency_is_still_refused` | PASS |

## Verification Commands

```text
python3 scripts/project_os.py check
python3 -m pytest tests -q
pre-commit run --all-files
```

## Compatibility

- Platforms/providers/schemas exercised: control plane only; no product surface, no schema change.

## Performance / operability

- None. The change replaces a string comparison with a set membership test.

## Documentation updated

- `docs/development/WORK_ITEM_MODEL.md`

## Method defects

- **What happened**: `CLOSED_EPIC_STATUS` was defined, carried an explanatory comment, and was asserted by `tests/test_process.py::test_done_no_release_is_an_epic_status_and_not_a_story_status` - and was consulted by no code at all. The test checked the definition's contents, never that anything read it, so a constant that names the correct answer sat beside a literal giving the wrong one and every gate stayed green. **Prevented by**: none exists; nothing here says that a constant asserting a semantic rule must be shown to be consulted, and a test over a definition reads exactly like a test over a behaviour. **Disposition**: proposed
- **What happened**: the defect was reported to the owner in a plan on 2026-09-15 as "E12 and E13 are ready now", derived from epic dependency states without running the transition that would have exposed it. The claim was wrong for the same reason E28 was blocked, and no gate contradicted it because nothing had tried. **Prevented by**: none exists; a readiness claim is currently allowed to be computed rather than demonstrated. **Disposition**: proposed

## Residual risks

- The fix widens which states satisfy a dependency, and widening a gate is the direction that hides work rather than surfacing it. The compensating test refuses an unfinished dependency, but no test asserts that the accepted set is *minimal* - a future status added to `CLOSED_EPIC_STATUS` would silently satisfy every dependency in the backlog.
- Three epics were affected for as long as ADR-0025 has existed and nothing measured it. There may be other literals restating a definition that a constant already owns; only this one was looked for.
- `E12` and `E13` are now able to advance, which is correct, and neither has been exercised beyond `project_os.py check`. The first story either of them opens is the real test of this change.

## Verifier verdict

pending

# Evidence Packet - E25-S10

- Commit/PR: the E25 governance commit on `feat/e24-agent-execution-layer-v2`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR0
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the case has a named state | `done_no_release` in `scripts/project_os.py`'s `VALID_EPIC_STATUS` and in `project/schemas/epic.schema.json`. ADR-0025 defines it: every story complete, nothing in the shipped artifact changed. | PASS |
| AC2 - visible rather than indistinguishable from work in progress | It is a distinct epic status, so `project_os status`, the generated backlog and the metrics report all show it as closed-and-shipped-nothing rather than as `in_progress`. The separate case - all stories done but the release train is busy - stays `in_progress` and ADR-0025 names it. | PASS |
| AC3 - release.py still refuses two in-flight releases | Unchanged; only the epic-closure loop was touched, and the comment records why. `test_done_no_release_does_not_demand_release_evidence` pins that `release.py` looks at `done` and not at `done_no_release`. | PASS |
| AC4 - ADR-0014 amended rather than contradicted | ADR-0025's Supersession section states ADR-0014 is not superseded and that both of its decisions stand. `check_process.check_adrs` accepts the lifecycle. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | A story closing into a state no gate interprets | `test_done_no_release_is_an_epic_status_and_not_a_story_status`. Nothing about a story is shippable on its own, so admitting the value for stories would create a status with no meaning. | PASS |

## Verification Commands

```text
python3 scripts/project_os.py check   -> governance OK: 27 epics, 127 stories
python3 scripts/check_schemas.py check -> schemas OK
python3 scripts/release.py check       -> release OK
python3 -m pytest tests -q             -> 392 passed
```

## Compatibility

- Additive. No existing epic uses the new status, and every existing status keeps its meaning.

## Residual risks

- **The state is available but untested in anger.** No epic has closed as `done_no_release` yet;
  E24 is the first candidate and is currently blocked by the release train instead.
- **Nothing verifies the claim.** An epic can be marked `done_no_release` while having changed the
  shipped artifact, and no gate would notice. The mechanical test would be whether the epic's
  commits touched anything the release archive contains; it is not in this story.
- **Two terminal states are more to hold in mind than one**, and a future reader may close a
  product epic this way to avoid cutting a release. ADR-0025 states the test; nothing enforces it.

## Verifier verdict

pending

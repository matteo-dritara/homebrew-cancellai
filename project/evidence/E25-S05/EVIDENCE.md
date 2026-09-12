# Evidence Packet - E25-S05

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
| AC1 - the achievable level is stated in the standards' vocabulary | `docs/development/AGENT_PROTOCOL.md`, "What \"independent\" can mean here": IEC 61508's person/department/organisation ladder, DO-178C's verifier-is-not-the-author, ISO 26262's I1/I2/I3. Counting parties rather than roles puts this project on the **lowest rung**, and the section says so and says what it is not. | PASS |
| AC2 - a self-review is defined | Same section: it may find, repair and be recorded; it may not close a CR3/CR4 story on its own and may not be the sole basis for a Safety Verdict. | PASS |
| AC3 - the naming rule is a rule | Same section, with the reason: `scripts/process_metrics.py` classifies rounds by filename, so a misnamed record silently inflates the independent-review statistic the rest of the measurement rests on. `E24-VERIFIER-REVIEW.md` was renamed to `E24-SELF-REVIEW.md` when this was found. | PASS |
| AC4 - a self-review states its limitation on its first page | `project/evidence/E24-SELF-REVIEW.md` carries "Reviewer independence - read this before trusting the verdicts" as its first section. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | A self-review counted as independent | `tests/test_process_metrics.py::ReviewRecordClassificationTests` pins the filename distinction. The measurement's headline number depends on it. | PASS |

## Verification Commands

```text
python3 scripts/process_metrics.py check -> the split reports 3 self-review rounds correctly
python3 scripts/check_docs.py check      -> docs OK
python3 -m pytest tests -q               -> 392 passed
```

## Compatibility

- Prose. Read by any agent that reads the protocol.

## Performance / operability

- n/a.

## Documentation updated

- `docs/development/AGENT_PROTOCOL.md`, `AGENTS.md`.

## Residual risks

- **No checker enforces the naming rule.** A self-review committed as `VERIFIER-REVIEW` still
  inflates the statistic; only the prose forbids it. The check is one regex and belongs in
  `check_process.py`; it is not in this story and is recorded here rather than assumed.
- **"May not close a CR3/CR4 story" is not mechanically enforced either.** `project_os.py` does not
  know who wrote a review record.
- **The independence claim is a judgement about model families**, not a measurement. Claude and
  Codex having different failure modes is plausible and untested here.

## Verifier verdict

pending

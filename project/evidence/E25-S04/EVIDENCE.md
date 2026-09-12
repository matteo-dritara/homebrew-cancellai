# Evidence Packet - E25-S04

- Commit/PR: the E25 governance commit on `feat/e24-agent-execution-layer-v2`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR1
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - yield and residual determine another round, recorded in an ADR | `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`, "Review stops on yield": another round while yield is at or above 10%; below that review may close. ADR-0014 is amended rather than superseded, and the ADR says so explicitly. | PASS |
| AC2 - a ceiling close is distinguished from an evidence close | The ADR makes the ceiling a cost control whose use is an owner decision recorded in the review record. `scripts/check_process.py`'s `MAX_REVIEW_ROUNDS` is 3 and its message cites ADR-0025, so the tool no longer contradicts the contract. | PASS |
| AC3 - zero overlap escalates | The ADR: "zero overlap between two rounds' findings - the population is unbounded; escalate to the owner rather than closing, whatever the yield." `scripts/process_metrics.py` reports the overlap and prints UNDEFINED rather than a residual of zero, which is the inversion this rule exists to prevent. | PASS |
| AC4 - the ceiling remains | Three rounds, enforced by `check_process.check_review_rounds`; `test_an_unexcepted_epic_cannot_exceed_the_review_ceiling` still fails an epic past it. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | The tool and the contract disagreeing about the ceiling | `test_review_rounds_are_bounded` asserts `MAX_REVIEW_ROUNDS == 3` with the reason inline, so a future change to one forces a change to the other. | PASS |

## Verification Commands

```text
python3 -m pytest tests -q            -> 392 passed, 146 subtests
python3 scripts/check_process.py check -> process OK (E00 and E07 remain recorded exceptions)
```

Replaying the recorded rounds against the new rule: E00 round 2 (100% yield) requires a third round,
which is what actually happened; E20 round 2 (0% yield) stops, which is what should have happened.

## Compatibility

- No artifact change. `MAX_REVIEW_ROUNDS` rising from 2 to 3 loosens a check; the two recorded
  exceptions (E00, E07) still exceed it and remain warnings.

## Performance / operability

- n/a.

## Documentation updated

- `docs/adrs/0025-...md` (new), `AGENTS.md`, `docs/development/WORK_ITEM_MODEL.md`,
  `scripts/check_process.py`, `tests/test_process.py`.

## Residual risks

- **The 10% threshold is a judgement, not a derivation.** It is recorded in the ADR so it can be
  argued with, and it should be re-fitted once enough rounds exist to fit it rather than assert it.
  Asserting a number and then defending it is how a threshold becomes a target.
- **Nothing enforces the stopping rule yet.** `process_metrics.py` computes the yield; a human
  reads it. An epic could still close at a 40% yield and no gate would fire. Mechanising it needs
  the review record to carry a machine-readable round outcome, which is not in this story.
- **Raising the ceiling costs time.** A third round is now permitted, and an epic that uses it takes
  longer. That is the cost ADR-0014 paid in the other direction, moved to where the data says it belongs.

## Verifier verdict

pending

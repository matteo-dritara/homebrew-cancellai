# Evidence Packet - E25-S08

- Executor: Claude | Independent verifier: self-review | Change Risk: CR0
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PASS, with the enforcement point chosen deliberately and stated below.

## Acceptance Criteria Evidence

**The contract was amended after review**, and the amendment is the honest part of this packet.
AC1 originally required a declared EARS field per criterion, and AC2 required an unclassifiable
criterion to fail. An independent review showed the first was not implemented and the second was
*unimplementable as built*, because a prose classifier falls back to `ubiquitous` and therefore can
never fail to classify. Rather than mark them PASS against an implementation that did not meet
them, both were rejected in the story with the reason - a declared field means rewriting 357
criteria across 27 epics, most in closed contracts whose evidence discharges the text as written -
and replaced by the rule below, which is what the pattern exists to protect.

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - every criterion of every non-cancelled story is classified | `scripts/check_ears.py`; 357 criteria classified across 27 epics. `test_the_five_patterns_are_recognised`. | PASS |
| AC2 - a CR2/CR3/CR4 story with no unwanted-behaviour criterion fails | `ENFORCED_AT = {"CR2","CR3","CR4"}`; `test_a_cr4_story_with_no_unwanted_criterion_is_an_error`, `test_the_same_story_at_cr1_is_not`. The pattern was tightened after review: an incidental "if" in a subordinate clause no longer satisfies it (`test_an_incidental_if_does_not_satisfy_the_rule`), which raised the failing set from 28 to 82. | PASS |
| AC3 - the ratio is reported in PROCESS_METRICS.md | "Requirements shape" section, generated from `check_ears.py ratio` as data rather than by importing across two checkers. | PASS |
| AC4 - the rule binds from authoring time | `planned` stories are judged; only `cancelled` are skipped. `test_a_planned_story_is_judged_too`, `test_a_cancelled_story_is_not_judged`. An earlier version skipped `planned`, so the gate never bound at the one moment a contract can still be written differently. | PASS |
| AC5 - pre-rule contracts are recorded one at a time | `project/ears_baseline.json`, 82 entries, each naming a real story. `test_every_baseline_entry_names_a_real_story`. | PASS |

## What it found

**9 of 357 acceptance criteria describe unwanted behaviour - 3%.** Eighty-two stories at CR2 and
above, on a tool whose defining risk is deleting the wrong thing, have acceptance criteria that
describe only the happy path. Most epics have a ratio of exactly zero.

The number moved from 5% to 3% when the review tightened the pattern, because the looser version
was counting an incidental "if" as a hazard requirement. The worse number is the true one.

Four of the failures were **E25's own stories**, written in the same session as the rule. Their
contracts were repaired - each gained the unwanted-behaviour criterion it always needed, with the
matching evidence row - rather than added to the baseline.

That number is the story's whole point. It is not a formatting observation: at CR3 and above the
change can mutate or holds authority, and a requirement set that never says what must happen when
something is wrong has not specified the part that matters.

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | "When X fails" classified as an event rather than as unwanted behaviour | `test_unwanted_behaviour_wins_over_a_bare_when` - the unwanted pattern is checked first, because an unwanted-behaviour requirement wearing an event keyword is the common case. | PASS |

## Verification Commands

```text
python3 scripts/check_ears.py check  -> 265 criteria classified, 14 unwanted (5%)
python3 scripts/check_ears.py report -> the per-epic ratio
python3 -m pytest tests/test_governance_extras.py -q -> 27 passed
```

## Compatibility

- Stdlib only; reads the control plane.

## Documentation updated

- `AGENTS.md` check list, `.pre-commit-config.yaml`, both workflows, `pyproject.toml`.

## Residual risks

- **The classifier is a heuristic over prose.** "If" inside a subordinate clause counts; a
  criterion that says "the system shall handle errors appropriately" classifies as ubiquitous and
  is worthless. EARS constrains syntax, not semantics, and this constrains it less than EARS does.
- **The 82-entry baseline will not shrink on its own.** Repairing one means adding the
  unwanted-behaviour criterion the contract always needed, which changes a closed contract and
  needs its own story. Nothing schedules that.
- **AC1 and AC2 were rejected and replaced, not met.** The amendment is recorded in the story
  outcome and above. A reader who wants the original mechanism - a declared pattern per criterion,
  machine-checked - should treat this story as having decided against it, with reasons, rather than
  as having delivered it.
- **The classifier still cannot fail to classify.** `ubiquitous` is the catch-all, so the
  distribution is a measurement and never a gate. Only the unwanted-behaviour rule gates.
- **No criterion is linked to the check that discharges it.** The story's deeper goal - machine
  checkability - is not reached; only the distribution is measured.

## Verifier verdict

See `project/evidence/E25-E26-SELF-REVIEW-ROUND2.md`.

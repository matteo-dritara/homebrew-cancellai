# Evidence Packet - E35-S01

- Commit/PR: the E35-S01 commit on `main`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR2
- Spec version/commit: `project/epics/E35.json` at this commit

## Outcome

PASS

## Why this story exists

E06 review round 10 (`project/evidence/E06-S14/ROUND10_FINDINGS.md`) showed that a Safety Verdict
`## Round 10 ... FAIL` followed by `## Owner note ... PASS` read as passing, because the gate took the
last verdict-shaped line in the whole file (E32-S01's rule). E06-S15 repaired `release.py`'s own
reader and recorded the control-plane gate - which every CR4 closure goes through - as out of scope.
This story repairs that gate.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - with `## Round <n>` headings, the final round decides | `project_os.final_round_text`: the last round's body plus the template's `## Verdict` section that follows it; the last standalone verdict line there decides. `test_the_final_round_decides_and_a_later_section_cannot` | PASS |
| AC2 - a verdict-shaped line in a later section refuses | Same function returns None; the owner-note counterexample and a later `REJECT` after a passing `## Verdict` both refuse | PASS |
| AC3 - no round headings: E32-S01's rule; every committed verdict judged as before | Files without round headings take the unchanged path; E32-S01's tests pass unchanged. All 50 committed `*VERDICT*.md` files were judged before and after the change: identical (40 pass, 10 do not). A first draft refused E14-S04's and E14-S05's closed verdicts, whose decision sits in a `## Verdict` section after the round heading; `test_every_committed_safety_verdict_keeps_its_judgement` pins both | PASS |

Mutation checks, each killed: letting a later section's verdict pass silently (2 tests fail); not
admitting the `## Verdict` section (7 tests fail).

`scripts/release.py`'s reader (E06-S15) now uses the same rule, so the release and the control
plane cannot disagree on the same file.

## Verification Commands

```text
python3 -m pytest tests/test_project_os.py tests/test_release.py -q   -> 98 passed
```

## Residual risks

- **A file without round headings keeps last-line-wins.** A single-round verdict followed by an
  "owner note" with a verdict-shaped line is still decided by that line, as before; every
  multi-round verdict written through the harness uses round headings.
- **The two readers are two implementations of one rule**, kept in step by tests on both; a later
  change to one must change the other.

## Verifier verdict

pending

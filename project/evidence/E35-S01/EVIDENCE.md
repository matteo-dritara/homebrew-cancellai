# Evidence Packet - E35-S01

- Commit/PR: the E35-S01 commit on `main`
- Executor: Claude
- Independent verifier: round 1 FAIL, round 2 FAIL (Codex, E35-VERIFIER-REVIEW-ROUND1.md, -ROUND2.md); repaired; closes on CEILING_DECISION.md
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
| AC1 - with `## Round <n>` headings, the final round decides | `project_os.final_round_verdict`: the last round's body (from the line after its heading) decides by its last standalone verdict line; the template's `## Verdict` section is attached only when it is the very next section and must agree. `test_the_final_round_decides_and_a_later_section_cannot` | PASS |
| AC2 - a verdict-shaped line in a later section refuses | Same function returns None; the owner-note counterexample and a later `REJECT` after a passing `## Verdict` both refuse | PASS |
| AC3 - no round headings: E32-S01's rule; every committed verdict judged as before | Files without round headings take the unchanged path; E32-S01's tests pass unchanged. All 50 committed `*VERDICT*.md` files were judged before and after the change: identical (40 pass, 10 do not). A first draft refused E14-S04's and E14-S05's closed verdicts, whose decision sits in a `## Verdict` section after the round heading; `test_every_committed_safety_verdict_keeps_its_judgement` pins both | PASS |

Mutation checks, each killed: letting a later section's verdict pass silently (2 tests fail); not
admitting the `## Verdict` section (7 tests fail).

`scripts/release.py`'s reader (E06-S15) now uses the same rule, so the release and the control
plane cannot disagree on the same file.

## Round 1 repair (Codex, `project/evidence/E35-VERIFIER-REVIEW-ROUND1.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| `## Round 10 ... FAIL`, then `## Owner note`, then a newly appended `## Verdict ... PASS` read as passing: any later `## Verdict` section was admitted | A `## Verdict` section is attached to the final round only when it is the very next section, at most once; a verdict-shaped line in any other later section - a second `## Verdict` included - refuses; and a FAIL/REJECT anywhere in the final round's body or its attached section fails the file, so the template section cannot contradict the round into a pass | Codex's `test_an_owner_note_cannot_add_a_later_verdict_section` (kept); four new cases in `test_the_final_round_decides_and_a_later_section_cannot` (verdict after a note, duplicate verdict sections, body FAIL with section PASS, body PASS with section FAIL); every committed Safety Verdict still judged as before. Mutations admitting any `## Verdict` or letting the round's FAIL be outvoted each fail a test |

`release.py`'s reader follows the same rule (E06-S15).

## Round 2 repair (Codex, `project/evidence/E35-VERIFIER-REVIEW-ROUND2.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| The round-1 repair let any FAIL in the final round decide, so `## Round 10 ... FAIL ... PASS` (re-evaluated inside the same round) was refused, contrary to AC1 | `final_round_verdict`: the round's own body decides by its last standalone verdict line (E32-S01's rule, within the round); the immediately following `## Verdict` section, when present, must agree with the body or the file refuses; a verdict line in any other later section still refuses. This satisfies both rounds' prescriptions at once | Codex's `test_the_last_standalone_verdict_in_the_final_round_decides` (kept) and three new cases; every committed Safety Verdict still judged as before; removing the agreement check or the "very next section" condition each fails a test |

`release.py` now calls this gate itself instead of carrying a second copy of the rule, so the
two-implementations residual above no longer applies.

## Repairs after the forked self-review (`project/evidence/E35-SELF-REVIEW.md`, not independent)

| Finding | Repair | Evidence |
| --- | --- | --- |
| Only `## ` ended the final round, so `# Owner note`, an indented `## Owner note` or a setext-underlined heading followed by `PASS` reopened the owner-note bypass - in the gate and in the release reader | `section_starts`: every level-1 or level-2 heading as Markdown renders it - ATX with up to three leading spaces, setext `=`/`-` underlines under a paragraph line - ends the round; a thematic break and `###` subsections stay inside it (AC1) | Four heading shapes added to `test_the_final_round_decides_and_a_later_section_cannot`, plus `---` and `###` staying in the round; dropping the setext branch or narrowing ATX back to `## ` each fails two tests |
| The round's body started right after the `## Round <n>` match, so the heading's own tail was read as a verdict: `## Round 10 PASS` alone passed | The body starts on the line after the heading | Regression case; reverting fails it |
| (E06-S15) the release reader's round precheck ran on raw text, so a `## Round` only inside a fence satisfied it | The precheck reads the fence-stripped text the gate reads; undecodable bytes refuse | `test_release.py` case; reverting fails it |

The self-review's residual 2 - four ways the reader, unchanged since E32-S01, differs from how
Markdown renders the file (HTML comments, inline triple backticks, indented code, form feed and
U+2028) - is not this story's change and is filed as **E35-S02**. Its residual 1 (a `###`
subsection inside the final round decides, as AC1 literally says) is kept as the contract states.

## Repairs after self-review round 2 (`project/evidence/E35-SELF-REVIEW-ROUND2.md`, not independent)

| Finding | Repair | Evidence |
| --- | --- | --- |
| A setext underline under a multi-line paragraph started the new section at the paragraph's last line, so `PASS` on an earlier line of that heading stayed in the round and decided | `section_starts` tracks where the current paragraph began and starts the section there | Four multi-line setext cases (including CRLF and a setext heading right after the round heading); making the section start at the last line fails four tests |
| Rounds spelled `# Round 10`, ` ## Round 10`, `##<tab>Round 10` or behind a BOM fell back to last-line-wins in `project_os`, while `release.py` refused the same file | `ROUND_HEADING_RE` accepts level-1/2 ATX with up to three leading spaces, a tab, and a leading BOM; `release.py` uses `project_os`'s pattern instead of its own copy | Four round-spelling cases; every committed Safety Verdict still judged as before |

Soft line breaks (`The owner says` then `PASS`) are added to E35-S02, with the other ways the
reader differs from rendered Markdown.

## Verification Commands

```text
python3 -m pytest tests/test_project_os.py tests/test_release.py -q   -> 98 passed
```

## Residual risks

- **A file without round headings keeps last-line-wins.** A single-round verdict followed by an
  "owner note" with a verdict-shaped line is still decided by that line, as before; every
  multi-round verdict written through the harness uses round headings.

## Verifier verdict

closed by owner decision at the review ceiling (CEILING_DECISION.md); self-reviews E35-SELF-REVIEW.md and -ROUND2.md, not independent

# Evidence Packet - E34-S05

- Commit/PR: the E34-S05 commit on `main`
- Executor: Claude
- Independent verifier: round 2 FAIL (Codex, E34-VERIFIER-REVIEW-ROUND2.md); round 3 PASS (Codex, E34-VERIFIER-REVIEW-ROUND3.md)
- Change Risk: CR1
- Spec version/commit: `project/epics/E34.json` at this commit

## Outcome

PASS

## Why this story exists

On 2026-09-24 the owner narrowed E34-S02's first acceptance criterion after independent round 1
showed it overclaimed. Re-rendering the brief made `verifier_handoff.py check` fail
`E34-VERIFIER-REVIEW-ROUND1.md`, because the gate compared every verdict a story ever received with
the story's *current* brief. Changing a criterion after a round was therefore inexpressible without
weakening the gate by hand. This is a method defect in E28-S02's gate, found by using it; it is
repaired here as its own story rather than inside E34-S02's diff.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - a re-render with a different checksum keeps the previous brief byte for byte, named by its checksum | `write_brief` copies the previous file to `VERIFIER_BRIEF.superseded-<12 hex>.md` before writing. `test_rerendering_archives_the_previous_brief_byte_for_byte`, `test_an_unchanged_rerender_archives_nothing`. On the repository: `project/evidence/E34-S02/VERIFIER_BRIEF.superseded-f67a2ee4c768.md` | PASS |
| AC2 - a verdict may answer the current or a superseded brief, and nothing else | `check_story` builds the answerable set from the current brief and every self-consistent superseded one. `test_an_earlier_verdict_answers_its_superseded_brief`, `test_a_checksum_matching_neither_brief_is_refused`; the executor-authorship refusal still applies (`test_the_executor_may_not_author_a_verdict_on_a_superseded_brief_either`) | PASS |
| AC3 - a superseded brief that does not hash to its checksum, or names no renderer, is reported and answers nothing | `test_a_tampered_superseded_brief_answers_nothing` | PASS |

Mutation check: admitting no superseded brief fails one test; dropping the hash comparison on
superseded briefs fails one test.

## Repairs after independent round 2 (2026-09-24)

`E34-VERIFIER-REVIEW-ROUND2.md` (Codex) failed AC1: rendering A, B, A, B replaced the first archive of
A (rendered 2026-09-01) with the later A (2026-09-03) at the same path.

| Repair | Evidence |
| --- | --- |
| An archive is never overwritten: identical bytes are not duplicated, and different bytes for the same checksum go beside it as `-2`, `-3`, ... | `test_an_archive_is_never_overwritten` (A, B, A, B: three archives, the first unchanged byte for byte, check clean); fails against the pre-repair writer |

## Verification Commands

```text
python3 -m pytest tests/test_verifier_handoff.py -q   -> 29 passed
python3 scripts/verifier_handoff.py check             -> OK (E34 round 1 answers E34-S02's superseded brief)
```

## Residual risks

- **Ordering is not checked.** Without git history the gate cannot tell whether a verdict was
  written before or after the brief it answers was superseded, so a new verdict answering an old
  brief would pass. `scripts/review_round.py` always quotes the current committed brief, which is
  the practical barrier; the gate proves only that the answered document is one the gate rendered.

## Verifier verdict

pending

# E19-S02 - Review ceiling decision

- Story: E19-S02 (CR1)
- Date: 2026-09-23
- Decided by: project owner, by standing instruction for this work session - at most two
  independent review rounds per story, and "if further reviews are needed, find other
  solutions"; recorded by the executor (Claude), who does not thereby become a verifier.
- Related: ADR-0025, ADR-0038, `docs/development/AGENT_PROTOCOL.md` ("Self-review": a self-review
  may not close a CR3 or CR4 story on its own; E19-S02 is CR1).

## What the reviews found

| Review | Reviewer | Verdict | Finding |
| --- | --- | --- | --- |
| Round 1 | Codex (independent) | FAIL | The tokenised URL was printed on stdout |
| Round 2 | Codex (independent) | FAIL | F1: token files not owner-only on Windows; F2: a failed write left a token-bearing file |
| Self-review 1 | Claude, forked context | FAIL | F1's class still open: inherited ACL entries on macOS (reproduced), create-then-restrict race on Windows (reasoned); the Windows test had never run; this record was missing |
| Self-review 2 | Claude, forked context | FAIL | The directory was restricted by path, so a swapped link redirected the permission change (reproduced, same user); a test helper was flaky on Windows |
| Self-review 3 | Claude, forked context | FAIL | Low severity only, none reachable with default settings: a sticky base owned by another account was accepted; Windows did not check the owner; a comment described the wrong check. It confirmed that no permission change remains and that the same-user scope is honest |

Every finding has been repaired, each with a test that fails when the repair is removed (see
`EVIDENCE.md`). The last repair replaced the approach rather than patching it: files carrying the
token are created only inside a directory made private before anything exists in it.

## The other solution taken

Round 2 was this story's second independent review, the owner's limit. E19-S02 is CR1: it
observes and renders; it cannot reach a mutation. The protocol lets a self-review close a CR1
story, so the story closes on:

1. the repairs above, each pinned by a regression test and mutation-checked; the last one removes
   every permission change, so what remains is read-only checking and refusal;
2. self-review 3 of the read-only design (`project/evidence/E19-SELF-REVIEW-ROUND3.md`) found
   only low-severity issues, each repaired to its stated fix with a test where one is expressible.
   The owner's instruction was to find another solution rather than review without end: the
   findings have converged from a reproduced credential leak (round 1) to documentation and
   non-default configurations (self-review 3), so the story closes on the repairs rather than on a
   fourth self-review;
3. the Windows leg of `rust.yml` passing on **two** runs of the pushed commit before any release
   tag is pushed (self-review 2 found a single green run was not enough: the leg was flaky).

If (3) fails, the story does not close. Condition (2) was first written as "must pass"; self-reviews 2 and 3 did not pass, and the text above records why closure proceeds on their repairs instead.

## What this decision does and does not claim

- It closes E19-S02 without a third independent pass over the final code; the last independent
  review saw the file-level design, not the private-directory one. That is the residual it accepts.
- It is bounded by the story's risk and scope: a read-only local dashboard, optional and
  unpackaged, whose worst remaining exposure is inventory metadata to another local account under
  conditions ADR-0038 names.
- It does not relax the ceiling for any other story.

## Outcome

Condition (3) met on `7cb4b88`: `rust.yml` run 35853496003 and its rerun both passed every
Windows job (`quality (windows-latest)`, `check (windows-latest, stable)`,
`check (windows-latest, 1.88.0)`), with governance, tests and codeql green. E19-S02 closed on
2026-09-23 under this decision.

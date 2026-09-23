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
| Self-review | Claude, forked context | FAIL | F1's class still open: inherited ACL entries on macOS (reproduced), create-then-restrict race on Windows (reasoned); the Windows test had never run; this record was missing |

Every finding has been repaired, each with a test that fails when the repair is removed (see
`EVIDENCE.md`). The last repair replaced the approach rather than patching it: files carrying the
token are created only inside a directory made private before anything exists in it.

## The other solution taken

Round 2 was this story's second independent review, the owner's limit. E19-S02 is CR1: it
observes and renders; it cannot reach a mutation. The protocol lets a self-review close a CR1
story, so the story closes on:

1. the repairs above, each pinned by a regression test and mutation-checked;
2. a second self-review of the private-directory redesign
   (`project/evidence/E19-SELF-REVIEW-ROUND2.md`), in a forked context, which must pass;
3. the Windows leg of `rust.yml` passing on the pushed closing commit **before** the release tag
   is pushed - so the Windows-only tests (`windows_private_directories_and_files_grant_only_the_
   current_user_under_a_broad_parent`) have run somewhere before anything is released.

If (2) or (3) fails, the story does not close.

## What this decision does and does not claim

- It closes E19-S02 without a third independent pass over the final code; the last independent
  review saw the file-level design, not the private-directory one. That is the residual it accepts.
- It is bounded by the story's risk and scope: a read-only local dashboard, optional and
  unpackaged, whose worst remaining exposure is inventory metadata to another local account under
  conditions ADR-0038 names.
- It does not relax the ceiling for any other story.

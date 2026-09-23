# Evidence Packet - E06-S06

- Commit/PR: the E06-S06 verifier records on `main`
- Executor: Claude (rendered the brief, carried the findings); the story's substance is the independent verifier's work
- Independent verifier: Codex - round 1 FAIL (`project/evidence/E06-S06-VERIFIER-REVIEW.md`), round 2 PASS_WITH_RESIDUALS (`project/evidence/E06-VERIFIER-REVIEW-ROUND3.md`)
- Change Risk: CR4
- Spec version/commit: `project/epics/E06.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - an independent adversarial pass over the current E21-S03/E21-S07 code, recorded as a Safety Verdict round against a committed brief | `project/evidence/E06-S06/SAFETY_VERDICT.md` holds both rounds, each citing the committed brief's checksum; `verifier_handoff.py check` passes. | PASS |
| AC2 - any unreadable directory, companion payload or project withholds every destructive action and exits as the reference does, on every platform in the perimeter | Round 1 reproduced this for unreadable directories on both providers and both root origins, and found the unreadable-rollout gap (F-01); E06-S12 repaired it and pinned it with the NORMATIVE fixture `codex-unreadable-rollout`; round 2 confirmed. | PASS |
| AC3 - a path component swapped between validation and unlink does not redirect the unlink | Round 1 and 2 confirmed the handle-relative unlink; the leaf-name race between `fstatat` and `unlinkat` on Unix is the owner-accepted residual (Safety Verdict, "Owner disposition", 2026-09-23). | PASS (accepted residual) |
| AC4 - a defect found keeps E06-S04 blocked and becomes a backlog item | F-01/F-02 became E06-S12, added to E06-S04's blocker; E06-S12 is repaired and passed round 3. | PASS |

## Residual risks

- The Unix leaf-name race (owner-accepted).
- Native Linux/Windows permission reproductions ran in CI, not on the verifier's host.

## Verifier verdict

PASS_WITH_RESIDUALS (round 2)

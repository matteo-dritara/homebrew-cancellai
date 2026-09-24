# E35-S01 - Review ceiling decision

- Story: E35-S01 (CR2)
- Date: 2026-09-24
- Decided by: project owner, by standing instruction - at most two independent review rounds per
  story; recorded by the executor (Claude), who does not thereby become a verifier.
- Related: ADR-0025, `E35-VERIFIER-REVIEW-ROUND1.md`, `E35-VERIFIER-REVIEW-ROUND2.md`.

## What the rounds found

| Round | Verifier | Verdict | Finding |
| --- | --- | --- | --- |
| 1 | Codex | FAIL | A `## Verdict` section appended after an owner note overrode the final round's FAIL. Required: attach a `## Verdict` only when it immediately follows the final round, at most once; a final-round FAIL must not be overridable by a later passing section. |
| 2 | Codex | FAIL | The round-1 repair let any FAIL in the round decide, so a FAIL re-evaluated to PASS inside the same round was refused, contrary to AC1 ("the last standalone verdict line inside the final round's section"). |

## The other solution taken

The two prescriptions are reconciled rather than traded: the round's body decides by its last
verdict line (AC1, round 2), and the template section attached right after it must agree with the
body or the file refuses (round 1). Both rounds' counterexamples are kept as regression tests, the
repair was mutation-checked, and every committed Safety Verdict is judged as before.

The story is CR2: it closes on this decision plus a forked self-review, not a third independent
round.

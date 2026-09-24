# E34 Closure - owner-directed at the review cost ceiling, with residuals accepted

- Epic: E34 - Reviewer Pool and Advisory Pre-Review
- Decided by: **project owner**, 2026-09-24 ("Chiudi con residui"), after independent round 3
- Closes as: `done_no_release` - every change is development tooling (`scripts/`, `.opencode/`,
  tests, process documents); nothing in the shipped artifact changed (ADR-0025)

## Review history

| Round | Verifier | S01 | S02 | S03 | S04 | S05 | S06 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 (`E34-VERIFIER-REVIEW-ROUND1.md`) | Codex | FAIL | FAIL | PASS | FAIL | - | - |
| 2 (`E34-VERIFIER-REVIEW-ROUND2.md`) | Codex | FAIL | FAIL | PASS | FAIL | FAIL | - |
| Self-review (`E34-SELF-REVIEW.md`) | Claude, not independent | FAIL | FAIL | - | FAIL | PASS_WITH_RESIDUALS | - |
| 3 (`E34-VERIFIER-REVIEW-ROUND3.md`) | Codex | FAIL | FAIL | PASS | PASS | PASS | - |
| Self-review 2 (`E34-SELF-REVIEW-ROUND2.md`) | Claude, not independent | PASS_WITH_RESIDUALS | PASS_WITH_RESIDUALS | - | - | - | PASS |

Round 3 is ADR-0025's cost ceiling and rejected 40% of what it judged, so closure is the owner's
decision, recorded here rather than implied by a status. S03, S04 and S05 close on an independent
PASS. S01 and S02 close on repairs made after round 3 to its exact findings, pinned by regression
tests and checked by a forked self-review; S06 - the OS sandbox the owner chose on 2026-09-24 when
the first self-review showed OpenCode ran unconfined - closes on the self-review alone.

**Honest assurance level for S01, S02 and S06: no independent confirmation of the final repairs.**
Each was mutation-checked (reverting it fails its regression), and the evidence packets and
`project/evidence/E34-S0{1,2,4}/CEILING_DECISION.md` record the history.

## Residual risks the owner accepts

- A reviewer child that calls `setsid` survives the process-group kill. It stays inside the
  sandbox, and any worktree byte it changes after the check fails `import`'s re-digest.
- The command allow-list is defence in depth: a reviewer-written test run by `pytest` is arbitrary
  code, bounded by the sandbox (writes) but not by it on the network, which stays open for the
  model provider.
- The sandbox is macOS-only; elsewhere an OpenCode review is refused. The sandbox tests skip inside
  another sandbox, so Codex cannot run them.
- The free OpenCode model is often overloaded (503): three runs on 2026-09-24 ended without output.
  That is availability, not safety; the harness fails such a run closed.
- The handoff gate accepts a verdict answering any self-consistent superseded brief, regardless
  of when it was written (E34-S05).

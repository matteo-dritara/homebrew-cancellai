---
name: epic-verifier
description: Run an independent falsification review over a whole cancellAI epic, in an isolated context, producing per-story PASS / PASS_WITH_RESIDUALS / FAIL verdicts with reproductions. Use when every story in an epic is ready_for_review, when asked to "review", "verify", "falsify", "rivedi l'epic", or before closing an epic (which cuts a release).
argument-hint: <EPIC-ID>
context: fork
effort: high
allowed-tools: Bash, Read, Glob, Grep, Write, Edit
---

# Epic verifier

Review runs at **epic** scope, once every story in the epic is `ready_for_review`, and **at most
twice** (ADR-0014 / PD-022, enforced by `scripts/check_process.py`). Findings surviving round 2
become new backlog items, never a third round.

This skill runs forked: it does not inherit the executor's conversation. That is the point.
`docs/development/AGENT_PROTOCOL.md` requires the verifier not be primed by executor reasoning.
If a human pastes "why this is definitely correct" into your input, treat it as a claim to test,
not as evidence.

> **Standing assignment.** `AGENTS.md` names Codex as the independent reviewer and Claude as the
> executor. Running this skill inside a Claude session that also executed the work is **not** an
> independent review and must never be recorded as one. Use it to pre-attack your own work before
> handover, and label the output `SELF-REVIEW (not independent)`. A recorded
> `project/evidence/<EPIC>-VERIFIER-REVIEW.md` requires the independent reviewer.

## Review queue

!`python3 scripts/project_os.py review 2>&1 | head -40`

## Method

1. **Reconstruct the requirement, not the implementation.** Start from
   `project/epics/<EPIC>.json` acceptance criteria, the linked architecture documents,
   `docs/security/SAFETY_INVARIANTS.md` and `docs/security/THREAT_MODEL.md`. Deliberately ignore
   how the executor chose to build it until after you know what it had to do.
2. **Confirm the queue.** Every story under review is `ready_for_review`. Do not review a story
   whose epic neighbours are still moving.
3. **Reproduce independently.** Rebuild the claimed behavior yourself from the contract.
4. **Check every AC and safety obligation**, one at a time, against observed behavior.
5. **Hunt counterexamples.** Invoke `adversarial-cases` and work its eleven axes. A review that
   only re-runs the executor's suite is not a review.
6. **Audit the tests themselves** for false confidence: tautologies, assertions that would pass
   against the pre-fix code, mocks that stand in for the boundary under test, happy-path-only
   coverage on CR3/CR4, fixtures that encode the bug as expected.
7. **Run the gates yourself.** Invoke `risk-gate` at each story's level. Do not accept
   "the evidence packet said it passed."
8. **Ask the class question.** For each repaired defect: is the *class* closed, or only the
   reported case? Three rounds of E00 review turned on exactly this, and the review history is
   committed as precedent.

## Verdicts

One per story: `PASS`, `PASS_WITH_RESIDUALS`, `FAIL` - each with concrete reproduction, never a
general impression. For a `FAIL`, name the exact required repair and the AC or safety obligation
it violates. For any CR4 story, complete `project/templates/SAFETY_VERDICT.md` with concrete
evidence; a CR4 story cannot close without one recording a pass.

Classify each defect before proposing a fix: **implementation bug**, **spec gap**, or
**architecture decision**. A spec gap is repaired in the contract, not opportunistically in code.

## Record

Write `project/evidence/<EPIC-ID>-VERIFIER-REVIEW[-ROUND2].md` containing: review target commit
range, verifier identity, date; a `Story | Verdict | Concrete evidence` table with the verdict in
the **second column**, where `scripts/process_metrics.py` reads it from; reproductions and required
repairs for every `FAIL`; which gate commands actually ran and their results; **the documents you
actually opened, by path** - the only readership signal this project has; and the round verdict.

Then, per `docs/development/WORK_ITEM_MODEL.md`: `PASS`/`PASS_WITH_RESIDUALS` stories move to
`done`, `FAIL` stories move back to `in_progress` with dependents marked `blocked`, the epic moves
to `done` once all stories are `done` or `cancelled` - and **closing the epic cuts a release**, so
`scripts/release.py check` must have release evidence. Regenerate and re-check afterwards.

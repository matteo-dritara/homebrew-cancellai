---
name: story-executor
description: Execute one cancellAI story end to end as the executor role - plan verification before code, smallest coherent change, tests and docs in the same change, evidence packet, exit at ready_for_review. Use when implementing a story ID (E##-S##), when asked to "implement", "fix", "build" something in this repository, or when the user names a story.
argument-hint: <STORY-ID>
allowed-tools: Bash, Read, Edit, Write, Glob, Grep, Skill
---

# Story executor

The executor implements exactly one story and stops at `ready_for_review`. This is the
`docs/development/AGENT_PROTOCOL.md` procedure made executable. The protocol document remains the
contract; this skill is a runner over it, not a second copy of it.

## Hard boundaries

These are not negotiable by a story, a shortcut, or a user request phrased as urgency:

- **Never** set your own story to `verification` or `done`. Never write your own CR4 Safety Verdict.
- **Never** hand-edit `docs/DECISION_REGISTER.md`, `docs/ROADMAP.md`, `docs/BACKLOG.md`, or
  `project/generated/PROJECT_STATUS.md`. Edit `project/*.json` and run
  `python3 scripts/project_os.py generate`.
- **Never** widen scope. A defect you find outside this story becomes a backlog item, not a
  silent fix. A refactor for cleanliness alone is out of scope by itself.
- **Never** touch `cancellai.py` outside the four accepted categories in `AGENTS.md`
  ("Python reference freeze"). New product capability targets Rust.
- A conflict between the story and `docs/CONSTITUTION.md` / `docs/security/SAFETY_INVARIANTS.md`
  means the **story** is wrong. Escalate for an ADR/RFC/owner decision; do not implement the conflict.

## Procedure

### 1. Orient and contract

```sh
python3 scripts/project_os.py check
python3 scripts/project_os.py brief $1 --role executor
```

Read the generated brief, then read every architecture/security document the story links and the
Safety Invariants it names. The brief is an input packet, not a replacement for them.

Restate before coding: **outcome, acceptance criteria, Change Risk Level, safety obligations,
documentation impact.** If any of the five is unclear, the story is not Ready - say so.

### 2. Plan verification before code

Write, in the work log, what would **falsify** this implementation before you write it. For CR2+
invoke the `adversarial-cases` skill and fold its counterexample axes into the plan. A verification
plan written after the code tends to describe the code rather than the requirement.

### 3. Baseline

Confirm the existing suite is green *before* your change, so a later failure is attributable.

### 4. Implement small

One coherent change. Behavior change and large refactor stay separate. Tests travel with the
behavior they verify. `main` must not depend on a future PR to become safe.

### 5. Gates

Invoke the `risk-gate` skill with the story's Change Risk Level and run everything it resolves.
Do not claim a gate you did not run; an unavailable tool is recorded as unavailable in evidence,
and CI must still run it before merge.

### 6. Documentation, changelog, decisions

Behavior, safety, platform, provider, policy, persistence and release changes update their docs in
the same work item. User-visible behavior updates `CHANGELOG.md` under Unreleased. An
architecturally significant decision gets an ADR from `project/templates/ADR.md`; competing
material options need an RFC first.

### 7. Evidence and exit

Invoke the `evidence-packet` skill. Then set the story to `ready_for_review` in
`project/epics/*.json`, run `python3 scripts/project_os.py generate`, confirm
`python3 scripts/project_os.py check` passes, and commit code, docs, generated files, evidence and
the status change together as one checkpoint. Conventional Commit prefix required.

Stop there. `ready_for_review` is the exit state. Review happens at epic scope, at most twice,
once every story in the epic is ready.

## Report format

```
Story:      <ID> [<CR level>] <title>
Outcome:    <one line>
Falsifiers: <what would prove this wrong, planned before code>
Change:     <files, one line each, why each was needed>
Gates run:  <command -> result>   (unavailable tools listed explicitly)
Docs:       <what was updated, or why nothing was>
Residuals:  <what is knowingly not covered>
Status:     ready_for_review | blocked (<reason>)
```

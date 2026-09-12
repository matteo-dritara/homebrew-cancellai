---
name: orient
description: Orient a session inside the cancellAI control plane before any work. Loads live project state (check/status/next/review), the working-tree delta, and the reading order AGENTS.md mandates. Use at the start of any session, when asked "what should I work on", "where are we", "cosa c'è da fare", or before touching code in this repository.
allowed-tools: Bash(python3 scripts/project_os.py:*), Bash(git status:*), Bash(git log:*), Read, Glob, Grep
---

# Orient

`AGENTS.md` forbids starting implementation from chat context. This skill replaces chat context
with repository state. It reads; it never changes project state.

## Live control-plane state

Integrity of the machine-readable plane:

!`python3 scripts/project_os.py check 2>&1 | tail -20`

Phase and story counts:

!`python3 scripts/project_os.py status 2>&1`

What is ready to start:

!`python3 scripts/project_os.py next 2>&1 | head -30`

What is waiting on the independent reviewer:

!`python3 scripts/project_os.py review 2>&1 | head -30`

Working-tree delta (uncommitted work is in-flight story work until proven otherwise):

!`git status --short 2>&1 | head -30`

Recent checkpoints:

!`git log --oneline -8 2>&1`

## What to do with this

1. **Read before concluding.** The state above is an index, not the contract. The contract is
   `docs/INDEX.md` -> `docs/CONSTITUTION.md` -> the story in `project/epics/*.json` -> the
   architecture/security documents that story links -> `docs/development/ENGINEERING_SYSTEM.md`
   -> `docs/development/AGENT_PROTOCOL.md`. Read them in that order for the story actually selected.
2. **Reconcile the working tree.** Untracked or modified files that belong to no `in_progress`
   story are an anomaly - say so rather than building on top of them.
3. **Name the story ID.** Every change identifies one. If the requested work has no story, decide
   whether it is a defect inside an existing story or needs a control-plane entry first, and say
   which. Do not create product scope in code.
4. **Generate the brief, do not paraphrase it:**
   `python3 scripts/project_os.py brief <STORY-ID> --role executor`
5. **Then hand off** to `story-executor` (implementation) or `epic-verifier` (review).

## Reporting

Answer with: current phase, the selected story ID and its Change Risk Level, the gates that level
requires, what is in the working tree, and the single next action. No more.

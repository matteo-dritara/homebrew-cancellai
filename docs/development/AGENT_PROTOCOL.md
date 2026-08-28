# Claude / Codex Agent Protocol

The user intends to develop cancellAI with one coding agent as executor and another as independent verifier. This protocol makes that useful rather than ceremonial.

## Roles

### Owner / Orchestrator

Owns product decisions, scope, risk acceptance, roadmap priority, and CR4 Safety Verdict acceptance. The owner does not hand destructive authority decisions to an agent.

### Executor

Implements exactly the selected story/spec, creates/updates tests and documentation, and produces an implementation evidence summary.

### Independent Verifier

Attempts to falsify the implementation from the story, acceptance criteria, invariants, threat cases, and code. It should not be primed by the executor's chain of reasoning.

Roles may rotate between Claude and Codex across stories/sprints to reduce systematic bias.

**Current standing assignment:** Claude is the executor and hands work over at
`ready_for_review`; Codex performs the independent review, once the whole epic is ready and
at most twice per epic.

## Context isolation

Verifier input should include:

- work item ID and generated story contract;
- relevant architecture documents;
- relevant Safety Invariants/Threat Model sections;
- the final code diff/branch;
- commands needed to run the suite.

Verifier input should not include:

- executor private reasoning;
- "why the implementation is definitely correct" narratives;
- pressure to confirm expected success.

The verifier may read executor-authored tests, but must design independent counterexamples rather than treating those tests as proof.

## Generated role brief

Start a handoff from repository state, not copied chat prose:

```sh
python3 scripts/project_os.py brief E00-S01 --role executor
python3 scripts/project_os.py brief E00-S01 --role verifier
```

The brief is generated from the story contract and canonical Safety Invariants. It is an input packet, not a replacement for reading the linked architecture/threat documents.

## Executor procedure

1. Run `python3 scripts/project_os.py check` and `next/status`, then generate the executor brief for the selected story.
2. Read `AGENTS.md`, the story, dependencies, linked docs, invariants, and threat cases.
3. Confirm repository baseline tests are green.
4. Write a short implementation plan in the PR/work log, including verification plan before code.
5. Make the smallest coherent change.
6. Add/update tests in the same change.
7. Update docs/changelog/ADR/RFC as required.
8. Run all required local gates for the CR level.
9. Produce evidence using `project/templates/EVIDENCE_PACKET.md` and commit it under `project/evidence/`.
10. Set the story status to `ready_for_review` in `project/epics/*.json`, regenerate, and hand off the final diff + spec to the verifier.

An executor's work is finished at `ready_for_review`. It does not set `verification` or `done` for its own change, and it does not write its own Safety Verdict. `python3 scripts/project_os.py check` refuses a `ready_for_review` story that has no committed executor evidence.

## Verifier procedure

Review runs at **epic** scope, when every story in the epic is `ready_for_review`, and at
most **twice** per epic (ADR-0014 / PD-022). Do not review a story in isolation while its
neighbours are still moving, and do not open a third round: findings that survive round 2
become new backlog work items recorded as accepted residual risk.

Pick up work from the review queue rather than from chat context:

```sh
python3 scripts/project_os.py review
python3 scripts/project_os.py brief <STORY-ID> --role verifier
```

Move the epic's stories to `verification` while reviewing, then to `done` on a passing
verdict with evidence committed, or back to `in_progress` on a rejection. Closing the epic
cuts a release; see [WORK_ITEM_MODEL.md](WORK_ITEM_MODEL.md).

1. Ignore the executor's intended mechanism; start from required behavior.
2. Reproduce the baseline/claimed fix independently.
3. Check every AC and safety obligation.
4. Search for counterexamples in:
   - path/identity changes;
   - partial reads/permissions;
   - links/mounts/reparse points;
   - provider version/layout drift;
   - concurrency;
   - crash/failure/retry;
   - boundary values;
   - policy/trust conflicts;
   - platform differences;
   - malformed/untrusted input;
   - performance/large datasets.
5. Inspect tests for false confidence, tautology, and gaps.
6. Run or add adversarial tests where necessary.
7. Issue one verdict: `PASS`, `PASS_WITH_RESIDUALS`, or `FAIL`.
8. For CR4, populate the Safety Verdict template with concrete evidence.

## Failure cycle

A verifier defect is not patched opportunistically without updating the relevant contract when it exposes a missing requirement.

```text
DEFECT
  -> classify: implementation bug | spec gap | architecture decision
  -> fix/update contract
  -> executor reruns gates
  -> verifier reruns affected + regression suite
```

No "verifier says okay after a quick glance" shortcut.

## Prompt templates

Canonical prompts live in:

- `project/templates/EXECUTOR_PROMPT.md`
- `project/templates/VERIFIER_PROMPT.md`

They reference repository documents rather than copying large specs into prompts, reducing drift.

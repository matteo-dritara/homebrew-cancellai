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
`ready_for_review`; Codex performs the independent review, once the whole epic is ready. How
many rounds that takes is decided by what the rounds find, not by a constant
([ADR-0025](../adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md)).

## What "independent" can mean here

The word is borrowed from standards that define it precisely, and it is worth saying what this
project can and cannot supply. IEC 61508 scales required independence with risk on a three-rung
ladder: **independent person**, **independent department**, **independent organisation**. DO-178C
requires the verifier not be the author and that the separation be recorded as evidence. ISO 26262
grades it I1/I2/I3 and subjects the risk classification itself to a confirmation review.

Counting parties here rather than roles: one human owner, one executor model, one reviewer model.
That is the **independent person** rung - the lowest one. It is genuinely better than same-model
review, because Claude and Codex are different model families with different failure modes, and
the measured 47% first-round rejection rate is evidence that the separation does real work. It is
not an independent department and it is not an independent organisation, and a CR4 gate that says
"independent verification" should be read as the first rung and no further.

### Self-review

A review performed by the agent that executed the work is a **self-review**. Context isolation
removes priming; it does not remove self-preference bias, which is a property of the model rather
than of the conversation.

- A self-review **may** find defects, require repairs, and be recorded. Three of the last four
  epics were self-reviewed and they found real, reproduced defects.
- A self-review **may not** close a CR3 or CR4 story on its own, and may not be the sole basis for
  a Safety Verdict.
- A self-review record is committed as `<EPIC>-SELF-REVIEW.md`, never under a `VERIFIER-REVIEW`
  name. This is a rule, not a habit: `scripts/process_metrics.py` classifies rounds by that
  filename, and a misnamed record silently inflates the independent-review statistic that the rest
  of the measurement rests on.
- A self-review record states its own limitation on its first page, so a later reader cannot mistake
  it for the gate the contract names.

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

## Loadable form of this protocol

[`.claude/skills/`](../../.claude/skills/README.md) carries this protocol as Agent Skills (E24-S01): `orient`, `story-executor`, `epic-verifier`, `adversarial-cases`, `risk-gate`, `rust-kernel-guard`, `evidence-packet`. `epic-verifier` runs with `context: fork` so the review does not inherit the executor's conversation - the context isolation this document requires, enforced by the harness rather than by good intentions. That is not a substitute for role separation: a review performed by the agent that executed the work is a self-review, and the skill labels its own output that way. This document remains the contract; the skills are runners over it.

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
10. Set the story status to `ready_for_review` in `project/epics/*.json`, regenerate.
11. Commit the story as a checkpoint - code/docs, generated files, evidence packet, and the status change together - before starting the next story.

An executor's work is finished at `ready_for_review`. It does not set `verification` or `done` for its own change, and it does not write its own Safety Verdict. `python3 scripts/project_os.py check` refuses a `ready_for_review` story that has no committed executor evidence.

**Every story ends in a commit.** The commit is the checkpoint that keeps `main` an accurate record of what is implemented versus what is still contract-only, independent of when the epic's review round happens. It is not a handoff signal to the verifier by itself - review only starts once every story in the epic is `ready_for_review`, per [Review is per epic](../development/WORK_ITEM_MODEL.md#review-is-per-epic-and-bounded-to-two-rounds) - so a same-epic story may commit at `ready_for_review` and the next story may start immediately without waiting for a review round. See [Intra-epic dependency chains](../development/WORK_ITEM_MODEL.md#intra-epic-dependency-chains-are-satisfied-at-ready_for_review) for how a chained story's dependency is satisfied before that review round happens.

## Verifier procedure

Review runs at **epic** scope, when every story in the epic is `ready_for_review`. It stops on
**measured yield**, not on a round count: another round is required while a round rejects 10% or
more of the stories it judges, and two rounds whose findings do not overlap escalate to the owner
rather than closing ([ADR-0025](../adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md),
amending ADR-0014 / PD-022, which fixed the number at two). Three rounds is a cost ceiling that
`scripts/check_process.py` enforces, and reaching it is an owner decision recorded as such, not an
automatic close. Do not review a story in isolation while its neighbours are still moving.
Findings that survive the last round become new backlog work items recorded as accepted residual
risk.

Pick up work from the review queue rather than from chat context:

```sh
python3 scripts/project_os.py review
python3 scripts/project_os.py brief <STORY-ID> --role verifier
```

Move the epic's stories to `verification` while reviewing, then to `done` on a passing
verdict with evidence committed, or back to `in_progress` on a rejection. Closing the epic
cuts a release; see [WORK_ITEM_MODEL.md](WORK_ITEM_MODEL.md).

**Once an epic's closing commit is made** - its status set to `done` or `done_no_release`,
generated files regenerated, `python3 scripts/project_os.py check` passing - the agent
pushes it to `origin main` without a separate confirmation step for the push itself. The
gates already run (governance/process/evidence checks, and `scripts/release.py check` for a
`done` epic) are the actual safety boundary; withholding the push after they pass adds a
ceremony step, not a checked property. This does not extend to force-pushing, rewriting
history, or any push to a branch other than `main` - those remain confirm-first regardless of
epic status.

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

## Method defects

Every rule in `AGENTS.md` worth having was written after something went wrong, and until E28-S03
the record of what went wrong was nowhere. A session notices that a rule is missing, wrong or
unreachable; the correction happens in conversation; the session ends and the observation is gone.
The evidence packet already records residual risks about the *product*. The **Method defects**
section records the same class of thing about *how the work was done*.

An entry carries three things and the gate refuses it without them:

- **what happened** - expectation, outcome, and the correction;
- **what would have prevented it** - a document, gate or skill, or `none exists`, which is itself
  the finding;
- **a disposition** - `proposed`, `accepted <date>` or `declined <date> - reason`.

`proposed` is a legitimate resting state: it means the owner has it and has not ruled. Absence is
not, because an observation nobody dispositioned is a note rather than a finding. A **declined**
entry stays in the packet, so a later session does not re-propose it as though it were new.
`none` is the common and honest answer for most stories.

**The mechanism proposes and never writes.** It does not edit a skill, `AGENTS.md`, or any
canonical document - that is precisely why the package this idea came from was rejected, and a
test asserts the property rather than trusting the author of the next change to remember it. A
skill generated from an observation is a second copy of the contract produced without review.

## The handoff itself

Executor/verifier separation is the method this repository is built on, and until E28-S02 it was
the only part of it with no artifact. `project_os.py brief <ID> --role verifier` rendered the
verifier's input and nothing carried it anywhere: a human copied it into the other agent and
copied the verdict back. Two things the ledger could not see follow from that - whether the
verifier was given the brief the gate rendered or a paraphrase of it, and whether the verdict
committed is the verdict the verifier produced.

```sh
python3 scripts/verifier_handoff.py brief <STORY-ID> --rendered-by "<who>"
python3 scripts/verifier_handoff.py check
```

The brief is written to `project/evidence/<STORY-ID>/VERIFIER_BRIEF.md` with a checksum over its
own body. A verdict answering it repeats that checksum on a `Brief-Checksum:` line and names
itself on a `Verifier:` line. `check` refuses a verdict that answers a checksum which is not the
committed brief's, a brief edited after it was rendered, a verdict that ignores an existing brief,
and a verdict with no author.

**The refusal that matters most** is the one that keeps the two roles apart: a verdict whose
`Verifier:` is the party that rendered the brief is refused outright. Automating a handoff between
two roles is the most direct way to collapse them, so the rule that an executor's work ends at
`ready_for_review` and that it does not write its own Safety Verdict is checked instead of merely
being documented. The fields are still unverified identity claims: a party willing to write a
different name can defeat this refusal. The artifact prevents accidental role collapse and makes
the claimed separation auditable; it does not prove the human or model behind either claim.

Three properties a more convenient design would have lost:

- **An unavailable verifier is not a fallback.** No path here produces a verdict without one. A
  story with a brief and no verdict stays where it is.
- **A CR4 Safety Verdict stays attributable.** `Verifier:` is required, never defaulted, so an
  unattributed verdict is not expressible.
- **The mechanism is optional.** A handoff performed by a human stays valid - the method is the
  separation, not the automation of it. A story with no brief is reported as having used the
  manual route, not failed. Once a brief exists, however, a verdict cannot ignore it and claim the
  manual route after the fact.

## What a review round measures

ADR-0025 makes another round mandatory while a round's yield is at or above 10%, and
`scripts/process_metrics.py` computes it. E28's first round found four defects across four of five
stories and the table reported nothing, for three reasons now closed (E29-S01):

- **a record is named when it is not counted.** A story-scoped record is still not an epic round,
  but the report lists it rather than skipping it in silence - the same argument the tool already
  made about records it cannot classify.
- **a record can declare what it is.** `Review-Scope: epic` and `Round: N` in the record's header
  make it countable whatever its filename says.
- **`REPAIRED` is a verdict.** A reviewer who repairs a defect instead of failing the story found
  something, and the yield column now says so. `Rejected` stays a separate column, because the
  first-pass rejection rate asks a different question - whether the story was sent back.

A verdict of `REPAIRED` belongs in a round record whenever the reviewer fixed what it found. Using
`PASS` there makes a productive round indistinguishable from an empty one, which is the confusion
this repository measures itself to avoid.

A method-defect proposal carries a date, and one older than 90 days is reported as **unexamined**
rather than wrong (E29-S04). It never fails the gate: ageing a proposal must not convert it into a
defect.

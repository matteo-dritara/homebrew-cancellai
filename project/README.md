# Project Control Plane

This directory is the machine-readable source of truth for cancellAI planning and engineering governance.

Human-readable generated views live in `docs/DECISION_REGISTER.md`, `docs/ROADMAP.md`, `docs/BACKLOG.md`, and `project/generated/PROJECT_STATUS.md`.

## Commands

```sh
python3 scripts/project_os.py check
python3 scripts/project_os.py generate
python3 scripts/project_os.py status
python3 scripts/project_os.py next
python3 scripts/project_os.py brief E00-S01 --role executor
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_process.py check
```

## Templates

Every artefact the process asks for has a template. Use them rather than inventing a shape;
`scripts/check_process.py` validates the parts that carry decisions.

| Template | Used for |
| --- | --- |
| [STORY.md](templates/STORY.md) | drafting a story contract before it enters an epic file |
| [ADR.md](templates/ADR.md) | an architecturally significant decision |
| [RFC.md](templates/RFC.md) | a proposal with competing material options, before an ADR |
| [EXECUTOR_PROMPT.md](templates/EXECUTOR_PROMPT.md) | handing a story to an implementing agent |
| [VERIFIER_PROMPT.md](templates/VERIFIER_PROMPT.md) | handing a story to an independent reviewer |
| [EVIDENCE_PACKET.md](templates/EVIDENCE_PACKET.md) | executor evidence for a story or batch |
| [SAFETY_VERDICT.md](templates/SAFETY_VERDICT.md) | the reviewer's CR4 verdict, required to close a CR4 story |
| [RELEASE_EVIDENCE.md](templates/RELEASE_EVIDENCE.md) | evidence for a release |

## Editing rules

- Edit `decisions.json`, `roadmap.json`, or `epics/*.json`, then run `generate`.
- Never edit generated views by hand.
- Story IDs and decision IDs are permanent once merged. Do not reuse them.
- Supersede decisions rather than erasing history.
- A story entering `ready` must satisfy Definition of Ready in `docs/development/ENGINEERING_SYSTEM.md`.
- A story entering `done` must have evidence consistent with its Change Risk Level.

## Evidence

[`evidence/`](evidence/README.md) contains lightweight, version-controlled evidence summaries where needed. Large raw test logs belong in CI artifacts; the committed evidence packet links to the run/commit and records conclusions/residuals.

## Story handoff

`brief` renders acceptance criteria, verification requirements, referenced Safety Invariants, dependencies, documentation impact, and role instructions directly from the control plane. Use it as the starting packet for Claude/Codex rather than maintaining copied prompts that can drift.

## Ordering semantics

Dependencies define what is *allowed*; file/story order defines the default owner-facing execution sequence when several stories are simultaneously eligible. Do not add fake dependencies merely to serialize work that is technically independent. `next` may therefore show multiple ready stories; choose the earliest unless the owner intentionally parallelizes them.

---
name: evidence-packet
description: Produce a cancellAI executor evidence packet under project/evidence/ from real command output, so a story may sit at ready_for_review. Use when closing out a story, when asked for "evidence", "prova", "closure packet", or when project_os check refuses a ready_for_review story for missing evidence.
argument-hint: <STORY-ID>
allowed-tools: Bash, Read, Write, Edit, Glob, Grep
---

# Evidence packet

`python3 scripts/project_os.py check` refuses a `ready_for_review` story with no committed
executor evidence. The packet is the ledger entry for the change - it is written from command
output that actually ran, not from intent.

## Template

!`cat project/templates/EVIDENCE_PACKET.md`

## How to fill it

- **Every claim is traceable to a command or a file.** "Tests pass" is not evidence; the command
  and its result are. Paste real output for anything that failed.
- **Acceptance criteria, one row each**, quoted from `project/epics/<EPIC>.json` - not paraphrased.
  Paraphrasing an AC is how an AC quietly gets easier.
- **Safety obligations** the story names, each with the test or argument that discharges it.
- **Residual risk is mandatory, not optional.** What is knowingly not covered, and why that is
  acceptable at this level. An empty residual-risk section on a CR3/CR4 story is itself a finding.
- **Unavailable tooling** is stated, with who runs it instead (CI). Never imply a gate ran.
- **No private reasoning.** The verifier must not be primed by "why the implementation is
  definitely correct". Record what was done and what was observed.

## Where it goes

`project/evidence/<STORY-ID>/EVIDENCE.md`, committed in the same checkpoint as the code, the
docs, the regenerated files and the status change.

For a CR4 story, the packet does **not** include a Safety Verdict - that is the independent
reviewer's output, gated at `done`. Leaving it blank is correct.

## Close

```sh
python3 scripts/project_os.py generate
python3 scripts/project_os.py check
```

Then commit. Report the packet path, the AC coverage count, and the residual risks in one block.

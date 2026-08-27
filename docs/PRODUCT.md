# Product

## North star

**cancellAI is the local-first, cross-platform control plane for state created by AI agents.** It shows what agents leave behind, explains its value and risk, safely reclaims what is disposable, governs what is retained, and prevents runaway storage before it becomes a problem.

Short promise:

> Your AI agents create. cancellAI keeps their footprint under control.

## Why this product exists

Agentic development creates a new class of local state: transcripts, session graphs, checkpoints, tool outputs, file history, snapshots, temporary environments, indexes, caches, worktrees, debug output, and provider databases. Individual vendors can and increasingly do add their own retention and delete controls. What they cannot naturally provide is a neutral view across every agent on the machine.

The durable problem is therefore not "delete old Claude/Codex files." It is:

- Where is agent-generated state consuming storage?
- Which project/provider/session created it?
- What is disposable, rebuildable, resumable, important, or unknown?
- What would be lost if it were removed?
- How much space can actually be reclaimed?
- How do we stop abnormal growth before the disk becomes critical?
- How can one policy govern multiple agent ecosystems without giving a cloud service destructive authority over the workstation?

## Product sequence

cancellAI grows through a deliberate value ladder:

1. **SEE + RECLAIM** - inventory and safe cleanup.
2. **UNDERSTAND** - explain artifacts, projects, providers, risk, and reclaimability.
3. **PREVENT** - budgets, velocity, pressure, anomaly detection, Guardian.
4. **GOVERN** - policy, pinning, quarantine, archive, restore, bounded autonomy.
5. **FULL LIFECYCLE** - lifecycle control across local and eventually remote agent environments.

A later capability must never make an earlier safety guarantee weaker.

## Wedge market

### Initial user

Power developer / AI-native builder who:

- uses multiple coding agents;
- runs long or parallel sessions;
- understands terminal workflows;
- cares about disk pressure and local control;
- values an inspectable safety model more than one-click magic.

### Expansion

The architecture must allow a later zero-config developer experience without removing power-user controls. Team/enterprise is a third-stage market based on fleet coordination rather than restricting local OSS capabilities.

## Product boundary

In scope:

- state generated or managed by AI coding/development agents;
- agent session/transcript/checkpoint/file-history/tool-output/debug/cache state;
- agent-created worktrees or Git/checkpoint artifacts where attribution is strong;
- provider databases and indexes for inspection, with mutation only when supported safely;
- per-agent/project storage budgets;
- local anomaly detection and disk-pressure prevention;
- remote development targets in later phases.

Out of scope unless directly attributable to an agent workflow:

- generic browser cleanup;
- Downloads cleanup;
- generic npm/pip/Homebrew caches;
- antivirus or malware scanning;
- unrelated disk optimization;
- password management;
- generic system tuning.

This boundary prevents cancellAI from degenerating into a general-purpose system cleaner.

## Interfaces

CLI and TUI are first-class clients of the same engine. Neither may contain domain or safety logic. Guardian and Desktop are later clients of the same engine.

```text
                    cancellAI Engine
                           |
                 +---------+---------+
                 |                   |
                CLI                 TUI
        scripting / agents   exploration / review
                 |                   |
                 +---------+---------+
                           |
                  same plans/policy/safety
                           |
                  Guardian / Desktop later
```

## Open-source and commercial boundary

The single-machine product remains open source: scanner, artifact model, provider framework, safety kernel, CLI/TUI, quarantine, Guardian, and local policy engine.

Potential commercial capabilities live above one machine: fleet visibility, central policy distribution, organization audit, cross-machine analytics, enterprise identity/integrations, and managed knowledge distribution. The local node remains the destructive authority even in a fleet.

## Success measures

Product metrics are outcome-oriented, not vanity metrics:

- reclaimable bytes identified with confidence;
- bytes safely reclaimed;
- percentage of destructive actions that were reversible first;
- false-positive destructive recommendations: target zero;
- unknown/partial states correctly refused;
- mean scan latency and memory footprint on representative datasets;
- provider compatibility coverage by capability;
- Guardian anomaly precision/recall on synthetic and dogfood corpora;
- restore success rate;
- cancellAI self-state footprint versus self-budget;
- percentage of CR3/CR4 changes with complete evidence packets.

Never optimize adoption by relaxing safety defaults.

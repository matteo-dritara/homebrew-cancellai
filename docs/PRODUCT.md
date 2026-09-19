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

The TUI's Atlas screen (E09-S02) is the first exploration surface built on this shared engine:
total footprint and an estimated-reclaimable subset - always shown as two distinct numbers,
never blended into one - broken down by provider and by project, with an explicit
"Unattributed" bucket rather than a silent drop, and the individually largest contributors to
that footprint. An incomplete or unknown scan is flagged prominently rather than hidden inside
the totals it could be undercounting - the same "unknown is protected, never hidden" posture
`docs/CONSTITUTION.md`'s C-02 states for destructive authority, applied here to what the user is
told, not just to what the engine is allowed to do.

The TUI's Explain screen (E09-S03) answers "why does this exist and what would happen to it": for
one artifact at a time, why it exists (project, structural relationships), how it is classified,
what evidence backs it, its risk and reversibility, the authority it can actually reach, and the
concrete policy outcome - and when that outcome is destructive, the same human-readable reason a
`plan` document would show, never a second, possibly-diverging explanation invented for the
screen (C-06: evidence before action). Confidence weaker than fully verified is always flagged in
the text itself, not only by color.

The policy engine (E11) answers a related but distinct question: not "what would happen to this
artifact" but "why does the *system* allow exactly this much." Every policy resolution exposes
the same ordered explanation graph - what was requested and which configured scope asked for it,
every safety constraint considered in a fixed, deterministic order, and which one(s) actually
bound the final result when a request was not granted in full (E11-S03). The same graph is meant
for CLI, TUI, Guardian, and later fleet UI - one shared account of "why," never a per-surface
narrative that could quietly disagree with another.

## Open-source and commercial boundary

The single-machine product remains open source: scanner, artifact model, provider framework, safety kernel, CLI/TUI, quarantine, Guardian, and local policy engine. None of it requires an account, a network connection, or any fleet-coordination component to be present - `cancellai-cli`/`cancellai-tui` depend on no networking crate at all, and every authority decision (`cancellai_safety::authority::effective_authority`) is fully computable from purely local inputs (E18-S03, `local_authority_is_unaffected_by_an_unconfigured_commercial_service`).

Potential commercial capabilities live above one machine: fleet visibility, central policy distribution, organization audit, cross-machine analytics, enterprise identity/integrations, and managed knowledge distribution. The local node remains the destructive authority even in a fleet: E18-S01/E18-S02 give this a concrete, open shape rather than a future aspiration. `cancellai_safety::remote_execution::RemoteExecutionRequest` **is** the open local node protocol this section promises - a fully-specified, versioned, signed JSON document (`SUPPORTED_SCHEMA_VERSIONS`, `parse_request`) that any coordinator, commercial or self-hosted, produces identically. A commercial fleet service is one possible *producer* of that document; it is never a second, privileged path into the target's authority. Verification narrows every accepted request to exactly one `target`/`requested_action` pair (`VerifiedRemoteIntent` - a semantic `ActionClass`, never an `AuthorityLevel` directly, ADR-0032), capped by a locally-configured ceiling the target's own operator sets (`TrustedRemoteControllers`) - a commercial service can request coordination up to that ceiling, never grant itself more, and never supply a plan, a pre-decided outcome, or any other authority input. "Commercial services add coordination, not local destructive capabilities" is this boundary, not a separate promise about it.

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

# Policy Model

## Goal

Policy expresses human intent without becoming an imperative scripting language or a bypass around safety.

Human-facing policy is declarative. The engine compiles it into typed constraints and produces a deterministic `EffectivePolicy` for each artifact/action.

## Scope hierarchy

Policies can be scoped to:

```text
GLOBAL
  MACHINE
    PROVIDER
      PROJECT
        ARTIFACT TYPE
          SESSION / EXPLICIT PIN
```

More specific user policy can refine less specific user policy, but cannot override constitutional or artifact/provider ceilings.

## Constitutional precedence

From strongest to weakest:

1. Safety invariants.
2. Artifact Authority Ceiling.
3. Provider capability and trust ceiling.
4. Explicit pin/protection.
5. Specific user policy.
6. General user policy.
7. Product defaults.

A conflict is resolved deterministically and is explainable. There is no "last parser wins" behavior for security-relevant fields.

## Example human policy

The initial format may be YAML or another readable declarative form once the Rust implementation selects parsing dependencies. The semantics, not the serialization, are normative.

```yaml
global:
  authority: recommend
  retention: 30d
  total_budget: 50GB

providers:
  codex:
    budget: 20GB

projects:
  cancellai:
    retention: 90d
    authority: quarantine

artifact_types:
  rebuildable_debug:
    authority: autopilot

pins:
  - session: abc123
```

If the artifact ceiling is `QUARANTINE`, the effective result remains `QUARANTINE` even if the user asks for `AUTOPILOT`.

## Policy is not code execution

Policy files may not embed arbitrary shell commands, scripts, or provider-native commands. Operations come from verified engine/provider capabilities. This prevents a policy or federated knowledge file from becoming a code-execution channel.

## Explanation contract

Every result can be explained in ordered steps:

```text
Requested: AUTOPILOT
Global policy: AUTOPILOT
Project retention: eligible
Artifact: R3 RESUMABLE
Artifact authority ceiling: QUARANTINE
Provider quarantine capability: VERIFIED
Final: QUARANTINE
```

The engine exposes the same explanation graph to CLI, TUI, Guardian, and later fleet UI.

## Policy migration

Policy schemas are versioned. Unknown security-relevant keys fail validation. Automatic migration may rewrite syntax only when semantics are provably equivalent; otherwise the user receives a migration plan rather than silent reinterpretation.

## Rust schema (E11-S01)

`cancellai-policy::schema` implements the scope hierarchy above as a versioned document type -
`PolicyDocument` (`schema_version`, `global`, `machine`, `providers`, `projects`,
`artifact_types`, `pins`), each scope a `ScopePolicy { authority, retention, budget }`. It is
serialized as JSON today, reusing this workspace's existing `serde`/`serde_json` dependency
rather than adding a YAML parser for this story; the example above remains the normative
statement of *semantics*, and the serialization format stays an open decision as this document's
own text already anticipated ("YAML or another readable declarative form once the Rust
implementation selects parsing dependencies").

Every struct is `#[serde(deny_unknown_fields)]` and `parse_policy` rejects any `schema_version`
other than `CURRENT_SCHEMA_VERSION` (`1` today) - the same versioned-document pattern
`cancellai-provider-api::manifest` and `cancellai-safety::knowledge_bundle` already use. A scope
with nothing set deserializes as absent, never as a default authority (SI-025's monotonic
narrowing depends on "unset" and "weakest" staying distinct facts). `retention`/`budget` are
carried as their written text (`"30d"`, `"50GB"`); parsing that into a duration/byte count is
E11-S04's ("Budgets, retention, pinning") outcome, not this schema's.

This module parses and structurally validates a document only. Turning several scopes into one
deterministic `EffectivePolicy` for an artifact/action - the explanation contract and
constitutional-precedence resolution described above - is E11-S02's constraint resolver.

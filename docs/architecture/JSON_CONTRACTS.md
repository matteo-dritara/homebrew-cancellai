# JSON Contracts

Versioned, machine-readable document shapes for the four artifacts the engine produces:
**inventory**, **plan** (a serialized [`SealedPlan`](DOMAIN_MODEL.md#sealedplan)),
**explanation** (the per-action policy trace described in
[`POLICY_MODEL.md`](POLICY_MODEL.md#explanation-contract)), and **result** (serialized
[`Results`](DOMAIN_MODEL.md#results)). This document is the single normative definition of
each shape; [`tests/fixtures/schemas/golden/`](../../tests/fixtures/schemas/golden/) holds
worked examples and `scripts/check_schemas.py` enforces this contract mechanically against
them, per E01-S03.

These are specifications for the target engine, not a description of current Python CLI
output. `cancellai.py` is frozen (E01, [`AS_IS.md`](AS_IS.md)) and is not being changed to
emit this shape; E01-S04 records what it actually emits, and any gap between the two is a
tracked migration item, not silently normalized away.

Terms below (`AgentArtifact`, `ProviderRoot`, `Evidence`, `KnowledgeConfidence`, `Action`,
`SealedPlan`, `Results`, `Effective Authority`) are defined once in
[`DOMAIN_MODEL.md`](DOMAIN_MODEL.md) and reused here without redefinition, per E01-S01.

## Common envelope

Every document below starts with the same four keys, **in this exact order**:

```text
schema_version   integer, starts at 1; bumped only for a breaking change
document_type    "inventory" | "plan" | "explanation" | "result"
generated_at     ISO-8601 UTC timestamp
generator        { "name": string, "version": string }
```

Fixed leading order is not cosmetic: a consumer (or a human diffing two documents) checks
`schema_version` before anything else, and a byte-identical envelope prefix makes that check
possible without first parsing the whole document. `scripts/check_schemas.py` checks this
order on every golden document, not just that the keys are present somewhere.

## Compatibility policy

- **`schema_version` is checked first.** An unrecognized `schema_version` means the document
  cannot be safely interpreted for anything destructive: a consumer must treat it as
  `OBSERVE`-only and never infer intent from a document shape it does not recognize (C-03).
- **Unknown fields outside the safety-critical Action envelope are permitted and ignored.**
  A consumer built against an older `schema_version` reading a document with additive fields
  from a newer minor addition ignores what it does not know. This is how the format stays
  forward-compatible without a version bump for every addition.
- **Unknown values inside the safety-critical Action envelope never fall back
  permissively.** The Action envelope is `action_class`, `authority`, `reversibility`,
  `evidence_ids`, and `execution_preconditions` (defined under [Plan document](#plan-document-sealedplan-projection)
  below). An unrecognized enum value there (for example an `authority` string this consumer
  version does not know) is treated as the strictest/lowest value on that axis - `authority`
  collapses to `OBSERVE`, an unrecognized `action_class` is treated as non-actionable - never
  inferred upward (C-05, SI-016). This is a hard requirement on any implementation reading
  these documents, not a convention a consumer may opt out of.
- **Removing or renaming a required field, or changing a field's meaning, is a breaking
  change.** It requires bumping `schema_version` and an ADR (docs/adrs/); it is never done
  silently inside an existing version.

## Inventory document

One entry per observed [`AgentArtifact`](DOMAIN_MODEL.md#agentartifact), plus the scan
completeness state each entry's classification depends on.

```text
schema_version, document_type="inventory", generated_at, generator   (common envelope)
inventory_id
runtime_environment: "wsl2" | "native"
provider_roots: [ ProviderRoot projection, ... ]   (id, provider_id, origin, confidence, mutation_eligible, filesystem_context)
scan_completeness: [ { scope, complete: bool, error_count: integer }, ... ]  (see note below)
artifacts: [ AgentArtifact projection, ... ]
```

`runtime_environment` and each root's `filesystem_context` (E20-S02/E20-S03) surface
`cancellai_platform::{RuntimeEnvironment, FilesystemContext}` (`docs/architecture/
PLATFORM_MODEL.md`'s "WSL" section): whether this process is running inside a WSL2 guest, and
whether a given provider root's own storage is the guest's native filesystem
(`filesystem_context: "linux"`), a mounted Windows drive (`"windows_mounted"`), an
unrecognized mount (`"other:<fstype>"`), or not classifiable at all (`"unsupported:<reason>"` -
always the case on non-Linux hosts). Purely descriptive: neither field grants or withholds
mutation authority by itself - `mutation_eligible` (root-origin authority, SI-002/ADR-0013) and
the safety kernel's own device-identity boundary check (SI-018) are unaffected by either value.

`scan_completeness[].error_count` is the number of distinct paths the scope could not
observe, and `complete` is `false` whenever that number is non-zero. It was previously derived
as `u32::from(!complete)`, so it never exceeded `1` while the Python reference enumerates every
unreadable path - a consumer treating it as a count was misled (`CR-TE-10`, repaired in
E21-S03). An artifact produced from an incomplete scope carries `knowledge_confidence` no higher
than `LOW/UNKNOWN`, and no `delete` action is proposed for that scope at all.

Each `artifacts[]` entry carries at minimum: `artifact_id`, `identity_token`, `provider_id`,
`artifact_type`, `risk_class`, `reversibility`, `knowledge_confidence`, `activity_state`,
`residency_state`, `protection_state`, `integrity_state`, `authority_ceiling`, `evidence_ids`
(>= 1).

`artifact_id` is an opaque, engine-assigned identifier - two conformant engines observing the
same fixture are never required to assign the same one. `identity_token` is not: it is
[`AgentArtifact`](DOMAIN_MODEL.md#agentartifact)'s `IdentityToken` field, derived from stable
provider-observable facts (for example a session UUID plus its provider-relative path), and
two conformant engines observing the same underlying state MUST produce the same
`identity_token` for the same artifact. This is what a differential comparator matches
records on - see [`VERIFICATION_STRATEGY.md`](../development/VERIFICATION_STRATEGY.md#differential-comparison-contract).
E01-S03's first draft of this document omitted `identity_token`; E01-S05 added it back
before the epic's review round, once building the differential harness showed a document
with no content-derived matching key cannot be differentially compared at all.

An artifact produced from a `PARTIAL` or `UNKNOWN` `scan_completeness` scope must carry
`knowledge_confidence` no higher than `LOW/UNKNOWN` for that scope (SI-008, SI-009) - the
document cannot express higher confidence than the scan that produced it actually earned.

## Plan document (`SealedPlan` projection)

```text
schema_version, document_type="plan", generated_at, generator   (common envelope)
plan_id
inventory_snapshot_id
runtime_environment: "wsl2" | "native"
provider_roots: [ ProviderRoot projection, ... ]
actions: [ Action, ... ]
notes: [ string, ... ]
safety_invariant_refs: [ "SI-###", ... ]
```

`runtime_environment` and `provider_roots[].filesystem_context` are the same fields the
inventory document carries - see its own section above.

Each `Action` is the safety-critical envelope this document exists to carry. Every entry -
**including `OBSERVE`-class entries** - requires:

```text
action_id
target_artifact_ids: [ ArtifactId, ... ]   (>= 1)
action_class          OBSERVE | QUARANTINE | ARCHIVE | DELETE
reason                non-empty human-readable string: why this action is proposed
authority              OBSERVE | RECOMMEND | QUARANTINE | GOVERN | AUTOPILOT
reversibility          REBUILDABLE | QUARANTINABLE | ARCHIVABLE | VENDOR_CONDITIONAL | IRREVERSIBLE | UNKNOWN
evidence_ids: [ EvidenceId, ... ]          (>= 1)
execution_preconditions: [ Precondition, ... ]
```

`execution_preconditions` may be an empty list only when `action_class == "OBSERVE"`
(observation mutates nothing, so there is nothing to revalidate). Every action whose class is
`QUARANTINE`, `ARCHIVE`, or `DELETE` requires at least one precondition - this is the literal
requirement behind AC3 of E01-S03 and behind SI-013/SI-016: a plan that proposes to mutate
without stating what must still be true immediately before it does is not a sealed plan.

A `Precondition` names what is re-observed immediately before mutation and what makes the
action `STALE_PLAN` (SI-013): at minimum `{ "kind": string, "expected": value }` - for
example `{"kind": "root_identity_token", "expected": "<token>"}` or
`{"kind": "process_not_running", "expected": true}`.

## Explanation document

The deterministic per-action policy trace described in
[`POLICY_MODEL.md`](POLICY_MODEL.md#explanation-contract), exposed identically to CLI, TUI,
and Guardian.

```text
schema_version, document_type="explanation", generated_at, generator   (common envelope)
plan_id
explanations: [ ActionExplanation, ... ]
```

Each `ActionExplanation`:

```text
action_id
steps: [ { factor: string, input: value, resulting_authority: AuthorityLevel }, ... ]   (ordered, >= 1)
final_authority: AuthorityLevel
```

`steps` is ordered from the least specific factor to the most specific (matching the
constitutional precedence order in `POLICY_MODEL.md`), and `final_authority` must equal the
`resulting_authority` of the last step - the document cannot assert a final authority the
step trace does not derive.

## Result document (`Results` projection)

```text
schema_version, document_type="result", generated_at, generator   (common envelope)
plan_id
action_results: [ ActionResult, ... ]
summary: { attempted, succeeded, safely_skipped, failed: integer; reclaimed_bytes: integer }
```

Each `ActionResult`:

```text
action_id
status              attempted | succeeded | safely_skipped | failed
reason_code          stable machine-readable string (e.g. "STALE_PLAN", "PROTECTED_NAME", "OK")
reclaimed_bytes      integer, >= 0 (0 when status != "succeeded")
post_action_state    ResidencyState the artifact ended up in
```

A `safely_skipped` result is not equivalent to success for automation exit semantics
(SI-014) - `summary.safely_skipped` is a distinct counter from `summary.succeeded`, and a
consumer that collapses them together is misreading this document, not merely being
imprecise.

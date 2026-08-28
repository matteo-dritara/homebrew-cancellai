# Domain Model

## AgentArtifact

`AgentArtifact` is the provider-neutral unit of state cancellAI reasons about. A file may be an artifact, but an artifact may also be a directory tree, session graph, vendor-native object, database row group, or logical checkpoint.

Minimum conceptual fields:

```text
ArtifactId
MachineId
ProviderId + provider version/layout fingerprint
ProjectRef? / Unattributed
SessionRef?
ArtifactType
IdentityToken
LogicalSize
AllocatedSize? / ReclaimEstimate?
Observed timestamps
RiskClass
Reversibility
KnowledgeConfidence
ActivityState
ResidencyState
ProtectionState
IntegrityState
AuthorityCeiling
Evidence[]
Capabilities[]
```

Provider adapters map raw observations into this model; they do not redefine safety semantics.

## ProviderRoot

`ProviderRoot` is the fingerprinted, confidence-scored capability a provider adapter observes or mutates under. It is a claim about a filesystem/vendor location, not proof of identity, and it is distinct from the `Effective Authority` it can help grant.

Minimum conceptual fields:

```text
RootId
ProviderId
Origin (DEFAULT | CUSTOM)
FingerprintEvidence[]
KnowledgeConfidence
MutationEligible
CapabilitySnapshot
```

A low-confidence or unfingerprinted custom root remains inspectable but is never `MutationEligible`: it can back `Evidence` and `AgentArtifact` observation, but cannot by itself authorize a destructive `Action`. See [`PROVIDER_MODEL.md`](PROVIDER_MODEL.md) and [`../security/SAFETY_INVARIANTS.md`](../security/SAFETY_INVARIANTS.md) (SI-002).

## Risk classes

The final names may be encoded as an enum, but the meaning is:

- `R0_DISPOSABLE` - intended to be throwaway.
- `R1_REBUILDABLE` - can be regenerated from authoritative state.
- `R2_RECOVERABLE` - removable if a cancellAI/vendor recovery path is available.
- `R3_RESUMABLE` - removal can destroy session/history/resume value.
- `R4_SENSITIVE` - high-value or structurally sensitive provider state.
- `R5_PROTECTED` - never destructive under ordinary cancellAI authority.

Risk class is not itself permission.

## Reversibility

- `REBUILDABLE`
- `QUARANTINABLE`
- `ARCHIVABLE`
- `VENDOR_CONDITIONAL`
- `IRREVERSIBLE`
- `UNKNOWN`

## Knowledge confidence

Confidence is categorical and evidence-backed, not a fake percentage:

- `VERIFIED` - tested built-in/provider knowledge for the observed version/layout.
- `OBSERVED` - direct structural evidence supports the claim but no verified contract exists.
- `INFERRED` - heuristic relationship useful for recommendations.
- `LOW/UNKNOWN` - insufficient evidence.

Inferred/unknown knowledge cannot silently raise destructive authority.

## Lifecycle axes

### Activity

`ACTIVE | IDLE | STALE | ORPHANED | UNKNOWN`

### Residency

`HOT | COLD | ARCHIVED | QUARANTINED | PURGED`

### Protection

`NORMAL | PINNED | PROTECTED`

### Integrity

`HEALTHY | PARTIAL | CORRUPTED | UNKNOWN`

These axes are independent. Example:

```text
activity: ORPHANED
residency: HOT
protection: PINNED
integrity: HEALTHY
```

is protected from cleanup despite orphan status.

## Evidence

Evidence objects explain what cancellAI knows and why. Examples:

- provider metadata says project path X;
- transcript filename contains session UUID Y;
- layout fingerprint matches built-in Claude profile Z;
- project path no longer exists;
- last observed mutation was 93 days ago;
- provider process is currently active;
- filesystem identity token changed after planning.

Each safety-relevant classification references evidence IDs.

## Effective Authority

Conceptually:

```text
EffectiveAuthority = minimum(
  UserAuthority,
  ArtifactAuthorityCeiling,
  ConfidenceAuthority,
  ReversibilityAuthority,
  LifecycleAuthority,
  ProviderCapabilityAuthority,
  ProviderTrustAuthority,
  ReleaseChannelAuthority,
  ConstitutionalSafetyFloor
)
```

Authority levels:

`OBSERVE < RECOMMEND < QUARANTINE < GOVERN < AUTOPILOT`

The ordering is capability ordering, not risk score. An `AUTOPILOT` user preference cannot elevate an artifact whose ceiling is `QUARANTINE`.

## Action

`Action` is one candidate unit of work inside a `SealedPlan`: an observation, quarantine, archive, or delete operation targeting a single `AgentArtifact` (or a bounded artifact group, such as a Codex subagent tree). An `Action` is inert data until its parent plan is approved, and its preconditions are revalidated immediately before execution.

Minimum conceptual fields:

```text
ActionId
TargetArtifactId[]
ActionClass (OBSERVE | QUARANTINE | ARCHIVE | DELETE | ...)
Reversibility
Evidence[]
ExecutionPreconditions
```

`Action` never mutates on its own. Only the plan executor behind the single safety kernel (C-07) may turn an approved `Action` into a `Result`.

## SealedPlan

A mutating plan is a first-class immutable object. See [`JSON_CONTRACTS.md`](JSON_CONTRACTS.md) for the versioned, machine-readable serialization of this object and of `Results` below. Minimum fields:

- plan ID/schema version/time;
- inventory snapshot ID;
- approved root capability/fingerprint;
- provider identity/version/layout/capability snapshot;
- artifact identity tokens;
- requested/effective policy explanation;
- action class and reversibility;
- expected size/reclaim estimate and confidence;
- execution preconditions;
- safety invariant references;
- knowledge bundle/version references where relevant.

Immediately before mutation the executor re-observes all safety-critical preconditions. Any relevant drift makes the action `STALE_PLAN` and non-destructive.

## Results

Mutation results are per action and aggregate without hiding partial outcomes:

- attempted;
- succeeded;
- safely skipped/blocked;
- failed;
- observed reclaimed bytes;
- post-action reconciliation state;
- stable machine-readable reason/error codes.

A skipped safety block is not equivalent to success for automation exit semantics.

## Legacy vocabulary

This vocabulary is canonical: architecture, schemas, tests, and UI contracts define each term once, here, and reuse it rather than inventing synonyms. The Python v1 reference (`cancellai.py`) predates it and is not being renamed - [`AS_IS.md`](AS_IS.md) freezes Python as a behavioral oracle rather than a target for architectural cleanup - so every legacy name is mapped onto exactly one canonical term instead.

| Legacy Python name | Canonical term | Notes |
| --- | --- | --- |
| `Action` (`cancellai.py`) | `Action` | Same concept, narrower fields; the canonical `Action` adds artifact identity, evidence, and confidence. |
| `Plan` | `SealedPlan` | The legacy `Plan` is mutable-by-construction; `SealedPlan` is immutable once approved and is revalidated before execution. |
| `CleanResult` | `Result` | The legacy result is clean-command-specific; the canonical `Result` is provider-neutral and reported per `Action`. |
| `RootAuthority` | `ProviderRoot` | The legacy name conflates root identity with the authority it grants; the canonical model separates root identity (`ProviderRoot`) from `Effective Authority`. |
| `Scan` | feeds `KnowledgeConfidence` | `Scan` remains a Python-only discovery-scope helper. Its COMPLETE/PARTIAL/UNKNOWN output is evidence that lowers or withholds confidence; it is not promoted to an independent canonical noun. |
| `CoverageBucket` | classification coverage over `AgentArtifact[]` | No independent canonical noun; coverage reporting is a projection over classified/unclassified artifacts, not a new domain object. |
| `ProcessObservation` | feeds `Evidence` (activity evidence) | Feeds `ActivityState` through `Evidence`; not promoted to a standalone canonical term. |

Deprecated/ambiguous phrasing that must not appear in new architecture, schema, or UI text:

- "cleanup" as a noun standing in for a specific `ActionClass` - name the class (`QUARANTINE`, `DELETE`, ...) instead;
- "safe to delete" as a bare boolean - express it as `Effective Authority` meeting or exceeding the required class, never a yes/no flag;
- "root path" used interchangeably for the raw filesystem path and the fingerprinted capability - use `ProviderRoot` for the capability and "root path" only for the literal filesystem path inside it.

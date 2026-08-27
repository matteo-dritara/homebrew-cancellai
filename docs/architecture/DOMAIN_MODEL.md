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

## SealedPlan

A mutating plan is a first-class immutable object. Minimum fields:

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

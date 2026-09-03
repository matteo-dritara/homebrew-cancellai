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

### FileFacts: the OBSERVE-stage evidence `AgentArtifact` is built from

E04-S01 implements the `LogicalSize`/`AllocatedSize?`/"Observed timestamps"/`ArtifactType`/
`IdentityToken` slice of `AgentArtifact` as `FileFacts` at
`rust/crates/cancellai-inventory/src/file_facts.rs` - deliberately *not* the full
`AgentArtifact` itself. `FileFacts` carries only what a filesystem observation can establish
on its own; `RiskClass`, `Reversibility`, `KnowledgeConfidence`, the lifecycle axes, and
`AuthorityCeiling` are classification decisions that require provider/policy knowledge no
story before E05/E06 has, and `FileFacts` never invents one. `observe_file_facts` composes
three independent `cancellai-platform` seams - `FsObserver` (logical size, kind, modified
time), `IdentityObserver` (identity token, SI-013/SI-017), and the new `AllocationObserver`
(E04-S01, allocated/physical size) - and every metric a seam cannot report is an explicit
`SizeMetric::Unsupported`/`IdentityObservation::Unsupported` value, never a fabricated zero
or a silent copy of a different metric (SI-008, SI-009, SI-010). A path's `ScopeBoundary`
(within its traversal scope, crosses a filesystem/volume boundary, or unknown) and a
per-fact `FactConfidence` (complete or partial, with named reasons) round out the record; the
outer `FactObservation` enum mirrors `Observation`/`IdentityObservation`'s absent-vs-unreadable
split so a missing path and an unreadable one are never conflated here either.

`provider_hint`/`category_hint` are present on `FileFacts` but always `None` - they exist so
the struct's shape does not need to change once a provider-adapter epic (E05) or a
classification stage populates them, not because this story infers either now
(AGENTS.md: "Do not silently create product scope in code").

### One traversal per scope, and scan completeness

E04-S02 (`rust/crates/cancellai-inventory/src/scan.rs`) replaces the pattern
[`AS_IS.md`](AS_IS.md) documents for the Python reference - status, planning, and
top-consumers each re-walking the same directory tree - with a single recursive walk,
`scan_scope`, that produces one `InventorySnapshot`; `status_summary`, `top_consumers`, and
`planning_candidates` are pure reads over that same snapshot, proven by traversal counters
(`directories_visited`, `paths_observed`) that a test asserts are unchanged after calling all
three. Traversal never follows a symlink and never descends across a device/filesystem
boundary a directory's identity reveals (SI-018) - a boundary-crossing directory is still
recorded as a fact, just not read into.

`scan_scope` is not the traversal the shipped CLI runs. E05/E06 built layout-specific discovery
inside the provider adapters instead, and E21-S04
([ADR-0018](../adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md))
confirmed that split while closing what it had cost: the adapters must express what they
observed as `ScopeCompleteness`, and `cancellai-policy`'s `ProviderResolution` hands out planning
candidates only through a `ProviderPlanningView` that carries it - the same construction-level
guarantee `planning_view` gives here, applied to the layer that actually feeds `clean`. What
E04 contributes to the product is therefore this model, not this walk.

E04-S03 (`rust/crates/cancellai-inventory/src/completeness.rs`) classifies every scope
`Complete`, `Partial`, or `Unknown` from that same snapshot - the scope root itself being
unobservable is `Unknown`; a readable root with some unreadable/permission-denied/vanished
descendant, or a descendant whose identity/allocation could not be established, is `Partial`
with every reason named (SI-010: scan errors are visible, never summarized away). The one
public way to hand a caller planning-facing candidates, `planning_view`, returns a
`PlanningView` bundling `candidates` and `completeness` in the same struct with no
bare-candidates accessor - the type shape itself is what keeps planning from silently
erasing completeness information, the same pattern `cancellai-safety::SealedPlan` uses for
its own invariants.

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

E03-S04 implements the formula above as a generic monotonic minimum at
`rust/crates/cancellai-safety/src/authority.rs`: `compute_effective_authority` takes any
number of named `AuthorityConstraint`s and returns the minimum plus a deterministic
explanation trace naming which constraint(s) actually bound the result (never hiding a tie).
`effective_authority` wires up the constraints buildable today - `UserAuthority`,
`ArtifactAuthorityCeiling` (supplied by the caller; deriving one from `RiskClass` is a
classification decision this story does not invent), `ConfidenceAuthority` (from
`KnowledgeConfidence`), `LifecycleAuthority` (from `ActivityState`/`ProtectionState`/
`IntegrityState`), and an explicit `ConstitutionalSafetyFloor` restating SI-001.
`ProviderTrustAuthority` joins these in E05-S02, from `cancellai_safety::TrustedTier` (an
opaque wrapper around `cancellai_model::ProviderTrust`, `docs/PROVIDERS.md` "Trust levels",
SI-021) - see [`PROVIDER_MODEL.md`](PROVIDER_MODEL.md) "Trust chain" for the full account,
including `TrustedTier::promote`, the sole gate that can raise a trust tier, and the E05
verifier round 1 repair that made `AuthorityInputs::provider_trust` require this opaque type
rather than accepting a bare, externally-constructible `ProviderTrust` directly. `Reversibility`
authority, `ProviderCapabilityAuthority`, and
`ReleaseChannelAuthority` are not wired in yet - no capability-classification or
release-channel subsystem exists to supply them - and adding them later is a matter of
supplying more named constraints to the same generic function, not a redesign.

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

E03-S02 implements the identity-bound core of this at `rust/crates/cancellai-safety/src/sealed_plan.rs`:
`SealedPlan` (root fingerprint, root identity, artifact identity, action class, authority,
reversibility) is immutable by API shape (private fields, no mutating methods), and
`revalidate` is the fail-closed `STALE_PLAN` check - it exhaustively matches every
`IdentityObservation` from `cancellai-platform` (E03-S01) and returns `Proceed` only for an
exact identity match; every other case, including a filesystem/platform that cannot
re-establish identity at all, is `StalePlan`. The full field list above (inventory snapshot
ID, a batch of `Action`s, evidence references, knowledge-bundle version references) is not
yet populated - those belong to subsystems that do not exist yet (E04 inventory engine,
provider knowledge) and are deferred to the stories that build them, not stubbed out here.

E03 verifier review round 1 found the first version of `SealedPlan` recorded a root
fingerprint and an artifact identity with no structural connection to each other or to the
target actually executed against - a plan sealed with root A's fingerprint executed
successfully against a target bound under root B. `SealedPlan::seal` is now the only public
constructor: it takes a real `ApprovedRoot`/`BoundedPath` pair (E03-S03) and derives
`root_identity`/`artifact_identity` directly from them, never from independent caller-
supplied values; `mutation_executor::execute` (E03-S05) compares `plan.root_identity()`
against `target.root_identity()` for whatever target is actually passed to it at execution
time - not merely whatever was used at sealing time - closing the gap for good.

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

## Diagnostics

E02-S03 defines the canonical error taxonomy for the Rust target, implemented in
`rust/crates/cancellai-model/src/diagnostic.rs`. Six categories, each with a stable string
code and a stable exit code that is never renumbered or repurposed once released:

```text
InvalidInput           INVALID_INPUT           exit 2
SafetyBlock             SAFETY_BLOCK             exit 4
IncompleteInventory     INCOMPLETE_INVENTORY     exit 4
CompatibilityFailure    COMPATIBILITY_FAILURE    exit 4
MutationFailure         MUTATION_FAILURE         exit 3
InternalFault           INTERNAL_FAULT           exit 3
```

This generalizes the exit taxonomy [`AS_IS.md`](AS_IS.md) documents for the Python reference
(0 success / 1 declined / 2 invalid usage / 3 mutation failure / 4 safety block-or-defer):
Python collapsed several distinct failure modes into the same exit code because it had no
typed error model to keep them apart. The Rust taxonomy is not required to reuse Python's
numeric codes 1:1 - `SafetyBlock`, `IncompleteInventory`, and `CompatibilityFailure` share
exit code 4 (all withhold destructive authority, matching Python's exit 4), and
`MutationFailure`/`InternalFault` share exit code 3 (both are failures of requested work),
but each keeps a distinct machine-readable string code.

A `Diagnostic` carries a category and a message. Both its human-readable (`Display`) and
machine-facing (JSON `Serialize`) renderings read the string code from the same
`ErrorCategory::code()` function - neither representation computes or stores its own copy,
so they cannot drift apart. `docs/CLI.md` remains generated from the frozen Python reference
(`AGENTS.md`'s Python reference freeze) and is not updated by this story; the Rust CLI's own
reference documentation, once E06 (Rust CLI Parity and Cutover) exists, is what will expose
this taxonomy at the CLI-docs layer.

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

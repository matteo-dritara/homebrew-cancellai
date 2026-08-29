# Safety Invariants

These invariants are constitutional runtime properties. They have stable IDs so work items, tests, Safety Verdicts, and incident reports can reference the same rule. A story may add stronger constraints; it may not silently weaken these.

## Protection and authority

### SI-001 Protected/unknown state is non-destructive

Artifacts classified `PROTECTED`, `UNKNOWN`, or with insufficient safety confidence cannot receive destructive authority. User policy cannot override this floor.

### SI-002 Provider root must be positively bounded

Every mutation occurs under a validated provider root capability. Catastrophically broad, ambiguous, or low-confidence custom roots are non-destructive.

### SI-003 Mutation cannot escape or delete the approved root

No filesystem mutation may target outside its approved root or the root object itself, including via path normalization tricks or link indirection.

### SI-004 Unknown provider layout/version reduces capability

Provider/version/layout drift cannot preserve destructive capabilities merely because the provider name is recognized.

### SI-005 Category expansion does not erase independent policy

Modes that broaden artifact categories do not silently disable retention, protection, activity, confidence, or authority constraints.

### SI-006 Protected-name/category barriers are defense in depth

Known protected provider state is checked both before planning and at execution. Scanner selectivity alone is never the only protection.

### SI-007 Ambiguous CLI/configuration is non-destructive

Destructive commands require explicit destructive intent. Parser ambiguity, missing subcommands, invalid policy, or incompatible flags never imply mutation.

### SI-008 Partial scan is non-destructive

A `PARTIAL` inventory scope cannot produce irreversible actions for artifacts whose safety depends on the missing information.

### SI-009 Unknown scan state is non-destructive

Missing evidence is not interpreted as absence of active/protected data.

### SI-010 Scan errors are visible

Permission/I/O/disappearance failures are represented as explicit evidence/diagnostics and are not collapsed silently into zero size or empty state when that could affect safety.

## Concurrency and identity

### SI-011 Shared provider metadata is not rewritten under unsafe concurrency

Provider metadata that may be concurrently written is not rewritten while the provider is active unless the provider supplies a concurrency-safe native operation with verified semantics.

### SI-012 Dry-run and execution select the same semantic plan

Between identical observations/policy inputs, dry-run does not use weaker selection logic than real execution. Execution may only remove actions due to revalidation; it may not add stronger actions.

### SI-013 Identity is revalidated immediately before mutation

A path alone is insufficient. Safety-critical object/root identity and relevant preconditions are re-observed at execution. Identity drift produces `STALE_PLAN`/block.

Implemented for artifact identity at `rust/crates/cancellai-safety/src/sealed_plan.rs::revalidate`
(E03-S02), consuming `cancellai-platform`'s `IdentityObserver` (E03-S01).

### SI-014 Safety-blocked/partial is not success

Automation receives a distinct non-zero or structured status when requested mutation was materially blocked/skipped for safety or incomplete execution.

### SI-015 Atomic metadata rewrite preserves concurrent correctness

When cancellAI rewrites a metadata file it uses streaming/temp-file/fsync/atomic-replace semantics appropriate to the platform and refuses when safe concurrency cannot be established.

### SI-016 Mutations require a sealed plan

Every mutation is derived from an immutable plan carrying artifact/root identity, policy explanation, authority, action class, reversibility, provider capability, and execution preconditions.

`SealedPlan` (`rust/crates/cancellai-safety/src/sealed_plan.rs`, E03-S02) implements
immutability by API shape: private fields, one constructor, no mutating methods. It does not
yet carry policy explanation or provider capability (E03-S02's scope is the identity/action/
authority/reversibility core; those two arrive with the policy and provider stories that
produce them).

### SI-017 Platform-native identity semantics

Unix inode/device assumptions are not applied to Windows reparse/file identity or other platforms without a verified mapping. Unsupported identity semantics lower authority.

### SI-018 Filesystem/volume boundaries are explicit

Recursive mutation and quarantine do not silently cross mounts, volumes, junction boundaries, or equivalent filesystem boundaries.

## Execution

### SI-019 One mutation boundary, evidence-gated

All filesystem/vendor mutations route through the safety executor. CR4 changes to this boundary require independent verification and owner-visible Safety Verdict.

### SI-020 Irreversible actions are explicit and stronger-gated

Purge/permanent vendor delete is represented separately from reversible/conditionally reversible actions and cannot be disguised as cleanup metadata.

## Provider and knowledge trust

### SI-021 Provider manifest trust bounds authority

Manifest-only/untrusted/community knowledge cannot self-assign a trust level or destructive capability above locally verified policy.

### SI-022 Knowledge is data, not executable authority

Remote/local knowledge bundles cannot inject arbitrary commands/code or raise local destructive authority. Invalid signatures/provenance are rejected.

### SI-023 Attribution uncertainty cannot become cleanup confidence

Uncertain project/session attribution remains explicit and does not become a stronger lifecycle or deletion claim simply to complete a view.

### SI-024 Persistent cache is never destructive truth

Cached inventory/current-state data may accelerate reads but mutation preconditions depend on fresh revalidation. Stale cache cannot authorize mutation.

## Policy and Guardian

### SI-025 Policy cannot override constitutional ceilings

Effective policy is monotonic under the precedence model. More specific configuration may narrow or select within authority; it cannot elevate above safety/artifact/provider/trust ceilings.

### SI-026 cancellAI reset/self-budget cannot target provider payload

Internal compaction/reset operations are restricted to cancellAI-owned state and cannot reuse provider-root deletion primitives.

### SI-027 Detection severity does not create authority

Pressure/anomaly/forecast state influences urgency and recommendation ordering but never increases mutation authority by itself.

### SI-028 Guardian cannot self-escalate

Guardian actions are equal to or weaker than the Effective Policy computed by the shared engine at action time.

## Supply chain and network

### SI-029 Knowledge rollback/tamper fails closed

Invalid, expired, replayed, or unauthorized knowledge updates are rejected or downgraded to non-destructive use; a bad update cannot brick basic offline inspection.

### SI-030 Release channel bounds default authority

Experimental/nightly builds do not inherit stable-level autonomous destructive defaults merely because user configuration exists from a stable install.

### SI-031 Remote controller cannot bypass target-node safety

Remote/fleet requests are intents. The target node independently authenticates, resolves policy, builds/revalidates plans, and retains final mutation authority.

## Safety proof style

For a CR4 story, verification should answer:

1. Which invariant(s) could the change violate?
2. What counterexample would prove failure?
3. Which automated tests reproduce those counterexamples?
4. Which residual risk cannot be eliminated and why?
5. What rollback/recovery exists if the assumption is wrong in production?

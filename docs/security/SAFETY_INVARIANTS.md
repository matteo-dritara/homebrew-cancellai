# Safety Invariants

These invariants are constitutional runtime properties. They have stable IDs so work items, tests, Safety Verdicts, and incident reports can reference the same rule. A story may add stronger constraints; it may not silently weaken these.

## Protection and authority

### SI-001 Protected/unknown state is non-destructive

Artifacts classified `PROTECTED`, `UNKNOWN`, or with insufficient safety confidence cannot receive destructive authority. User policy cannot override this floor.

Implemented at `rust/crates/cancellai-safety/src/authority.rs` (E03-S04):
`effective_authority` is a monotonic minimum over named constraints, one of which -
`constitutional_safety_floor` - restates this rule directly (`PROTECTED` protection or
`LowUnknown` confidence caps at `Recommend`, non-destructive) as an always-present input no
other constraint can raise past, regardless of user-requested authority.

### SI-002 Provider root must be positively bounded

Every mutation occurs under a validated provider root capability. Catastrophically broad, ambiguous, or low-confidence custom roots are non-destructive.

For `cancellai-cli`'s `configure` command specifically - a vendor settings-file write that
deliberately does not go through `ApprovedRoot` (see SI-019 below) -
`cancellai-sealedfs::SealedRoot::establish` (E07-S07 round-1 repair, ADR-0017) is the positively
bounding capability: it opens the root once with `O_NOFOLLOW` and retains that descriptor for
every subsequent operation, rather than re-checking and re-resolving the path each time.

E07-S07 round-2 independent verifier review found that round-1's `O_NOFOLLOW` bound only the
*final* path component: `establish`'s pre-check and its `OpenOptions::open(path)` both still
resolved every component *above* the leaf through the kernel's normal, link-following name
resolution, so a `$HOME` (or any intermediate directory) that was itself a symlink was silently
followed, and the real, non-symlink leaf it led to was then sealed and mutated as if it were the
approved root. E07-S09 closes this: `establish` walks every component handle-relatively from the
filesystem root (`/`, which cannot itself be a symlink) via `openat`/`O_NOFOLLOW`, refusing the
moment any component - intermediate or final - is a link, and creates only the final, absent
component via `mkdirat` against the already-held parent descriptor.

### SI-003 Mutation cannot escape or delete the approved root

No filesystem mutation may target outside its approved root or the root object itself, including via path normalization tricks or link indirection.

`cancellai-sealedfs::SealedRoot`'s child operations are issued via `openat`/`renameat` against
the retained root descriptor, using a validated bare-filename child name (no `/`, `.`/`..`) -
they cannot resolve outside the bound directory regardless of what its original path resolves
to by the time the operation runs.

### SI-004 Unknown provider layout/version reduces capability

Provider/version/layout drift cannot preserve destructive capabilities merely because the provider name is recognized.

### SI-005 Category expansion does not erase independent policy

Modes that broaden artifact categories do not silently disable retention, protection, activity, confidence, or authority constraints.

### SI-006 Protected-name/category barriers are defense in depth

Known protected provider state is checked both before planning and at execution. Scanner selectivity alone is never the only protection.

### SI-007 Ambiguous CLI/configuration is non-destructive

Destructive commands require explicit destructive intent. Parser ambiguity, missing subcommands, invalid policy, or incompatible flags never imply mutation.

Structurally supported (not fully closed) by `rust/crates/cancellai-safety/src/authority.rs`
(E03-S04): `user_authority` is one of several independent minimum constraints in
`effective_authority`, so even a CLI layer that mistakenly resolved ambiguity to a high
requested authority could not by itself grant destructive authority - the artifact-ceiling,
confidence, lifecycle, and constitutional-floor constraints still apply independently. Full
closure of this invariant is the CLI layer's own job (a future story, E06 Rust CLI Parity and
Cutover): refusing to resolve genuinely ambiguous input to *any* non-`Observe` `user_requested`
value in the first place.

### SI-008 Partial scan is non-destructive

A `PARTIAL` inventory scope cannot produce irreversible actions for artifacts whose safety depends on the missing information.

Implemented for the artifact-integrity axis at `rust/crates/cancellai-safety/src/authority.rs`
(E03-S04): `lifecycle_authority` collapses to `Recommend` (non-destructive) whenever
`IntegrityState` is `Partial`, `Corrupted`, or `Unknown`, independent of every other input.

E04-S03 (`rust/crates/cancellai-inventory/src/completeness.rs`) implements the inventory-scope
half of this invariant: `derive_completeness` classifies a scope `Partial` whenever any
directory-listing failure or per-file degraded observation exists beneath an otherwise
readable root, and `planning_view` is the *only* way to obtain planning-facing candidates -
it always returns them bundled with that `ScopeCompleteness` in one `PlanningView` struct,
so a caller cannot reach candidates without also seeing whether they were produced under a
`Partial` scope. Wiring `ScopeCompleteness::Partial`/`Unknown` into `KnowledgeConfidence` so
`authority.rs`'s existing `IntegrityState`-based collapse actually fires for these scopes is
E05/E06 scope (no classification stage exists yet to make that connection) - see this
story's evidence packet for the residual.

### SI-009 Unknown scan state is non-destructive

Missing evidence is not interpreted as absence of active/protected data.

Implemented at `rust/crates/cancellai-safety/src/authority.rs` (E03-S04): `lifecycle_authority`
also collapses to `Recommend` for `ActivityState::Unknown` and `IntegrityState::Unknown`
specifically - an unknown fact is never read as "safe to act on."

E04-S03 implements the inventory-scope evidence this depends on: `derive_completeness`
returns `ScopeCompleteness::Unknown` when a scope's own root could not be observed at all
(absent or unreadable) - the strongest form of missing evidence this model expresses,
reserved specifically for "we know essentially nothing about this scope" rather than
conflated with the more common `Partial` case (a readable root with some unreadable
descendant).

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
(E03-S02), consuming `cancellai-platform`'s `IdentityObserver` (E03-S01). Also implemented for
root identity: `mutation_executor::execute` (E03-S05) refuses unless `plan.root_identity()`
matches the actual target's bound root at execution time (E03 verifier review round 1 - see
SI-016 below). `cancellai-platform::mutation::MutationExecutor::mutate` (repaired in the same
round) additionally re-confirms a plain file's identity via an open file descriptor
immediately around the unlink syscall itself, narrowing (though, without an OS-specific
handle-relative unlink this workspace does not have, not perfectly closing) the residual
revalidate-then-delete race.

E07-S07 round-1 independent verifier review found the identical *shape* of race, but fully
closable this time, one layer up: `cancellai-cli::configure` re-checked `roots::is_symlink`
immediately before its own writes, and that re-check was still a separate syscall from the
path-based reads/writes that followed it, leaving a real window for a root-directory symlink
swap. Unlike the `MutationExecutor` file-unlink case above, `cancellai-sealedfs::SealedRoot`
(ADR-0017) closes this one completely rather than narrowing it: it retains an
`O_NOFOLLOW`-opened directory descriptor across every operation and issues them via
`openat`/`renameat` against that descriptor, so identity is not merely revalidated immediately
before mutation but bound for the mutation's entire duration - no path re-resolution ever
happens again after `establish` returns.

E07-S09 extends this to every component `establish` itself walks to reach that final
descriptor, not only the descriptor it ends with: each intermediate directory is opened via
`openat`/`O_NOFOLLOW` against the descriptor already held for its own parent, so an
intermediate symlink is refused at the instant it is reached rather than silently resolved by
the initial path-based lookup that used to precede the final `O_NOFOLLOW` open.

### SI-014 Safety-blocked/partial is not success

Automation receives a distinct non-zero or structured status when requested mutation was materially blocked/skipped for safety or incomplete execution.

### SI-015 Atomic metadata rewrite preserves concurrent correctness

When cancellAI rewrites a metadata file it uses streaming/temp-file/fsync/atomic-replace semantics appropriate to the platform and refuses when safe concurrency cannot be established.

### SI-016 Mutations require a sealed plan

Every mutation is derived from an immutable plan carrying artifact/root identity, policy explanation, authority, action class, reversibility, provider capability, and execution preconditions.

`SealedPlan` (`rust/crates/cancellai-safety/src/sealed_plan.rs`, E03-S02) implements
immutability by API shape: private fields, no mutating methods, and (E03 verifier review
round 1 repair) exactly one *public* constructor, `SealedPlan::seal`, which derives
`root_identity`/`artifact_identity` from a real `ApprovedRoot`/`BoundedPath` pair rather than
accepting independent caller-supplied values a plan could be sealed against a root/target
pair that were never actually bound together. It does not yet carry policy explanation or
provider capability (E03-S02's scope is the identity/action/authority/reversibility core;
those two arrive with the policy and provider stories that produce them) - this is a
deliberate, documented scope boundary, not a silent omission (see `sealed_plan.rs`'s own
module doc comment and `docs/architecture/DOMAIN_MODEL.md`'s "SealedPlan" section).

### SI-017 Platform-native identity semantics

Unix inode/device assumptions are not applied to Windows reparse/file identity or other platforms without a verified mapping. Unsupported identity semantics lower authority.

### SI-018 Filesystem/volume boundaries are explicit

Recursive mutation and quarantine do not silently cross mounts, volumes, junction boundaries, or equivalent filesystem boundaries.

## Execution

### SI-019 One mutation boundary, evidence-gated

All filesystem/vendor mutations route through the safety executor. CR4 changes to this boundary require independent verification and owner-visible Safety Verdict.

Implemented at `rust/crates/cancellai-safety/src/mutation_executor.rs::execute` (E03-S05),
the sole production caller of `cancellai-platform::mutation::MutationExecutor`.
`scripts/check_mutation_boundary.py` statically enforces that the raw OS primitive and the
capability wrapping it are referenced only from those two files - E03 verifier review round 1
found the capability itself was `pub`, re-exported at `cancellai_platform`'s crate root, and
directly callable (with an unconstrained raw path) by any crate that imported it; repaired by
removing the re-export and extending the static check (`docs/architecture/TARGET.md`,
`docs/architecture/PLATFORM_MODEL.md`).

### SI-020 Irreversible actions are explicit and stronger-gated

Purge/permanent vendor delete is represented separately from reversible/conditionally reversible actions and cannot be disguised as cleanup metadata.

`mutation_executor::execute` (E03-S05) enforces this directly: `authority.rs`'s
`minimum_authority_for(ActionClass::Delete)` requires `AuthorityLevel::Govern` (strictly above
`Quarantine`'s requirement), and `reversibility_allowed` refuses a `Delete` action class
unless the plan's own recorded `Reversibility` is `Irreversible` - a plan claiming
`Reversibility::Quarantinable` while carrying `ActionClass::Delete` is refused outright, not
executed as a disguised irreversible deletion (E03 verifier review round 1 found `execute`
originally checked neither authority nor reversibility at all).

## Provider and knowledge trust

### SI-021 Provider manifest trust bounds authority

Manifest-only/untrusted/community knowledge cannot self-assign a trust level or destructive capability above locally verified policy.

Implemented at `rust/crates/cancellai-safety/src/authority.rs` and
`rust/crates/cancellai-safety/src/trust_promotion.rs` (E05-S02, repaired after E05 verifier
review round 1): `effective_authority`'s `provider_trust_authority` constraint caps the
monotonic-minimum result by tier (`Untrusted` at `Observe`, `LocalCustom` at `Quarantine`,
`CommunityVerified` at `Govern`, `BuiltinVerified` at `Autopilot`, `docs/PROVIDERS.md` "Trust
levels"). `AuthorityInputs::provider_trust` accepts only `TrustedTier` - an opaque wrapper
around `cancellai_model::ProviderTrust` with a private field and no `From<ProviderTrust>` - not
the bare, freely-constructible `ProviderTrust` enum itself; `TrustedTier`'s only public
constructors are `untrusted()` (the safe, evidence-free default) and a checked `promote()`
requiring a non-empty named verifier and at least one fixture reference, refusing anything that
is not a strict upgrade, fail-closed. Round 1 found the first version of this invariant's
implementation typed `AuthorityInputs::provider_trust` as bare `ProviderTrust`, so an external
caller could construct `ProviderTrust::BuiltinVerified` directly and reach `Autopilot` with no
promotion evidence at all - `promote` existed and worked correctly in isolation, but nothing
forced a caller through it. `TrustedTier` closes that gap by making the type itself
unconstructible outside the gate, proven by a `compile_fail` doctest on `TrustedTier` (the
exact round-1 reproduction, restated) and enforced by `cargo test` for every `pub` surface in
`cancellai-safety`. No other code path in the workspace reads a trust claim out of a manifest
and treats it as authoritative.

### SI-022 Knowledge is data, not executable authority

Remote/local knowledge bundles cannot inject arbitrary commands/code or raise local destructive authority. Invalid signatures/provenance are rejected.

Partially implemented at `rust/crates/cancellai-safety/src/trust_promotion.rs` (E05-S02):
`TrustPromotionEvidence` carries only inert strings (a verifier name, fixture reference
identifiers) with no command/code field for anything to execute, and raising authority through
it requires passing through `TrustedTier::promote`'s fail-closed checks - the only public path
from which a `TrustedTier` above `Untrusted` can be obtained (see SI-021 above for the round-1
repair that made this actually true, not merely intended). Signature/provenance verification
for a distributed knowledge bundle is a later story (E16 Provider Ecosystem and Federated
Knowledge) - nothing that verifies a bundle's signature exists yet.

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

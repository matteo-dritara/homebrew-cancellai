# ADR-0037: Provider layout is revalidated fresh immediately before mutation

- Status: Accepted
- Date: 2026-09-22
- Owners: owner (Matteo Pugliese)
- Related: E14-S05, SI-004, SI-013, ADR-0034, ADR-0035, ADR-0036,
  `docs/architecture/GUARDIAN_MODEL.md`, `docs/architecture/PLATFORM_MODEL.md`,
  `docs/architecture/DOMAIN_MODEL.md`, `docs/security/SAFETY_INVARIANTS.md`

## Context

ADR-0036 (E14-S04, round 5) closed four independent review rounds' worth of forgeability
defects and produced a non-forgeable primitive - `cancellai_platform::BoundLayoutObservation`
and `cancellai_safety::resolve_provider_execution_authority` - but explicitly disclosed two
residuals as future, separately-scoped work: (1) no trusted source of "known-recognized"
layouts exists yet, and (2) **no production call site mints a permit, and the mutation
boundary does not require one**. E14-S05 closes residual (2) only; residual (1) remains open
and out of scope here, unchanged.

Closing residual (2) literally - wiring `resolve_provider_execution_authority`'s
`ProviderExecutionPermit` into `mutation_executor::execute` - is not available without also
touching residual (1): that function requires a `known_signatures` argument with no trusted
source, and every current call resolves to `AuthorityLevel::Observe` for the layout
constraint regardless of the real layout, which would collapse every real destructive
mutation's authority to below `minimum_authority_for(ActionClass::Quarantine)` - refusing
every provider-governed action outright, not a scoped test of freshness.

## Decision

E14-S05 closes a narrower, independently valuable property instead: **a plan's own
provider-layout snapshot, taken at seal time, must still match a fresh observation taken
immediately before mutation** - the same principle `SealedPlan::seal`/`revalidate` already
apply to `artifact_identity` (SI-013), extended to the provider root's own structural layout
for the first time. This is a live-vs-stale TOCTOU check, not a "known good" classification;
it does not require `known_signatures` and does not touch ADR-0036's residual (1).

`SealedPlan` gains a new field, `provider_layout: Option<LayoutSignature>`, populated inside
every `seal*` constructor from a real `cancellai_platform::provider_layout::
ProviderLayoutObserver` (`observe(root.path())`) supplied by the caller - never a
caller-suppliable pre-built value, mirroring how `root_identity`/`artifact_identity` are
already derived internally rather than accepted as bare fields (the exact defect ADR-0034
through ADR-0036 spent four rounds closing one layer down). `None` records that no platform
capability could observe it at seal time.

`cancellai_safety::sealed_plan::revalidate_provider_layout` (new, alongside the existing
`revalidate`) compares a fresh observation, taken by `mutation_executor::execute` immediately
before any mutation (after every existing check, right before the operation is built), to
that seal-time snapshot:

- fresh observation `Err` (unobservable root) -> refuse;
- fresh root identity differs from `plan.root_identity()` -> refuse (this also closes a
  latent, unrelated gap: `execute` previously never re-observed the *root's* identity fresh
  at all, only the target's - only the plan's own stale, seal-time root identity was ever
  compared against `target`'s own stale, bind-time root identity);
- fresh markers, normalized into `LayoutSignature`, differ from the plan's recorded signature
  -> refuse;
- plan recorded `None` (no seal-time baseline) -> refuse, never an unconstrained default;
- otherwise -> proceed.

`cancellai_platform::provider_layout::ProviderLayoutObserver` is a new capability seam
(`SystemProviderLayoutObserver`/`SyntheticProviderLayoutObserver`, mirroring
`IdentityObserver`/`ProcessObserver`). `execute_with_system_capabilities` hardcodes
`SystemProviderLayoutObserver` - no production caller can substitute a synthetic observation,
the same closed-production-path guarantee the other three injected capabilities already have.
`BoundedPath` gains a `root_path` field (populated by `ApprovedRoot::bind` from the root it
was already holding), so `execute` can locate what to re-observe without accepting a bare
path from its caller.

## Alternatives considered

### Wire `resolve_provider_execution_authority`/`ProviderExecutionPermit` directly

Rejected for this round: it requires `known_signatures` (ADR-0036 residual (1), explicitly
out of scope), and with no trusted source, every call would resolve to `AuthorityLevel::Observe`
- collapsing every real Delete/Quarantine below their required minimum authority regardless of
whether anything actually drifted. That is a correct behavior once a trusted signature source
exists; today it would make this story indistinguishable from "refuse every provider-governed
mutation," which is not what a freshness/TOCTOU story should do. A future story wiring a
trusted provider-layout-manifest source is the natural point to switch the mutation boundary
from this ADR's live-vs-stale check to a live-vs-known-good permit.

### Skip the root-identity comparison, only compare markers

Rejected: markers alone cannot distinguish "the same root, unchanged" from "a different real
object at the same path with coincidentally identical top-level names." The root-identity
comparison is nearly free (already read by the same observation) and closes a real,
independently-found gap (`execute` never freshly re-observed root identity at all before this
story).

## Consequences

### Positive

- Closes ADR-0036 residual (2): the mutation boundary now requires a live provider-layout
  observation for every destructive plan, and a plan with no live-vs-stale match cannot
  execute merely because its own seal-time data still claims to be fine.
- Closes a second, previously-undisclosed gap as a side effect: `execute` had never freshly
  re-observed the *root's* identity before mutating, only the target's - this story's fresh
  root-identity comparison inside `revalidate_provider_layout` is the first place that happens.

### Negative / cost

- **`cancellai_platform::BoundLayoutObservation::observe` only succeeds on Unix today**
  (ADR-0036 round 5's own disclosed residual: `cancellai-sealedfs::SealedRoot::metadata`/
  `list_child_names` fail closed with `Unsupported` on every other platform). Because this
  story's check fails closed on every unobservable-root outcome, **every real, destructive
  mutation through `mutation_executor::execute` - Delete included - is now refused on
  non-Unix platforms**, including Windows, where a real, verified Delete pipeline already
  shipped (E20-S01/E20-S05). This is a genuine, deliberate narrowing of already-shipped
  capability, not merely "a capability that was never implemented" (contrast
  `cancellai-sealedfs`'s pre-existing Quarantine refusal on Windows, which never shipped in
  the first place). It is accepted here on the same fail-closed principle this codebase
  already applies everywhere else a safety-critical observation has no verified non-Unix
  implementation (SI-017: "a plausible-but-unverified implementation... is a worse outcome
  than an honest refusal") - a real Windows implementation of the same handle-bound
  observation this story's Unix path already uses is separately-scoped future work
  (`cancellai-sealedfs::windows_sealed`, mirroring `establish`'s existing Windows walk).
- `SealedPlan::seal`/`seal_with_process_guard`/`seal_quarantine`/`seal_restore`/
  `seal_archive`, and `mutation_executor::execute`/`execute_all`, each gain one required
  parameter - every existing production and test call site was updated in this change.

### Neutral / follow-up

- ADR-0036 residual (1) (a trusted provider-layout-manifest source) remains open, unchanged,
  and is the natural next story to also let this mechanism recognize "known good" layouts
  rather than only "unchanged since seal."
- A real Windows implementation of `SealedRoot::metadata`/`list_child_names` (or an equivalent
  handle-bound NT-native observation) is required before Windows Delete regains real, working
  status - tracked as a residual, not silently deferred.

## Safety and compatibility impact

- Change Risk implication: CR4 - modifies the sole mutation boundary (`cancellai_safety::
  mutation_executor::execute`) and the sealed-plan contract every mutating action flows
  through.
- Safety Invariants affected: SI-004 (unknown/drifted provider layout reduces capability -
  now enforced with a live, immediately-pre-mutation observation rather than none at all),
  SI-013 (identity revalidated immediately before mutation - extended from the target
  artifact to the provider root itself, for both identity and structural layout).
- Migration/rollback: no persisted data format changes. Windows/non-Unix Delete becomes
  unavailable until a verified Windows layout-observation implementation lands - this is the
  reviewable, disclosed consequence an independent CR4 Safety Verdict must weigh explicitly,
  not a silent regression.

## Supersession

None yet.

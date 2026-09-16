<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E12-S01
Rendered-by: Claude (executor handoff to Codex)
Rendered-on: 2026-09-16
Brief-Checksum: eae5c25c2224707972f6731bae2035dcd9f16e291ce9bd7e5f41c76e3ecce734

<!-- end handoff header -->
# Verifier Brief - E12-S01 - Quarantine store semantics

Status: ready_for_review | Change Risk: CR4
Outcome: Define same-volume move-first quarantine with explicit capacity and boundary checks.
Dependencies: E11-S02, E03-S05

## Acceptance Criteria
- Quarantine never copies huge data when atomic/same-volume move is required by policy.
- Insufficient space or cross-volume uncertainty fails closed.
- Original identity and restore metadata are recorded contentlessly.

## Verification Contract
- Cross-volume, full-disk, rename-failure, and crash-injection tests.

## Safety Obligations

### SI-018 Filesystem/volume boundaries are explicit

Recursive mutation and quarantine do not silently cross mounts, volumes, junction boundaries, or equivalent filesystem boundaries.

`cancellai-safety::root_capability::ApprovedRoot::bind` implements the check
(`IdentityToken::device()`, `rust/crates/cancellai-platform/src/identity.rs`). E20-S01
(ADR-0020) extended `device()` with a real Windows arm (the volume serial number, widened to
`u64`) alongside the existing Unix device number, so this one comparison enforces the boundary
on both platforms without platform-specific branching at the call site. `cancellai-sealedfs::
SealedRoot`'s separate no-follow root-*establishment* walk (used by `configure` and `clean`'s
default-root re-check, `docs/architecture/PLATFORM_MODEL.md`'s "Boundary rules") remains
`Unsupported` on Windows - a distinct, still-open residual, not something this boundary-check
extension closes.

## Execution

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

## Documentation Impact
- docs/architecture/PERSISTENCE_MODEL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

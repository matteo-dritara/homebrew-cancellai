<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E16-S08
Rendered-by: Claude (executor)
Rendered-on: 2026-09-23
Brief-Checksum: 1c3fa48786d70231c07a865f709041d444e19248af8a1139ca7d462b3979b308

<!-- end handoff header -->
# Verifier Brief - E16-S08 - A trust promotion's fixture evidence must exist inside the repository

Status: ready_for_review | Change Risk: CR3
Outcome: Close the residual the E16 round-1 independent review recorded against E16-S06: `scripts/check_provider_trust.py` accepted a `builtin_verified` registry entry whose `fixture_references` named a path that does not exist, because it checked that evidence was listed, not that it was there. A promotion above Untrusted now names fixtures that resolve to real files or directories inside the repository, so a reference can be neither invented nor pointed outside the reviewed tree.
Dependencies: E16-S06

## Acceptance Criteria
- If a registry entry above Untrusted names a fixture reference that does not exist in the repository, then the provider trust check shall fail and name that reference.
- If a fixture reference is absolute, contains a parent-directory component, or resolves outside the repository (including through a symbolic link), then the provider trust check shall fail.
- When every fixture reference of a promoted entry resolves to an existing path inside the repository, the provider trust check shall accept the entry.
- The provider trust check shall remain read-only and shall not execute the referenced fixtures.

## Verification Contract
- Synthetic registries against a temporary repository root: nonexistent, absolute, parent-escaping and symlink-escaping references are refused; an existing in-tree fixture file and directory are accepted.
- The real committed registry still passes.

## Safety Obligations

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

## Documentation Impact
- .github/CONTRIBUTING.md
- docs/development/ENGINEERING_SYSTEM.md
- CHANGELOG.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

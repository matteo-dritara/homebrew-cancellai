<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E17-S07
Rendered-by: Claude (executor)
Rendered-on: 2026-09-23
Brief-Checksum: 255d8f22ed2d1d30165dd673fdbc2d6d045646d1ee799f5a416669efb31c62ea

<!-- end handoff header -->
# Verifier Brief - E17-S07 - Safety incident containment and capability downgrade

Status: ready_for_review | Change Risk: CR4
Outcome: Operationalize a least-authority incident path that can stop promotion and remotely distribute signed capability downgrades without introducing a remote destructive control plane.
Dependencies: E16-S05, E17-S03, E17-S05

## Acceptance Criteria
- A compromised provider/version/capability can be downgraded to Observe or Recommend through trusted signed knowledge without granting any new mutation authority.
- Offline operation remains available under the installed local safety kernel when the knowledge service is unavailable.
- Incident evidence records affected release/knowledge provenance, capability, provider, platform, and invariant references without collecting provider payload content.

## Verification Contract
- Replay, rollback, invalid-signature, unavailable-network, and attempted authority-elevation incident simulations.
- Independent CR4 Safety Verdict.

## Safety Obligations

### SI-022 Knowledge is data, not executable authority

Remote/local knowledge bundles cannot inject arbitrary commands/code or raise local destructive authority. Invalid signatures/provenance are rejected.

Partially implemented at `rust/crates/cancellai-safety/src/trust_promotion.rs` (E05-S02):
`TrustPromotionEvidence` carries only inert strings (a verifier name, fixture reference
identifiers) with no command/code field for anything to execute, and raising authority through
it requires passing through `TrustedTier::promote`'s fail-closed checks - the only public path
from which a `TrustedTier` above `Untrusted` can be obtained (see SI-021 above for the round-1
repair that made this actually true, not merely intended). E16-S02 implemented bundle signature/provenance verification in
`rust/crates/cancellai-safety/src/knowledge_bundle.rs`. E17-S08 independent review found that
its call used permissive verification despite ADR-0024 requiring `verify_strict`; the repair
uses strict Ed25519 verification and rejects weak local-policy keys/signatures before a bundle
can replace the current store. A regression proves refusal preserves the full current value.

### SI-029 Knowledge rollback/tamper fails closed

Invalid, expired, replayed, or unauthorized knowledge updates are rejected or downgraded to non-destructive use; a bad update cannot brick basic offline inspection.

Implemented for bundles at `rust/crates/cancellai-safety/src/knowledge_bundle.rs` (E16-S02) and
for incident containment at `rust/crates/cancellai-safety/src/incident.rs` (E17-S07): a refused
containment update - unreachable service, malformed text, bad signature, unknown publisher,
expiry, replay - leaves the containment ledger exactly as it was, and no refusal, rollback or
expiry can remove a containment already recorded. Only a local lift does.

### SI-030 Release channel bounds default authority

Experimental/nightly builds do not inherit stable-level autonomous destructive defaults merely because user configuration exists from a stable install.

Implemented at `rust/crates/cancellai-safety/src/authority.rs` and
`rust/crates/cancellai-safety/src/build_channel.rs` (E17-S05, [ADR-0023](../../../docs/adrs/0023-release-channel-authority-as-opt-in-function.md)):
`effective_authority_for_channel`'s `release_channel_authority` constraint caps the
monotonic-minimum result by channel (`Nightly` at `Recommend` - strictly below what
`ActionClass::Quarantine` requires, let alone `Delete` - `Beta` at `Govern`, `Stable` with no
additional cap; `docs/security/SUPPLY_CHAIN.md`'s "Release channels" table). It accepts only
`BuildChannel` - an opaque wrapper around `cancellai_model::ReleaseChannel` with a private field
and no `From<ReleaseChannel>` - not the bare, freely-constructible `ReleaseChannel` enum itself,
mirroring SI-021's `TrustedTier` split for the identical reason. `BuildChannel`'s only
production constructor, `from_compiled_env`, reads `CANCELLAI_CHANNEL` via `option_env!` at the
moment `cancellai-safety` is compiled - baked permanently into the resulting binary, never a
runtime value a user running that binary could set to claim a higher channel than it actually
is; an absent or unrecognized value resolves to `Nightly`, the fail-closed floor, matching
`BuildChannel::default()` exactly. `.github/workflows/release.yml`'s `build-artifacts` job
(E17-S02) sets `CANCELLAI_CHANNEL=stable` for every canonical tier-1 build.

`effective_authority_for_channel` is a second function alongside the pre-existing
`effective_authority`, not a new required field on `AuthorityInputs` - ADR-0023 records why
(the one production caller of `AuthorityInputs`, `cancellai-policy::retention::
reachable_authority`, predates release-channel awareness and has no honest channel value to
supply yet, since `cancellai-cli` remains a beta, source-built artifact with no packaged
release). No caller in this workspace invokes `effective_authority_for_channel` yet - this
constraint is implemented and adversarially tested (`cancellai-safety::authority`'s own test
suite) but not yet enforced on any real mutation path. Wiring `cancellai-cli`'s classification
pipeline to it is deferred to E06-S04, the cutover story that makes `cancellai-cli` the
canonical, packaged-release engine.

## Documentation Impact
- docs/security/INCIDENT_RESPONSE.md
- docs/security/SUPPLY_CHAIN.md
- docs/development/RELEASE_GATES.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

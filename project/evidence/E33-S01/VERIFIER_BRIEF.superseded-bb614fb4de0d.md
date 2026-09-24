<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E33-S01
Rendered-by: Claude
Rendered-on: 2026-09-24
Brief-Checksum: bb614fb4de0d31b814e7497488c4e83eac0c298b389fc899b433a3d92b1854a9

<!-- end handoff header -->
# Verifier Brief - E33-S01 - Signed containment notices are fetched from a published feed

Status: ready_for_review | Change Risk: CR4
Outcome: Fetch signed containment notices from a published feed and ingest them through the same path as containment install, so an incident reaches installations without a manual step, while an unreachable, oversized, stale or forged feed leaves the ledger exactly as it was. Owner decisions (2026-09-23): fetch with the system curl, no HTTP client in the binary; publish one cumulative signed notice as containment/notice.json in the canonical repository, fetched by an explicit containment refresh.
Dependencies: E06-S07

## Acceptance Criteria
- When the feed is reachable, the system shall ingest every new notice through the same verification and persistence path as containment install.
- If the feed is unreachable, oversized, malformed, replayed, rolled back or signed by an untrusted publisher, then the ledger shall stay byte-identical and authority shall keep being computed from local state.
- The system shall never lift, narrow or loosen a containment from feed content.

## Verification Contract
- Adversarial tests against a local test feed cover every refusal case, including a response larger than the byte cap.

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

Since E06-S07 the ledger persists as a history of raw signed bundles and local lifts that the
kernel re-verifies on every load, skipping only the expiry check (expiry never lifts). The
guarantee covers every input from outside the owner's account. It does not cover the owner's own
state: a process running as the same user can delete, truncate or append to the history, and
each of those lifts containment - an accepted limit (owner decision, 2026-09-23;
`docs/security/INCIDENT_RESPONSE.md`), not a gap in the verification.

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

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

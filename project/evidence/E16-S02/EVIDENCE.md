# Evidence Packet - E16-S02

- Commit/PR: this working-tree change (executor session, 2026-09-09)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md) - CR4, requires an
  independent Safety Verdict per AGENTS.md; not written by the executor
- Change Risk: CR4
- Spec version/commit: `project/epics/E16.json` (E16-S02), as of this change

## Outcome

PASS

`cancellai-safety::knowledge_bundle` (new module) packages provider/version/layout
intelligence separately from the binary as a signed `KnowledgeBundle`, verified with Ed25519
against a caller-supplied `LocalTrustPolicy` ([ADR-0024](../../../docs/adrs/0024-ed25519-dalek-for-knowledge-bundle-signatures.md)
records the dependency decision). `KnowledgeStore` adds apply/rollback with anti-replay.

## Design summary

- `KnowledgeBundle { schema_version, publisher_id, sequence, issued_at, expires_at,
  content_digest, payload, signature }` - `#[serde(deny_unknown_fields)]`, every field required.
  `payload` is opaque to this module (typically a `cancellai-provider-api::ProviderManifest`'s
  JSON text, though this story does not wire that connection - see Residual risks).
- `parse_bundle` is the sole entry point from untrusted bytes; a bundle missing `signature`
  entirely (the literal "unsigned" case) fails here with a named-field diagnostic, before
  `verify_bundle` is ever reached.
- `verify_bundle(bundle, policy: &LocalTrustPolicy, now_unix) -> Result<VerifiedKnowledgeBundle,
  KnowledgeBundleError>` checks, in order: schema version support, publisher lookup in
  `policy` (unknown-signer), SHA-256 content digest, Ed25519 signature over a length-prefixed
  encoding of every other field (`KnowledgeBundle::signing_bytes` - length-prefixed, not
  delimiter-joined, so no field value can be crafted to make two different logical bundles sign
  identically), then expiry.
- `VerifiedKnowledgeBundle::tier` is always `policy`'s own assignment for `publisher_id` -
  `KnowledgeBundle` has no field a publisher could set to claim a tier; there is nothing for a
  malicious or compromised bundle to elevate.
- `KnowledgeStore::apply` refuses a bundle whose `sequence` does not strictly exceed the
  currently installed bundle's from the *same* publisher (replay/rollback-of-trust protection),
  scoped per publisher so a first bundle from a newly trusted publisher is never "stale"
  relative to an unrelated publisher's numbering.
- `KnowledgeStore::rollback` reinstates the bundle a later one replaced, re-checking its expiry
  against the current clock - a failed rollback (no prior bundle, or the prior bundle has since
  expired) never disturbs the still-current bundle.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Unsigned/invalid bundles are ignored with diagnostics | `ac1_a_bundle_missing_the_signature_field_entirely_fails_to_parse` (literal unsigned case, diagnostic names the missing field), `ac1_an_unparseable_bundle_never_reaches_verification`; `tamper_*` (three tests: payload-only edit, payload+digest edit, bit-flipped signature) and `unknown_signer_*` (two tests: absent publisher id, correct id signed by the wrong key) each prove a specific `KnowledgeBundleError` variant, not a silent pass | PASS |
| AC2 - Knowledge update cannot raise authority beyond local trust policy | `ac2_tier_comes_from_local_policy_never_from_the_bundle` proves `VerifiedKnowledgeBundle::tier` equals exactly what `LocalTrustPolicy` assigned - `KnowledgeBundle`'s own definition (visible in the same file) has no field that could have asserted a different one; this is a structural guarantee (the wire format cannot express the claim), the same class of proof `cancellai-provider-api::manifest`'s AC1 already uses for capability/trust claims, independently re-derived here since `cancellai-safety` cannot depend on that crate (`docs/architecture/TARGET.md`) | PASS |
| AC3 - Rollback to prior trusted bundle is supported | `rollback_ac3_a_replaced_bundle_can_be_restored`, `rollback_fails_closed_with_no_prior_bundle` (failed rollback leaves `current` untouched), `rollback_refuses_to_reinstate_a_bundle_that_has_since_expired` (rollback re-checks expiry rather than blindly trusting a previously-verified state) | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-022 (knowledge is data, not executable authority) | A bundle cannot inject code (payload is an opaque string this module never executes or interprets) or claim a trust tier it was not locally assigned | `ac2_tier_comes_from_local_policy_never_from_the_bundle`; module doc's own AC2 note | PASS |
| SI-029 (rollback/tamper fails closed) | Tampered payload, tampered payload+digest pair, bit-flipped signature, replayed/stale sequence, unknown signer, expired bundle, rollback with no prior bundle, rollback to an since-expired prior bundle | `tamper_*`, `replay_*`, `expiry_*`, `unknown_signer_*`, `rollback_*` (11 tests total covering every named failure class in the story's own verification contract: "Tamper, rollback, expiry, and unknown-signer tests") | PASS |

CR4 note: this evidence packet is the executor's submission. Per AGENTS.md, the executor does
not write its own CR4 Safety Verdict - `project/templates/SAFETY_VERDICT.md` is filled in by
the independent verifier (Codex) during epic-scoped review, not here.

## Verification Commands

```text
cd rust
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
cd ..
python3 scripts/check_rust_workspace.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/check_provider_compatibility.py check
python3 scripts/rust_python_parity.py self-test
python3 scripts/rust_python_parity.py check
python3 scripts/check_platforms.py check
python3 scripts/check_docs.py check
python3 scripts/check_process.py check
```

All PASS locally. `cargo test -p cancellai-safety`: 99/99 (24 new in `knowledge_bundle.rs`).
Full workspace `cargo test --workspace`: every crate green. `cargo deny check`: `advisories ok,
bans ok, licenses ok, sources ok` with `ed25519-dalek`/`sha2` added (see ADR-0024 for the full
dependency-tree/license accounting).

## Compatibility

- No existing public API changed; this story adds one new module
  (`cancellai-safety::knowledge_bundle`), its public re-exports at the crate root, and two new
  dependencies (`ed25519-dalek` verification-only, `sha2`) scoped to `cancellai-safety` alone.

## Documentation updated

- `docs/security/SUPPLY_CHAIN.md` - "Knowledge updates" section now names the real
  implementation, its guarantees, and what remains deferred.
- `docs/adrs/0024-ed25519-dalek-for-knowledge-bundle-signatures.md` - new ADR for the kernel-ring
  dependency addition (`AGENTS.md`'s kernel-ring rule: "a dependency requires a dedicated,
  reviewed ADR naming the specific capability `std` cannot express").
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- **Not wired to any real payload interpreter or distribution channel.** This story delivers
  the bundle format and its verification/rollback machinery, adversarially tested in isolation.
  Connecting a verified bundle's `payload` to `cancellai-provider-api::parse_manifest`, and
  actually fetching/distributing bundles, is deferred to a future story - `docs/security/
  SUPPLY_CHAIN.md`'s own updated text discloses this explicitly rather than implying a running
  update mechanism exists today.
- **`LocalTrustPolicy` has no persistence or CLI surface yet** - it is constructed in-process
  by whatever future caller wires this in; there is no config file format, no `cancellai
  knowledge trust` command, and no default policy shipped. This story's scope is the
  verification primitive, not its operational surface.
- **No provenance/timestamp-authority beyond the bundle's own self-reported `issued_at`.** A
  publisher's compromised signing key could still backdate `issued_at`/`sequence` arbitrarily
  within a single bundle (the signature only proves the *content* is what the key signed, not
  that the claimed timestamp is truthful) - detecting that class of compromise is a key-rotation/
  revocation concern outside a single bundle's own verification, consistent with
  `docs/security/SUPPLY_CHAIN.md`'s existing incident-containment model (E17-S07) rather than
  this story's format-level scope.
- **`cancellai-safety` gained its first non-workspace, non-`serde` runtime dependencies.**
  ADR-0024 records why (verification-only feature set, pure-Rust, BSD-3-Clause, MSRV-matching,
  `cargo deny check` clean) and what was rejected (`ring`, hand-rolled verification). Accepted
  as the cost of a real cryptographic capability `std` cannot provide, scoped as narrowly as the
  need allows.
- **This is a CR4 change without an independent Safety Verdict yet** - by design (see the
  Safety Evidence section's CR4 note); `ready_for_review` is this executor's correct terminal
  state, not `verification`/`done`.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`; CR4 Safety Verdict to
be produced independently by Codex, not self-attested here)

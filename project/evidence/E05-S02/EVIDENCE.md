# Evidence Packet - E05-S02

- Commit/PR: round 1 `5d62a00..44c175b`; round 2 repair, this commit
- Executor: Claude
- Independent verifier: Codex - round 1 verdict `FAIL` (`project/evidence/E05-VERIFIER-REVIEW.md`,
  `project/evidence/E05-S02/SAFETY_VERDICT.md`)
- Change Risk: CR4
- Spec version/commit: `rust/crates/cancellai-model/src/vocabulary.rs` (`ProviderTrust`,
  unchanged by the repair), `rust/crates/cancellai-safety/src/authority.rs`
  (`provider_trust_ceiling`, `AuthorityInputs::provider_trust` now `TrustedTier`),
  `rust/crates/cancellai-safety/src/trust_promotion.rs` (`TrustedTier` added; `promote` made
  private) as changed in the round 2 repair

## Round 2 repair (E05 verifier review round 1: FAIL)

Codex's round 1 review reproduced this exact bypass against the round 1 code:

```rust
let authority = effective_authority(AuthorityInputs {
    user_requested: AuthorityLevel::Autopilot,
    artifact_ceiling: AuthorityLevel::Autopilot,
    confidence: KnowledgeConfidence::Verified,
    activity: ActivityState::Orphaned,
    protection: ProtectionState::Normal,
    integrity: IntegrityState::Healthy,
    provider_trust: ProviderTrust::BuiltinVerified, // no promote() call, no evidence
});
assert_eq!(authority.level, AuthorityLevel::Autopilot);
```

`AuthorityInputs::provider_trust` was typed as the bare, publicly-constructible
`cancellai_model::ProviderTrust` enum. `trust_promotion::promote` existed and enforced its
checks correctly *when called*, but nothing in the type system required a caller to call it -
exactly SI-021's "cannot self-assign a trust level" case, undetected by round 1's own tests
because every one of them called `promote` (or the free function it wrapped) rather than
exercising the public API the way an external, adversarial caller would.

**Repair:** `TrustedTier` (`trust_promotion.rs`), an opaque wrapper around `ProviderTrust` with
a private field and no `From<ProviderTrust>` conversion. Its only public constructors are
`TrustedTier::untrusted()` (the safe, evidence-free default) and `TrustedTier::promote()` (the
checked gate, delegating to what is now a *private* free `promote` function).
`AuthorityInputs::provider_trust` is now typed `TrustedTier`, not `ProviderTrust` - the
reproduction above no longer compiles (`ProviderTrust::BuiltinVerified` is not a `TrustedTier`,
and there is no conversion). A `compile_fail` doctest on `TrustedTier` in `trust_promotion.rs`
restates this exact reproduction as a permanent regression test, and a runtime test
(`si021_the_verifier_round1_reproduction_is_no_longer_reachable_through_trusted_tier`) documents
the same property for `cargo test` runs that do not execute doctests. This mirrors the pattern
`cancellai-safety::SealedPlan` already used to close an analogous "correct logic existed, but
nothing forced a caller through it" gap in E03 round 1 (private fields, no mutating methods,
exactly one public constructor for the checked path).

`mutation_executor::execute` still consumes authority recorded directly on a `SealedPlan`
rather than a live `effective_authority` call - the round 1 review's safety verdict also named
this. This is not unique to provider trust: `SealedPlan` does not yet consume *any* of
`effective_authority`'s constraints (confidence, lifecycle, artifact ceiling included) - that
full PLAN-stage wiring is `cancellai-safety::SealedPlan`'s own documented, pre-existing scope
boundary (`docs/architecture/DOMAIN_MODEL.md` "SealedPlan": "does not yet carry policy
explanation or provider capability... a deliberate, documented scope boundary", set in E03-S02,
restated unchanged through E04-S03's `ScopeCompleteness` residual). Building that pipeline for
provider trust alone, ahead of the PLAN-stage builder that will eventually call
`effective_authority` and construct `SealedPlan` from its result, would be an unreviewed,
single-purpose special case rather than the real integration - left as the same residual this
codebase already carries for every other constraint, not newly introduced by this repair. See
Residual risks.

## Outcome

PASS (round 2, pending re-verification)

## Scope

Implements Built-in Verified / Community Verified / Local Custom / Untrusted as a real
authority input (`docs/PROVIDERS.md` "Trust levels"), closing the "not wired in yet" gap
`docs/architecture/DOMAIN_MODEL.md` explicitly called out for `ProviderTrustAuthority` since
E03-S04. `ProviderTrust` lives in `cancellai-model` (pure vocabulary, no decision logic,
matching that crate's existing role); the ceiling mapping and the promotion gate live in
`cancellai-safety`, since `scripts/check_rust_workspace.py` forbids `cancellai-safety`
depending on anything but `cancellai-model`/`cancellai-platform` and this is squarely
authority-kernel logic, not provider-adapter logic. No provider adapter exists yet to *supply*
a `ProviderTrust` value from a real manifest (that is E05-S03/E05-S04/E16 scope); this story
wires the consuming side (the authority computation) and the promotion gate, matching how
E03-S04 itself was wired ahead of the inventory engine that would later feed it real
confidence/lifecycle facts.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Untrusted manifests cannot produce irreversible actions | `provider_trust_ceiling(ProviderTrust::Untrusted) == AuthorityLevel::Observe`, wired into `effective_authority` as the `provider_trust_authority` constraint (a monotonic minimum - E03-S04's own AC1 proof already covers "no other input can raise the result past this one"). `e05s02_ac1_untrusted_provider_trust_collapses_to_observe_even_at_maximum_everything_else` proves the result is `Observe` even when every other input is maximally permissive, and that `Observe` is strictly below `minimum_authority_for(ActionClass::Quarantine)` *and* `ActionClass::Delete` - stronger than the AC requires (an untrusted provider cannot even quarantine, let alone delete). `e05s02_ac1_local_custom_trust_can_quarantine_but_not_delete` and `e05s02_ac1_community_verified_trust_can_delete_but_is_not_unbounded` exercise the other two documented ceilings so the full table is proven, not only the `Untrusted` case. | PASS |
| AC2 - Trust promotion requires explicit verified provenance and tests | `TrustedTier::promote` is the sole *public* path in the workspace that can raise a trust tier (the free `promote` function it delegates to is private, round 2); it requires `evidence.verified_by` to be non-empty/non-whitespace and `evidence.fixture_references` to be non-empty, and refuses (`NotAnUpgrade`) any request that is not a strict upgrade. `ac2_promotion_succeeds_with_a_named_verifier_and_at_least_one_fixture`, `si021_a_self_attested_claim_with_no_named_verifier_is_rejected`, `si021_whitespace_only_verifier_is_treated_as_missing`, `ac2_a_named_verifier_with_zero_fixtures_is_rejected` (free-function level) and `trusted_tier_promote_mirrors_the_free_function_gate`/`trusted_tier_promote_rejects_missing_evidence_exactly_like_the_free_function` (`TrustedTier`-level, the actual public API) cover the positive case and each missing-evidence failure independently. `every_tier_can_be_promoted_to_every_strictly_higher_tier_given_valid_evidence` is exhaustive over all 16 `(from, to)` pairs. Round 2 additionally proves, by construction, that AC2 cannot be *bypassed*: see the compile_fail doctest on `TrustedTier` and `si021_the_verifier_round1_reproduction_is_no_longer_reachable_through_trusted_tier`. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-021 (provider manifest trust bounds authority; cannot self-assign a trust level above locally verified policy) | **Round 1 counterexample (the actual defect):** `AuthorityInputs { provider_trust: ProviderTrust::BuiltinVerified, .. }` constructed directly, no `promote` call - reached `Autopilot`. **Round 2 repair:** the same construction no longer compiles (`ProviderTrust` is not `TrustedTier`); `compile_fail` doctest on `TrustedTier` is the permanent regression. Also: a hostile manifest attempting to self-attest ("trust me") with a fixture attached but no named verifier (`si021_a_self_attested_claim_with_no_named_verifier_is_rejected`); the same with only whitespace as a verifier name (`si021_whitespace_only_verifier_is_treated_as_missing`); a request to promote `Untrusted` all the way to `BuiltinVerified` in one call, proving the gate does not require incremental single-tier steps (`ac2_promotion_across_more_than_one_tier_is_allowed_when_evidenced`) | Every bypass attempt now fails to compile or returns `Err`; `TrustedTier::untrusted()` is the only zero-evidence starting point reachable from outside `cancellai-safety`, and `provider_trust_ceiling` caps it at `Observe` regardless of what a manifest claims about itself | PASS |
| SI-022 (knowledge is data, not executable authority) | `TrustPromotionEvidence` has no field of a type that could carry a command/code (`String`/`Vec<String>` only); `trust_promotion.rs`'s module doc records the residual that signature/provenance verification for a *distributed* knowledge bundle does not exist yet (E16) | `TrustedTier::promote`'s only effect is returning a `TrustedTier` value or an error - it performs no I/O, spawns no process, and evaluates no caller-supplied code; round 2 additionally ensures no path exists to *skip* this effect and obtain a raised tier some other way | PASS (partial - see Residual risks) |

## Verification Commands

```text
# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# Python governance (repository-wide)
python3 scripts/check_docs.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/project_os.py check
```

`cargo test -p cancellai-safety` runs 63 unit tests plus 1 `compile_fail` doctest, all green
(round 1 had 59 unit tests; round 2 adds 4: `si021_a_fresh_trusted_tier_defaults_to_untrusted`,
`trusted_tier_promote_mirrors_the_free_function_gate`,
`trusted_tier_promote_rejects_missing_evidence_exactly_like_the_free_function`, and
`si021_the_verifier_round1_reproduction_is_no_longer_reachable_through_trusted_tier`, plus the
new `compile_fail` doctest on `TrustedTier`). `grep` confirms no crate outside
`cancellai-safety` referenced `AuthorityInputs`/`promote` before this repair, so retyping
`provider_trust` and privatizing `promote` breaks no other crate - the full workspace
`cargo test --workspace` remains green with no count change elsewhere.

## Compatibility

- `AuthorityInputs::provider_trust`'s type changed from `ProviderTrust` to `TrustedTier` - a
  breaking change to `AuthorityInputs`'s own shape, but (as above) nothing outside
  `cancellai-safety` constructed it yet, so nothing outside this crate needed updating.
- `cancellai-safety`'s dependency graph is unchanged (`cancellai-model`, `cancellai-platform`
  only) - `scripts/check_rust_workspace.py` still passes.

## Performance / operability

- Not applicable - `provider_trust_ceiling`, the private `promote`, and `TrustedTier`'s methods
  are pure, allocation-free (aside from the caller-supplied `Vec<String>`/`String` in
  `TrustPromotionEvidence`) functions with no I/O.

## Documentation updated

- `docs/architecture/PROVIDER_MODEL.md` - new paragraph under "Trust chain" (the story's
  declared documentation impact).
- `docs/security/THREAT_MODEL.md` - new paragraph under TM-10 pointing at the implementation
  (the story's declared documentation impact).
- `docs/security/SAFETY_INVARIANTS.md` - "Implemented at ..." notes added under SI-021/SI-022,
  matching every other implemented invariant's existing convention (documentation impact
  expanded beyond the story's original two-file declaration, since this is exactly the kind of
  cross-reference every other implemented SI already carries - AGENTS.md: "add more if
  implementation changes more contracts").
- `docs/architecture/DOMAIN_MODEL.md` - updated the "Effective Authority" section, which
  previously stated `ProviderTrustAuthority` was "not wired in yet"; that sentence is now
  false and would mislead a future reader if left uncorrected (documentation impact expanded
  for the same reason as above).

## Residual risks

- No provider adapter exists yet to supply a real `TrustedTier` value from an actual manifest -
  this story wires the consuming/authority side and the promotion gate; a manifest loader that
  calls `TrustedTier::promote` (or correctly defaults to `TrustedTier::untrusted()`) is
  E05-S03/E05-S04/E16 scope. (E05-S03/E05-S04, already `done`, do not construct `TrustedTier`
  at all - they answer `ProviderCapabilities` questions independently of authority.)
- SI-022's "invalid signatures/provenance are rejected" for a *distributed* knowledge bundle
  is not implemented here - `TrustPromotionEvidence` is in-process data with no signature
  concept; that arrives with E16 (Provider Ecosystem and Federated Knowledge). This story only
  closes the "knowledge is data, not executable authority" half of SI-022 for the promotion
  path that exists today.
- **Named explicitly in the round 1 review, deliberately not closed by this repair:**
  `provider_trust_authority` (and every other `effective_authority` constraint - confidence,
  lifecycle, artifact ceiling) is not yet consulted by `mutation_executor::execute`, which
  reads authority directly off a `SealedPlan` recorded at sealing time. `SealedPlan` does not
  yet call `effective_authority` at all for any of its inputs; wiring that is the PLAN-stage
  builder's job (E06-era), a pre-existing, cross-cutting scope boundary
  `docs/architecture/DOMAIN_MODEL.md` already documented for `SealedPlan` before this story
  existed (E03-S02), not a gap this repair introduced or could close in isolation for provider
  trust alone without inventing unreviewed PLAN-stage architecture under repair-round time
  pressure. What this repair *does* close is the concrete, demonstrated bypass: calling
  `effective_authority` directly with a manifest-supplied trust claim can no longer yield an
  unearned ceiling.

## Verifier verdict

Round 1: **FAIL** (SI-021/SI-022 bypass - see "Round 2 repair" above and
`project/evidence/E05-VERIFIER-REVIEW.md` / `project/evidence/E05-S02/SAFETY_VERDICT.md`).
Round 2 (this repair): awaiting re-verification per the owner's disposition - see the owner's
own decision recorded alongside this evidence packet for whether a second independent
verification round was run or the owner accepted this repair directly. CR4 Safety Verdict
recording a pass remains the verifier's/owner's output, never the executor's own attestation.

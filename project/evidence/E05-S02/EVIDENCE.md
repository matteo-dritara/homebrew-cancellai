# Evidence Packet - E05-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E05)
- Change Risk: CR4
- Spec version/commit: `rust/crates/cancellai-model/src/vocabulary.rs` (`ProviderTrust`),
  `rust/crates/cancellai-safety/src/authority.rs` (`provider_trust_ceiling`,
  `AuthorityInputs::provider_trust`), `rust/crates/cancellai-safety/src/trust_promotion.rs`
  (new) as added in this change

## Outcome

PASS

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
| AC2 - Trust promotion requires explicit verified provenance and tests | `trust_promotion::promote` is the sole function in the workspace that can raise a `ProviderTrust` tier; it requires `evidence.verified_by` to be non-empty/non-whitespace and `evidence.fixture_references` to be non-empty, and refuses (`NotAnUpgrade`) any request that is not a strict upgrade. `ac2_promotion_succeeds_with_a_named_verifier_and_at_least_one_fixture`, `si021_a_self_attested_claim_with_no_named_verifier_is_rejected`, `si021_whitespace_only_verifier_is_treated_as_missing`, and `ac2_a_named_verifier_with_zero_fixtures_is_rejected` cover the positive case and each missing-evidence failure independently (not just their conjunction). `every_tier_can_be_promoted_to_every_strictly_higher_tier_given_valid_evidence` is exhaustive over all 16 `(from, to)` pairs. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-021 (provider manifest trust bounds authority; cannot self-assign a trust level above locally verified policy) | A hostile manifest attempting to self-attest ("trust me") with a fixture attached but no named verifier (`si021_a_self_attested_claim_with_no_named_verifier_is_rejected`); the same with only whitespace as a verifier name (`si021_whitespace_only_verifier_is_treated_as_missing`); a request to promote `Untrusted` all the way to `BuiltinVerified` in one call, proving the gate does not require incremental single-tier steps that a caller could otherwise bypass by skipping straight to the top with weak evidence at just the final hop (`ac2_promotion_across_more_than_one_tier_is_allowed_when_evidenced` - shows the *evidenced* case succeeds; the missing-evidence tests above show the same jump fails without it) | Every case returns `Err`/leaves the tier unchanged; `provider_trust_ceiling(Untrusted)` independently caps any manifest that never went through `promote` at `Observe` regardless of what it claims about itself, since nothing else in the codebase reads a trust value from anywhere but this gate or the `Untrusted` default | PASS |
| SI-022 (knowledge is data, not executable authority) | `TrustPromotionEvidence` has no field of a type that could carry a command/code (`String`/`Vec<String>` only); `evidence.rs` (`trust_promotion.rs`) module doc records the residual that signature/provenance verification for a *distributed* knowledge bundle does not exist yet (E16) | `promote`'s only effect is returning a `ProviderTrust` value or an error - it performs no I/O, spawns no process, and evaluates no caller-supplied code | PASS (partial - see Residual risks) |

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

`cargo test -p cancellai-model -p cancellai-safety` runs 68 tests (5 + 59 unit, plus the
diagnostic golden suite), all green - 6 new `ProviderTrust`-driven `authority` tests, 8 new
`trust_promotion` tests, 1 new `ProviderTrust` ordering test, with no regression in any
pre-existing test (the only change to an existing test is `permissive_inputs`' helper gaining
`provider_trust: ProviderTrust::BuiltinVerified`, the fully-permissive default, so every prior
assertion about the other five axes is unaffected).

## Compatibility

- `AuthorityInputs` gained a new required field (`provider_trust`); the only production and
  test call sites in the workspace are within `authority.rs` itself (`grep` confirms no
  external crate constructs `AuthorityInputs` yet), so this is not a breaking change to any
  other crate.
- `cancellai-safety`'s dependency graph is unchanged (`cancellai-model`, `cancellai-platform`
  only) - `scripts/check_rust_workspace.py` still passes.

## Performance / operability

- Not applicable - `provider_trust_ceiling` and `promote` are pure, allocation-free (aside
  from the caller-supplied `Vec<String>`/`String` in `TrustPromotionEvidence`) functions with
  no I/O.

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

- No provider adapter exists yet to supply a real `ProviderTrust` value from an actual
  manifest - this story wires the consuming/authority side and the promotion gate; a manifest
  loader that calls `promote` (or correctly defaults to `Untrusted`) is E05-S03/E05-S04/E16
  scope.
- SI-022's "invalid signatures/provenance are rejected" for a *distributed* knowledge bundle
  is not implemented here - `TrustPromotionEvidence` is in-process data with no signature
  concept; that arrives with E16 (Provider Ecosystem and Federated Knowledge). This story only
  closes the "knowledge is data, not executable authority" half of SI-022 for the promotion
  path that exists today.
- `provider_trust_authority` is not yet consulted by `mutation_executor::execute` (which reads
  authority directly off a `SealedPlan`, not from `effective_authority`) - wiring
  `effective_authority`'s full output into `SealedPlan` construction is E03-S02/E06-era
  plan-building scope this story does not touch, matching the same residual E04-S03 already
  recorded for `ScopeCompleteness` not yet feeding `KnowledgeConfidence`.

## Verifier verdict

PENDING - epic E05 review runs once every story in E05 is `ready_for_review` (at most twice
per epic, per ADR-0014). CR4 Safety Verdict is the verifier's output, required at `done`, not
at this handoff.

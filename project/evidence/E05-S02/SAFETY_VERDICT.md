# Safety Verdict - E05-S02

- Change: Provider trust tiers, promotion, and effective-authority input
- Risk: CR4
- Commit/PR: round 1 `5d62a00..44c175b` (REJECTED); round 2 repair `462c21e`
- Independent verifier: Codex (`/root`), round 1 - `project/evidence/E05-VERIFIER-REVIEW.md`
- Owner review of round 2 repair: Matteo Pugliese (repository owner), 2026-08-31

## Verdict

`PASS`

## Safety surface changed

Round 1 claimed to make provider trust an unbypassable authority ceiling and to make
`trust_promotion::promote` the only route by which a provider can acquire a higher trust
tier. It did not: `AuthorityInputs::provider_trust` was typed as the bare, publicly
constructible `cancellai_model::ProviderTrust` enum, so any external caller could construct
`ProviderTrust::BuiltinVerified` directly and reach `AuthorityLevel::Autopilot` through
`effective_authority` with zero promotion evidence - the round 1 verifier's own reproduction,
recorded in `project/evidence/E05-VERIFIER-REVIEW.md`.

Round 2 (commit `462c21e`) changes the safety surface itself: `AuthorityInputs::provider_trust`
is now typed `cancellai_safety::TrustedTier`, an opaque wrapper around `ProviderTrust` with a
private field and no `From<ProviderTrust>` conversion. `TrustedTier`'s only public constructors
are `untrusted()` (the safe, evidence-free default) and a checked `promote()` requiring a named
verifier and at least one fixture reference. The previously-public `promote` free function is
now private, reachable only through `TrustedTier::promote`.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-021 | A manifest/untrusted input cannot self-assign authority above locally verified policy. | The round 1 reproduction (`AuthorityInputs { provider_trust: ProviderTrust::BuiltinVerified, .. }` constructed directly) no longer compiles: `ProviderTrust` is not `TrustedTier`, and there is no conversion between them. A `compile_fail` doctest on `TrustedTier` (`rust/crates/cancellai-safety/src/trust_promotion.rs`) restates this exact reproduction as a permanent, executable regression - `cargo test -p cancellai-safety` runs it on every invocation and fails the build if it ever starts compiling again. `TrustedTier::untrusted()` is the only zero-evidence value reachable from outside `cancellai-safety`. | PASS |
| SI-022 | Knowledge cannot raise local destructive authority outside the verified promotion path. | `TrustedTier::promote` is the sole public path that can produce a `TrustedTier` above `Untrusted`; it delegates to the same fail-closed checks round 1 already verified (non-empty verifier, non-empty fixture references, strict-upgrade-only), but round 2 additionally makes those checks unavoidable - there is no longer any public way to obtain a raised `TrustedTier` that skips them. | PASS |

## Adversarial cases

- **External-consumer API reconstruction (the round 1 finding, re-run against round 2):**
  `AuthorityInputs { provider_trust: ProviderTrust::BuiltinVerified, .. }` - confirmed this no
  longer compiles (`error[E0308]: mismatched types`, `expected TrustedTier, found ProviderTrust`);
  proven permanently by the `compile_fail` doctest, and restated as a plain `cargo test` case in
  `si021_the_verifier_round1_reproduction_is_no_longer_reachable_through_trusted_tier`.
- A hostile manifest self-attesting ("trust me") with a fixture attached but no named verifier,
  and the same with a whitespace-only verifier name: both rejected
  (`si021_a_self_attested_claim_with_no_named_verifier_is_rejected`,
  `si021_whitespace_only_verifier_is_treated_as_missing`).
- A jump straight from `Untrusted` to `BuiltinVerified` in one call: succeeds only with valid
  evidence (`ac2_promotion_across_more_than_one_tier_is_allowed_when_evidenced`), proving the
  gate does not depend on incremental per-tier steps a caller could otherwise exploit by
  supplying weak evidence at only the final hop.
- A downward or same-level "promotion" request: refused (`NotAnUpgrade`), not silently treated
  as a no-op that could mask a failed check
  (`a_downward_or_same_level_request_is_refused_not_silently_a_no_op`).

## Differential / compatibility evidence

- `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo check --workspace --all-targets`, `cargo test --workspace` (including the new
  `compile_fail` doctest), and `cargo deny check` all pass.
- `grep` across the workspace confirms no crate outside `cancellai-safety` referenced
  `AuthorityInputs` or the free `promote` function before this repair, so retyping
  `provider_trust` and privatizing `promote` breaks no other crate.
- Full Python governance suite (`check_docs`, `check_rust_workspace`, `check_mutation_boundary`,
  `check_provider_compatibility`, `project_os check`, `pytest`) passes unchanged.

## Known residual risks

- **Not closed by this repair, and named explicitly in the round 1 review:**
  `mutation_executor::execute` still consumes authority recorded directly on a `SealedPlan`,
  not a live `effective_authority` call. This is a pre-existing, cross-cutting gap that applies
  to *every* `effective_authority` constraint (confidence, lifecycle, artifact ceiling - not
  only provider trust): `SealedPlan` has never called `effective_authority` for any input,
  since E03-S02 explicitly deferred that wiring to the PLAN-stage builder
  (`docs/architecture/DOMAIN_MODEL.md` "SealedPlan"), which does not exist in any story yet.
  What this repair closes is the concrete bypass the round 1 review demonstrated: calling
  `effective_authority` directly with a manifest-supplied trust claim can no longer yield an
  unearned ceiling. It does not, by itself, make `SealedPlan`/`mutation_executor::execute`
  consult provider trust at all - nothing did before this story either.
- No provider adapter (E05-S03/E05-S04, already `done`) constructs a `TrustedTier` from a real
  manifest today; both existing adapters answer `ProviderCapabilities` questions independently
  of the authority lattice. A future manifest/knowledge-loading story is what will first call
  `TrustedTier::promote` (or intentionally leave a provider at `untrusted()`) against real data.
- SI-022's signature/provenance verification for a *distributed* knowledge bundle remains out
  of scope (E16 Provider Ecosystem and Federated Knowledge); `TrustPromotionEvidence` is
  in-process data with no signature concept.

## Rollback / recovery

No production data or shipped surface depends on this change - `cancellai-provider-api`'s
`ProviderCapabilities` contract (E05-S01, already `done`) and both reference adapters
(E05-S03/E05-S04) do not consume `TrustedTier`/`effective_authority` at all, so this repair has
no migration or rollback concern. If a defect were found in `TrustedTier` after this verdict,
reverting commit `462c21e` alone (restoring the round 1 `ProviderTrust`-typed field) would
reintroduce the round 1 bypass - the correct recovery is a forward fix, not a revert to round 1.

## Owner decision

`ACCEPT`

Owner note: Reviewed the round 1 finding, the round 2 repair (`TrustedTier` as an opaque,
sealed-construction type - the same pattern already used for `SealedPlan`), and the adversarial
reproduction proof (`compile_fail` doctest + regression test). The demonstrated bypass is
closed by construction, not merely by a runtime check that a future caller could still route
around. Accepting the documented residual (no PLAN-stage wiring of `effective_authority` into
`SealedPlan` yet, for any constraint, not only trust) as pre-existing scope, not a defect of
this story. E05-S02 and epic E05 may close.

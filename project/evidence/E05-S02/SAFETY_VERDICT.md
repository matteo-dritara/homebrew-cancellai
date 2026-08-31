# Safety Verdict - E05-S02

- Change: Provider trust tiers, promotion, and effective-authority input
- Risk: CR4
- Commit/PR: `5d62a00..44c175b`
- Independent verifier: Codex (`/root`)
- Date: 2026-08-31

## Verdict

`FAIL`

## Safety surface changed

The change claims to make provider trust an unbypassable authority ceiling and to make
`trust_promotion::promote` the only route by which a provider can acquire a higher trust tier.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-021 | A manifest/untrusted input cannot self-assign authority above locally verified policy. | `ProviderTrust` remains a public enum and `AuthorityInputs::provider_trust` remains a public field. Any downstream caller can construct `AuthorityInputs { provider_trust: ProviderTrust::BuiltinVerified, .. }` directly and call `effective_authority`, without calling `promote` or providing verifier/fixture evidence. | FAIL |
| SI-022 | Knowledge cannot raise local destructive authority outside the verified promotion path. | `promote` checks strings when called, but it does not mint an opaque trusted capability and `effective_authority` accepts the raw public enum. Therefore callers can bypass its checks completely. | FAIL |

## Adversarial cases

- External-consumer API reconstruction: directly construct the public `AuthorityInputs` with
  permissive artifact/lifecycle facts and `ProviderTrust::BuiltinVerified`. The result is
  `Autopilot`; no promotion evidence is required by the type system or authority function.
- `mutation_executor::execute` consumes authority recorded in `SealedPlan`, not the
  `effective_authority` result. The E05 trust ceiling is therefore not an execution
  precondition even for callers that do invoke the calculation.

## Differential / compatibility evidence

- `cargo fmt --check`, clippy, check, test, and cargo-deny passed locally.
- The passing matrix tests cover the tier-to-ceiling table only; they do not test that an
  untrusted manifest cannot supply the selected tier to the authority calculation.

## Known residual risks

- This is an unresolved authority-bypass defect, not an acceptable closure residual.

## Rollback / recovery

Do not consume provider trust to authorize destructive plans until the repair below is in
place. The current code introduces no provider manifest loader, so no production data needs
migration or rollback.

## Owner decision

`REJECT`

Owner note: Required repair: make the trust tier accepted by authority computation an opaque,
safety-owned value that defaults to `Untrusted` and can be raised only by a provenance-validating
promotion gate; bind that computed ceiling into sealed-plan construction/execution so raw
caller-supplied trust cannot authorize a delete. Add an external-consumer regression that cannot
compile or cannot obtain `Govern`/`Autopilot` from an untrusted manifest claim.

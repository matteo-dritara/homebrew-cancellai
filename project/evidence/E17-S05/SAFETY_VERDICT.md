# Safety Verdict - E17-S05

- Change: opaque build-channel identity and channel-aware authority ceiling
- Risk: CR4
- Commit/PR: `29cfea6fb18247c9c3cdc736a63efa121edc9aea..4d19ac62b864a12f6599b9875053bd6ccf7c6796`
- Independent verifier: Codex
- Date: 2026-09-11

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

`BuildChannel` binds a compiled release channel to a second authority calculation. Nightly is
capped at `Recommend`, beta at `Govern`, and stable receives no extra cap. The channel wrapper
cannot be constructed from the public `ReleaseChannel` enum by an external crate.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-030 | Nightly cannot obtain stable-equivalent irreversible autonomy from user configuration. | `effective_authority_for_channel` includes `release_channel_authority`; exhaustive channel/policy tests passed. Even permissive inputs produce `Recommend` for nightly, below quarantine and delete requirements. | PASS |
| SI-030 | Runtime environment cannot claim a higher channel. | `BuildChannel::from_compiled_env` uses `option_env!`; absent or unknown values resolve to nightly. The external-crate `compile_fail` doctest passed in `cargo test --workspace`. | PASS |

## Adversarial cases

- Maximum user/artifact inputs under nightly remained below both quarantine and delete authority.
- Raising user authority did not lift nightly above the channel ceiling.
- Stable, beta, nightly, and absent compiled channel values were exercised by the safety test suite.
- A workspace-wide caller scan found `effective_authority_for_channel` only at its definition and `cancellai-safety` tests; there is no hidden production mutation-path caller.

## Differential / compatibility evidence

- `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo deny check` passed.
- The existing `effective_authority` API and CLI classify/plan/clean pipeline have no behavior change until the explicitly deferred integration story supplies a real build-channel input.

## Known residual risks

- The new channel-aware function is intentionally not wired into a real mutation path yet. ADR-0023 and `docs/security/SUPPLY_CHAIN.md` disclose that E06-S04 (or an earlier explicitly scoped story) must make it the production calculation. This is a bounded implementation residual, not evidence that the invariant is currently enforced by the shipped CLI path.

## Rollback / recovery

- The change is stateless. Reverting the channel wrapper and channel-aware authority function restores the prior calculation; no provider data, bundle, or user configuration is migrated or mutated.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: Independent verifier recommends acceptance of the kernel constraint with the documented, non-production-caller residual carried forward to the integration story.

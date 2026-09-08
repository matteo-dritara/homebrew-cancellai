# ADR-0023: Release-channel authority as an opt-in function, not a required `AuthorityInputs` field

- Status: Accepted
- Date: 2026-09-08
- Owners: project owner / cEOS
- Related: PD-017, ADR-0009, E17-S05, SI-030

## Context

E17-S05 implements SI-030 ("Experimental/nightly builds do not inherit stable-level autonomous
destructive defaults merely because user configuration exists from a stable install") inside
`cancellai-safety::authority`, which already computes Effective Authority as a monotonic
minimum over named constraints (`cancellai_safety::authority::effective_authority`,
`AuthorityInputs`). The module's own top-of-file documentation had pre-declared
`ReleaseChannelAuthority` as one of the nine constraints in the documented formula, alongside
`ProviderTrustAuthority` (E05-S02's `TrustedTier`, SI-021).

The direct, symmetrical implementation - add `release_channel: BuildChannel` as a new required
field on `AuthorityInputs`, wired into `effective_authority` exactly like `provider_trust` - was
built first and reverted after it broke real existing tests: `cancellai-policy::retention`'s
`reachable_authority`, the one production caller of `AuthorityInputs` in this workspace
(`cancellai-cli::resolve_all` -> `resolve_claude`/`resolve_codex` -> `classify` ->
`reachable_authority` -> `effective_authority`), predates release-channel awareness and has no
honest channel value to supply yet: `cancellai-cli` is a beta, source-built artifact with no
packaged release of its own (`docs/RELEASING.md`'s "Beta side-by-side" section), so nothing in
this workspace has decided what channel a plain `cargo build`/`cargo test` of it represents in
product terms. The only "safe" placeholder consistent with SI-030's own fail-closed floor
(`BuildChannel::default()`, i.e. `Nightly`) would have silently capped every one of
`retention.rs`'s existing, already-verified Delete-reaching test scenarios (deletion mechanics,
symlink-safety adversarial cases, keep-latest protection, and others unrelated to release
channel at all) at `Recommend` - a large, misleading regression across unrelated, previously
green safety-relevant test coverage, for a constraint nothing yet asked those tests to enforce.

Cargo's per-crate `#[cfg(test)]` semantics compounded the difficulty of a narrower fix: a
dependency's `#[cfg(test)]` items are absent from a downstream crate's build in *every* mode
(a plain `cargo test -p cancellai-policy` compiles `cancellai-safety` as an ordinary, non-test
dependency), so `cancellai-policy`'s own tests could not reach a `BuildChannel::for_tests`-style
escape hatch the way `TrustedTier::for_tests`'s `pub(crate)` restriction assumes only
`cancellai-safety`'s own tests would ever need one - here, a real downstream test author needed
one and structurally could not get it without either a cross-crate `dev-dependencies` feature
flag (evaluated and rejected below) or eliminating the need for one entirely.

## Decision

`cancellai_safety::authority::effective_authority_for_channel(inputs: AuthorityInputs, channel:
BuildChannel) -> EffectiveAuthority` is a second, opt-in public function alongside
`effective_authority`, not a new required field on `AuthorityInputs`. Both share a private
`base_constraints` helper (the six pre-existing constraints), so they cannot silently diverge on
those; `effective_authority_for_channel` appends exactly one more:
`release_channel_ceiling(channel.level())`, matching `docs/security/SUPPLY_CHAIN.md`'s Release
channels table (`Stable` -> no additional cap, `Beta` -> `Govern`, `Nightly` -> `Recommend`).

[`BuildChannel`] itself is implemented exactly like `TrustedTier` (SI-021's own pattern): an
opaque wrapper around `cancellai_model::ReleaseChannel` whose only production constructor,
`BuildChannel::from_compiled_env`, reads `CANCELLAI_CHANNEL` via `option_env!` - baked in at the
moment `cancellai-safety` is compiled, not a runtime environment variable a user running the
binary could set to claim a higher channel than the build actually is. An absent or
unrecognized value resolves to `ReleaseChannel::Nightly`, the fail-closed floor - matching
`BuildChannel::default()` exactly, so a struct-update fixture that forgets to set it can never
accidentally start out more privileged than SI-030 allows.

No caller in this workspace calls `effective_authority_for_channel` yet.
`.github/workflows/release.yml`'s `build-artifacts` job (E17-S02) sets
`CANCELLAI_CHANNEL=stable` for every canonical tier-1 build regardless, so the value is already
correct and ready the moment a real caller (E06-S04, when `cancellai-cli` becomes the canonical,
packaged-release engine) adopts it - adoption is then a one-line change (call the `_for_channel`
variant instead of the plain one and supply `BuildChannel::from_compiled_env()`), not a new
design decision.

## Alternatives considered

### Required field on `AuthorityInputs`, with a `Default` covering existing callers

Rejected: `AuthorityInputs` does not (and per its own `#[derive(Copy)]`/module-doc style should
not) implement `Default` as "whatever value makes existing callers pass" - the one honest
default, `Nightly`, is exactly the value that broke unrelated existing test coverage, described
above. A required field whose only safe default actively changes 8+ existing tests' outcomes is
not a backward-compatible addition in practice, whatever its type signature claims.

### `dev-dependencies` feature flag exposing `BuildChannel::for_tests` across crates

Built and then reverted along with the required-field approach it was meant to support (a
`test-util` Cargo feature on `cancellai-safety`, requested only from other crates'
`[dev-dependencies]` so it is active for `cargo test` but absent from every production build
regardless of which crate is compiled). Mechanically correct and still available as a pattern
if a future story needs cross-crate test-only construction of an opaque safety type, but once
`effective_authority_for_channel` made the field non-required, `cancellai-policy`'s tests had
nothing left to construct a `BuildChannel` for, so the added Cargo complexity (a second,
feature-gated dependency entry on the same crate) was removed rather than kept for a need that
no longer existed in this story.

### Workspace-wide `CANCELLAI_CHANNEL=stable` default via `rust/.cargo/config.toml`

Rejected outright, not merely deferred: `[env]` in `.cargo/config.toml` applies unconditionally
to every cargo invocation cargo discovers it for, including a contributor's own local
`cargo build --release` run from within a checked-out clone - exactly the "user configuration"
SI-030 exists to prevent from granting stable-level authority. This would not have fixed the
retention.rs test regression in a way that also kept the fail-closed default meaningful; it
would have defeated the default everywhere, permanently, which is a materially worse outcome
than the test breakage it would have avoided.

## Consequences

### Positive

- SI-030's constraint is fully implemented and adversarially tested (`cancellai-safety::
  authority`'s own test suite: nightly/beta/stable ceilings, monotonicity under rising user
  authority, binding-constraint naming, and agreement with `effective_authority` when channel is
  not the bottleneck) without disturbing any existing, unrelated test coverage;
- `cancellai-cli`'s real release build already bakes in the correct `stable` value today, so
  E06-S04 (or any future caller) adopts real enforcement with a one-line change, not a new
  design decision or a second implementation.

### Negative / cost

- release-channel authority is not actually enforced anywhere yet - `cancellai-cli`'s classify/
  plan/clean pipeline still computes authority exactly as it did before this story. This is a
  disclosed residual, not a silent gap: `docs/security/SUPPLY_CHAIN.md`'s "Release channels"
  section states it explicitly, and this ADR's own Decision section names the future caller
  responsible for closing it.
- two public functions (`effective_authority`, `effective_authority_for_channel`) exist where
  the originally-documented formula implied one; a reader must know to reach for the `_for_
  channel` variant to get SI-030 enforcement, rather than it being unconditional.

### Neutral / follow-up

- E06-S04 (or an earlier story that specifically decides to wire this in sooner) switches
  `cancellai-policy`'s callers from `effective_authority` to `effective_authority_for_channel`,
  threading a real `BuildChannel::from_compiled_env()` value through
  `RetentionPolicy`/`classify`/`reachable_authority` - at which point that story's own test
  fixtures will need to decide their own channel value deliberately, informed by real product
  requirements at cutover time, not a placeholder chosen ahead of that decision.

## Safety and compatibility impact

- Change Risk implication: CR4 (E17-S05) - this is the constitutional authority-lattice module,
  even though the new constraint is not yet wired into any mutation path; the kernel-ring
  dependency rules and adversarial-test bar apply regardless of current callers.
- Safety Invariants affected: SI-030 (implements it), SI-021 (mirrors its opaque-wrapper
  pattern exactly, so `BuildChannel` inherits the same "cannot be forged by an external crate"
  guarantee `TrustedTier`'s own `compile_fail` doctest proves - `BuildChannel` carries an
  identical one).
- Migration/rollback: reverting is deleting `effective_authority_for_channel`,
  `release_channel_ceiling`, `BuildChannel`, and `cancellai_model::ReleaseChannel`; no state
  migration exists because no caller depends on them yet.

## Supersession

If a future story makes `release_channel` a required `AuthorityInputs` field once every caller
has a real, decided value to supply, keep this ADR and mark it superseded by the ADR that makes
that change.

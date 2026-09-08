# Evidence Packet - E17-S05

- Commit/PR: this working-tree change (executor session, 2026-09-08)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR4
- Spec version/commit: `project/epics/E17.json` (E17-S05), as of this change

## Outcome

PASS

`rust/crates/cancellai-safety/src/build_channel.rs` (new) adds `BuildChannel`, an opaque
wrapper around the new `cancellai_model::ReleaseChannel` (`Nightly < Beta < Stable`) - mirroring
`TrustedTier`'s split from `ProviderTrust` (SI-021) for the identical reason: a caller must not
be able to claim "stable" from a nightly binary by constructing the plain vocabulary type
directly. `BuildChannel`'s only production constructor, `from_compiled_env`, reads
`CANCELLAI_CHANNEL` via `option_env!` - baked in at the moment `cancellai-safety` is compiled,
never a runtime environment variable a user running the binary could set. An absent or
unrecognized value resolves to `Nightly`, the fail-closed floor.

`rust/crates/cancellai-safety/src/authority.rs` adds `effective_authority_for_channel`, a
second public function alongside the pre-existing `effective_authority` (both share a private
`base_constraints` helper so they cannot diverge on the six pre-existing constraints). It adds
exactly one more: `release_channel_authority`, capping the monotonic-minimum result by channel
(`Stable` -> no additional cap, `Beta` -> `Govern`, `Nightly` -> `Recommend` - strictly below
what `ActionClass::Quarantine` requires, let alone `Delete`).

[ADR-0023](../../../docs/adrs/0023-release-channel-authority-as-opt-in-function.md) records why
this is a second function rather than a new required `AuthorityInputs` field: the direct,
symmetrical implementation was built first and reverted after it broke 8 real, pre-existing,
unrelated `cancellai-cli` integration tests (deletion mechanics, symlink-safety adversarial
cases, keep-latest protection) whose compiled test binary - correctly, per SI-030's own
fail-closed floor - has no `CANCELLAI_CHANNEL` set and therefore defaults to `Nightly`, capping
every one of those tests' expected `Delete` outcomes at `Recommend`. `cancellai-cli` is a beta,
source-built artifact with no packaged release yet (`docs/RELEASING.md`'s "Beta side-by-side"
section), so nothing in this workspace has decided what channel it represents in product terms;
forcing a required field would have meant picking an arbitrary placeholder for a caller nothing
yet asks to enforce this against. `effective_authority_for_channel` keeps the constraint fully
implemented and adversarially tested, ready for E06-S04 (the cutover story) to adopt with a
one-line change, without disturbing unrelated, already-verified coverage today.

`.github/workflows/release.yml`'s `build-artifacts` job (E17-S02) now sets
`CANCELLAI_CHANNEL=stable` for the real `cargo build --release` step, so the value is already
correct for whenever a caller consumes it - currently inert, since no production code path calls
`effective_authority_for_channel` yet (disclosed as a residual risk below, not hidden).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Nightly defaults cannot execute stable-equivalent irreversible autonomy | `release_channel_ceiling(ReleaseChannel::Nightly) == AuthorityLevel::Recommend`, strictly below both `minimum_authority_for(ActionClass::Quarantine)` and `minimum_authority_for(ActionClass::Delete)` (`Govern`) - proven directly by `e17s05_ac_nightly_can_never_reach_the_authority_delete_or_even_quarantine_requires` even with every *other* input maximally permissive (`AuthorityLevel::Autopilot` user/artifact ceiling, `Verified` confidence, `Idle` activity, `Normal` protection, `Healthy` integrity, `BuiltinVerified` provider trust); `e17s05_raising_user_authority_never_raises_a_nightly_build_past_its_channel_ceiling` proves no `user_requested` value can buy back what the channel does not grant (exhaustive over all 5 `AuthorityLevel`s) | PASS |
| AC2 - Channel information is signed/attested release metadata | Already true as of E17-S01/S02/S03: `release-manifest.json`'s `channel` field (E17-S01's schema) is populated by `release-manifest-generate` (E17-S02) and the manifest is a release asset covered by `build-artifacts`' build-provenance attestation chain (E17-S03) - this story adds the *authority-binding* half (SI-030's actual enforcement mechanism, `BuildChannel`/`effective_authority_for_channel`), which did not exist before it | PASS |

## Safety Evidence

CR4 proof style (`docs/security/SAFETY_INVARIANTS.md` "Safety proof style"):

1. **Which invariant(s) could the change violate?** SI-030 itself (the invariant this story
   implements) and, transitively, SI-021 (the pattern `BuildChannel` copies - a defect in the
   copy would be a defect in both).
2. **What counterexample would prove failure?** A nightly (or beta) build reaching
   `AuthorityLevel::Autopilot` (or, for nightly, reaching `Quarantine`/`Delete`'s minimum at
   all) through some combination of otherwise-permissive inputs; or an external crate
   constructing `BuildChannel(ReleaseChannel::Stable)` directly without going through
   `from_compiled_env`.
3. **Which automated tests reproduce those counterexamples?**
   - `e17s05_nightly_channel_collapses_to_non_destructive_even_at_maximum_everything_else` and
     `e17s05_ac_nightly_can_never_reach_the_authority_delete_or_even_quarantine_requires` -
     nightly with every other input maxed out.
   - `e17s05_raising_user_authority_never_raises_a_nightly_build_past_its_channel_ceiling` -
     exhaustive over all 5 `user_requested` values.
   - `e17s05_beta_channel_can_reach_delete_but_not_unattended_autopilot` - proves `Beta`'s
     ceiling is exactly `Govern`, not vacuously lower nor accidentally `Autopilot`.
   - `e17s05_stable_channel_does_not_cap_below_a_fully_permissive_result` and
     `e17s05_effective_authority_for_channel_agrees_with_effective_authority_when_channel_is_not_the_bottleneck` -
     prove `Stable` adds no spurious cap (not vacuously true: a bug that always returned
     `Recommend` would fail the nightly tests too, but a bug that always capped at some fixed
     level below `Autopilot` would only be caught here).
   - `e17s05_a_fresh_default_build_channel_is_nightly_and_collapses_the_same_way` - proves
     `BuildChannel::default()` (what a struct-update fixture gets if unset) has the same teeth
     as an explicit `Nightly`, in a real `effective_authority_for_channel` call, not only in
     `build_channel.rs`'s own isolated unit tests.
   - `build_channel::tests::from_compiled_env_in_this_test_build_is_the_fail_closed_floor` - a
     *real*, not synthetic, proof that this workspace's own `cargo test` (no `CANCELLAI_CHANNEL`
     set) resolves to `Nightly`.
   - `crates/cancellai-safety/src/build_channel.rs`'s `compile_fail` doctest - the same
     regression class SI-021's `TrustedTier` doctest proves: `BuildChannel(ReleaseChannel::
     Stable)` does not compile from outside this crate (private field, no public tuple
     constructor, no `From<ReleaseChannel>`).
4. **Which residual risk cannot be eliminated and why?** See "Residual risks" below - the
   constraint is not yet enforced on any real mutation path, disclosed rather than hidden.

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
python3 scripts/check_workflows.py check
python3 scripts/check_docs.py check
python3 scripts/check_process.py check
python3 -m pytest tests -v
```

All PASS locally. `cargo test -p cancellai-safety`: 81/81 unit + 2/2 doctests (10 new: 9
`e17s05_*` plus `build_channel::tests`'s 3). Full workspace `cargo test --workspace`: every
crate green, including `cancellai-cli`'s and `cancellai-policy`'s full pre-existing suites
unchanged (confirms the scoped, opt-in design did not regress anything).
`scripts/check_mutation_boundary.py` confirms this story added source files without adding any
new deletion capability - the reported deleting-file set is unchanged.

## Compatibility

- `BuildChannel`/`ReleaseChannel` are new, additive types; no existing public API signature
  changed (`AuthorityInputs`, `effective_authority` are byte-for-byte unchanged from before this
  story - confirmed by every pre-existing test in `cancellai-safety`/`cancellai-policy`/
  `cancellai-cli` passing without modification).

## Documentation updated

- `docs/adrs/0023-release-channel-authority-as-opt-in-function.md` (new) - the design-decision
  ADR (why a second function, not a required field; alternatives considered and rejected,
  including a workspace-wide `.cargo/config.toml` default, rejected because it would defeat
  SI-030's fail-closed default for every local build, not merely fail to fix the test
  regression).
- `docs/security/SAFETY_INVARIANTS.md` - SI-030 gained an "Implemented at..." paragraph
  matching SI-021's own style exactly, including the explicit "not yet enforced on any real
  mutation path" disclosure.
- `docs/security/SUPPLY_CHAIN.md` - "Release channels" section describes the real mechanism.
- `docs/RELEASING.md` - "Target Rust release factory" section notes `CANCELLAI_CHANNEL=stable`
  and links to SUPPLY_CHAIN.md/ADR-0023.
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- **The constraint is not yet enforced on any real mutation path.** No caller in this workspace
  invokes `effective_authority_for_channel`; `cancellai-cli`'s actual `classify`/`plan`/`clean`
  pipeline still computes authority exactly as before this story, via the unmodified
  `effective_authority`. This is the story's central, deliberate scope boundary (see Outcome
  and ADR-0023), not an oversight - disclosed here, in the ADR, and in two documentation files
  rather than left implicit. E06-S04 (the cutover story) is where `cancellai-cli` is expected to
  adopt it, since that is also when the product first needs to answer "what channel does this
  binary actually represent" for real.
- `CANCELLAI_CHANNEL=stable` in `release.yml`'s `build-artifacts` job is currently inert for the
  same reason - set now so the value is already correct when a caller exists, not because it
  changes today's binary's behavior.
- No beta/nightly tag scheme exists yet (`docs/RELEASING.md`), so `Beta`'s ceiling
  (`AuthorityLevel::Govern`) has no real build to exercise it against beyond the unit tests -
  disclosed, matching this repository's existing pattern for CI/build-only claims not yet
  exercised by a real distinct artifact.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`; CR4 Safety Verdict is
the verifier's output, not the executor's, per `docs/development/AGENT_PROTOCOL.md`)

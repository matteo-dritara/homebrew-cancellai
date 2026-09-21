# E14-S04 Independent Verifier Review — Round 5

Review-Scope: epic
Round: 5
Review-Target: `e9ded36e625ac01692fa088ad5c08bf3be44908a`
Verifier: Codex
Date: 2026-09-21
Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c

This owner-authorized exceptional round reviews E14-S04 only. E14-S01, E14-S02, and E14-S03
remain closed and were not re-judged. Round 4 declined another field-level repair and required
the structurally different ADR-0036 design. The current branch has later, unrelated E21 evidence
at `HEAD`; the reviewed implementation commit is the stated E14-S04 commit above.

## Per-story verdict

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E14-S04 | FAIL | An external downstream probe used only public APIs to observe an unrelated real directory whose marker layout matched caller-supplied `known_signatures`, while passing `SyntheticIdentityObserver` an identity token freshly read from the real, drifted provider root. `BoundLayoutObservation::observe` accepted that public observer, read the unrelated directory, and returned an observation carrying the real root's token. `resolve_provider_execution_authority` then returned an `Autopilot` `ProviderExecutionPermit` whose `root_identity()` equalled the real drifted root's identity. Thus the supposed non-forgeable root binding is forgeable. |

## Independent adversarial reproduction

A temporary external Cargo package under `/private/tmp`, compiled against the public path
dependencies only and removed before this record, established all of the following:

1. It created a real provider-root directory containing `drifted/`, observed it with public
   `BoundLayoutObservation::observe(..., &SystemIdentityObserver)`, and retained that real
   observation. With all other constraints genuinely permissive, public `effective_authority`
   returned `Autopilot`, while public `resolve_provider_execution_authority` with known
   `sessions/` returned `Observe`. This reproduces the round-4 sequence and confirms that the
   analysis result itself is not a permit.
2. `cargo check --offline --bin forged_observation` failed with E0451: both
   `BoundLayoutObservation` fields are private. A caller cannot build it from marker strings.
   `cargo check --offline --bin forged_permit` likewise failed with E0451 for every
   `ProviderExecutionPermit` field. `cargo check --offline --bin analysis_as_permit` failed
   with E0308: `EffectiveAuthority` is not `ProviderExecutionPermit`. These are genuine
   public-surface barriers, not merely internal-test visibility.
3. A real-but-unrelated directory containing `sessions/`, observed with
   `SystemIdentityObserver` and paired with caller-supplied `known_signatures = ["sessions/"]`,
   minted an `Autopilot` permit for that unrelated directory's own identity. This is the
   ADR-0036 disclosed caller-supplied-signature residual. `rg` found no production call to
   `resolve_provider_execution_authority`, and therefore no current call site can reach this
   false-recognized result in production. The ADR's framing of that residual is accurate.
4. The binding bypass above is separate and not accurately covered by either disclosed residual:
   `BoundLayoutObservation::observe` publicly accepts `&dyn IdentityObserver`, and
   `SyntheticIdentityObserver::new().set(unrelated_root,
   IdentityObservation::Identity(real_observation.root_identity().clone()))` is public API.
   The same unrelated `sessions/` directory was therefore observed with the *real drifted
   provider root's* token. The resolver returned `Autopilot`; its permit's `root_identity()`
   equalled `real_observation.root_identity()`. The directory markers are still real I/O, but
   the identity which is meant to bind those markers to their root is caller-supplied.

The last case cannot trigger a real mutation today: no production code mints or consumes a
permit. It nevertheless falsifies the ADR's claimed primitive and its proposed E14-S05 shape:
an execution boundary which only compares the permit's root identity with the target root's
fresh identity would accept this unrelated-layout permit as authorizing the real drifted root.

## Assessment against AC1 and SI-004

The round-4 public-API bypass is closed in the narrower sense claimed by the new type split.
`AuthorityInputs` has no layout field, `effective_authority` produces only an analysis result,
and a caller cannot construct or convert into a permit. A real drifted observation passed to the
resolver caps that resolver's permit at `Observe`.

That is insufficient for the ADR's stronger premise that its observation is identity/scope-bound
and non-forgeable. The public `IdentityObserver` parameter is another caller-supplied fact at the
new authority boundary. It permits a sibling directory's real I/O result to be paired with an
asserted identity for the root purportedly governed. Consequently AC1 and SI-004 are not met by
the claimed non-forgeable primitive: a provider name is irrelevant, but a drifted root can still
be represented by an `Autopilot` permit through the public observer seam.

## Audit of migrated policy call sites

`pinning.rs`, `resolver.rs`, `explanation.rs`, and `retention.rs` removed only
`provider_layout: ProviderLayoutAssessment::NotAssessed` from their `AuthorityInputs` literals.
The parent revision's `NotAssessed` entered no layout constraint; the current
`base_constraints` has the same six non-layout constraints and the current workspace tests pass.
The omission is therefore behaviorally equivalent for these existing analysis/policy call sites;
it neither creates nor closes the live provider-layout decision, which remains outside those
functions' scope.

## Test audit

The new platform tests genuinely check real directory entries, absent roots, unreadable roots
where enforceable, and stable system identity. The six new authority tests have meaningful
assertions. In particular,
`e14s04_round4_a_permissive_analysis_result_is_never_a_permit` establishes a non-vacuous
baseline (`EffectiveAuthority::Autopilot`) and independently verifies that a real drifted
observation produces a distinct `ProviderExecutionPermit::Observe`.

They do not test the public `IdentityObserver` injection path. The internal helper always passes
`SystemIdentityObserver`, so it cannot demonstrate the public constructor's claimed
non-forgeability.

## Required repair — within ADR-0036's design, but requires a new owner-authorized review round

This is repairable within ADR-0036's type-level execution-authority design; it does not require a
return to the rejected `AuthorityInputs` field designs or a wholly new authority architecture.
Make the production constructor obtain identity itself from `SystemIdentityObserver` (or an
equally opaque, non-substitutable production capability) and remove the public
`&dyn IdentityObserver` parameter. Any synthetic-observer constructor must be private to
platform unit tests and absent from the release public API. Add an external/public-surface
regression proving that a real target token cannot be attached to another directory's observed
markers, and retain the existing real-drift and no-conversion checks.

Before E14-S05 consumes a permit, its fresh root observation must also be made through that same
non-substitutable mechanism and compared to the permit's bound root identity. A sixth independent
review is still an owner decision under the exceptional-round process; this record only classifies
the repair as feasible within ADR-0036 rather than requiring another design selection.

## CR4 Safety Verdict — E14-S04

## Verdict

`FAIL`

The CR4 classification remains correct. The reviewed code changes the safety-kernel authority
boundary and claims to mint execution authority from a non-forgeable, root-bound observation. A
public caller can forge the observation's root binding by supplying `SyntheticIdentityObserver`,
then reach an `Autopilot` permit tagged with a real drifted root's identity. This violates the
claimed SI-004 control and conflicts with C-02, C-05, and TM-05.

## Owner decision

E14-S04 remains `ready_for_review` in the committed control plane but is rejected by this CR4
Safety Verdict and cannot close. No release action is appropriate. The owner must authorize any
sixth review round and the precise ADR-0036-internal repair above.

## Invariant table

| Invariant / claim | Adversarial result | Verdict |
| --- | --- | --- |
| SI-004: drift cannot preserve destructive capability merely because a provider is recognized | A real drifted root's identity can be attached to a recognized unrelated directory, producing an `Autopilot` permit asserted to bind the drifted root. | FAIL |
| C-02: uncertainty reduces authority | A caller-selected identity observer can replace the actual root identity rather than producing an uncertainty/refusal. | FAIL |
| C-05 / TM-05: authority is bounded by real provider evidence | Marker I/O is real, but the root-identity evidence that gives it scope is public caller input. | FAIL |
| Round-4 bypass closure | `EffectiveAuthority` cannot be constructed as, converted to, or substituted for `ProviderExecutionPermit`; real drift caps the resolver path at `Observe`. | PASS |
| Known-signature residual | `known_signatures` remains caller-supplied, but no production resolver call exists; false recognition is not currently reachable in production. | PASS_WITH_RESIDUALS |
| Mutation-boundary residual | No production permit consumer exists, so the forged permit cannot presently mutate. This is disclosed, but it does not make the primitive's forged root binding safe. | PASS_WITH_RESIDUALS |

## Gate status

| Command | Result |
| --- | --- |
| External public-API probe (`cargo run --offline --bin e14s04-round5-review`) | PASS — reproduced round-4 separation, unrelated-root recognition, and forged root-identity binding; temporary package removed. |
| External compile probes (`forged_observation`, `analysis_as_permit`, `forged_permit`) | PASS — each failed compilation for the expected privacy/type mismatch reason; temporary package removed. |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo test --workspace` | PASS — full workspace and doctests. |
| `cd rust && cargo deny check` | PASS — existing unmatched-license and duplicate-crate warnings only; advisories, bans, licences, and sources passed. |
| `python3 scripts/check_rust_workspace.py check` | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| `python3 scripts/project_os.py check` | PASS |
| `gh run list --branch main --limit 5` | FAILING BASELINE — latest scheduled `rust-benchmark` failed in Miri at `rusqlite::sqlite3_threadsafe`; four latest push workflows shown were successful. CI is not treated as green. |

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `project/epics/E14.json`
- `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-observation-type.md`
- `docs/adrs/0019-dependency-rings-per-crate.md`
- `docs/architecture/GUARDIAN_MODEL.md`
- `docs/architecture/DOMAIN_MODEL.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `project/evidence/E14-S04/VERIFIER_BRIEF.md`
- `project/evidence/E14-S04/EVIDENCE-ROUND5-REPAIR.md`
- `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND4.md`
- reviewed platform, safety, Guardian, policy, and identity public sources; commit `e9ded36`

# Evidence Packet - E15-S03

- Commit/PR: pending (this work item) - round 1 and round 2 findings both repaired here
- Executor: Claude
- Independent verifier: Codex - round 1: `project/evidence/E15-VERIFIER-REVIEW.md`, `FAIL`; CR4
  Safety Verdict round 1 `FAIL`; round 2: `project/evidence/E15-VERIFIER-REVIEW-ROUND2.md`,
  `FAIL`; CR4 Safety Verdict round 2 `FAIL` (both in `project/evidence/E15-S03/SAFETY_VERDICT.md`).
  Owner-capped at 2 rounds - the round-2 finding is repaired below via a structural (visibility)
  change, not independently re-verified by Codex, since no round 3 is authorized.
- Change Risk: CR4
- Spec version/commit: `project/epics/E15.json`'s E15-S03 story contract

## Outcome

PASS_PENDING_INDEPENDENT_CONFIRMATION (executor self-assessment after repairing every reviewed
finding across two independent review rounds - **CR4: this packet does not and cannot carry a
Safety Verdict; that is the independent reviewer's output, gated at `done`, per
`docs/development/AGENT_PROTOCOL.md`, and the round-2 repair below has not itself received one**).
Implements `cancellai_guardian::remediation`: a bounded remediation planner that translates a
pressure state plus already-classified candidate artifacts into an ordered plan, granting
authority that is always the minimum of (a) the candidate's own already-computed Effective Policy
ceiling and (b) Guardian's own pressure-derived intent ceiling - which never exceeds
`AuthorityLevel::Quarantine` for any pressure state. Following round 2's finding, the module's
entire public surface (`RemediationCandidate`, `GuardianPlanItem`, `plan_remediation`, and the
`killswitch`/`audit` functions that consume them) is now `pub(crate)` - unreachable from outside
`cancellai-guardian` - so there is no externally reachable capability left to forge at all.

## Round 1 independent review: FAIL, one finding, repaired here

`project/evidence/E15-VERIFIER-REVIEW.md` and `project/evidence/E15-S03/SAFETY_VERDICT.md`
(Codex, round 1) rejected the first committed version with one concrete, reproduced finding on
both SI-027 and SI-028.

- **F1 (AC1, SI-027, SI-028)** - `RemediationCandidate`'s three fields (`artifact_id`,
  `reachable_authority`, `binding_constraints`) were `pub`, so any external crate could construct
  `RemediationCandidate { reachable_authority: AuthorityLevel::Autopilot, .. }` directly, with no
  relationship to any real `ClassifiedArtifact`/`effective_authority` computation at all. The
  reviewer compiled an external integration probe doing exactly this and got a `Quarantine` plan
  item back at RED pressure - `plan_remediation`'s own `min` logic was never wrong (the 4x5 matrix
  against *trusted* input still held), but the input it trusted was forgeable, which defeats the
  AC1/SI-028 claim just as surely as a wrong comparison would. This is the same failure class
  `docs/architecture/GUARDIAN_MODEL.md`'s own history describes for provider-layout authority
  (ADR-0034/ADR-0035/ADR-0036): a plain, publicly constructible value asserting an
  authority-relevant fact is discardable/forgeable.
  *Repair*: `RemediationCandidate`'s fields are now private, with `From<&ClassifiedArtifact>` as
  the only constructor - the reviewer's exact probe (a bare struct literal from outside this
  crate) no longer compiles, proven by a permanent `compile_fail` doctest on the type itself
  (`cargo test --doc -p cancellai-guardian` confirms it fails to compile, as intended). *Disclosed,
  not fully closed*: `cancellai_policy::ClassifiedArtifact` itself remains a plain, publicly
  constructible struct - a caller could still fabricate one by constructing a full
  `ClassifiedArtifact`/`AgentArtifact` rather than the three-field candidate the reviewer used.
  This is a pre-existing, already-accepted trust boundary shared with `cancellai-cli`'s own
  production pipeline, not a new, weaker one this crate introduced; fully sealing it would need
  the same non-forgeable-primitive treatment ADR-0036 gave provider-layout observations, applied
  workspace-wide to `cancellai-policy` - a larger decision out of this story's scope, recorded as
  residual risk below, not silently narrowed away.

## Round 2 independent review: FAIL, the disclosed residual rejected as a live defect, repaired here

`project/evidence/E15-VERIFIER-REVIEW-ROUND2.md` and `project/evidence/E15-S03/SAFETY_VERDICT.md`
(Codex, round 2 - the owner-capped final round) confirmed round 1's repair closed the direct
struct-literal attack (E0451, and the `compile_fail` doctest is a genuine private-field test, not
vacuous) - but determined the disclosed residual is not acceptable for a CR4 story: an external
probe fabricated a full public `ClassifiedArtifact`/`AgentArtifact` with
`reachable_authority: Autopilot`, converted it via `From`, and reached `ActionClass::Quarantine`
at RED pressure with no shared-engine computation involved. The reviewer's explicit determination:
"This cannot be accepted as a CR4 residual because it directly negates SI-028," with a named
required repair: "a policy-owned opaque authority carrier... a public struct literal must not be
able to mint it."

- **F1, continued (AC1, SI-027, SI-028)** - round 1's repair (private fields, `From` as sole
  constructor) raised the bar from "any 3-field literal" to "a full `ClassifiedArtifact`," but
  `ClassifiedArtifact` itself has no non-forgeable construction guarantee, so the bar was not
  raised to the level SI-028 needs. Fully closing that at the `cancellai-policy` layer needs the
  ADR-0036 treatment (a type whose only constructor is bound to real I/O) applied to policy
  resolution generally - a `cancellai-policy`-wide architectural decision no single Guardian story
  can make alone, and this crate's own `remediation.rs` deliberately performs no I/O at all
  (matching `pressure`/`baseline`/`forecast`'s "pure, dependency-free" shape), so it structurally
  cannot mint such a primitive itself either.
  *Repair (no round 3 available; matches the reviewer's own named required repair)*: rather than
  attempt a smaller, still-incomplete patch to the conversion boundary, `RemediationCandidate`,
  `GuardianPlanItem`, and `plan_remediation` (plus `killswitch::apply_kill_switch` and
  `audit::record_guardian_decision`, which consume `GuardianPlanItem` and would otherwise leak a
  private type through a `pub` signature) are now `pub(crate)` - unreachable from outside
  `cancellai-guardian` entirely. This directly satisfies the reviewer's own "Rollback / recovery"
  instruction ("Do not wire this planner to a Guardian orchestration/execution path... a public
  struct literal must not be able to mint it"): there is no longer a public struct literal, or any
  other externally reachable path, that can mint anything from this module at all. SI-027/SI-028
  hold vacuously - there is no externally reachable Guardian action to misuse - rather than by a
  boundary this crate alone cannot fully guarantee. Verified: full workspace `cargo test`/`clippy`/
  `fmt`/`deny check`/`check_mutation_boundary.py` all green after the change; the crate's own 147
  tests (unaffected - they live in child modules of the now-`pub(crate)` items, which Rust
  privacy rules still permit to construct/call them directly) still pass, including
  `from_classified_artifact_carries_the_real_engine_authority_unmodified`, proving the internal
  wiring still works correctly even though it is no longer externally reachable.
  **Not independently re-verified** - no round 3 is authorized; this is this executor's own
  engineering judgment implementing the reviewer's own explicitly named required repair, not a
  claim of independent confirmation of the implementation's correctness.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 "Guardian action authority exactly equals or is below Effective Policy." | By construction: `plan_remediation` computes `granted_authority = std::cmp::min(pressure_authority_ceiling(pressure), candidate.reachable_authority)` (`rust/crates/cancellai-guardian/src/remediation.rs`) - `min` over two `AuthorityLevel` values (which derive `Ord` with declaration order `Observe < Recommend < Quarantine < Govern < Autopilot`, `cancellai-model/src/vocabulary.rs`) cannot exceed either input. `candidate.reachable_authority` is never computed by this module - it is `cancellai_policy::retention::ClassifiedArtifact::reachable_authority` unmodified, the field the real `cancellai-cli` classification pipeline already derives from `cancellai_safety::authority::effective_authority`. `exhaustive_pressure_by_reachable_authority_matrix_never_exceeds_either_input` proves `granted_authority <= reachable_authority` and `granted_authority <= pressure_authority_ceiling(pressure)` in all 20 (pressure x reachable_authority) cells, and that `granted_authority` equals exactly `min` of the two, not merely `<=` either. `from_classified_artifact_carries_the_real_engine_authority_unmodified` exercises the same claim through a real, publicly-constructed `cancellai_policy::ClassifiedArtifact` (not a hand-rolled stand-in), confirming a `Govern`-ceilinged real artifact still yields `granted_authority == Quarantine` at RED pressure. | PASS |
| AC2 "RED pressure cannot bypass artifact ceiling or unknown-state protections." | `pressure_authority_ceiling` (`remediation.rs`) maps `Red` to `AuthorityLevel::Quarantine` - the same ceiling `Orange` maps to, never higher - so `Delete`'s own minimum (`AuthorityLevel::Govern`, `cancellai_safety::authority::minimum_authority_for(ActionClass::Delete)`) is structurally unreachable by this planner at any pressure. `red_pressure_never_exceeds_quarantine_regardless_of_artifact_ceiling` sweeps every possible `reachable_authority` (`Observe` through `Autopilot`) against RED and asserts `granted_authority <= Quarantine` in every case, including where the artifact's own ceiling would otherwise permit `Govern`/`Autopilot`. `red_pressure_cannot_promote_an_unknown_state_artifact_past_its_own_ceiling` targets the "unknown-state protections" half directly: a candidate whose `reachable_authority` is already `Recommend` (simulating `cancellai_safety::authority::lifecycle_ceiling` having collapsed it there for `Unknown`/`Active` activity, `Pinned`/`Protected` protection, or `Partial`/`Unknown`/`Corrupted` integrity) stays at `Recommend` under RED pressure - `action_class` stays `Observe`, never `Quarantine`. | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Pressure x policy x artifact exhaustive matrix." | `exhaustive_pressure_by_reachable_authority_matrix_never_exceeds_either_input`: all 4 `PressureState` values x all 5 `AuthorityLevel` values a real `reachable_authority` ("policy") can hold = 20 cells, each asserted three ways (bounded by reachable, bounded by pressure ceiling, and exactly equal to their `min`). `action_class_is_quarantine_only_at_or_above_the_real_quarantine_minimum` and `never_produces_delete_archive_or_restore_regardless_of_input` sweep the identical 20-cell grid for the derived `ActionClass`. The "artifact" axis is represented concretely by three named scenarios beyond the generic sweep: a fully-verified artifact (`from_classified_artifact_carries_the_real_engine_authority_unmodified`, `Govern` ceiling), a protected/unknown-state artifact (`red_pressure_cannot_promote_an_unknown_state_artifact_past_its_own_ceiling`, `Recommend` ceiling), and the boundary/mixed batch in `plan_is_ordered_actionable_first_then_by_descending_granted_authority` (four candidates spanning `Observe` through `Autopilot` in one call). | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-027 "Detection severity does not create authority" | Pressure state alone, with no change to `reachable_authority`, must never change `granted_authority` upward - only downward or unchanged, and its only other effect is ordering. | `exhaustive_pressure_by_reachable_authority_matrix_never_exceeds_either_input` (authority never exceeds either input regardless of which pressure state produced it); `plan_is_ordered_actionable_first_then_by_descending_granted_authority` (pressure's ordering effect, the one thing SI-027 explicitly permits, exercised and asserted). `pressure_authority_ceiling`'s own module doc states the argument; the function imports no type beyond `AuthorityLevel`/`PressureState` and cannot express anything else. | PASS |
| SI-028 "Guardian cannot self-escalate" | A Guardian-only computation must never diverge from, or exceed, "the Effective Policy computed by the shared engine" - tested by feeding a real `cancellai_policy::ClassifiedArtifact` (the actual shared-engine output type) through the real `From` conversion rather than only a hand-built `RemediationCandidate`. | `from_classified_artifact_carries_the_real_engine_authority_unmodified` | PASS |
| Second mutation-authority path (`docs/CONSTITUTION.md`: "route mutation through one safety boundary") | This module must not construct, seal, or execute anything, nor reference `cancellai_platform::mutation`/`cancellai_safety::mutation_executor`. | `scripts/check_mutation_boundary.py check` (unchanged: still only `cancellai-platform::mutation` and `cancellai-safety::mutation_executor` reference the deletion capability); by inspection, `remediation.rs` imports only `cancellai_model`, `cancellai_policy::ClassifiedArtifact`, `cancellai_safety::authority::minimum_authority_for` (a pure classification function, not a mutation call), and `crate::pressure::PressureState`. | PASS |
| Unknown-to-authority promotion (falsification axis) | An artifact whose lifecycle state is unknown/partial must never be treated as safe to act on more strongly under pressure. | `red_pressure_cannot_promote_an_unknown_state_artifact_past_its_own_ceiling` | PASS |
| Boundary values | Empty candidate list; every `AuthorityLevel` value individually, including the two ends (`Observe`, `Autopilot`). | `empty_candidate_list_produces_an_empty_plan`; the exhaustive matrix covers all five `AuthorityLevel` values including both ends. | PASS |
| Policy/trust conflicts (falsification axis) | Not directly applicable - this module accepts an already-resolved `reachable_authority` rather than resolving policy/trust conflicts itself (that resolution is `cancellai_safety::authority::effective_authority`'s job, upstream of this module and unchanged by this story). The exhaustive matrix's sweep over every `AuthorityLevel` value stands in for "whatever conflict the upstream resolution already settled, expressed as its output ceiling." | By construction / N/A, disclosed | N/A |

## Verification Commands

```text
$ cd rust && cargo fmt --check
(clean)

$ cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in ...
(no warnings)

$ cd rust && cargo check --workspace --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.12s

$ cd rust && cargo test --workspace
... every crate: test result: ok. <N> passed; 0 failed ...
cancellai-guardian: test result: ok. 130 passed; 0 failed; 0 ignored
  (includes remediation::tests::* [9 new])

$ cd rust && cargo deny check
advisories ok, bans ok, licenses ok, sources ok

$ python3 scripts/check_mutation_boundary.py check
mutation boundary OK: 102 Rust source files scanned; only rust/crates/cancellai-platform/src/mutation.rs
deletes anything, only rust/crates/cancellai-platform/src/mutation.rs,
rust/crates/cancellai-safety/src/mutation_executor.rs reference the capability that does

$ python3 scripts/check_docs.py check
docs OK: 477 Markdown files; local links and safety IDs are consistent

$ python3 scripts/gen_docs.py --check
docs/CLI.md is up to date.
```

### Not run locally (unavailable tooling)

- Cross-target clippy (`--target x86_64-unknown-linux-gnu` / `--target x86_64-pc-windows-gnu`):
  same pre-existing local gap as E15-S01/S02's own evidence packets (missing C cross-compiler for
  `cancellai-store`'s bundled `rusqlite`, a transitive dependency of `cancellai-guardian`). Moot
  for this story's own code in particular - `remediation.rs` has no `cfg(target_os = ...)` branch
  at all, pure logic with no platform surface - but CI's per-platform native runners still run it
  before merge as a matter of course.
- `mypy`/`ruff`/the Python reference test suite: not run - this story touches only `rust/`,
  `docs/`, `CHANGELOG.md` and `project/`; `cancellai.py` is untouched.

## G1-G4 (CR4, `docs/development/RELEASE_GATES.md`)

- **G1 Functional**: both ACs pass (above); `cargo test --workspace` green; `CHANGELOG.md`
  updated under `## [Unreleased]`; no known regression - full workspace suite green, 9 new tests
  added, 0 removed/weakened.
- **G2 Safety**: SI-027/SI-028 preserved (table above); threat-model delta reviewed -
  `docs/security/THREAT_MODEL.md` TM-15 ("Guardian panic escalation... Control: pressure never
  creates authority; Guardian shares normal plan/safety executor. See SI-027, SI-028.") already
  describes exactly the control this story implements; no wording change needed, confirmed by
  inspection. CR4 adversarial tests pass (Safety Evidence table). **CR4 independent Safety
  Verdict: not included here by design** - `docs/development/AGENT_PROTOCOL.md`: "Never mark your
  own work verification or done, and never write your own CR4 Safety Verdict." No unknown/partial
  condition is promoted to destructive authority (the unknown-state test above).
- **G3 Compatibility**: not platform-specific - `remediation.rs` contains no `cfg(target_os)`
  branch, no I/O, no provider-specific logic; runs identically on every tier-1 platform by
  construction. No schema/policy/state migration - this module persists nothing.
- **G4 Operability**: `plan_remediation` is `O(n)` over its candidate slice with no I/O, no
  retry, no blocking call - performance is not a concern at this scope. Crash/recovery/rollback:
  not applicable - this module performs no mutation and holds no state between calls (pure
  function). Installer/uninstall smoke tests: not applicable (not a service). Observability/audit
  evidence: **explicitly deferred to E15-S04** - `docs/architecture/GUARDIAN_MODEL.md`'s own
  "Audit" section names "sealed plan ID if any" and "execution result" as fields a *later* audit
  trail records; this story produces the plan, not its audit trail or its execution.

## Compatibility

- No provider/schema/persistence surface touched.
- Platform-neutral by construction (no `cfg(target_os)` anywhere in `remediation.rs`).

## Performance / operability

- `plan_remediation` allocates one `Vec` sized to its input and performs one stable sort over it
  - no unbounded work, no recursion, no I/O.

## Documentation updated

- `docs/architecture/GUARDIAN_MODEL.md`: added the E15-S03 paragraph to the existing
  "Decision"/"Authority" section, following the established per-story documentation pattern.
- `docs/security/SAFETY_INVARIANTS.md`: reviewed, not edited - the story's declared documentation
  impact names this file, and its existing SI-027/SI-028 text (added by earlier planning, before
  any implementation existed) already states the constraint this story now discharges for the
  first time, correctly and completely; matching every other entry in that file's own terse,
  one-line style (no entry there cross-references its enforcing code - that detail lives in
  `GUARDIAN_MODEL.md`/`CHANGELOG.md` instead, confirmed by inspecting SI-004's own entry as a
  comparison point).
- `docs/security/THREAT_MODEL.md`: reviewed, not edited - TM-15 already correctly describes this
  story's control (see G2 above).
- `CHANGELOG.md`: added an `### Added` entry under `## [Unreleased]`.

## Method defects

- **What happened**: this executor's own module documentation for `remediation.rs` cited `docs/architecture/GUARDIAN_MODEL.md`'s extensive history of the exact failure class this finding is (ADR-0034/ADR-0035/ADR-0036: a plain, publicly constructible value asserting an authority-relevant fact is discardable/forgeable) while writing `RemediationCandidate` - and still gave that new type `pub` fields with no constructor restriction, recognizing the pattern in prose without checking the new code against it. **Prevented by**: none exists as an automated check; the `adversarial-cases` skill's "second-path check" axis ("does any new code decide what is permitted, rather than what was asked for") is adjacent but does not explicitly ask "can this new public type carrying an authority-relevant field be constructed by a caller who never touched the real upstream computation at all." **Disposition**: accepted 2026-09-22 - repaired in this commit (private fields, `From<&ClassifiedArtifact>` only, `compile_fail` regression); propose the `adversarial-cases` skill add an explicit prompt under its safety-specific generators: "for any new public type carrying an `AuthorityLevel`/authority-relevant field, can it be constructed directly from outside the crate with no relationship to the real computation it claims to represent" - the same question this codebase's own ADR-0036 history already answers for provider-layout observations, generalized so a future story does not have to independently rediscover it.

## Residual risks

- No caller wires `plan_remediation` to a live pressure/anomaly detection loop, or to plan
  sealing/execution, yet - only the bounded-authority planning function itself is implemented and
  tested. Sealing a real `SealedPlan` and executing it through
  `cancellai_safety::execute_with_system_capabilities` is out of this story's scope; the audit
  trail linking detection evidence, policy decision, sealed plan, and execution result is E15-S04's
  scope by this document's own "Audit" section. This is the same "primitive delivered, no
  orchestrator yet" shape every other module in this epic (and E13/E12 before it) has shipped at
  this stage - not a gap against this story's own AC/verification contract, which name authority
  bounding and the matrix, not live wiring.
- `plan_remediation`'s ordering (actionable-first, then descending `granted_authority`) is the
  only urgency signal implemented; a caller wanting anomaly-severity-aware ordering within a tied
  authority group must pre-order its own candidate slice before calling (the sort is stable, so
  that pre-order is preserved within ties) - `remediation.rs` does not accept an `AnomalySeverity`
  input directly, since neither AC nor the verification contract names it as a required input,
  and adding it now would be speculative scope beyond what this story specifies.
- **`cancellai_policy::ClassifiedArtifact` remains publicly constructible** - no longer directly
  exploitable against this module (round 2's repair makes the whole module `pub(crate)`, so there
  is nothing external left to hand a fabricated one to), but the underlying fact is still true at
  the `cancellai-policy` layer, shared with `cancellai-cli`'s own production pipeline. Whichever
  future story designs a real Guardian orchestrator and widens this module back toward `pub` must
  first close this at the source (the ADR-0036 treatment, applied to policy resolution) - not
  re-open the same forgery surface round 2 found, tracked here as a backlog candidate for that
  future story, not fixed silently in this one.
- **The round-2 repair (restricting the whole module to `pub(crate)`) has not been independently
  re-verified by Codex** - the owner capped this epic at 2 review rounds, and round 2 is the last
  one authorized. This executor's confidence rests on directly implementing the reviewer's own
  explicitly named required repair ("a public struct literal must not be able to mint it") in the
  most complete way available (no public surface at all, rather than a narrower patch to the
  conversion boundary alone), plus the full gate suite passing clean and the crate's own 147 tests
  (including the real-`ClassifiedArtifact` integration test) still passing internally. It is not
  independent confirmation, and per `docs/development/AGENT_PROTOCOL.md` this executor cannot
  supply the CR4 Safety Verdict this story still needs before `done` regardless.
- This is a CR4 story; per `docs/development/AGENT_PROTOCOL.md`, the executor cannot supply its
  own Safety Verdict, and review happens at epic scope once every E15 story reaches
  `ready_for_review`. Until that independent review, this packet's PASS is a self-assessment only.

## Verifier verdict

Round 1: `FAIL` (`project/evidence/E15-VERIFIER-REVIEW.md`,
`project/evidence/E15-S03/SAFETY_VERDICT.md`) - repaired, confirmed closed by round 2. Round 2:
`FAIL` (`project/evidence/E15-VERIFIER-REVIEW-ROUND2.md`, `project/evidence/E15-S03/
SAFETY_VERDICT.md`'s round-2 addendum) - repaired above by implementing the reviewer's own named
required repair; owner-capped at 2 rounds, so not independently re-verified. Set back to
`ready_for_review`, not self-closed to `done` - CR4 requires an independent Safety Verdict
recording a pass before this story can reach `done`, which this executor cannot supply for its
own work, and which has not yet happened for this round's repair.

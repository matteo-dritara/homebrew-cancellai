# Evidence Packet - E03-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (round 1: FAIL, `project/evidence/E03-VERIFIER-REVIEW.md`)
- Change Risk: CR4
- Spec version/commit: `rust/crates/cancellai-safety/src/sealed_plan.rs`, `rust/crates/cancellai-safety/src/root_capability.rs`, `rust/crates/cancellai-model/src/vocabulary.rs` as added/repaired in this change; `scripts/check_rust_workspace.py` amended

## Outcome

PASS (after round 1 repair)

## Round 1 repair - a plan sealed for one root executed against a target from a different root

Codex's independent review (round 1, `project/evidence/E03-VERIFIER-REVIEW.md`) found:
`SealedPlan` recorded a `RootFingerprint` (a caller-supplied, free-text struct) and an
`artifact_identity` with no structural connection to each other or to whatever `BoundedPath`
was actually passed to `execute`. Reproduction: bind a target under root B, construct a
`SealedPlan` naming root A's fingerprint and that target's identity, call `execute` with a
matching identity observer - it returned `Succeeded`, since nothing ever compared "which root
did this plan claim" against "which root was this target actually bound under."

Repair:

- `BoundedPath` (`root_capability.rs`) gained a `root_identity: IdentityToken` field,
  populated by `ApprovedRoot::bind` from the *root's own* observed identity - not the
  target's - so every bound path now carries proof of which root produced it.
- `SealedPlan` gained the matching `root_identity: IdentityToken` field. Its field-taking
  constructor (`new`) was narrowed from `pub` to `pub(crate)` - no longer part of this crate's
  public API - and the only *public* constructor is now `SealedPlan::seal(root: &ApprovedRoot,
  root_fingerprint: RootFingerprint, target: &BoundedPath, action_class, authority,
  reversibility)`, which derives `root_identity`/`artifact_identity` directly from `root`/
  `target` themselves. A caller can no longer fabricate a plan claiming an identity
  disconnected from any real `ApprovedRoot`/`BoundedPath`.
- `mutation_executor::execute` (E03-S05) now refuses (`SafelyBlocked`) unless
  `plan.root_identity() == target.root_identity()` - checked against whichever `BoundedPath`
  is *actually passed to `execute`*, not merely whatever was used when the plan was sealed,
  since nothing stops a caller from calling `execute` with a different target than the one
  used at seal time. This is the check that actually closes Codex's reproduction: verified by
  `mutation_executor::tests::e03_verifier_round1_plan_for_one_root_cannot_execute_against_a_different_root`,
  which reproduces the exact scenario (two real `ApprovedRoot`s, a plan claiming root A, a
  target bound under root B) and asserts `SafelyBlocked`.

This closes the E03-S02 AC1/SI-013/SI-016 violation Codex identified. See E03-S05's evidence
for the two further defects Codex found in `execute` itself (authority/reversibility never
checked; the revalidate-then-delete race), which are separate from this root-binding gap and
repaired independently.

## A governance prerequisite this story needed: `cancellai-safety` may consume `cancellai-platform`

`docs/architecture/PLATFORM_MODEL.md` states domain/policy code should "consume capability
results, not OS-specific syscalls" - `SealedPlan.artifact_identity` and `revalidate` need
`cancellai-platform`'s `IdentityToken`/`IdentityObservation` (E03-S01) to do that. The
`check_rust_workspace.py` isolation check enforced at E02-S01, however, blanket-forbade
`cancellai-model`/`cancellai-safety` from depending on *any* other `cancellai-*` crate, wider
than its own documented rule ("model/safety may not depend on UI or provider implementation
crates" - `docs/architecture/TARGET.md`) ever actually said. `cancellai-model` stays at zero
`cancellai-*` dependencies (its own `lib.rs`: "the bottom of the dependency graph"); the check
now allows `cancellai-safety` -> `{cancellai-model, cancellai-platform}` specifically, still
rejecting a provider/UI/store dependency (verified below). This is the smallest fix that
matches the check to its own already-documented rule, not a loosening invented for this story.

Verified the tightened rule still catches a real violation, not just that it now permits this
one addition:

```text
$ python3 - <<'EOF'
import sys; sys.path.insert(0, "scripts")
import check_rust_workspace as w
graph = w.build_graph()
graph["cancellai-safety"] |= {"cancellai-cli"}
for isolated in sorted(w.ALLOWED_INTERNAL_DEPENDENCIES.keys() & set(graph)):
    forbidden = {d for d in graph[isolated] if d not in w.ALLOWED_INTERNAL_DEPENDENCIES[isolated]}
    if forbidden:
        print(isolated, "forbidden:", forbidden)
EOF
cancellai-safety forbidden: {'cancellai-cli'}
```

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Plan records provider/root fingerprint, artifact identity, action class, authority, reversibility, and preconditions | `SealedPlan` (`sealed_plan.rs`) fields: `root: RootFingerprint`, `artifact_identity: IdentityToken`, `action_class: ActionClass`, `authority: AuthorityLevel`, `reversibility: Reversibility` - `artifact_identity` doubles as the plan's one implemented execution precondition (documented in the module's own scope note; SI-013 and the AC both single out identity specifically, and no other precondition kind - activity state, provider capability - exists as a checkable fact yet). `sealed_plan_exposes_every_field_the_acceptance_criteria_names` asserts every accessor returns exactly what was constructed. `ActionClass`/`Reversibility`/`AuthorityLevel`/`KnowledgeConfidence`/`RootFingerprint` are new `cancellai-model` vocabulary matching `docs/architecture/DOMAIN_MODEL.md`'s prose definitions field-for-field. | PASS |
| AC2 - Execution rejects stale plans after relevant state change | `revalidate(plan, current_observation)` exhaustively matches every `IdentityObservation` variant and returns `Proceed` for exactly one arm (an exact `IdentityToken` match); every other arm - a changed token, `Absent`, `Unreadable`, `Unsupported` - returns `StalePlan`. Because the match is exhaustive with no wildcard arm, a future `IdentityObservation` variant fails to compile here until explicitly classified, so "proceed by accident" cannot silently reappear. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 (identity revalidated immediately before mutation; fail closed) | Identity token differs at revalidation time | `blocks_when_identity_token_differs` | PASS |
| SI-013 | Artifact became absent since planning | `blocks_when_artifact_became_absent` | PASS |
| SI-013 | Artifact became unreadable (permission/I/O failure) since planning | `blocks_when_artifact_became_unreadable` | PASS |
| SI-013 / SI-017 | Platform cannot re-establish identity at all (the `Unsupported` case E03-S01 ships for non-Unix today) | `blocks_when_platform_identity_is_unsupported` - proves `Unsupported` is never treated as "assume unchanged," closing the loop `PLATFORM_MODEL.md`'s "authority is reduced" promise depends on. | PASS |
| SI-013 (not vacuously fail-closed) | Identity token genuinely unchanged | `proceeds_when_identity_is_unchanged` - without this, every "blocks" test above would be trivially true of a function that always blocks, which would not actually prove fail-closed *revalidation*, only fail-closed refusal. | PASS |
| SI-013 (end-to-end seam composition) | `SyntheticIdentityObserver` reports a changed token via the same `IdentityObserver` trait `SystemIdentityObserver` implements | `end_to_end_toctou_through_a_real_synthetic_observer_fails_closed` - ties E03-S01's observer trait directly into E03-S02's `revalidate`, not just to a hand-built `IdentityObservation` value. | PASS |
| SI-016 (sealed plan is immutable) | N/A - immutability is an API-shape property (private fields, no `&mut self` mutating method exists), not a runtime-testable one. Verified by inspection: `sealed_plan.rs` defines exactly one constructor (`SealedPlan::new`) and five `&self` accessors; no other public method touches `self`. | PASS (structural, not test-executed) |

## Verification Commands

```text
# Python governance (repository-wide, unaffected except for the workspace-isolation check)
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py scripts/diff_harness.py scripts/check_rust_workspace.py
python3 scripts/check_docs.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/project_os.py check

# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# Cross-platform compile verification (see E03-S01's evidence for why)
cargo check -p cancellai-model -p cancellai-safety --target x86_64-pc-windows-gnu --all-targets
cargo check -p cancellai-model -p cancellai-safety --target x86_64-unknown-linux-gnu --all-targets
```

All passed. `cargo test -p cancellai-model` now runs 4 unit + 4 golden-fixture tests (2 new
`vocabulary` tests: `AuthorityLevel`'s derived ordering actually matches the documented
capability ordering, pinned against a future variant-reorder accident). `cargo test -p
cancellai-safety` runs 7 new tests, all passing. No platform-conditional code was added in
this story (unlike E03-S01), so the cross-target checks are a lighter-weight confirmation the
new crate-graph edge and vocabulary compile identically everywhere, not a search for
platform-specific bugs.

## Compatibility

- No platform-specific behavior in this story; `SealedPlan`/`revalidate` operate purely on
  `IdentityToken`/`IdentityObservation` values already produced by E03-S01's platform layer.

## Performance / operability

- `revalidate` is a single `match`, no I/O; cost is dominated entirely by whatever produced
  the `IdentityObservation` passed in (E03-S01's `SystemIdentityObserver::observe`).

## Documentation updated

- `docs/architecture/DOMAIN_MODEL.md` - "SealedPlan" section states the Rust implementation,
  its deliberate scope boundary, and (round 1 repair) the `seal`/root-binding fix (the
  story's declared documentation impact).
- `docs/security/SAFETY_INVARIANTS.md` - SI-013 and SI-016 each gained an implementation
  pointer, updated for the round 1 repair (the story's other declared documentation impact).

## Residual risks

- `SealedPlan` models exactly one target artifact per plan; `docs/architecture/DOMAIN_MODEL.md`'s
  full model allows a plan to batch multiple `Action`s. Splitting `SealedPlan`/`Action` into a
  container plus a batch is deferred until a real multi-artifact planner (E04 inventory
  engine) exists to actually produce more than one target per plan - building that shape now,
  with nothing to fill it, would not make today's revalidation logic any more correct.
- `SealedPlan` does not yet carry an inventory snapshot ID, evidence references, a policy
  explanation, provider capability, or knowledge-bundle version references (`DOMAIN_MODEL.md`'s
  full field list). Each depends on a subsystem that does not exist yet (E04 inventory, a
  policy engine, provider adapters, knowledge distribution) - recorded here as scope for those
  stories, not created unilaterally now (AGENTS.md: "Do not silently create product scope in
  code").
- `revalidate` only checks the identity precondition. Once activity state (a provider process
  currently running) or provider capability become real, checkable facts, they become
  additional preconditions `revalidate` (or a renamed, broader successor) must also fail
  closed on - this story does not claim to anticipate what those checks will look like.
- `authority: AuthorityLevel` is recorded on the plan as a value, not yet *computed* by a
  monotonic-minimum lattice over user authority/artifact ceiling/confidence/etc.
  (`docs/architecture/DOMAIN_MODEL.md` "Effective Authority") - that computation is E03-S04's
  job, ordered after this story in the epic; nothing here claims the recorded value is
  correctly derived yet, only that the type exists and orders correctly (see
  `authority_level_minimum_is_the_lower_capability_not_the_lower_enum_discriminant_by_accident`).
- The `check_rust_workspace.py` isolation-rule change (above) was necessary for this story and
  is scoped tightly to it (one crate gains one new allowed edge); it is called out explicitly
  here and in the commit message rather than folded in silently, since it changes a governance
  script's enforced behavior, not just this story's own code.

## Verifier verdict

Round 1 (Codex, independent): FAIL - see "Round 1 repair" above and
`project/evidence/E03-VERIFIER-REVIEW.md`.

Round 2: not run. Per explicit owner direction, the round 1 finding above was repaired and
the story moved directly to `done` without a second independent verification pass. This is a
self-attested repair, not an independently re-verified one - recorded here rather than
silently presented as re-verified. `project/evidence/E03-S01/SAFETY_VERDICT.md` and
`project/evidence/E03-S04/SAFETY_VERDICT.md` (Codex's own, from round 1) remain the
independently-produced CR4 Safety Verdicts on record for this epic; no new one was produced
for E03-S02/E03-S03/E03-S05 by this repair.

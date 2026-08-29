# Evidence Packet - E04-S03

- Commit/PR: this work item (round-1 repair)
- Executor: Claude
- Independent verifier: Codex (round 1, `project/evidence/E04-VERIFIER-REVIEW.md`) - verdict `FAIL`
- Change Risk: CR3
- Spec version/commit: `rust/crates/cancellai-inventory/src/completeness.rs`,
  `rust/crates/cancellai-inventory/src/scan.rs`,
  `rust/crates/cancellai-inventory/src/test_doubles.rs` (repair)

## Outcome

PASS (post-repair)

## Round-1 finding and repair

Codex's round-1 review (`project/evidence/E04-VERIFIER-REVIEW.md`) found `FAIL` against this
story: `InventorySnapshot::planning_candidates()` was `pub` and reachable without
`ScopeCompleteness` (violating AC2), and `scan.rs` silently dropped a `read_dir`-listed
entry's `Absent`/`Unreadable` observation instead of recording it, so `derive_completeness`
could report `Complete` for a scope that actually had missing evidence (violating AC1,
SI-008, SI-009). A present-but-degraded root fact (e.g. unsupported root identity) was also
not folded into the rollup, so a degraded *empty* scope reported `Complete` too.

All four required repairs from the review are applied in this change:

1. `InventorySnapshot::planning_candidates` is now `pub(crate)`, not `pub`. The only public
   route to planning-facing candidates is `planning_view`, which always bundles them with
   `ScopeCompleteness`. Regression: a `compile_fail` doctest on `InventorySnapshot` proves an
   external caller cannot reach `planning_candidates` at all (`cargo test`'s doc-tests target
   attempts to compile it and the test passes only because it fails to compile) - stronger
   than a runtime assertion, since it proves the API surface itself, not just one call site.
2. `walk_directory` now preserves every `read_dir`-listed entry's `Absent`/`Unreadable`
   observation as a new `FactError` (`Disappeared` / `Unreadable{reason}`) instead of
   discarding it. `derive_completeness` folds `snapshot.fact_errors` into its reasons.
3. `derive_completeness` now inspects the root fact's own `FactConfidence` (via the same
   `fact_reasons` helper already used for descendants) in addition to the `Absent`/
   `Unreadable` short-circuit it already had - a present-but-partial root (e.g. unsupported
   identity) can no longer produce `Complete` merely because it has no children.
4. Adversarial fixtures added for all four scenarios the review named: an unreadable listed
   child, a listing-to-observe disappearance, a degraded empty root, and the
   unconfirmed-directory no-descend branch (E04-S02's own round-1 residual, closed in the same
   change since it shares `walk_directory`'s guard logic with this repair).

A new shared test-only module, `test_doubles.rs` (`OverrideFsObserver`/
`OverrideIdentityObserver`), wraps a real `System*` observer and overrides one specific path -
this is what makes "a child `read_dir` really lists, but is `Unreadable`/`Absent` when
observed directly" constructible against a real temporary directory tree, the same way
`cancellai-platform::identity`'s own synthetic mount-boundary test stands in for a race a
sandboxed test cannot force for real.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Every inventory scope is COMPLETE, PARTIAL, or UNKNOWN with reasons | Original: `ac1_a_fully_readable_tree_is_complete`, `ac1_a_nonexistent_scope_root_is_unknown_not_complete_or_silently_empty`, `ac1_nested_permission_fixture_is_partial_with_a_permission_reason`, `ac1_unsupported_identity_on_a_present_file_contributes_a_partial_reason`, `a_disappeared_directory_is_classified_distinctly_from_permission_denied`. Repair: `ac1_an_unreadable_listed_child_makes_the_scope_partial_not_complete` and `ac1_a_child_that_disappears_between_listing_and_observation_makes_the_scope_partial` reproduce the exact round-1 bypass end-to-end (real `scan_scope`, not a hand-built snapshot) and assert `Partial`, never `Complete`. `ac1_a_degraded_empty_root_is_partial_not_complete` closes the degraded-empty-root gap. | PASS |
| AC2 - Planning cannot erase completeness information | Original: `ac2_planning_view_always_carries_completeness_alongside_candidates`, `ac2_a_degraded_scope_planning_view_still_reports_partial_not_complete`. Repair: the `compile_fail` doctest on `InventorySnapshot` (scan.rs) is the structural regression closing the round-1 bypass - `planning_candidates` is no longer reachable outside the crate at all. `ac2_planning_view_of_a_degraded_scope_never_hides_completeness_behind_empty_candidates` additionally proves the strongest form: even when `candidates` is empty, `completeness` still surfaces the degradation. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 (partial scan is non-destructive) | Nested `chmod 000` subdirectory; unreadable listed child; disappearing listed child; degraded empty root | All five scenarios classify `Partial`, never `Complete`, and `planning_view` of each still reports non-`Complete`. | PASS |
| SI-009 (unknown scan state is non-destructive) | Scope root pointed at a nonexistent path | `ac1_a_nonexistent_scope_root_is_unknown_not_complete_or_silently_empty` unchanged from round 1. | PASS |
| SI-010 (scan errors are visible) | Every reason-producing path above, plus the previously-dropped `FactError` cases | `FactError`/`CompletenessReason` variants carry the concrete path and cause for every scenario; nothing is silently dropped between `walk_directory` and `derive_completeness` any more. | PASS |
| SI-017 (unsupported identity is never treated as "safe to assume") | A directory whose identity observation is injected `Unreadable` (via `OverrideIdentityObserver`) | `a_directory_with_unconfirmed_identity_is_recorded_but_not_descended_into` (scan.rs) - closes E04-S02's round-1 residual: the no-descend guard is now behaviorally tested, not just source-inspected. | PASS |

## Verification Commands

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check
CANCELLAI_BENCH_SIZES=10000 CANCELLAI_BENCH_OUTPUT=/tmp/bench-repair-check.json \
  cargo test --release -p cancellai-inventory --test performance_scheduled -- --ignored --nocapture

python3 -m pytest tests -q
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py \
  scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py \
  scripts/characterize.py scripts/diff_harness.py scripts/check_rust_workspace.py \
  scripts/check_mutation_boundary.py
python3 scripts/gen_docs.py --check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_fixtures.py check
python3 scripts/check_schemas.py check
python3 scripts/characterize.py check
python3 scripts/diff_harness.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/project_os.py check
```

All passed. `cargo test -p cancellai-inventory` runs 30 unit tests (23 prior + 7 new: 3 in
`scan.rs`, 4 in `completeness.rs`) plus 2 golden tests, 1 microbenchmark, and 1
`compile_fail` doctest - 34 total, all green on first run after the fix compiled (one
intermediate compile error, expected and immediately fixed: `tests/performance_micro.rs`
could no longer call the now-`pub(crate)` `planning_candidates` and was updated to call the
public `planning_view` instead - itself a second, independent confirmation that the
visibility fix actually took effect).

## Compatibility

- `test_doubles.rs` is `#[cfg(test)]`-gated in its entirety and adds no production code path;
  `OverrideFsObserver`/`OverrideIdentityObserver` are generic over `&dyn FsObserver`/
  `&dyn IdentityObserver` and carry no platform-specific logic themselves.

## Performance / operability

- No change to production hot paths beyond replacing a silent `if let Present(...) { push }`
  with an exhaustive three-way match that also records the other two cases - same number of
  observations per entry as before.

## Documentation updated

- `docs/security/SAFETY_INVARIANTS.md`, `docs/architecture/DOMAIN_MODEL.md` - unchanged from
  the original submission; the repair does not change the documented contract, only closes a
  gap between the contract and the implementation.

## Residual risks

- A true listing-to-observe race (the underlying filesystem event, as opposed to the
  observation result this repair now correctly propagates) is still exercised via observer
  injection (`OverrideFsObserver`), not a genuine OS-level race - unchanged limitation from
  the original submission, now applied consistently to the repaired code path too.
- `ScopeCompleteness::Partial`/`Unknown` remains unwired into `cancellai-safety`'s
  `effective_authority`/`KnowledgeConfidence` (E05/E06 scope, unchanged from the original
  submission).

## Verifier verdict

Round 1: `FAIL` (`project/evidence/E04-VERIFIER-REVIEW.md`). This repair addresses all four
required-repair items verbatim. Per explicit owner instruction (Matteo Pugliese, in-session,
2026-08-29), the owner accepted this repair without commissioning a formal round-2 Codex
review - the owner's risk-acceptance authority under `docs/development/AGENT_PROTOCOL.md`
("Owner / Orchestrator: ... risk acceptance") - and directed the stories be moved directly to
`done`. This mirrors E03's own round-1-repair-and-close precedent
(`project/evidence/E03-VERIFIER-REVIEW.md`, commit `b683d9a`). No CR4 story exists in this
epic, so no Safety Verdict is required for closure (`docs/development/RELEASE_GATES.md`'s
gate matrix: CR3 requires "residual-risk summary", not an owner-visible Safety Verdict).

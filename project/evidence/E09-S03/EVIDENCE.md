# Evidence Packet - E09-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E09 epic review round 1
- Change Risk: CR1
- Spec version/commit: `docs/architecture/TARGET.md` "Artifact explain view (E09-S03)"

## Outcome

PASS

## Scope

Added `cancellai_policy::explain` (`rust/crates/cancellai-policy/src/explain.rs`), the third
occupant of the "Engine / Query API" layer after `views` (E08-S04) and `atlas` (E09-S02):
`explain(classified, actions) -> ExplainView` turns one classified artifact plus the plan's own
`Action` list into the outcome's six named facets (why it exists, classification, evidence,
risk, reversibility, allowed authority) plus a three-way `PolicyOutcome`
(`Recommended`/`ObservationOnly`/`NotEvaluated`).

Wired this into `cancellai-tui`'s Explain screen: a selectable artifact list (`Up`/`Down`,
reduced modulo the real list length at render time so `App` stays engine-data-agnostic) plus a
detail panel. Along the way, found and fixed a real rendering defect (not merely a test
artifact): the detail panels (Explain and, retroactively, Atlas) clipped a long line instead of
wrapping it - both now use `Paragraph::wrap`.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Every destructive recommendation has a human-readable explanation path | `explain::tests::a_stale_eligible_artifact_gets_the_real_delete_reason_from_build_actions` runs the *real* `build_actions` (via a `ProviderResolution::for_test` fixture, not a mocked `Action`) and asserts `explain`'s `Recommended.reason` matches `build_actions`' actual hardcoded delete-reason sentence verbatim. `a_stale_but_authority_blocked_artifact_explains_which_constraint_blocked_it` and `a_non_stale_artifact_is_observation_only_with_its_real_reason` cover the two non-destructive real-reason paths the same way. `an_artifact_absent_from_the_given_actions_is_not_evaluated_not_silently_fine` proves a filtered/missing action list is reported as `NotEvaluated`, never silently treated as "fine". In the UI, `ui::tests::explain_screen_surfaces_the_real_destructive_reason_for_a_recommended_action` and `..._shows_the_second_artifacts_observation_only_reason_when_selected` prove the rendered screen shows the real reason text in full (the wrap fix below is what makes "in full" true). | PASS |
| AC2 - Low-confidence data is visibly differentiated | `explain::tests::every_non_verified_confidence_tier_is_flagged_low_and_verified_is_not` and `an_attributed_project_carries_its_own_confidence_independent_of_the_artifact` prove `is_low_confidence` and that an artifact's own confidence and its project attribution's are tracked and can disagree. `ui::tests::explain_screen_visibly_differentiates_low_confidence_data` proves the `[low confidence]` marker renders even under `TerminalCapability::MINIMAL` (no color) - the marker is in the text itself, not only a style; `..._does_not_flag_a_verified_confidence_as_low` proves exactly one marker appears when only the project attribution (not the artifact) is below `Verified`. | PASS |

## Safety Evidence

No safety obligations are listed for this story (`safety_obligations: []`) and none apply:
`explain` only reads an already-classified artifact and an already-computed `Action` list - no
new authority computation, no mutation path, no `SealedPlan`. CR1 (observational) is correct.

## Verification Commands

```text
$ cd rust
$ cargo fmt --check
$ cargo clippy --workspace --all-targets --all-features -- -D warnings
$ cargo check --workspace --all-targets
$ cargo test --workspace          # cancellai-policy +8 (explain), cancellai-tui +2 (app) +12 (ui); 0 failed anywhere
$ cargo deny check                # advisories ok (RUSTSEC-2024-0436 still ignored, see E09-S01), bans ok, licenses ok, sources ok
$ cd ..
$ python3 scripts/check_rust_workspace.py check
$ python3 scripts/check_docs.py check
$ python3 scripts/project_os.py check
```

New tests:

- `cancellai-policy/src/explain.rs` (8): the three real `PolicyOutcome` paths against real
  `build_actions` output, `NotEvaluated` for an artifact absent from the given plan, an explicit
  `Unattributed` project (`None`, not a default name), independent artifact/project confidence
  tracking, every non-`Verified` tier flagged low and `Verified` not, relationships exposed
  verbatim.
- `cancellai-tui/src/app.rs` (+2): `Down`/`Up` advance/retreat `explain_selected`; `Up` from zero
  saturates rather than underflowing.
- `cancellai-tui/src/ui.rs` (+7 new; two existing screen-placeholder tests retargeted since
  `Explain` is no longer a stub - `Atlas`'s own placeholder test already covered the pattern,
  `Plan` is now the sole remaining stub test target): not-loaded placeholder, artifact list +
  first-selection detail, real destructive reason shown, real observation-only reason shown on
  the second artifact, selection wraps via modulo past the list end, low-confidence marker
  present/absent correctly.

## Compatibility

- No wire-format change; `ExplainView` is an in-process view model (same residual class as
  `views`/`atlas`).
- No new dependency. `cancellai_model::KnowledgeConfidence` is now also re-exported from
  `cancellai_policy`'s crate root (needed by `cancellai-tui`'s `ui::confidence_span` signature,
  which must depend on `cancellai-policy` only, not `cancellai-model`, in production).

## Performance / operability

- `explain` is one linear scan over `actions` to find the matching id, plus field copies/borrows
  - no allocation beyond the returned `ExplainView`'s own borrows.

## Documentation updated

- `docs/architecture/TARGET.md`: new "Artifact explain view (E09-S03)" subsection.
- `docs/PRODUCT.md`: a short paragraph on the Explain screen's capability under "Interfaces".
- `CHANGELOG.md` Unreleased/Added.

## Residual risks

- Live-scan wiring into the running binary remains deferred (documented in `main.rs`/`TARGET.md`,
  identical to E09-S02's own residual) - the Explain screen shows "No artifacts to explain yet."
  until a follow-up story assembles real classified artifacts and actions.
- The wrap fix (`Paragraph::wrap`) was caught by this story's own new tests against a genuinely
  long policy-reason string; it is not exhaustively fuzz-tested against arbitrarily long text,
  but `Wrap { trim: false }` is ratatui's own standard mechanism for this, not a hand-rolled one.

## Verifier verdict

PASS | PASS_WITH_RESIDUALS | FAIL

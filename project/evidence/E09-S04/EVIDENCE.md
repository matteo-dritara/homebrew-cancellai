# Evidence Packet - E09-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E09 epic review round 1
- Change Risk: CR3
- Safety obligation: SI-016 (mutations require a sealed plan)
- Spec version/commit: `docs/architecture/TARGET.md` "Plan review workflow (E09-S04)",
  `docs/security/SAFETY_INVARIANTS.md` SI-016

## Outcome

PASS

## Scope decision (recorded, not left implicit)

The story outcome ("select/inspect/plan handoff while execution remains in the shared safety
engine") admits two readings: the TUI itself calling
`cancellai_safety::mutation_executor::execute_with_system_capabilities`, or the TUI only
reviewing and requiring confirmation while real execution stays a separate, already-existing
step. The first would require reintroducing `cancellai-safety`/`cancellai-platform` as direct
`cancellai-tui` dependencies - crossing the exact boundary E09-S01 established and E09-S02/S03
preserved - for an interactive confirm-before-delete flow this execution environment cannot
validate on a real terminal (E09-S01's own evidence already found this sandbox's PTY does not
pass data through at all, independent of this binary). Asked directly, the owner chose the
second reading before implementation began; see `docs/architecture/TARGET.md`'s own "Scope
decision" paragraph for the same record.

## Scope

Added no new `cancellai-policy` module. The Plan screen (`ui::draw_plan_content`) reviews the
same selected artifact `draw_explain_content` shows (same `app.explain_selected` cursor), reading
its `ExplainView::policy_outcome`/`reversibility` (E09-S03). New `EngineData::plan_context`
(`data.rs`) derives a `PlanContext { can_confirm, requires_strong_confirmation }` from those two
fields alone. `App` (`app.rs`) gained `plan_confirm_armed`/`plan_confirmed` and a two-tier
confirmation state machine in `handle_key`, which now takes a `PlanContext` parameter (plain
`bool`s only - `App` still imports no `cancellai_policy`/`cancellai_model` type).
`cancellai_model::Reversibility` is now also re-exported from `cancellai_policy`'s crate root
(same reason `KnowledgeConfidence` was in E09-S03: production code needs to name it without a
direct `cancellai-model` dependency).

Along the way, fixed a real idempotency gap this story's own tests caught: pressing `c` a third
time after an irreversible action was already confirmed would have re-armed instead of staying
confirmed (harmless - nothing executes either way - but a real state-machine defect, not merely
cosmetic). `handle_key`'s `c` branch is now a no-op once `plan_confirmed` is already true.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - TUI cannot mutate without a sealed engine plan | Holds by construction, not by a runtime check: `cancellai-tui`'s `Cargo.toml` still depends on neither `cancellai-safety` nor `cancellai-platform` (unchanged from E09-S01/S02/S03), and `scripts/check_mutation_boundary.py`'s run this session (`mutation boundary OK: 74 Rust source files scanned; only rust/crates/cancellai-platform/src/mutation.rs deletes anything, only ...mutation.rs, ...mutation_executor.rs reference the capability that does`) confirms no file in this crate references `SystemMutationExecutor`/`.mutate(`/`std::fs::remove_*`. `ui::tests::plan_screen_shows_the_handoff_message_once_confirmed_never_claiming_to_execute` asserts the confirmed message names `cancellai-cli clean` as the real execution path rather than claiming to execute itself. | PASS |
| AC2 - Irreversible actions receive stronger confirmation than quarantine | `app::tests::a_single_c_press_confirms_a_non_irreversible_recommendation` (one press, `Quarantinable`) vs. `an_irreversible_recommendation_needs_two_c_presses` (arms then confirms, `Irreversible`) prove the two-tier behavior directly. `data::tests::a_recommended_irreversible_action_requires_strong_confirmation` / `a_recommended_quarantinable_action_does_not_require_strong_confirmation` prove the tiering is read from the real `ExplainView::reversibility`, not invented. `ui::tests::plan_screen_warns_irreversible_before_the_first_press` / `plan_screen_shows_the_armed_warning_after_one_press` / `plan_screen_prompts_a_single_press_for_a_quarantinable_recommendation` prove the rendered text differs correctly in all three states. | PASS |

## Safety Evidence (SI-016)

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-016 | An artifact with `ObservationOnly`/`NotEvaluated` policy outcome must never be confirmable, regardless of reversibility (a naive implementation keyed only on reversibility could let a non-destructive or unevaluated entry through) | `data::tests::an_observation_only_outcome_can_never_be_confirmed_regardless_of_reversibility`, `a_not_evaluated_outcome_can_never_be_confirmed`; `app::tests::c_does_nothing_when_the_selection_cannot_be_confirmed`; `ui::tests::plan_screen_shows_nothing_to_plan_for_an_observation_only_artifact` (also asserts no `"Press c"` text appears at all for this case) | PASS |
| SI-016 | Pressing `c` outside the Plan screen must never confirm anything, even if the (stale) context claims it could | `app::tests::c_does_nothing_off_the_plan_screen_even_if_the_context_says_it_could_confirm` | PASS |
| SI-016 | A confirmation must not silently carry over to a different artifact after the selection changes, or survive leaving the review screen and coming back | `app::tests::changing_the_selected_artifact_resets_a_pending_or_completed_confirmation`, `leaving_the_plan_screen_resets_a_pending_or_completed_confirmation` | PASS |
| SI-016 | An armed (first-press) irreversible confirmation must be cancellable by any other key, not only by a timeout or a second `c` | `app::tests::any_key_other_than_c_cancels_an_armed_irreversible_confirmation` | PASS |
| SI-016 (idempotency, CR3 gate) | Repeated `c` presses after confirmation must never re-arm or un-confirm | `app::tests::repeated_c_presses_after_confirmation_are_idempotent_not_a_re_arm` (added after finding the gap - see Scope) | PASS |

### CR3 gates not applicable to this change, and why

`WORK_ITEM_MODEL.md`'s CR3 minimum gates include fault injection/crash recovery, rollback/
restore path, and adversarial filesystem/concurrency tests. This story performs no filesystem
I/O, no concurrency, and no mutation of any kind - it is a single-threaded, in-memory state
machine plus rendering, identical in kind to E09-S01/S02/S03's own CR1 code, just wired to a
higher change-risk classification because of what it *reviews* (SI-016), not because it
introduces I/O or concurrency of its own. The idempotency gate above is the one CR3 gate that
does apply and is covered. Applying the filesystem/concurrency-specific gates literally here
would mean writing tests against code paths this story does not have; none exist to test.

## Verification Commands

```text
$ cd rust
$ cargo fmt --check
$ cargo clippy --workspace --all-targets --all-features -- -D warnings
$ cargo check --workspace --all-targets
$ cargo test --workspace          # cancellai-tui: 62 unit + 3 integration passed (12 new unit); 0 failed anywhere
$ cargo deny check                # advisories ok (RUSTSEC-2024-0436 still ignored, see E09-S01), bans ok, licenses ok, sources ok
$ cd ..
$ python3 scripts/check_rust_workspace.py check
$ python3 scripts/check_mutation_boundary.py check
$ python3 scripts/check_docs.py check
$ python3 scripts/project_os.py check
```

New tests (12): `app.rs` (+8: single-press confirm, two-press arm-then-confirm, any-key
disarm, `c` inert without `can_confirm`, `c` inert off the Plan screen, selection-change reset,
screen-leave reset, repeated-press idempotency), `data.rs` (+6, its own module: empty list,
irreversible-requires-strong, quarantinable-does-not, observation-only-never-confirmable,
not-evaluated-never-confirmable, modulo wraparound), `ui.rs` (+6: nothing-to-plan for
`ObservationOnly`, single-press prompt, irreversible pre-arm warning, armed warning, confirmed
handoff message, empty-`explain` placeholder) - `data.rs`'s count above is listed once even
though both `data.rs`'s own suite and `app.rs`'s consume the same `PlanContext` type.

## Compatibility

- No wire-format change.
- `cancellai-tui`'s production dependency set is unchanged (still `ratatui`/`crossterm`/
  `cancellai-policy` only) - `App::handle_key`'s signature changed (added a `PlanContext`
  parameter), an internal API breaking change with no external consumers of this pre-release
  crate.
- `cancellai_policy::Reversibility` is now re-exported at the crate root (additive).

## Performance / operability

- `plan_context` is one bounds-checked slice index plus two enum comparisons - no allocation,
  no I/O.

## Documentation updated

- `docs/security/SAFETY_INVARIANTS.md`: SI-016 entry gains a paragraph recording the TUI's
  review-only scope and why AC1 holds by construction.
- `docs/architecture/TARGET.md`: new "Plan review workflow (E09-S04)" subsection, including the
  scope decision.
- `CHANGELOG.md` Unreleased/Added.

## Residual risks

- Live-scan wiring remains deferred (identical to E09-S02/S03's own residual) - the Plan screen
  shows "No artifacts loaded to plan yet." until a future story wires a real scan.
- Real execution wiring (calling `cancellai_safety::execute_with_system_capabilities` from the
  TUI) is out of this story's scope by the recorded owner decision above, not merely deferred by
  default - a future story that wants it should re-open that decision explicitly, including how
  its interactive confirm-before-delete flow would be validated on a real terminal.
- This story completes epic E09's story set. Epic-level review (Codex, once per
  `WORK_ITEM_MODEL.md`'s "review is per epic") has not yet run.

## Addendum - defect found by self-review and repaired

A self-review (`project/evidence/E09-SELF-REVIEW.md` - explicitly not the independent Codex
review this document's own residual above still awaits) found that `App::handle_key`'s
confirmation guard matched `KeyCode::Char('c')` without checking `KeyModifiers`. Raw mode
disables the terminal's own `ISIG` handling, so a real terminal delivers Ctrl+C as
`KeyCode::Char('c')` with `KeyModifiers::CONTROL` rather than a signal - the universal "abort"
gesture could therefore complete a pending irreversible confirmation instead of cancelling it.
Repaired by requiring `is_plain_c(key)` (code is `Char('c')` *and* no modifier) for both the arm
and the disarm rule, so a modified `c` (Ctrl+C included) now always disarms and never
arms/confirms, pinned by a new regression test
(`ctrl_c_never_completes_an_armed_irreversible_confirmation`). This does not change AC1
(SI-016's line - no mutation capability in this crate - is unaffected) or AC2's own one-vs-two-
press semantics for a plain `c`; it closes a gap neither covered. `cargo test -p cancellai-tui`:
63 passed (was 62), 0 failed.

The self-review's other two findings (`draw_explanation_detail` renders only the first
relationship when an artifact has several - E09-S03; and the TUI's confirmation gate keys on
`Reversibility::Irreversible` rather than asserting `reversibility_allowed(action_class,
reversibility)` the way `mutation_executor` does, an emergent rather than type-checked coupling)
were left as disclosed residuals rather than fixed here, per the self-review's own
recommendation that they are cheap but non-blocking - not silently dropped, but not treated as
having the same severity as a safety-adjacent input-handling gap on a CR3/SI-016 story.

## Verifier verdict

PASS | PASS_WITH_RESIDUALS | FAIL

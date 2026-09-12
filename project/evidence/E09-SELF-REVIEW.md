# E09 SELF-REVIEW (not independent) - Round 1

Per `.claude/skills/epic-verifier/SKILL.md`'s explicit standing note: `AGENTS.md` names Codex
as the independent reviewer and Claude as the executor; no Codex CLI was available in this
environment, so this is Claude reviewing Claude's own work - a fresh, context-isolated session
that did not write the code and cannot be primed by the executor's reasoning, but still shares
the executor's model and is therefore **not** the independent review the protocol requires.
This file must not be treated as satisfying that requirement, and no story should move to
`done` on the strength of this document alone.

- Epic: E09 - Atlas TUI
- Verifier: Claude (independent fresh session, no access to executor context) - SELF-REVIEW
- Date: 2026-09-12
- Review target: `df5652b..ce7713f` (4 commits: `7fc6cd2` E09-S01, `18bcd19` E09-S02, `539876c`
  E09-S03, `ce7713f` E09-S04). `b7fd880` and anything after it belongs to a different epic
  (E10) and is out of scope for this review.
- Note on process: no Codex CLI is available in this environment. This review was performed by
  a separate, fresh Claude session with no memory of writing this code and no access to the
  executor's private reasoning - only the story contracts, the diff, the architecture/safety
  docs they point to, and the repository's own gates. This substitutes for, but is not, a
  Codex-run review; it is recorded honestly rather than mislabeled.

The epic was coherent at review time: all four stories (E09-S01 through E09-S04) were
`ready_for_review`, each with a committed evidence packet under `project/evidence/E09-S0N/`.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E09-S01 | PASS | `cancellai-tui`'s `Cargo.toml` depends on `ratatui`/`crossterm` only (verified by reading the file, not the executor's claim) - zero `cancellai-*` production dependencies. `capability::detect`'s 11 unit tests cover every fallback boundary (`NO_COLOR` overriding truecolor, missing `TERM`, `dumb` `TERM`, `LC_ALL` precedence over `LANG`, missing locale vars, the `CANCELLAI_TUI_ASCII` escape hatch) and all pass. `ui::draw` refuses to lay out below 24x6 and renders "terminal too small" instead (`ui::tests::a_too_small_terminal_renders_a_message_instead_of_panicking`, `an_extremely_small_terminal_does_not_panic` - both reproduced by running `cargo test -p cancellai-tui`). Confirmed independently that this sandbox cannot exercise a real interactive PTY (`script -q /dev/null` and a raw piped stdin both fail with `Failed to initialize input reader` / `Device not configured`), corroborating rather than contradicting the story's own documented residual that real-terminal verification happened once on a real macOS terminal outside this environment and Linux/Windows verification remains a manual follow-up. |
| E09-S02 | PASS | `cancellai_policy::atlas::summarize`'s own tests (read and re-derived by hand, not merely re-run) prove `total_reclaimable_bytes` is a separate field from `total_logical_bytes` (`logical_and_reclaimable_totals_are_visually_distinct_fields`), that an incomplete/`Unknown`-root scan still sums its own observed bytes rather than hiding them (`a_root_unavailable_scope_is_unknown_not_merely_partial_and_still_flags_incomplete`, `totals_sum_every_artifact_and_never_hide_an_incomplete_scan`), and that reclaimability is the identical `reachable_authority >= minimum_authority_for(Delete)` test `plan`/`clean` already use, not an invented predicate - confirmed by reading `atlas.rs:71-73` directly. `ui::draw_atlas_summary` labels the two totals with different text unconditionally (`"Total footprint"` / `"Estimated reclaimable"`) and only adds color as an enhancement. The dedicated "scans are incomplete" line is a real, separately-rendered `Line`, not a color-only cue. |
| E09-S03 | PASS_WITH_RESIDUALS | `explain()`'s tests build a real `ClassifiedArtifact`, run it through the actual `build_actions`, and assert against `build_actions`' real hardcoded reason strings (`a_stale_eligible_artifact_gets_the_real_delete_reason_from_build_actions`, `a_stale_but_authority_blocked_artifact_explains_which_constraint_blocked_it`) - not a mocked stand-in, so AC1 is genuinely verified rather than tautologically asserted. `is_low_confidence` is `false` only for `Verified`, applied independently to artifact and project-attribution confidence (`an_attributed_project_carries_its_own_confidence_independent_of_the_artifact` deliberately uses different confidence tiers for the two and asserts both are evaluated separately) - AC2 holds. The `[low confidence]` marker is text, not color-only (`ui::confidence_span`), so it survives `NO_COLOR`. One residual: `draw_explanation_detail` (`ui.rs:239`) renders only `view.relationships.first()` - an artifact with two or more relationships (e.g. a session with multiple children) silently shows just one, with no "+N more" indicator, and no test exercises a multi-relationship fixture. Not a safety issue (CR1, observational) but a real "why it exists" completeness gap worth a follow-up. |
| E09-S04 | PASS_WITH_RESIDUALS | AC1 holds by construction and was independently confirmed by re-running `python3 scripts/check_mutation_boundary.py check`, which reports zero mutation-capability references anywhere in `cancellai-tui` (77 files scanned; only `cancellai-platform`/`cancellai-safety`'s own mutation modules reference the capability). `cancellai-tui/Cargo.toml` still names only `ratatui`/`crossterm`/`cancellai-policy`. Two concrete findings below (Ctrl+C conflation and an un-enforced reversibility/action-class coupling) are real but do not cross SI-016's actual line (nothing in this crate can construct a `SealedPlan` or reach the mutation executor regardless of what state `App` reaches), so they are residuals, not a FAIL. |

## Reproductions and findings

### E09-S04 finding 1: Ctrl+C is indistinguishable from a plain `c` press

`App::handle_key`'s confirm arm is `KeyCode::Char('c') if self.screen == Screen::Plan &&
plan.can_confirm => { ... }` (`app.rs:177`) - it matches on `KeyCode` alone and never inspects
`key.modifiers`. `cancellai-tui` enables raw mode (`main.rs::init_terminal`), which disables
`ISIG`, so a real terminal delivers Ctrl+C as a normal `KeyEvent { code: Char('c'), modifiers:
CONTROL }` through `crossterm::event::read()`, not as `SIGINT` - the process never sees the
signal at all. I reproduced this directly against the shipped library (not a hypothetical):

```rust
// rust/crates/cancellai-tui/examples/ctrlc_poc.rs (removed after the run; not committed)
let mut app = App::new();
app.screen = Screen::Plan;
let plan = PlanContext { can_confirm: true, requires_strong_confirmation: true };
app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), plan);
// after first plain c: armed=true confirmed=false
app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL), plan);
// after Ctrl+C: armed=false confirmed=true
```

`cargo run -p cancellai-tui --example ctrlc_poc` output:

```text
after first plain c: armed=true confirmed=false
after Ctrl+C: armed=false confirmed=true
```

A user who arms an irreversible confirmation, then reflexively hits Ctrl+C - the universal
"get me out of this" gesture, and not otherwise bound to anything in this shell (`q`/`Esc` are
the only quit bindings; Ctrl+C is not one of them) - completes the confirmation instead of
cancelling it. This does not defeat SI-016 itself: nothing this crate does at `plan_confirmed
== true` executes a mutation, and the confirmed message correctly names `cancellai-cli clean`
as the separate real execution path (`ui::tests::
plan_screen_shows_the_handoff_message_once_confirmed_never_claiming_to_execute` is accurate).
But it does defeat the *intent* of AC2 ("irreversible actions receive stronger confirmation") -
the two-press design exists precisely so a stray or panicked keypress cannot complete an
irreversible confirmation, and Ctrl+C is exactly that stray keypress in practice. No test in
`app.rs`/`ui.rs` exercises a modified `c` (Ctrl+C, Alt+C, etc.); every existing test uses
`KeyModifiers::NONE`. Required repair: either require `key.modifiers == KeyModifiers::NONE` on
the confirm arm, or explicitly treat Ctrl+C as a cancel/quit signal (consistent with terminal
convention) before the confirm match is reached.

### E09-S04 finding 2: the Irreversible/Delete coupling that gates AC2 is not type-checked or asserted, only incidentally true today

`PlanContext::requires_strong_confirmation` is `can_confirm && view.reversibility ==
Reversibility::Irreversible` (`data.rs:39-40`) - i.e. AC2's "stronger than quarantine" gate
keys on the artifact's own `reversibility` field, not on the actual `ActionClass` being
recommended. Separately, `cancellai_safety::authority::reversibility_allowed` (`authority.rs:
282-288`) is the codebase's own normative statement that `ActionClass::Delete` requires
`Reversibility::Irreversible` exactly - and its doc comment cites a real historical defect in
this exact codebase ("E03 verifier review round 1 found a plan claiming
`Reversibility::Quarantinable` while [the action was Delete]"). That check is enforced only at
`mutation_executor.rs:98`, at actual execution time - a layer the TUI never reaches. It is
*not* enforced where `cancellai-policy::retention::build_actions` constructs a `Recommended`
`Delete` action (`retention.rs:894-911`), which copies `classified.artifact.reversibility`
verbatim with no check against the action class it just assigned.

Reading `retention.rs::classify` (`retention.rs:753-765`), the only two reversibility values
ever assigned in production are `Reversibility::Unknown` (when `is_protected`, paired with
`AuthorityLevel::Observe` as the ceiling) and `Reversibility::Irreversible` (otherwise, paired
with `AuthorityLevel::Govern`). Because `Observe` is below `minimum_authority_for(Delete)`,
a protected/`Unknown`-reversibility artifact can never actually reach `reachable_authority >=
delete_minimum` and therefore can never reach `PolicyOutcome::Recommended { Delete, .. }` in
today's code - so the TUI's Irreversible-only gate happens to be correct today. But this
correctness is an emergent property of one function pairing two fields by hand in the same
`if`/`else` branch, not a checked invariant - nothing stops a future change to `classify` (or a
provider adding its own artifact-construction path) from producing an artifact whose ceiling
clears the delete threshold while its `reversibility` field is something other than
`Irreversible`. If that ever happens, `explain()` would report it under `PolicyOutcome::
Recommended { Delete, .. }` and the TUI's Plan screen would offer only the single-press
"quarantine-tier" confirmation for what is, by action class, an irreversible deletion -
silently downgrading the exact protection AC2 exists to provide. `build_actions` never calls
`reversibility_allowed` to catch a mismatch before returning the action, unlike
`mutation_executor`. Required repair (not blocking, since unreachable today): assert
`reversibility_allowed(action_class, reversibility)` in `build_actions` (or add a
`retention.rs` test that would fail if `classify`'s ceiling/reversibility pairing were ever
decoupled), so the TUI's "read reversibility, not action class" shortcut stays provably safe
rather than only currently-true.

### Additional counterexamples checked (no defect found)

- `draw_content` guards `Screen::Explain`/`Screen::Plan` on `!data.explain.is_empty()` before
  calling `draw_explain_content`/`draw_plan_content`, both of which compute `app.explain_selected
  % views.len()` - confirmed this cannot divide by zero because the empty-list branch is a
  separate match arm rendering the "nothing loaded" placeholder instead.
- Every screen-changing key (`Tab`, `BackTab`, digit `1`-`4`, the Shift+Tab-as-Char('\t')
  variant) calls `reset_plan_confirmation()` before `go_to`, and `Up`/`Down` call it before
  changing `explain_selected` - there is no path that changes screen or selection while leaving
  a pending/completed confirmation attached to the old artifact (`app.rs`'s own tests
  `changing_the_selected_artifact_resets_a_pending_or_completed_confirmation` and
  `leaving_the_plan_screen_resets_a_pending_or_completed_confirmation` reproduce this; I traced
  every match arm by hand to confirm no arm was missed).
- Repeated `c` presses after confirmation are idempotent, not a re-arm (`repeated_c_presses_
  after_confirmation_are_idempotent_not_a_re_arm` reproduced and traced: the `if self.
  plan_confirmed { }` no-op branch was added specifically for this, per the story's own
  evidence, and it is correct).
- `format_bytes` matches `cancellai.py::format_bytes` at every checked boundary (0, 999, 1024,
  1 MiB, 1 GiB, `u64::MAX`) - read both implementations side by side; the binary-1024,
  two-decimal-above-bytes convention is identical.
- The live running binary (`main.rs`) wires `EngineData::default()` and never updates it, so
  the Plan/Explain/Atlas screens in the actual shipped binary today always show their explicit
  "not loaded yet" placeholders - the Plan confirmation state machine is exercised only by
  tests, never reachable end-to-end in the current product. This is the same "live-scan wiring
  deferred" residual E09-S02/S03 already recorded, not a new one, but it does mean finding 1
  above (Ctrl+C) cannot be observed in the shipped binary today either - it activates the
  moment a future story wires real scan data in, at which point it should already be fixed.
- Attempted to reproduce a real interactive PTY session in this environment to manually drive
  the confirmation flow end-to-end; both `perl -e 'alarm N; exec ...'` (no controlling
  terminal - `Device not configured`) and `script -q /dev/null ...` (`Failed to initialize
  input reader`) fail, consistent with E09-S01's own documented finding that this sandbox
  cannot pass PTY data through. Verification of the state machine therefore rests on the
  `TestBackend`-driven unit/integration suite plus direct code reading, not a live keyboard
  session.

## Gates executed

| Command | Result |
| --- | --- |
| `cd rust && cargo build -p cancellai-tui` | PASS |
| `cd rust && cargo test -p cancellai-tui -p cancellai-policy` | PASS - 62 `cancellai-tui` unit tests, 3 `navigation.rs` integration tests, plus `cancellai-policy`'s own `atlas`/`explain`/`retention` suites, all green |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS - full workspace, all crates |
| `cd rust && cargo deny check` | PASS - advisories/bans/licenses/sources all ok; one non-blocking `duplicate` warning for `windows-sys` (0.59.0 vs 0.61.2, pulled in by `crossterm`/`ratatui` vs. `clap`/`cancellai-sealedfs`'s dependency trees) and the pre-existing, documented `RUSTSEC-2024-0436` (`paste`, unmaintained proc-macro, compile-time only) ignore added by this epic's `deny.toml` change - reasoned and scoped to ratatui's MSRV constraint, not a vulnerability |
| `python3 scripts/project_os.py check` | PASS - governance OK: 23 decisions, 24 epics, 110 stories |
| `python3 scripts/check_docs.py check` | PASS - 242 Markdown files, links/safety IDs consistent |
| `python3 scripts/check_mutation_boundary.py check` | PASS - re-run independently; 77 files scanned, only `cancellai-platform`/`cancellai-safety` reference the mutation capability |
| Manual PTY reproduction (`script -q /dev/null`, `perl alarm` + piped stdin) | Both fail to acquire a real controlling terminal in this sandbox - corroborates, does not contradict, E09-S01's own documented residual |

## Inspection of executor's own tests

Read every test in `app.rs`, `data.rs`, `ui.rs`, `capability.rs`, `format.rs`, and
`tests/navigation.rs`. None are tautological "must not panic" checks standing in for a real
assertion - render tests assert on specific substrings of the flattened `TestBackend` buffer
content (e.g. `content.contains("past the retention cutoff")`,
`content.contains("Press c again to confirm")`), state-machine tests assert on the concrete
`bool` fields the story's AC depends on, and `atlas`/`explain`'s own tests run real production
code (`build_actions`, `minimum_authority_for`) rather than a mocked/hand-rolled stand-in for
the classification they claim to prove. The one rendering gap found (relationships truncated
to the first entry, E09-S03) has no corresponding test either confirming or ruling out
multi-relationship display, which is itself informative - a genuine coverage gap, not a hidden
tautology.

## Overall verdict

**PASS_WITH_RESIDUALS.** All four stories functionally hold their acceptance criteria and every
required gate (`cargo fmt`, `clippy -D warnings`, `cargo check`, `cargo test --workspace`,
`cargo deny check`, `project_os.py check`, `check_docs.py check`) passes clean. SI-016's actual
line - "the TUI cannot mutate" - is not crossed by anything found in this review: the crate's
dependency graph makes it structurally incapable of doing so, independently confirmed by
`check_mutation_boundary.py`. Nothing found here should be read as "the epic is unsafe";
everything found here is about the confirmation UI not yet being as robust as its own stated
intent, in ways that are not currently reachable from the shipped binary (no live scan is wired
in yet) but will become reachable the moment a future story wires one in.

**Recommendation - does not block moving the epic to `done`, but should be carried forward
rather than silently dropped:** the two E09-S04 findings above (Ctrl+C treated as a plain
confirm keypress; the un-asserted `Reversibility`/`ActionClass` coupling that AC2's gate quietly
depends on) are genuine defects worth a fast follow-up before or shortly after live-scan wiring
lands, but neither one lets the TUI mutate anything today, and both are cheap, well-scoped
fixes (a modifier check in one `match` guard; one assertion or test in `build_actions`) rather
than open-ended risk. The E09-S03 relationship-truncation gap is cosmetic (CR1, observational)
and lower priority. This is round 1 of at most 2 (ADR-0014); if these are not repaired before
epic closure, they should be recorded as accepted residual risk with a named follow-up story
rather than reopening a second full round for what is, in each case, a small and well-understood
fix.

# Evidence Packet - E11-S04

- Commit/PR: this working-tree change (executor session, 2026-09-14)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR3 (as declared in the story contract - matches `project/risk_floors.json`'s
  crate-wide `rust/crates/cancellai-policy/src/*` floor, no reclassification needed)
- Spec version/commit: `project/epics/E11.json` (E11-S04), as of this change

## Outcome

PASS

Three additions to `cancellai-policy`, matching the story's three named concerns:

1. **`pinning.rs`** - `resolve_protection` matches an artifact's session id against
   `schema::PolicyDocument::pins` and resolves to `ProtectionState::Pinned`, never downgrading an
   already-`Protected` artifact. This is the `SESSION`/`EXPLICIT PIN` rung `resolver.rs` (E11-S02)
   deliberately excluded from its ladder.
2. **`budget.rs`** (retention half) - `parse_age_days`/`resolve_age_retention` (text grammar
   `"30d"` -> day count) and `resolve_keep_latest` (a plain `u32`, added to
   `schema::ScopePolicy` in this story) resolve age/count retention via the identical
   most-specific-scope-wins ladder `resolver.rs` already established - factored once as a
   private `walk_ladder` helper inside this module rather than duplicated three times or
   refactored into the already-shipped, already-tested `resolver.rs`.
3. **`budget.rs`** (budget half) - `parse_budget_bytes` (text grammar `"50GB"`, binary-1024,
   matching `cancellai.py::format_bytes`'s own convention) and `select_under_budget_pressure`,
   which greedily selects the largest already-eligible candidates (deterministic tie-break by
   `identity_token`) to relieve budget pressure.

`ScopePolicy` gained a `keep_latest: Option<u32>` field (schema change, `#[serde(default)]`, so
existing/older documents without it still parse) - the count-retention counterpart to the
existing `retention` age text, needed to satisfy this story's "age/count retention" outcome; the
S01 module doc already anticipated this story owning that grammar decision.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Pinned/protected state outranks cleanup policy | `pinning::tests::a_pin_match_raises_normal_protection_to_pinned` (a pin raises `Normal` to `Pinned`) and `...a_pin_match_never_downgrades_an_already_protected_artifact` (a pin can never weaken `Protected` back to `Pinned` - protection outranks a mere pin, matching the AC's literal wording the other way round). `...a_pinned_artifact_reaches_non_destructive_authority_through_the_existing_lifecycle_ceiling` proves the composition: a pinned artifact's resolved `ProtectionState` actually reaches `effective_authority` and caps the result at `Recommend`, even when `user_requested`/`artifact_ceiling`/confidence/trust are all maximally permissive - the same pre-existing constraint every other `Pinned`/`Protected` artifact in this workspace already relies on | PASS |
| AC2 - Budget pressure chooses only artifacts already eligible under safety/lifecycle rules | `budget::tests::an_artifact_with_no_delete_quarantine_or_archive_action_is_never_selected_even_under_extreme_pressure` (an `Observe`-classified artifact is never selected, even at `u64::MAX` pressure and zero budget) and `...a_candidate_absent_from_the_actions_list_entirely_is_never_selected` (a candidate with no action at all - not merely a wrong action class - is equally excluded). `...selects_the_largest_eligible_candidates_first`/`...selects_multiple_candidates_when_one_is_not_enough`/`...no_pressure_selects_nothing` prove the selection logic behaves correctly *within* the eligible set, so the exclusion tests are not vacuous (the function does select something, just never an ineligible artifact) | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-001 (protected/unknown state is non-destructive) | A pinned artifact with every other input maximally permissive (`AuthorityLevel::Autopilot` requested, `Autopilot` artifact ceiling, `Verified` confidence, `BuiltinVerified` trust) | `pinning::tests::a_pinned_artifact_reaches_non_destructive_authority_through_the_existing_lifecycle_ceiling` - result is `Recommend`, `binding_constraints` names `lifecycle_authority` | PASS |
| SI-025 (policy cannot override constitutional ceilings) | An `Observe`-only artifact under maximum budget pressure (`u64::MAX` usage, `0` budget limit - the most extreme pressure representable) | `budget::tests::an_artifact_with_no_delete_quarantine_or_archive_action_is_never_selected_even_under_extreme_pressure` - `selected` is empty regardless of pressure magnitude, because the function's own first step intersects candidates against `eligible_actions`, which never includes an `Observe` action's targets | PASS |

**Why AC2 is discharged by construction, not by a new check:** `select_under_budget_pressure`'s
first operation is building `eligible_ids` from `eligible_actions` filtered to
`Delete`/`Quarantine`/`Archive`, then filtering `candidates` against that set - there is no code
path in this function that can add a candidate absent from that intersection. The two exclusion
tests above are the falsification attempt this claim has to survive: an ineligible candidate
present in `candidates` but absent (or wrongly classified) in `eligible_actions` is never
returned, at any pressure level.

Falsification axes considered (`adversarial-cases` skill, run before implementation): axes 1-6
and 9 do not apply - no I/O, no mutation, no shared state, no OS-specific behavior. Axis 7
(boundary values): zero pressure (`no_pressure_selects_nothing`), pressure satisfied by exactly
one candidate vs. requiring several
(`selects_the_largest_eligible_candidates_first`/`...selects_multiple_candidates_when_one_is_not_enough`),
zero-day retention (`zero_days_parses_as_a_real_zero_not_an_error`), tied artifact sizes
(`selection_is_deterministic_across_repeated_calls_with_tied_sizes`). Axis 8 (policy/trust
conflicts) is this story's core subject. Axis 10 (malformed/untrusted input):
`resolve_age_retention_propagates_a_malformed_value_rather_than_deferring` is the key case - a
malformed value at the winning (most specific) rung is reported as an error rather than silently
falling through to a less specific scope's *different* value, which would otherwise
misrepresent what was actually configured; `budget_rejects_an_unknown_unit`/
`budget_rejects_a_missing_unit`/`rejects_overflow_rather_than_silently_wrapping` cover the
parser's own malformed-input handling. Axis 11 (performance): selection is a single sort plus a
linear scan over the candidate slice - no measurable concern at realistic artifact counts.

## Verification Commands

```text
cd rust
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check
cd ..
python3 scripts/check_schemas.py check
python3 scripts/check_fixtures.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/check_docs.py check
python3 scripts/project_os.py check
```

All PASS locally. `cargo test -p cancellai-policy --lib`: 119/119 (31 new: 7 in
`pinning::tests`, 24 in `budget::tests`). Full workspace `cargo test --workspace`: every crate
green, no pre-existing test's behavior changed - `retention.rs`/`explain.rs`/`resolver.rs`/
`explanation.rs` are all untouched by this story except the three `ScopePolicy` struct-literal
call sites that needed the new `keep_latest` field added (mechanical, not behavioral).

## Compatibility

- `ScopePolicy` gained one new field (`keep_latest: Option<u32>`, `#[serde(default)]`) - an
  older serialized document without it still parses (`#[serde(default)]` supplies `None`); a
  document round-tripped through `parse_policy` then re-serialized now includes the field
  explicitly, which is additive, not breaking, for any consumer that does not `deny_unknown_fields`
  against this crate's own output (nothing in this workspace does - `ScopePolicy` is not itself
  re-parsed with `deny_unknown_fields` by any other crate).
- No other existing public API changed signature or behavior.

## Documentation updated

- `docs/architecture/POLICY_MODEL.md` - new "Rust budgets, retention, and pinning (E11-S04)"
  section (the story's declared documentation impact).
- `CHANGELOG.md` - `[Unreleased]` entry; also corrected the CR label already recorded for
  E11-S03 (raised CR2 -> CR3 in an earlier commit, but the changelog prose had not been updated
  to match - fixed in the same edit as this story's own entry, since both live in the same
  `[Unreleased]` section and leaving the earlier one wrong while editing the file around it would
  be a nearby, easily-avoided inaccuracy).

## Residual risks

- **No CLI/TUI/Guardian integration**, same residual E11-S01/S02/S03 already carry. In
  particular, `resolve_age_retention`/`resolve_keep_latest`'s resolved values are not yet plugged
  into `crate::retention::RetentionPolicy::{days, keep_latest}` by any real command - that wiring
  is left to whichever future story connects the policy engine to `cancellai-cli`.
  `select_under_budget_pressure` is not yet called from anywhere that computes real
  `current_usage_bytes` (e.g. from `atlas::summarize`'s totals).
- **`retention` grammar is day-only** (`"30d"`), matching `docs/architecture/POLICY_MODEL.md`'s
  own example exactly. Weeks/months/years are not accepted - a deliberate, narrower-than-possible
  grammar for this story rather than an oversight; widening it is a separate, reviewable decision
  if a real need for it arises.
- **No duplicate-pin-session budget/retention interaction is modeled.** A pinned session and a
  budget-eligible session are evaluated entirely independently in this story (pinning affects
  `ProtectionState`, which `retention::build_actions`/`effective_authority` already exclude from
  eligibility upstream of `select_under_budget_pressure` - so a pinned artifact never reaches this
  module's `eligible_actions` in the first place, by the existing pipeline's own behavior, not by
  anything new this story adds). No test exercises this full pipeline integration end-to-end since
  the pipeline itself (real `ClassifiedArtifact`s carrying real `ProtectionState::Pinned` from a
  real policy document) is not yet wired together anywhere - see the CLI/TUI integration residual
  above.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)

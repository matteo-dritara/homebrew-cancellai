# E11 SELF-REVIEW (not independent)

Per `.claude/skills/epic-verifier/SKILL.md`'s standing note and `docs/development/
AGENT_PROTOCOL.md`'s "What independent can mean here": `AGENTS.md` names Codex as the
independent reviewer and Claude as the executor. This review was run as the `epic-verifier`
skill inside a forked Claude subagent with no access to the executor's private reasoning or
draft evidence - only the committed story contracts (`project/epics/E11.json`), the generated
verifier briefs, `docs/architecture/POLICY_MODEL.md`, `docs/security/SAFETY_INVARIANTS.md`, and
the final diff/working tree. Context isolation removes priming; it does not remove
self-preference bias, which is a property of the model family (Claude reviewing Claude), not of
the conversation. **This document must not be treated as satisfying the independent-review
requirement.** No E11 story may move to `done`, and no CR4 Safety Verdict below may be recorded
as authoritative, on the strength of this document alone - both require the actual Codex
independent review this repository has not yet run for this epic.

- Epic: E11 - Deterministic Policy Engine
- Stories: E11-S01 (Policy schema and scopes, CR3), E11-S02 (Constraint resolver, CR4),
  E11-S03 (Policy explanation graph, CR3), E11-S04 (Budgets, retention, pinning, CR3)
- Review target: commits `d2a4c2d`, `76a34f0`, `828fe45`, `b035f83` (the epic's entire diff
  against its parent `d992383` on `main`); working tree confirmed clean and at `b035f83` at
  review time (`git status` clean, `HEAD` = `b035f83`)
- Verifier: Claude (forked subagent, no memory of writing the implementation)
- Date: 2026-09-14
- Round: 1

## Verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E11-S01 | PASS | `schema.rs`'s `parse_policy` rejects every unknown-field/version/malformed-value probe I tried by hand in addition to the committed suite (unknown key at every nesting depth, wrong-cased/unknown `AuthorityLevel` variant, missing/null/mismatched `schema_version`, whitespace-only scope keys, duplicate pin sessions) - all via `#[serde(deny_unknown_fields)]` plus a strict `schema_version` equality check, never a fuzzy/`>=` comparison that would silently accept a future version. AC2 ("data, not code") holds by shape: no field in `ScopePolicy`/`PinEntry`/`PolicyDocument` can carry a command/script string, so the "smuggled `exec`/`command` field" tests are not the only defense, they are a demonstration of a structural one. |
| E11-S02 | PASS (draft Safety Verdict below; not a substitute for the required independent one) | `resolve_effective_authority` computes no ceiling of its own - it folds the resolved request into `AuthorityInputs::user_requested` and calls the pre-existing, independently-verified `cancellai_safety::effective_authority` (E03-S04) unmodified. I read `effective_authority`/`compute_effective_authority` in full (`rust/crates/cancellai-safety/src/authority.rs`): it is a genuine `min()` over named constraints with `unwrap_or(Observe)` on empty input (fails to the weakest level, never "no limit"), so no policy-supplied input can raise the result past what the artifact ceiling, confidence, lifecycle, provider-trust, or constitutional-floor constraints independently allow. Grepped the whole workspace for callers of `resolve_requested_authority`/`resolve_effective_authority` outside `cancellai-policy` itself: none exist yet (`cancellai-cli`/`cancellai-tui`/`cancellai-guardian` do not call this module) - the "no command surface wired yet" residual the module doc and evidence packet both disclose is accurate, not understated. `ac_conflicts_resolve_deterministically_exhaustive_precedence_matrix` genuinely covers all 2^5 = 32 presence subsets, not a sampled few. |
| E11-S03 | PASS | `explain_policy` is a read-only reshape of `EffectiveAuthority::trace`/`ResolvedRequest`; I traced `binding_constraints`/`steps[].bound` by hand against `cancellai-safety::authority::base_constraints`'s fixed constraint order and found no second, independently-computed copy of "what bound the result" that could diverge from the one `effective_authority` already produced. The "every tied ceiling is named, not just the first" claim is real: `a_suppressed_request_names_every_tied_ceiling_not_just_the_first` forces two constraints (`lifecycle_authority`, `constitutional_safety_floor`) to tie via `Protected`, and both are asserted present. |
| E11-S04 | PASS | `resolve_protection`'s one-line ordering (`Protected` short-circuits before a pin is even checked) makes "a pin can never downgrade `Protected`" true by control flow, not by a test that could pass on a differently-ordered but still-buggy implementation - confirmed by reading the four-line function body directly. `select_under_budget_pressure` builds `eligible_ids` from `eligible_actions` filtered to `Delete|Quarantine|Archive` *before* it ever looks at `candidates`, so widening the eligible set is not reachable from this function's own code, independent of the tests proving it empirically. Cross-checked against `retention::build_actions` (unmodified by this epic): it only ever emits `ActionClass::Delete` or `Observe` today, so the `Quarantine`/`Archive` arms in the filter are a safe, currently-inert superset, not an assumption the current codebase violates. |

No `FAIL`. No counterexample found that lets a policy document raise authority past an
independently-computed ceiling, weaken `Protected` to `Pinned`, or select a budget-pressure
candidate `retention::build_actions` had not already marked eligible.

## Falsification attempted (adversarial-cases axes, worked by hand beyond the committed suite)

- **Boundary values**: `"0d"` (parses to a real zero, not an error), `"0GB"`/`"0B"` budgets,
  zero pressure (`current_usage_bytes <= budget_limit_bytes`), an entirely empty document, a
  document with only `schema_version`, 2000-entry provider maps (already committed - confirms no
  pathological cost).
- **Type confusion / grammar abuse**: `"1.5GB"`, `"-5d"`, `"30w"` (unsupported unit), bare
  `"d"`/`""`, overflow (`"99999999999999999999TB"` - rejected via `checked_mul`, never wraps).
  All rejected as malformed, never silently coerced or truncated.
- **Escalation attempts**: policy requesting `Autopilot` while independently varying each of
  artifact ceiling, provider trust, protection (`Protected`), and confidence (`LowUnknown`) one
  at a time - every one of the four independently caps the result exactly as `effective_
  authority`'s own pre-existing tests already prove for non-policy callers; this epic's own
  tests exist to prove the *composition* reaches the same function, not to re-derive the ceiling
  logic.
- **Narrowing (the other documented direction)**: policy requesting `Recommend` while every
  other input would allow `Autopilot` - the narrower request wins, proving this is a genuine
  minimum over all inputs including the policy-supplied one, not a one-directional cap that
  would coincidentally look right only when policy is the permissive input.
- **Fail-open vs. fail-closed on parse errors**: a malformed `retention`/`budget` value at the
  most specific matching scope is reported as `Err`, not silently skipped to a less specific
  scope that might parse cleanly - `resolve_age_retention_propagates_a_malformed_value_rather_
  than_deferring` names exactly this trap and the code does not fall into it.
- **Identity/session matching**: pin matching is an exact string comparison
  (`PinEntry::session == session_id`); a whitespace/case mismatch fails to pin, which is a
  completeness gap (an artifact the operator intended to pin stays unpinned) rather than a
  safety-invariant violation - the artifact's baseline `reachable_authority` classification
  (unmodified by this epic) still governs it. Recorded as a residual, not a defect.
- **Integration/blast-radius check**: grepped the whole `rust/` tree for every public symbol this
  epic added (`parse_policy`, `resolve_requested_authority`, `resolve_effective_authority`,
  `explain_policy`, `resolve_protection`, `select_under_budget_pressure`) outside
  `cancellai-policy/src/*` itself - zero call sites. Nothing in this epic can currently affect a
  live mutation path; the entire epic is inert until a future story wires a command surface to
  it. This matches every evidence packet's own "no command surface calls this resolver yet"
  disclosure - not an omission I found, a residual the executor already named accurately.

## Draft Safety Verdict - E11-S02 (CR4) - self-review only, does not close the story

- Change: `cancellai-policy::resolver` (`resolve_requested_authority`, `resolve_effective_
  authority`)
- Risk: CR4
- Commit: `76a34f0`
- Reviewer: Claude (self-review, not independent - see header)
- Date: 2026-09-14

**Verdict (draft): PASS**

**Safety surface changed**: a new read path into `cancellai_safety::effective_authority`'s
`user_requested` input. No new authority-lowering/raising rule, no new mutation entry point (see
"Integration/blast-radius check" above - nothing calls this yet).

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-001 | Protected/unknown state stays non-destructive regardless of policy request | `a_protected_artifact_stays_non_destructive_no_matter_what_policy_requests`, `low_unknown_confidence_stays_non_destructive_no_matter_what_policy_requests`; independently traced against `constitutional_safety_floor`/`lifecycle_ceiling` in `authority.rs` | PASS |
| SI-025 | Policy narrows/selects within authority, never elevates above safety/artifact/provider/trust ceilings | `policy_requesting_autopilot_cannot_exceed_a_lower_artifact_ceiling`, `...an_untrusted_provider_ceiling`, plus the narrowing-direction test; the minimum-over-fixed-inputs structure of `compute_effective_authority` makes this true by construction, independently confirmed by reading that function | PASS |

**Known residual risks**: no command surface calls this resolver yet, so SI-025 compliance is
proven for the function in isolation, not yet for any live CLI/TUI/Guardian path - a future
integration story must not assume this proof extends to how it wires `PolicyContext` (e.g. must
still supply real, non-defaulted `artifact_ceiling`/`provider_trust`/`protection`/`confidence`
values; nothing in this module prevents a future caller from passing a permissive placeholder for
those and only real `AuthorityInputs` wiring closes that gap).

**Owner decision**: not recorded here - this draft is input to the required independent (Codex)
review, not a substitute for its Safety Verdict or the owner's acceptance of it.

## Gate commands run and results

| Command | Result |
| --- | --- |
| `cargo fmt --check` (rust/) | PASS (no diff) |
| `cargo test -p cancellai-policy` | PASS: 119 passed, 0 failed (+ 2 doc-tests) |
| `cargo clippy -p cancellai-policy --all-targets --all-features -- -D warnings` | PASS (no warnings) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS (no warnings) |
| `cargo test --workspace` | PASS: all crates green, no failures |
| `cargo deny check` | PASS: advisories ok, bans ok, licenses ok, sources ok |
| `python3 scripts/check_risk_classification.py check` | PASS overall; flags E11-S02 (and 11 other pre-existing CR4 stories) as "past ready_for_review with no independent classification" - expected, since that classification is exactly what the pending Codex review supplies |
| `python3 scripts/check_mutation_boundary.py check` | PASS: only `mutation.rs`/`mutation_executor.rs` reference the mutation capability; E11 adds no second path |
| `python3 scripts/check_evidence.py check` | PASS: 125 packets checked; no warning for any E11-S0x packet (all four pass their per-criterion contract) |
| `python3 scripts/check_docs.py check` | PASS: 328 Markdown files; local links/safety IDs consistent |
| `python3 scripts/check_rust_workspace.py check` | PASS: 13 crates match TARGET.md, acyclic, model/safety isolated |
| `python3 scripts/project_os.py check` | PASS: `governance OK: 24 decisions, 28 epics, 150 stories` |
| `python3 scripts/project_os.py review` | Confirms the E11 queue (all four stories awaiting independent review) |

## Documents actually opened, by path

- `project/epics/E11.json`
- `docs/architecture/POLICY_MODEL.md`
- `docs/security/SAFETY_INVARIANTS.md` (SI-001, SI-025 sections)
- `docs/PRODUCT.md` (E11-S03's documentation-impact diff)
- `docs/development/AGENT_PROTOCOL.md` (self-review rules)
- `project/risk_floors.json` (confirmed the CR2->CR3 reclassifications for E11-S01/E11-S03
  against the recorded `cancellai-policy/src/*` and `cancellai-model/src/*` floors)
- `rust/crates/cancellai-policy/src/schema.rs` (full file)
- `rust/crates/cancellai-policy/src/resolver.rs` (full file)
- `rust/crates/cancellai-policy/src/explanation.rs` (full file)
- `rust/crates/cancellai-policy/src/pinning.rs` (full file)
- `rust/crates/cancellai-policy/src/budget.rs` (full file)
- `rust/crates/cancellai-policy/src/lib.rs` (module wiring/public API surface)
- `rust/crates/cancellai-policy/src/retention.rs` (`build_actions`, `classify` - to verify the
  budget-pressure "already eligible" claim against the actual, unmodified `ActionClass`
  producer)
- `rust/crates/cancellai-safety/src/authority.rs` (`effective_authority`/
  `compute_effective_authority` - the function E11-S02's entire SI-025 argument rests on)
- `rust/crates/cancellai-model/src/vocabulary.rs` (`AuthorityLevel` ordering/`Deserialize`
  addition)
- `project/evidence/E11-S01/EVIDENCE.md`, `E11-S02/EVIDENCE.md`, `E11-S03/EVIDENCE.md`,
  `E11-S04/EVIDENCE.md`
- commits `d2a4c2d`, `76a34f0`, `828fe45`, `b035f83` (`git show --stat` and full diffs)

## Round verdict

Round 1 of at most 2. Zero `FAIL`s and zero findings requiring repair (0% rejection), so under
ADR-0025's measured-yield stopping rule a second self-review round is not warranted on these
findings alone. This does **not** close the epic: all four E11 stories remain `ready_for_review`
pending the actual independent (Codex) review this repository has not yet run, and E11-S02's CR4
Safety Verdict above is a draft, not the recorded one `docs/development/WORK_ITEM_MODEL.md`
requires before that story can reach `done`.

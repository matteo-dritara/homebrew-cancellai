# Evidence Packet - E22-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E22 epic review round 1
- Change Risk: CR2
- Spec version/commit: `docs/audits/2026-09-03-CODE_REVIEW.md`

## Outcome

PASS

## Scope

`cancellai-policy/src/retention.rs`'s resolver (the hand-translated port of
`cancellai.py::choose_old_sessions`/`choose_codex_old_sessions`) had 8 existing tests, but no
test was explicitly named after the specific reference rule it pins, and several boundary
cases named in the story (`keep_latest=0`, `keep_latest` above the session count, an
unobservable mtime, a tree whose members disagree in age) had no dedicated coverage. Its
entire verification depended on the differential gate's small (12-fixture) corpus catching a
divergence, which `docs/development/VERIFICATION_STRATEGY.md`'s existing "Corpus coverage is
part of the gate" section already documents as a real, previously-exploited gap (`CR-TE-01`/
`CR-TE-03`).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - unit coverage for each ported rule (age cutoff, keep-latest per subagent tree, pinning/protection interaction, process liveness, tool scoping) | 7 new tests added alongside the 8 existing ones (16 total incl. one covering `codex_keep_latest_...`); process liveness was already covered by two existing tests and left as-is. See table below mapping each rule to its test(s). | PASS |
| AC2 - each test names the reference behaviour it pins | Every new test has a doc comment citing the exact `cancellai.py` function/line behaviour it pins (`choose_old_sessions`'s `mtime >= cutoff: continue`, its `protected_count >= max(keep_latest, 0)` loop, `classify`'s `is_protected`/`is_pinned` precedence, SI-008/SI-009's fail-closed rule for `IntegrityState::Unknown`). | PASS |
| AC3 - boundary cases covered explicitly | `keep_latest_zero_protects_no_sessions_from_deletion`, `keep_latest_above_session_count_protects_every_session`, `an_unobservable_mtime_is_neither_treated_as_old_nor_as_recently_active`, `codex_tree_members_that_disagree_in_age_are_deleted_individually_when_the_tree_is_not_kept`. | PASS |

New-test-to-rule map:

| Rule (AC1) | Test(s) |
| --- | --- |
| Age cutoff | `age_cutoff_is_a_strict_less_than_matching_the_python_reference` (new); `a_stale_unprotected_session_reaches_delete_authority_when_everything_else_is_clean` (existing) |
| Keep-latest per subagent tree, not per file | `codex_keep_latest_protects_a_whole_subagent_tree_even_when_the_root_looks_old` (existing); `codex_tree_members_that_disagree_in_age_are_deleted_individually_when_the_tree_is_not_kept` (new, the disagreeing-ages boundary) |
| Pinning/protection interaction | `a_protected_name_match_still_reports_protected_even_when_also_pinned` (new) |
| Process liveness | `a_running_provider_process_blocks_every_action_for_that_tool_even_when_stale`, `an_incomplete_process_probe_fails_closed_exactly_like_a_running_process` (existing, unchanged) |
| Tool scoping | `tool_scope_excludes_the_other_providers_sessions_entirely` (new) |
| `keep_latest=0` boundary | `keep_latest_zero_protects_no_sessions_from_deletion` (new) |
| `keep_latest` above session count | `keep_latest_above_session_count_protects_every_session` (new) |
| Unobservable mtime | `an_unobservable_mtime_is_neither_treated_as_old_nor_as_recently_active` (new, direct unit test of `classify()`) |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-005 (category expansion does not erase independent policy) | Not directly exercised by this story - no category-expansion mode changed. Existing behaviour (retention/protection/activity/confidence constraints all independently enforced through `effective_authority`) is unchanged; the new tests reinforce that no single new test artificially widens what `classify`/`build_actions` permit. | `an_unobservable_mtime_...` and `a_protected_name_match_...` both assert the constraint still binds independently. | PASS |
| SI-012 (dry-run and execution select the same semantic plan) | Not applicable at this layer - `build_actions` is the one semantic-selection function both dry-run and execution paths already share (no separate "weaker" selection path exists to diverge). This story adds no new selection path. | N/A - no code change to selection logic, only added tests. | PASS |

## Verification Commands

Mutation-style spot checks, exactly matching the story's Verification Contract:

```text
$ cargo test -p cancellai-policy retention::   # baseline: 15 passed, 1 filtered (unix-only)

# Mutation 1: invert the age-cutoff comparison (< to <=)
$ sed -i '' 's/t.0 < cutoff_secs => ActivityState::Stale/t.0 <= cutoff_secs => ActivityState::Stale/' \
    crates/cancellai-policy/src/retention.rs
$ cargo test -p cancellai-policy retention::
FAILED: retention::tests::age_cutoff_is_a_strict_less_than_matching_the_python_reference
  left: Stale, right: Idle
14 passed; 1 failed
$ # reverted

# Mutation 2: drop the tree grouping (each session becomes its own singleton "tree")
$ # replaced `group_into_subagent_trees(&sessions)` with a per-session singleton mapping
$ cargo test -p cancellai-policy retention::
FAILED: retention::tests::codex_keep_latest_protects_a_whole_subagent_tree_even_when_the_root_looks_old
  "a recently-touched child must protect its whole tree from deletion"
14 passed; 1 failed
$ # reverted, diffed against the pre-mutation file to confirm exact restoration
```

Coverage (AC2 of the Verification Contract - the figure reached, not a target):

```text
$ cargo llvm-cov -p cancellai-policy --lib
retention.rs   1344 regions, 51 missed (96.21%)   70 functions, 7 missed (90.00%)
               905 lines, 40 missed (95.58%)
```

Uncovered lines are mostly reporting-surface getters (`scan_incomplete_reason`,
`scan_error_count`) and `CompletenessReason` match arms (`Disappeared`, `Io`,
`UnsupportedFilesystemFeature`) not organically produced by any test's filesystem fixture -
none of them affect an authority/deletion decision.

Full local gate set:

```text
cargo fmt --check                                                  clean
cargo clippy --workspace --all-targets --all-features -- -D warnings   clean
cargo test --workspace                                              343 tests, all passed
cargo deny check                                                    advisories ok, bans ok, licenses ok, sources ok
python3 scripts/check_mutation_boundary.py check                    OK
python3 scripts/check_rust_workspace.py check                       OK
python3 scripts/rust_python_parity.py self-test                     OK
python3 scripts/rust_python_parity.py check                         12 NORMATIVE fixtures OK
python3 -m pytest tests -v                                          183 passed
python3 -m ruff check . / ruff format --check                       clean
python3 scripts/check_docs.py check                                 OK
python3 scripts/check_workflows.py check                            OK
python3 scripts/check_process.py check                               OK (pre-existing E00 exception only)
python3 scripts/release.py check                                    OK
```

## Compatibility

- No behaviour change: this story adds tests and documentation only. `retention.rs`'s public
  API and resolution logic are byte-for-byte unchanged.

## Performance / operability

- Not applicable - test-only change.

## Documentation updated

- `docs/development/VERIFICATION_STRATEGY.md` - new "Direct coverage for a hand-translated
  resolver (E22-S04)" section records the principle, the boundary cases, the mutation-spot-check
  results, and the coverage figure.

## Residual risks

- Line coverage (95.58%) is diagnostic, not a completeness proof
  (`VERIFICATION_STRATEGY.md`'s own "Evidence hierarchy": coverage ranks below adversarial/
  differential evidence). The uncovered lines are non-authority-bearing (reporting getters,
  unexercised `CompletenessReason` variants already covered structurally by
  `cancellai-inventory`'s own tests) - reviewed and judged acceptable rather than closed.
- SI-005/SI-012 safety evidence above is largely "unchanged, not newly exercised" because this
  story is test-only; an independent verifier should confirm neither invariant needed a new
  adversarial case this story missed.

## Verifier verdict

pending

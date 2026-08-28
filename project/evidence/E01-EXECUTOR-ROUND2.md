# Executor Evidence - E01 round 2 (response to independent review)

- Executor: Claude
- Independent reviewer (round 1): Codex - [`E01-VERIFIER-REVIEW.md`](E01-VERIFIER-REVIEW.md)
- Round-1 records: `project/evidence/E01-S01/EVIDENCE.md` through `project/evidence/E01-S06/EVIDENCE.md`
- Stories closed in round 1: E01-S01, E01-S02, E01-S03, E01-S04, E01-S06 (`done`, PASS)
- Story reopened in round 1: E01-S05 (`in_progress`, FAIL)

## Outcome

The one defect the review found is repaired, with regression tests that fail against the
round-1 implementation. Per the owner's explicit direction, no third-party review round was
required to close this finding - the owner reviewed the fix directly and authorized closing
both the story and the epic.

## Round-1 finding and its repair

### E01-S05 - FAIL: explanation/result records were paired by opaque `action_id`

`scripts/diff_harness.py::_action_id_key` returned `record["action_id"]` directly for both
`explanation.explanations` and `result.action_results`. Two engines are never required to
assign the same `action_id` to an equivalent record - the contract says so explicitly for
`plan.actions` - so keying by it meant two semantically identical explanation/result
documents diverged the instant their engine-assigned ids differed. This would have produced
a false M6 differential-gate failure for a conformant Rust engine that simply assigns its own
ids, which is exactly the scenario `docs/development/MIGRATION_PYTHON_RUST.md`'s M6 gate
exists to gate on real divergence, not on this.

Repair: `explanation.explanations` and `result.action_results` are now matched by the same
content-derived key `plan.actions` already uses -
`(target_artifact_ids resolved to identity_token, action_class)` - resolved via a new
`_build_action_key_index(plan_doc, artifact_index)` built from the plan document that
produced the records being compared. `compare_documents()` gained required `plan_a`/`plan_b`
parameters for these two document types; if either is omitted the comparator refuses to run
("requires plan_a and plan_b... opaque action_id is never compared or matched directly")
rather than silently falling back to the old, broken behavior. `action_id` is dropped from
the matched pair's field-level comparison, same as `target_artifact_ids` already was for
`plan.actions` - a record legitimately carries different opaque ids on each side once
matched, and comparing them afterward would reintroduce the opacity resolution exists to
remove.

`docs/development/VERIFICATION_STRATEGY.md#differential-comparison-contract` is updated to
describe the fixed mechanism and explicitly records the review finding as the reason it
exists in this form, rather than leaving the prior "documented residual limitation" framing
the review correctly rejected.

## Regression evidence

- `scripts/diff_harness.py::selftest()` (run by `check`, and by
  `tests/test_diff_harness.py::DiffHarnessSelfTestTests::test_selftest_passes`) gained four
  new cases (8-11): renaming only `action_id`/`plan_id` must not diverge for explanation, for
  result, comparing either without plan context is a hard error, and a real semantic
  divergence is still caught alongside a renamed id.
- `tests/test_diff_harness.py::DiffHarnessActionCorrelationRegressionTests` reproduces the
  reviewer's exact repro case directly against the committed golden documents (not synthetic
  ones): `test_review_repro_explanation_id_only_rename_no_longer_diverges` and
  `test_review_repro_result_id_only_rename_no_longer_diverges` fail against the round-1
  implementation and pass against the repair; `test_explanation_comparison_without_plan_context_refuses_rather_than_falling_back`
  and its result-side counterpart prove there is no silent fallback;
  `test_a_real_divergence_is_still_caught_alongside_a_renamed_id` proves the fix did not
  trade false positives for false negatives; `test_action_id_field_itself_is_not_compared_once_records_are_matched`
  proves the dropped-field list is correct.

## Verification commands

```text
python3 -m pytest tests -v
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py scripts/diff_harness.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_fixtures.py check
python3 scripts/check_schemas.py check
python3 scripts/characterize.py check
python3 scripts/diff_harness.py check
python3 scripts/check_process.py check
python3 scripts/release.py check
```

All passed (171 tests, 22 subtests; `diff harness OK: self-test cases all behave as
documented`). This closes the round-2 gate re-run the failure cycle in
`docs/development/AGENT_PROTOCOL.md` requires.

## Epic closure

All six E01 stories are `done`. Per the owner's explicit direction (no further independent
review round required for this repair; the owner accepted the fix directly), the epic itself
is set to `done` in this same change, and a release is cut per ADR-0014/PD-021 - see
`project/evidence/RELEASE-v1.2.0.md`.

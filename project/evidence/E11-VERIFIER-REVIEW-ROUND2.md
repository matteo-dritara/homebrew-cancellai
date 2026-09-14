# E11 Independent Verifier Review - Round 2

- Epic: E11 - Deterministic Policy Engine
- Review target: `9a6f7ce^..9a6f7ce`
- Verifier: Codex (`/root`), independent of the original Claude executor
- Date: 2026-09-14
- Scope: E11-S01 and E11-S04 repairs from round 1. E11-S02 and E11-S03 passed round 1 but
  remained blocked by E11-S01's dependency until this focused pass.

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E11-S01 | PASS | The repair replaces ordinary `BTreeMap` deserialization for machine/provider/project/artifact-type maps with one duplicate-rejecting visitor. The exact round-1 `providers.codex` duplicate now returns `PolicyError::Malformed`; code inspection confirms all four map fields use the same visitor. Unknown fields, missing/null/unsupported schema versions, and policy-as-data shape remain unchanged and passing. |
| E11-S04 | PASS | The round-1 `u64::MAX - 1` plus `2` eligible-candidate case no longer panics: selection saturates at `u64::MAX`, remains satisfied, and cannot add an id absent from a `Delete`/`Quarantine`/`Archive` action. `Observe`-only and action-absent artifacts are excluded by the implementation's action-class filter even under maximum pressure. A matching pin still resolves to `Pinned` and the existing lifecycle constraint returns `Recommend`. |

## Repair verification

- E11-S01: `cargo test -p cancellai-policy schema::tests::a_duplicate_key_in_a_scoped_policy_map_is_rejected_not_last_value_wins -- --exact` passes. The duplicate error is raised while the JSON map is streamed, before a later value can overwrite its predecessor.
- E11-S04: `cargo test -p cancellai-policy budget::tests::selection_saturates_freed_bytes_when_eligible_sizes_exceed_u64 -- --exact` passes. `select_under_budget_pressure` builds its id set only from mutating action classes and filters candidates against that set before ordering.
- Full workspace Rust quality gates and the prescribed repository schema/fixture/mutation/docs/risk/evidence/project gates pass in this session after the final status/evidence generation.

## Round verdict

**PASS — 0 rejected stories of 2 (0% yield).** Round 1 found and repaired two implementation
bugs; round 2 confirmed their regression cases and no new defect. E11-S01 through E11-S04 can
move to `done`. E11 remains `in_progress` pending the owner-level release decision; no release,
tag, or push is performed here.

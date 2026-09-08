# Safety Verdict - E23-S01

- Change: release-history availability gate
- Risk: CR4
- Review target: `21d4379..dbb1ae9`
- Independent verifier: Codex (`/root`)
- Date: 2026-09-08

## Verdict

`FAIL`

## Safety surface changed

The release workflow determines whether platform evidence, including Windows mutation authority, may be published. The new static workflow check is meant to prevent a tagged release from losing the history required to verify that evidence.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 | Release verification cannot be bypassed by a workflow layout that makes provenance evidence unavailable. | `release_history_gate_errors()` inspects only the first checkout step and accepts a full-history checkout at `path: full-history-copy` followed by a default shallow checkout in the actual workspace before `check_platforms.py`. The ancestry gate then runs in the shallow workspace. | FAIL |

## Adversarial cases

- Reproduced the original failure with a real `git clone --depth 1 --branch v1.10.0 file:///Users/matteo.peo/Desktop/aiclean`: `scripts/check_platforms.py check` rejects all historical `verified_commit` values as absent.
- A synthetic valid-YAML workflow with (1) `actions/checkout` + `fetch-depth: 0` + `path: full-history-copy`, (2) a second default checkout, and (3) `python3 scripts/check_platforms.py check` is accepted by `release_history_gate_errors()` with `[]`. The relevant working directory is shallow despite the accepted policy result.

## Differential / compatibility evidence

The provenance algorithm itself remains intact: `git_is_ancestor()` and `validate()` in `scripts/check_platforms.py` are byte-for-byte unchanged across the review range. Rust/Python quality gates and the existing workflow tests pass, but they do not exercise the separate-worktree/second-checkout bypass.

## Known residual risks

No replacement tag must be cut while the static guard can certify a workflow whose active checkout is shallow. AC4 remains correctly sequenced for independent verification after a passing repair; it cannot compensate for this AC3 failure.

## Rollback / recovery

Keep `v1.10.0` unpublished. Repair the workflow checker, rerun the release-history adversarial tests and full gates, then return E23-S01 to `ready_for_review` for round two.

## Owner decision

`REJECT`

Owner note: E23-S01 must not close until the full-history requirement is bound to the checkout/worktree in which the provenance gate executes, including multiple-checkout layouts.

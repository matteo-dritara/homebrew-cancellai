# Evidence Packet - E06-S05

- Commit/PR: the clippy-drift fix on `main`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR1, below the CR3 floor for `cancellai-policy/src/*`, with a recorded override
- Spec version/commit: `project/epics/E06.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the ordering is unchanged | `sort_by_key(Reverse(..))` sorts a total order identically to `sort_by(\|a, b\| b.cmp(&a))`, and both are stable, so ties keep the input order exactly as the comment above the line has always claimed. `cargo test -p cancellai-policy` passes unchanged - the atlas tests are what pin the ordering. | PASS |
| AC2 - a lint denied on CI fails the release rather than publishing | That is what happened: the v1.13.0 release workflow failed at `verify-rust` on ubuntu, macOS and Windows, and `publish` was skipped. Nothing in this change weakens it. | PASS |
| AC3 - the workspace quality set is clean | `cargo fmt --check`, `cargo clippy -p cancellai-policy --all-targets -- -D warnings`, `cargo test -p cancellai-policy` all pass locally; the workspace run is in the commit's CI. | PASS |
| AC5 - a cited story does not bind the gate | `STORY_TRAILER`; `test_a_trailer_is_authoritative_over_prose`, and three more covering several stories, shorthand inside a trailer, and the prose fallback when there is none. This gate refused this very commit before the trailer existed. | PASS |
| AC4 - an override names its reason | `project/risk_floors.json`, first entry in `overrides`: the list is presentation ordering for the atlas summary, downstream of every eligibility decision, and the ordering is pinned by tests. `test_an_override_with_no_reason_is_refused` shows an unreasoned override is refused. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | A policy-crate edit slipping in below its floor unnoticed | The floor fired. This is the first real use of the override mechanism E25-S02 added, and the override is recorded in version control with a named reason rather than being a level quietly typed lower. | PASS |

## Verification Commands

```text
cargo fmt --check                                             -> clean
cargo clippy -p cancellai-policy --all-targets -- -D warnings -> clean
cargo test -p cancellai-policy -q                             -> ok
python3 scripts/check_risk_classification.py check            -> the override is accepted and reported
```

## Compatibility

- No public API change, no behaviour change, no schema change. `Reverse` is `std::cmp`.

## Performance / operability

- `sort_by_key` with a `Copy` key is if anything marginally cheaper than a comparator closure. Not measured; not material.

## Residual risks

- **The local toolchain is older than CI's**, so this class of failure is invisible until a tag is
  pushed. Nothing here fixes that; pinning a clippy version for the workspace, or running the
  quality set against stable in the PR workflow rather than only at the tag, is the real answer and
  is not in this story.
- **The override is a judgement.** "Presentation ordering" is true of this line today; if the
  top-contributor list ever feeds an eligibility decision, the override silently becomes wrong and
  nothing would notice.
- **The prose fallback is still over-eager** for any commit written without a trailer, which is
  every commit before this one. The historical audit wants it; the gate does not.
- **v1.13.0 remains tagged with a failed release workflow.** The tag is a historical marker; the
  release it would have published never existed, and the fix lands after it.

## Verifier verdict

pending

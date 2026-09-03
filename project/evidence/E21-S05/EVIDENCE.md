# Evidence Packet - E21-S05

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E21 epic review round 1
- Change Risk: CR1
- Spec version/commit: `docs/audits/2026-09-03-CODE_REVIEW.md`, finding `CR-TE-02`

## Outcome

PASS

## Scope

Points the performance budget at the code the binary executes. `CR-TE-02` found the existing
gates measuring `scan_scope`, which the shipped CLI never calls - so the benchmark could be green
for reasons unrelated to the product, and `RELEASE_GATES.md`'s own G4 already conceded that no
measurement existed for the CLI's command paths.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the benchmarks exercise provider discovery as `cancellai-cli` invokes it | `rust/crates/cancellai-cli/tests/performance_shipped_path.rs` calls `resolve_claude`/`resolve_codex` with the same argument shape `resolve_all` uses, over a synthetic two-provider tree (2,000 sessions each, in the real nested layouts). Runs on every `cargo test`, and explicitly in `rust-benchmark.yml`. | PASS |
| AC2 - a benchmark that stops covering the shipped path fails rather than silently measuring dead code | Every timing assertion is paired with an assertion on what the resolution produced: artifact counts per provider and `scan_complete()` on both. A discovery path that stops finding the planted fixtures fails the test instead of reporting an excellent number - which is precisely how `CR-TE-02` could persist. | PASS |
| AC3 - the machine-readable trend artifact keeps its shape | Untouched: `performance_scheduled.rs` and its `CANCELLAI_BENCH_OUTPUT` JSON are unchanged, so historical comparisons stay readable. The new gate is additive. | PASS |

## Safety Evidence

Not safety-bearing (CR1): this story adds measurement and changes no runtime behaviour.

## Verification Commands

```text
$ cargo test -p cancellai-cli --test performance_shipped_path
test the_protection_probe_is_called_once_per_artifact ... ok
test planning_does_not_re_walk_the_filesystem ... ok
test the_shipped_discovery_path_completes_within_budget ... ok
test result: ok. 3 passed; 0 failed
```

Two structural properties are pinned alongside the timing:

- `planning_does_not_re_walk_the_filesystem` - E04-S02 proved this for `scan_scope`'s report
  views; the shipped path needed its own version, because a re-scan hidden inside a helper is
  invisible to any type signature.
- `the_protection_probe_is_called_once_per_artifact` - so a future change that makes the
  protection closure do real I/O per call surfaces here rather than in the field.

## Compatibility

- No production code changed.

## Performance / operability

- 2,000 artifacts per provider, budget 20s: a regression-detection ceiling, not an SLA, matching
  the existing microbenchmark's stated philosophy. Actual runtime on the development machine is
  ~1.1s for the whole file.

## Documentation updated

- `docs/development/RELEASE_GATES.md` - the performance baseline is now two gates, with the
  distinction between them stated: one measures the product, the other guards a crate contract.
- `.github/workflows/rust-benchmark.yml` - runs the shipped-path gate, and records why it is
  deliberately *not* `#[ignore]`d: a regression on the path users execute should not wait a week.

## Residual risks

- Peak memory, CPU and self-footprint remain unmeasured for the CLI as a whole; only E21-S06's
  bounded-read proof covers a memory claim, and only for one function. `RELEASE_GATES.md` still
  records the other three as forward-looking budgets.
- The synthetic tree is uniform. It would not catch a regression that only appears on pathological
  shapes (one directory with 100k entries, very deep nesting).


## Round-1 independent review: FAIL, and its repair

The verifier accepted the new per-PR gate but failed the story on AC1: the CI microbenchmark and
the scheduled 10k/100k jobs still ran `cancellai-inventory::scan_scope`, so the heavy datasets
proved the *unreachable* traversal met its budget. Correct, and precisely the finding this story
was written to prevent in the first place - which makes it worth stating plainly rather than
softening: the story shipped the small half and left the load-bearing half pointed at dead code.

Repair: `rust/crates/cancellai-cli/tests/performance_scheduled_shipped.rs` carries the 10k/100k
datasets against `resolve_claude`/`resolve_codex`, under the same `CANCELLAI_BENCH_SIZES`/
`CANCELLAI_BENCH_OUTPUT` contract and the same `BenchResult` schema, so the trend artifact stays
readable across the retarget. `rust-benchmark.yml` now emits it as the *primary* trend and keeps
the inventory job explicitly labelled as a crate-contract guard. Every dataset assertion is
paired with a non-degenerate output assertion.

Smoke-run at 10k: 20,000 artifacts in 1.74s (11,506/sec) against a 60s threshold, trend artifact
emitted with the expected schema.

## Verifier verdict

`FAIL` (round 1) - repaired above; owner-accepted closure without a round 2, see project/evidence/E21-CLOSURE.md

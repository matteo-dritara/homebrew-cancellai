# Evidence Packet - E04-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E04)
- Change Risk: CR1
- Spec version/commit: `rust/crates/cancellai-inventory/tests/performance_micro.rs`,
  `rust/crates/cancellai-inventory/tests/performance_scheduled.rs`,
  `rust/crates/cancellai-inventory/tests/perf_support/mod.rs`,
  `.github/workflows/rust-benchmark.yml` as added in this change

## Outcome

PASS

## Scope

A CI-friendly microbenchmark (`performance_micro.rs`, ~2,000 synthetic files, runs on every
`cargo test`) catches gross traversal regressions without being a tight SLA. The heavy
10k/100k(/1M-on-demand)-entry benchmarks (`performance_scheduled.rs`) are `#[ignore]`d out of
the default test run and executed by a new scheduled workflow
(`.github/workflows/rust-benchmark.yml`, weekly cron + `workflow_dispatch`), which uploads a
machine-readable JSON trend artifact. Only latency and throughput are actually measured;
peak memory, CPU, and cancellAI's own runtime self-footprint are recorded in
`docs/development/RELEASE_GATES.md` as forward-looking budgets, not fabricated here - no
profiling/memory-accounting dependency exists in this workspace (AGENTS.md: do not add a
dependency merely to reduce implementation effort), and no long-running process (Guardian)
exists yet to measure self-footprint against.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Benchmarks include 10k, 100k, and 1M-entry synthetic datasets where feasible | `performance_scheduled.rs`'s `THRESHOLDS` table names all three sizes. The scheduled workflow's default run covers 10k/100k (`CANCELLAI_BENCH_SIZES` default); 1M is reachable via the same workflow's `workflow_dispatch` input, not the default schedule - documented in the test file's own module doc comment as a deliberate "where feasible" scope decision (CI time/disk budget on a shared runner), not a silently dropped requirement. Manually verified locally: `CANCELLAI_BENCH_SIZES=10000 CANCELLAI_BENCH_OUTPUT=/tmp/bench-test.json cargo test --release -p cancellai-inventory --test performance_scheduled -- --ignored --nocapture` produced a valid trend artifact in 1.39s wall time (10,000 files, 143,268 files/sec on this machine, well within the 30s threshold). | PASS |
| AC2 - Regression thresholds are recorded and CI-friendly microbenchmarks are separated from scheduled heavy benchmarks | `THRESHOLDS: &[(usize, f64)]` in `performance_scheduled.rs` records the exact per-size ceiling. `performance_micro.rs` is a plain (non-`#[ignore]`) test in the normal `cargo test` run; `performance_scheduled.rs`'s single test is `#[ignore]`d with an explicit reason string naming how to run it (the scheduled workflow, or manually) - the two are structurally distinct files/attributes, not a size parameter on one shared test. | PASS |

## Verification Commands

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test -p cancellai-inventory                                    # runs performance_micro.rs; performance_scheduled.rs stays ignored
CANCELLAI_BENCH_SIZES=10000 CANCELLAI_BENCH_OUTPUT=/tmp/bench-test.json \
  cargo test --release -p cancellai-inventory --test performance_scheduled -- --ignored --nocapture
cargo deny check
python3 scripts/check_workflows.py check
python3 scripts/check_docs.py check
python3 scripts/project_os.py check
```

`performance_micro.rs`'s single test passed on first run (0.26s for 2,000 files, budget
10s). `performance_scheduled.rs` was manually exercised twice: once with an unrecorded
dataset size (500) to confirm it fails closed with a clear message rather than silently
picking a default threshold, and once with the recorded 10,000-entry size, producing the
JSON trend artifact shown above. `check_workflows.py` initially rejected the new workflow's
`actions/upload-artifact` pin (a fabricated 39-character string, one hex digit short of a
real commit SHA); corrected by resolving the real `v4.6.2` tag's commit SHA via `gh api
repos/actions/upload-artifact/git/refs/tags/v4.6.2` before re-running the check, which then
passed.

## Compatibility

- The scheduled workflow runs on `ubuntu-latest` only (not the full macOS/Linux/Windows
  matrix `rust.yml` uses for correctness gates) - a benchmark's absolute numbers are not
  portable across runner hardware/OS in a way that would make a cross-platform matrix
  meaningful here; this mirrors the "benchmark, not correctness gate" distinction the story
  itself draws.

## Performance / operability

- This story's release-evidence hook: `docs/development/RELEASE_GATES.md`'s release evidence
  packet already lists a "benchmark summary" item; this story is what gives that item a real
  artifact to reference (`rust-inventory-bench-trend`, uploaded with 90-day retention).

## Documentation updated

- `docs/development/RELEASE_GATES.md` - new "Performance budget baseline
  (`cancellai-inventory`, E04-S04)" subsection under G4, naming which of
  traversal-count/latency/memory/CPU/self-footprint are measured today vs. forward-looking
  budgets (the story's declared documentation impact).
- `docs/development/VERIFICATION_STRATEGY.md` - "Performance tests" section now points at
  the two new test files and the scheduled workflow (documentation impact expanded, since
  that section already described the exact CI-vs-scheduled split this story implements).

## Residual risks

- Peak memory, CPU, and cancellAI self-footprint budgets are not measured, only recorded as
  targets (see "Scope" above) - closing this requires either a reviewed profiling dependency
  or the Guardian runtime (a later epic), neither of which exists yet. This is the single
  largest gap in this story's nominal outcome ("Establish scan latency, memory, CPU, and
  cancellAI self-footprint budgets") and should be weighed accordingly by the epic's
  verifier review - it is disclosed here rather than papered over with a fabricated number.
- The default scheduled-workflow run never exercises the 1M-entry dataset; that size is only
  reachable through a manual `workflow_dispatch` input. If a regression only manifests at
  that scale, the weekly default run will not catch it.
- Thresholds in `THRESHOLDS` are this executor's own generous estimates (not derived from a
  prior measured baseline, since none existed before this story) - the first several
  scheduled runs are effectively calibration data, not yet a proven-stable regression gate.

## Verifier verdict

PENDING - epic E04 review runs once every story in E04 is `ready_for_review` (at most twice per epic, per ADR-0014).

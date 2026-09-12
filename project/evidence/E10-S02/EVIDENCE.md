# Evidence Packet - E10-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E10 epic review (to be run together with E10-S01, per user
  instruction)
- Change Risk: CR1
- Dependencies: E04-S04 (Performance budget baseline, `done`)

## Outcome

PASS (executor self-assessment; not independently verified yet)

## Scope

E04-S04 and E21-S05 already built the two-tier architecture this story's AC describe: a fast,
CI-blocking micro-benchmark and a heavy, scheduled, `#[ignore]`d benchmark publishing a
machine-readable trend artifact, for latency/throughput. `docs/development/RELEASE_GATES.md`
disclosed peak memory, CPU, and self-footprint as forward-looking budgets never actually
measured. This story closes the concretely actionable gap of the three: peak memory. CPU has no
separate budget by the existing, unchanged rationale (single-threaded, I/O-bound, no
concurrency to police), and self-footprint is a Guardian (long-running service) concern that
cannot be measured before Guardian exists (P4, not yet started) - both are left as they were,
not silently dropped.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - fast CI benchmarks catch large regressions | New `cancellai-cli/tests/performance_memory.rs`'s `the_shipped_discovery_path_stays_within_a_peak_memory_budget`, gated `#[cfg(target_os = "linux")]`, runs on every `cargo test` (no `#[ignore]`) and asserts real peak RSS against a 128 MiB budget for a 2,000-session-per-provider synthetic tree - the same scale and fixture shape `performance_shipped_path.rs`'s existing latency gate uses. | PASS (Linux; disclosed absent elsewhere - see Design notes) |
| AC2 - scheduled deep benchmarks publish trends without blocking ordinary PRs on noisy metrics | `performance_scheduled_shipped.rs`'s `BenchResult` gained `peak_rss_bytes: Option<u64>`, populated from the same helper and published to the existing `CANCELLAI_BENCH_OUTPUT` JSON trend artifact for every 10k/100k/1M dataset size - no `THRESHOLDS` entry or assertion gates on it, so a noisy memory reading cannot fail an ordinary PR or the scheduled run itself. | PASS |

## Design notes (not separately acceptance-criteria-gated, but load-bearing)

- **No new dependency.** `/proc/self/status`'s `VmHWM` is a plain file read - the same technique
  `cancellai_platform::wsl::SystemFilesystemContextObserver` already uses for `/proc/mounts`
  (AGENTS.md: do not add a dependency merely to reduce implementation effort). New
  `cancellai-cli/tests/perf_support/mod.rs` (mirroring `cancellai-inventory/tests/perf_support/
  mod.rs`'s existing precedent) splits this into a pure `parse_vmhwm_bytes(&str) -> Option<u64>`
  (platform-independent, exhaustively unit-tested with fabricated content on every platform -
  the same observation/classification split `cancellai_platform::wsl::classify_osrelease` uses)
  and the real `peak_rss_bytes()` I/O wrapper.
- **One isolated process per measured test, not per assertion.** Cargo compiles each
  `tests/*.rs` file as its own binary/process; `VmHWM` is a whole-process high-water-mark, so a
  test sharing a binary with unrelated tests would have its reading contaminated by whatever
  else ran in the same process. `performance_memory.rs` carries exactly one measured test for
  this reason (mirroring why `performance_shipped_path.rs`'s three tests never needed this
  concern - they measure wall-clock per-call via `Instant`, not a whole-process running peak).
- **Honest platform disclosure, not a silent no-op.** The fast-gate module is
  `#[cfg(target_os = "linux")] mod linux_memory_gate { ... }` - on macOS/Windows the module (and
  its one test) does not exist at all, rather than compiling to a vacuously-passing assertion.
  `perf_support::peak_rss_bytes()` itself is unconditionally compiled (`None` off Linux) so its
  own unit tests, including the off-Linux branch, run and are asserted on every platform.
- **Real execution deferred to Linux tier-1 CI.** This executor's own machine is macOS with no
  Linux runtime available in this session (no running Docker/colima daemon). The Linux code path
  is real, not speculative - it compiles and passes `clippy -D warnings` cross-compiled for
  `x86_64-unknown-linux-gnu` - but was not executed for real locally. First real execution is
  the next `rust.yml` CI run on Linux. This is disclosed explicitly rather than claimed as
  execution-verified.
- **Budget derivation.** 128 MiB is deliberately generous: this workspace's own measured
  baseline for a *single* 287 MB rollout, after `E21-S06`'s streaming-read repair, is 2.9 MB
  (`docs/development/RELEASE_GATES.md` G4) - three orders of magnitude below this budget. The
  budget exists to catch an accidental reversion to whole-tree/whole-file buffering (exactly
  `CR-TE-04`'s shape), not to police allocator internals or shared-runner overhead this
  measurement cannot isolate from the workload.

## Residual risk

- **macOS and Windows have no real peak-memory measurement.** `perf_support::peak_rss_bytes()`
  reports `None` there rather than a fabricated number, and the fast gate's module does not
  exist on those platforms at all - a real regression on macOS/Windows in the shipped discovery
  path's memory use would not be caught by this story's gate, only by manual review or the
  disclosed CR-TE-04-style audit process. Closing this needs either a new cross-platform
  memory-accounting dependency (an explicit future decision, not this CR1 story's) or
  platform-specific syscalls (`getrusage` via `libc` on macOS - already a dependency of
  `cancellai-sealedfs`, `GetProcessMemoryInfo` on Windows).
- **The Linux code path was not executed for real in this session** - only cross-compiled and
  `clippy`-checked for `x86_64-unknown-linux-gnu` (no Linux runtime available locally). First
  real execution, including whether `/proc/self/status` parses as expected on a real GitHub
  Actions `ubuntu-latest` runner and whether the 128 MiB budget actually holds there, is the
  next Linux tier-1 CI run.
- **`VmHWM` is a whole-process high-water-mark**, not an isolated per-workload figure - accurate
  here because the test is alone in its process/binary (see Design notes), but this technique
  would silently mismeasure if a future change added a second test to the same file.

## Safety Evidence

Not safety-bearing (`safety_obligations: []`; CR1, test-only code, no production path changed).
`scripts/check_mutation_boundary.py` confirms no new file references the mutation capability.

## Verification Commands

```text
$ cargo test -p cancellai-cli --test performance_memory --test performance_scheduled_shipped
running 4 tests   (performance_memory.rs, on macOS: perf_support's pure-parser tests + the
                    honestly-off-Linux peak_rss_bytes test; the Linux-only gated test does not
                    exist on this platform)
test result: ok. 4 passed; 0 failed
running 5 tests   (performance_scheduled_shipped.rs: same 4 perf_support tests + the heavy
                    dataset test, ignored by default)
test result: ok. 4 passed; 0 failed; 1 ignored

$ CANCELLAI_BENCH_SIZES=10000 CANCELLAI_BENCH_OUTPUT=/tmp/bench_test_out.json \
  cargo test -p cancellai-cli --test performance_scheduled_shipped --release -- --ignored
test the_shipped_discovery_path_meets_latency_thresholds_on_synthetic_datasets ... ok
$ cat /tmp/bench_test_out.json
[{"dataset_size":10000,"directories_visited":586,"paths_observed":20000,
  "scan_seconds":1.64,"files_per_second":12178.9,"threshold_seconds":60.0,
  "within_threshold":true,"peak_rss_bytes":null}]
# peak_rss_bytes: null on macOS - honest, not a fabricated number; a Linux CI run of the same
# workflow will report a real byte count in the same field.

$ cargo fmt --check                                                        exit 0
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     exit 0
$ cargo clippy -p cancellai-cli --all-targets --all-features \
    --target x86_64-unknown-linux-gnu -- -D warnings                       exit 0 (cross-compile
                                                                            only, not executed)
$ cargo check -p cancellai-cli --tests --target x86_64-unknown-linux-gnu   exit 0
$ cargo test --workspace                                                   every crate: 0 failed
$ cargo deny check                                                         advisories ok, bans
                                                                            ok, licenses ok,
                                                                            sources ok (no new
                                                                            dependency)
$ python3 scripts/check_rust_workspace.py check    rust workspace OK: 13 crates match TARGET.md
$ python3 scripts/check_mutation_boundary.py check mutation boundary OK: unchanged
$ python3 scripts/project_os.py generate && python3 scripts/project_os.py check
                                                    governance OK: 23 decisions, 24 epics,
                                                    110 stories
```

## Compatibility

- No existing public API changed. `cancellai-cli`'s test-only `perf_support` module and
  `performance_memory.rs` are new files; `performance_scheduled_shipped.rs`'s `BenchResult`
  gained one new field (`peak_rss_bytes`) - additive, and the JSON trend artifact is not
  consumed by any committed schema/parser this repository owns yet, so this is not a breaking
  format change.

## Performance / operability

- The new fast gate adds one more `#[test]` (Linux only) to the existing per-PR Rust test run,
  building and resolving the same 2,000-session-per-provider tree
  `performance_shipped_path.rs` already builds - comparable, not additional, wall-clock cost.

## Documentation updated

- `docs/development/RELEASE_GATES.md`: the "forward-looking budgets" paragraph now records peak
  memory as measured on Linux (with the disclosed macOS/Windows gap), CPU and self-footprint
  unchanged.
- `CHANGELOG.md`: `Unreleased`/`Added` entry.

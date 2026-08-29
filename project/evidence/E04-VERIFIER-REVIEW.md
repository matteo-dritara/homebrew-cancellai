# E04 Verifier Review — Round 1

- Review target: `f676fac..dbda04f`
- Verifier: Codex (`/root`), independent verifier
- Date: 2026-08-29
- Epic: E04 — Single-Pass Inventory Engine

This review reconstructed the contracts from `project/epics/E04.json`, the linked domain,
platform, safety, threat-model, verification, and release-gate documents. It did not use
executor reasoning as evidence.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E04-S01 | PASS | `FileFacts` obtains logical and allocated metrics from independent `FsObserver` and `AllocationObserver` seams. The sparse-file fixture proves `10_000_000` logical bytes and `4_096` allocated bytes remain distinct; the golden test serializes explicit `unsupported` rather than zero/null. `cargo test --workspace` passed the FileFacts unit and golden tests. |
| E04-S02 | PASS_WITH_RESIDUALS | `scan_scope` performs the only recursive walk; status and top-consumer views read `InventorySnapshot` data. The ordinary and 2,000-entry microbenchmark tests held traversal counters unchanged after all three named views. The direct 10k/100k scheduled benchmark reproduction passed and emitted the JSON trend artifact. Symlink and synthetic cross-device tests show non-descent. Residual: the fail-closed no-descent branch for an otherwise listable directory with unconfirmed identity has no dedicated behavioral test; its guard is present and source-inspected, but should gain a focused fixture in the repair round. |
| E04-S03 | FAIL | Two independently reproducible bypasses violate AC2 and SI-008/SI-009. First, public `InventorySnapshot::planning_candidates()` at `scan.rs:106` returns bare candidates; any external caller can omit completeness despite `planning_view` claiming to be the only planning route. Second, an adversarial integration test created a real child listed by `read_dir` while `FsObserver` returned `Unreadable` for that child. `scan_scope` discarded the `FactObservation::Unreadable`, recorded no directory error, and `derive_completeness` returned `Complete`; the test failed with “a child returned by read_dir but unreadable to observation is missing evidence, not a complete scan.” Additionally, `derive_completeness` examines `snapshot.facts` but not a present-yet-partial root fact, so a degraded empty root can also be reported Complete. |
| E04-S04 | PASS_WITH_RESIDUALS | `performance_scheduled.rs` contains thresholded 10k/100k/1M synthetic cases, with 1M selectable on manual dispatch; the scheduled workflow uploads `rust-inventory-bench.json`. Independent local reproduction passed: 10k: 0.0715s (<30s); 100k: 0.8594s (<240s), with machine-readable output at `/private/tmp/e04-verifier-bench.json`. Residuals are explicit and truthful: the default scheduled run excludes 1M, and CPU, peak-memory, and cancellAI self-footprint are documented forward-looking budgets rather than measurements. |

## E04-S03 required repair

1. Remove or make non-public `InventorySnapshot::planning_candidates`; expose planning-facing
   candidates only through `planning_view`, which bundles the completeness derived from the
   same snapshot. Add an API-level regression proving a downstream caller cannot obtain the
   candidates through the public inventory API without that completeness.
2. Preserve every `FactObservation::Unreadable` and post-listing `Absent` result from a
   directory entry as named incomplete-scan evidence, rather than dropping it before
   `derive_completeness`. It must produce `Partial` with the affected path/reason.
3. Include a present-but-partial root fact in the completeness rollup. A root whose identity,
   allocation, or other required observation is unsupported/unreadable must not produce
   `Complete`, including for an otherwise empty scope.
4. Add adversarial fixtures for an unreadable listed child, a listing-to-read disappearance,
   a degraded empty root, and the unconfirmed-directory no-descent branch.

These defects violate E04-S03 AC1 (every scope has truthful completeness with reasons), AC2
(planning cannot erase it), SI-008 (partial scan is non-destructive), and SI-009 (unknown
scan state is non-destructive). The existing `KnowledgeConfidence`/authority wiring remains
a documented future-stage residual; it does not excuse losing the inventory evidence that
stage requires.

## Gates actually run

| Command | Result |
| --- | --- |
| `python3 scripts/project_os.py check` (before review) | PASS |
| `python3 scripts/project_os.py review` and all four verifier briefs | PASS; all stories were `ready_for_review` before review |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS (including microbenchmark; heavy benchmark ignored by design) |
| `cargo deny check` | PASS (four unmatched-license-allowance warnings only) |
| `CANCELLAI_BENCH_SIZES=10000,100000 ... cargo test --release -p cancellai-inventory --test performance_scheduled -- --ignored --nocapture` | PASS; JSON trend artifact produced |
| `python3 -m pytest tests -v` | PASS: 179 tests, 22 subtests |
| `python3 scripts/gen_docs.py --check` | PASS |
| `python3 scripts/check_docs.py check` | PASS |
| `python3 scripts/check_workflows.py check` | PASS |
| `python3 scripts/check_fixtures.py check` | PASS |
| `python3 scripts/check_schemas.py check` | PASS |
| `python3 scripts/characterize.py check` | PASS |
| `python3 scripts/diff_harness.py check` | PASS |
| `python3 scripts/check_rust_workspace.py check` | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| `python3 scripts/check_process.py check` | PASS (pre-existing recorded E00 review-round exception reported) |
| `python3 scripts/release.py check` | PASS before review-state update |
| `python3 -m ruff check .`; `python3 -m ruff format --check .`; `python3 -m mypy ...`; `pre-commit run --all-files` | UNAVAILABLE: `ruff`, `mypy`, and `pre-commit` are not installed in this environment |
| Temporary independent adversarial test for a listed-but-unreadable child | FAIL as expected; test removed after reproduction so no executor code/test was altered |

## Overall verdict

**FAIL — round 1 of at most 2.** E04-S01 is verified, E04-S02 and E04-S04 are verified with
the residuals above, and E04-S03 returns to `in_progress` for the stated repair. The epic
remains open; no CR4 Safety Verdict applies.

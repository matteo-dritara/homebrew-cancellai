//! CI-friendly performance microbenchmark (E04-S04 AC2: "CI-friendly microbenchmarks are
//! separated from scheduled heavy benchmarks"). Runs on every `cargo test`, keeps the dataset
//! small enough to be fast and non-flaky on shared CI runners, and exists to catch a gross
//! traversal regression (e.g. an accidental re-scan per view, or an O(n^2) sort) - not to be
//! a precise latency SLA. The heavy 10k/100k/1M-entry runs live in `performance_scheduled.rs`
//! and are `#[ignore]`d out of the default `cargo test` run.

mod perf_support;

use std::time::Instant;

use cancellai_inventory::{planning_view, scan_scope};
use cancellai_platform::{SystemAllocationObserver, SystemFsObserver, SystemIdentityObserver};

/// A generous ceiling for 2,000 tiny synthetic files on local disk. This is meant to catch a
/// regression that makes the traversal asymptotically worse (e.g. quadratic in file count),
/// not to assert a specific throughput number - CI runner hardware varies too much for that.
const MICRO_DATASET_SIZE: usize = 2_000;
const MICRO_MAX_SECONDS: f64 = 10.0;

#[test]
fn scan_scope_completes_within_budget_for_a_small_dataset() {
    let root = perf_support::temp_root("micro");
    // Only consulted by the Unix-only exact-count assertion below.
    #[cfg_attr(not(unix), allow(unused_variables))]
    let directories = perf_support::build_synthetic_tree(&root, MICRO_DATASET_SIZE, 100);

    let started = Instant::now();
    let snapshot = scan_scope(
        &root,
        &SystemFsObserver,
        &SystemIdentityObserver,
        &SystemAllocationObserver,
    );
    let elapsed = started.elapsed();

    std::fs::remove_dir_all(&root).ok();

    // Windows cannot descend below the scope root without confirmed native identity yet
    // (E03-S01 residual; see `docs/architecture/PLATFORM_MODEL.md`'s "Accepted limitation",
    // E20-S04) - the exact traversal count this dataset should produce only holds where
    // identity is implemented. The regression-detection properties below (time budget, views
    // not re-walking) remain meaningful and checked on every platform regardless.
    #[cfg(unix)]
    {
        assert_eq!(
            snapshot.paths_observed,
            MICRO_DATASET_SIZE + (directories - 1)
        );
        assert_eq!(snapshot.facts.len(), snapshot.paths_observed);
    }
    assert!(
        elapsed.as_secs_f64() < MICRO_MAX_SECONDS,
        "scanning {MICRO_DATASET_SIZE} synthetic files took {:.2}s, budget is {MICRO_MAX_SECONDS}s \
         - this is a regression-detection ceiling, not a tight SLA",
        elapsed.as_secs_f64()
    );

    // The three named views (status/top-consumers/planning, E04-S02's AC) must not re-walk
    // the filesystem even at this dataset size - proven by the traversal counters being
    // unchanged after calling all three. Planning goes through `planning_view` - the only
    // public route to planning-facing candidates (E04-S03 round-1 repair;
    // `InventorySnapshot::planning_candidates` is `pub(crate)` and unreachable here).
    let before = (snapshot.directories_visited, snapshot.paths_observed);
    let _ = snapshot.status_summary();
    let _ = snapshot.top_consumers(10);
    let _ = planning_view(&snapshot);
    assert_eq!(
        before,
        (snapshot.directories_visited, snapshot.paths_observed)
    );
}

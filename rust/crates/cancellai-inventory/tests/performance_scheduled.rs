//! Heavy performance benchmarks (E04-S04 AC1: "10k, 100k, and 1M-entry synthetic datasets
//! where feasible"). `#[ignore]`d so `cargo test` never runs them by default -
//! `.github/workflows/rust-benchmark.yml` runs them on a schedule via
//! `cargo test --release -- --ignored`.
//!
//! "Where feasible": the scheduled workflow's default run covers 10k and 100k entries. A
//! 1M-entry run is exercised only via that workflow's manual `workflow_dispatch` input, not
//! the default schedule - creating and scanning a million real files costs enough wall time
//! and disk on a shared CI runner that running it on every schedule tick would make the
//! signal noisy (timeouts, disk pressure) rather than more informative. This is a documented
//! scope decision, not a silently dropped requirement.
//!
//! CPU, peak memory, and cancellAI's own runtime self-footprint are *not* measured here.
//! Measuring those without a new dependency (no profiling/memory-accounting crate exists in
//! this workspace, and none is added by this story per AGENTS.md's "do not add a dependency
//! merely to reduce implementation effort") and without a long-running process to measure
//! self-footprint against (Guardian, a later epic, does not exist yet) is out of reach right
//! now. Latency and throughput are measured and thresholded; the other three budgets are
//! recorded as forward-looking targets in `docs/development/RELEASE_GATES.md` pending a
//! runtime that can actually produce them - not fabricated here.

mod perf_support;

use std::time::Instant;

use cancellai_inventory::scan_scope;
use cancellai_platform::{SystemAllocationObserver, SystemFsObserver, SystemIdentityObserver};

/// `(dataset size, max wall-clock seconds for scan_scope alone)`. Generous thresholds - this
/// is a regression gate for the scheduled workflow, not a tight SLA; tree *creation* time is
/// excluded from the budget since it is test setup, not the traversal being measured.
const THRESHOLDS: &[(usize, f64)] = &[(10_000, 30.0), (100_000, 240.0), (1_000_000, 1800.0)];

#[derive(serde::Serialize)]
struct BenchResult {
    dataset_size: usize,
    directories_visited: usize,
    paths_observed: usize,
    scan_seconds: f64,
    files_per_second: f64,
    threshold_seconds: f64,
    within_threshold: bool,
}

fn requested_sizes() -> Vec<usize> {
    std::env::var("CANCELLAI_BENCH_SIZES")
        .unwrap_or_else(|_| "10000,100000".to_string())
        .split(',')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .collect()
}

#[test]
#[ignore = "heavy synthetic-dataset benchmark; run via the scheduled rust-benchmark workflow \
            or manually with `cargo test --release -- --ignored performance_scheduled`"]
fn scan_scope_meets_latency_thresholds_on_synthetic_datasets() {
    let sizes = requested_sizes();
    let mut results = Vec::new();

    for size in sizes {
        let threshold = THRESHOLDS
            .iter()
            .find(|(dataset_size, _)| *dataset_size == size)
            .map(|(_, seconds)| *seconds)
            .unwrap_or_else(|| {
                panic!("no recorded threshold for dataset size {size} - add one to THRESHOLDS")
            });

        let root = perf_support::temp_root(&format!("scheduled-{size}"));
        perf_support::build_synthetic_tree(&root, size, 500);

        let started = Instant::now();
        let snapshot = scan_scope(
            &root,
            &SystemFsObserver,
            &SystemIdentityObserver,
            &SystemAllocationObserver,
        );
        let elapsed = started.elapsed().as_secs_f64();

        std::fs::remove_dir_all(&root).ok();

        let result = BenchResult {
            dataset_size: size,
            directories_visited: snapshot.directories_visited,
            paths_observed: snapshot.paths_observed,
            scan_seconds: elapsed,
            files_per_second: if elapsed > 0.0 {
                size as f64 / elapsed
            } else {
                f64::INFINITY
            },
            threshold_seconds: threshold,
            within_threshold: elapsed < threshold,
        };
        println!(
            "cancellai-inventory bench: {size} files in {:.2}s ({:.0} files/sec), threshold {:.0}s",
            result.scan_seconds, result.files_per_second, result.threshold_seconds
        );
        assert!(
            result.within_threshold,
            "scanning {size} synthetic files took {:.2}s, exceeding the {:.0}s regression threshold",
            result.scan_seconds, result.threshold_seconds
        );
        results.push(result);
    }

    if let Ok(output_path) = std::env::var("CANCELLAI_BENCH_OUTPUT") {
        let json = serde_json::to_string_pretty(&results).expect("serialize bench results");
        std::fs::write(&output_path, json).expect("write bench trend artifact");
    }
}

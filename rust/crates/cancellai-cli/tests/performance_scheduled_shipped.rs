//! Heavy scheduled benchmarks for the discovery path the CLI actually executes (E21-S05,
//! repaired after E21 round-1 independent review).
//!
//! E21-S05 originally added only a small per-PR gate on the shipped path and left the scheduled
//! 10k/100k datasets running `cancellai-inventory::scan_scope`. The verifier's finding was
//! precise: those datasets prove the old inventory traversal meets its budget, not the shipped
//! provider discovery, and `scan_scope` is not reachable from the binary at all
//! (`docs/audits/2026-09-03-CODE_REVIEW.md`, `CR-TE-02`). This file is that retarget.
//!
//! It deliberately mirrors `cancellai-inventory/tests/performance_scheduled.rs`'s shape - the
//! same `#[ignore]`, the same `CANCELLAI_BENCH_SIZES`/`CANCELLAI_BENCH_OUTPUT` environment
//! contract, and the same `BenchResult` field set - so the machine-readable trend artifact
//! `docs/development/RELEASE_GATES.md` references stays readable across the change.
//!
//! Every dataset assertion is paired with a non-degenerate output assertion, for the same reason
//! the per-PR gate carries one: a benchmark measuring an empty tree is indistinguishable from a
//! fast one, and that is exactly how a performance gate can stay green while measuring nothing.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cancellai_platform::{FrozenClock, SyntheticProcessObserver, SystemFsObserver};
use cancellai_policy::{
    RetentionPolicy, ToolScope, builtin_provider_trust, resolve_claude, resolve_codex,
};
use cancellai_provider_claude::ClaudeProvider;
use cancellai_provider_codex::CodexProvider;

/// `(sessions per provider, max wall-clock seconds for the resolve pass alone)`. Generous, like
/// the inventory thresholds: this is a regression gate for a shared runner, not an SLA. Tree
/// creation is excluded from the measurement - it is setup, not the path being measured.
const THRESHOLDS: &[(usize, f64)] = &[(10_000, 60.0), (100_000, 480.0), (1_000_000, 3600.0)];

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

fn temp_root(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cancellai-shipped-scheduled-{label}-{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("create temp root");
    std::fs::canonicalize(&dir).unwrap_or(dir)
}

fn uuid_at(index: usize) -> String {
    format!("{index:08x}-0000-4000-8000-000000000000")
}

/// Builds both provider roots with `count` artifacts each, in the real nested layouts, and
/// returns how many directories the trees contain so the trend artifact keeps reporting a
/// meaningful `directories_visited`.
fn build_trees(base: &Path, count: usize) -> (PathBuf, PathBuf, usize) {
    let codex_root = base.join(".codex");
    let claude_root = base.join(".claude");
    std::fs::create_dir_all(&codex_root).unwrap();
    std::fs::create_dir_all(&claude_root).unwrap();
    std::fs::write(codex_root.join("auth.json"), "{}").unwrap();
    std::fs::write(claude_root.join("settings.json"), "{}").unwrap();

    let mut directories = 2usize;
    let mut seen_days = std::collections::HashSet::new();
    let mut seen_projects = std::collections::HashSet::new();

    for index in 0..count {
        let day = format!("sessions/2026/{:02}/{:02}", index % 12 + 1, index % 28 + 1);
        if seen_days.insert(day.clone()) {
            directories += 1;
        }
        let dir = codex_root.join(&day);
        std::fs::create_dir_all(&dir).unwrap();
        let session_id = uuid_at(index);
        std::fs::write(
            dir.join(format!("rollout-2026-01-01T00-00-00-{session_id}.jsonl")),
            format!(
                "{{\"type\":\"session_meta\",\"payload\":{{\"meta\":{{\"id\":\"{session_id}\"}}}}}}\n"
            ),
        )
        .unwrap();

        let project_name = format!("projects/synthetic-project-{:04}", index % 500);
        if seen_projects.insert(project_name.clone()) {
            directories += 1;
        }
        let project = claude_root.join(&project_name);
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join(format!("{session_id}.jsonl")), "{}\n").unwrap();
    }

    (codex_root, claude_root, directories)
}

#[test]
#[ignore = "heavy synthetic-dataset benchmark; run via the scheduled rust-benchmark workflow \
            or manually with `cargo test --release -- --ignored performance_scheduled_shipped`"]
fn the_shipped_discovery_path_meets_latency_thresholds_on_synthetic_datasets() {
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

        let base = temp_root(&format!("{size}"));
        let (codex_root, claude_root, directories) = build_trees(&base, size);

        let policy = RetentionPolicy {
            days: 1,
            keep_latest: 0,
            tool: ToolScope::All,
            // The process probe would spawn a subprocess and put its latency inside the
            // measurement; this benchmark is about traversal and classification.
            allow_running: true,
        };
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());
        let clock = FrozenClock::at(4_000_000_000);
        let claude_provider = ClaudeProvider::new(&claude_root, true);
        let codex_provider = CodexProvider::new(&codex_root, true);

        let started = Instant::now();
        let claude = resolve_claude(
            &claude_root,
            |p: &Path| claude_provider.protection(p),
            &policy,
            &process,
            &clock,
            builtin_provider_trust(),
        );
        let codex = resolve_codex(
            &codex_root,
            |p: &Path| codex_provider.protection(p),
            &policy,
            &SystemFsObserver,
            &process,
            &clock,
            builtin_provider_trust(),
        );
        let elapsed = started.elapsed().as_secs_f64();

        let observed = claude.observed().len() + codex.observed().len();
        std::fs::remove_dir_all(&base).ok();

        // Non-degenerate output, asserted before the timing is trusted.
        assert_eq!(
            observed,
            size * 2,
            "the shipped discovery path found {observed} artifacts where {} were planted; the \
             timing below would be measuring an empty walk",
            size * 2
        );
        assert!(
            claude.scan_complete() && codex.scan_complete(),
            "a fully readable synthetic tree must resolve complete; an incomplete scope means \
             the walk failed and the timing is not comparable"
        );

        let result = BenchResult {
            dataset_size: size,
            directories_visited: directories,
            paths_observed: observed,
            scan_seconds: elapsed,
            files_per_second: if elapsed > 0.0 {
                observed as f64 / elapsed
            } else {
                f64::INFINITY
            },
            threshold_seconds: threshold,
            within_threshold: elapsed < threshold,
        };
        println!(
            "cancellai-cli shipped-path bench: {observed} artifacts in {:.2}s ({:.0} artifacts/sec), threshold {:.0}s",
            result.scan_seconds, result.files_per_second, result.threshold_seconds
        );
        assert!(
            result.within_threshold,
            "resolving {observed} artifacts through the shipped path took {:.2}s, exceeding the \
             {:.0}s regression threshold",
            result.scan_seconds, result.threshold_seconds
        );
        results.push(result);
    }

    if let Ok(output_path) = std::env::var("CANCELLAI_BENCH_OUTPUT") {
        let json = serde_json::to_string_pretty(&results).expect("serialize bench results");
        std::fs::write(&output_path, json).expect("write bench trend artifact");
    }
}

//! Peak-memory regression gate for the discovery path the CLI actually executes (E10-S02).
//!
//! `performance_shipped_path.rs` (E21-S05) gates latency for `resolve_claude`/`resolve_codex` -
//! the functions `cancellai-cli`'s `resolve_all` calls - but `docs/development/RELEASE_GATES.md`
//! disclosed peak memory as a forward-looking budget, never actually measured: no
//! profiling/memory-accounting dependency existed in this workspace (AGENTS.md: do not add a
//! dependency merely to reduce implementation effort). `/proc/self/status`'s `VmHWM` needs none -
//! it is a plain file read, the same technique this crate's own `wsl::
//! SystemFilesystemContextObserver` already uses for `/proc/mounts` - so this file measures it
//! for real on Linux, one of this workspace's tier-1 platforms, rather than leaving the budget
//! entirely aspirational everywhere. macOS and Windows have no equivalent without a new
//! dependency this story does not add; `perf_support::peak_rss_bytes` reports `None` there, and
//! the gate below does not run there (`#[cfg(target_os = "linux")]` on the module itself, not a
//! runtime skip) - an honest platform gap, not a silently-passing assertion. `perf_support`'s
//! own unit tests (not this module) still exercise both the Linux and non-Linux branches of
//! `peak_rss_bytes` on every platform - unconditionally including it here is what makes that
//! possible.
//!
//! Like the latency gate, this is a regression ceiling, not an SLA: shared-runner memory
//! accounting includes the whole process (test harness, allocator overhead, and this measured
//! workload together), so the threshold is generous. It exists to catch an accidental
//! whole-tree buffering regression (the exact shape `CR-TE-04` found and `E21-S06` repaired in
//! `cancellai-provider-codex`), not to police allocator internals.

mod perf_support;

#[cfg(target_os = "linux")]
mod linux_memory_gate {
    use std::path::{Path, PathBuf};

    use cancellai_platform::{FrozenClock, SyntheticProcessObserver, SystemFsObserver};
    use cancellai_policy::{
        RetentionPolicy, ToolScope, builtin_provider_trust, resolve_claude, resolve_codex,
    };
    use cancellai_provider_claude::ClaudeProvider;
    use cancellai_provider_codex::CodexProvider;

    use crate::perf_support;

    /// Matches `performance_shipped_path.rs`'s own `SESSIONS_PER_PROVIDER`: small enough to
    /// stay fast and non-flaky on a shared runner, large enough that a whole-tree-buffering
    /// regression is unmistakable.
    const SESSIONS_PER_PROVIDER: usize = 2_000;
    /// Generous: this workspace's own measured baseline for a *much* larger single 287 MB
    /// rollout was 2.9 MB after `E21-S06` (`docs/development/RELEASE_GATES.md` G4). 128 MiB
    /// budgets for process/allocator/test-harness overhead this measurement cannot isolate from
    /// the workload itself, while still catching an accidental switch back to whole-file
    /// buffering.
    const MAX_PEAK_RSS_BYTES: u64 = 128 * 1024 * 1024;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-memory-perf-{label}-{}",
                std::process::id()
            ));
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).expect("create temp root");
            Self(dir)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn uuid_at(index: usize) -> String {
        format!("{index:08x}-0000-4000-8000-000000000000")
    }

    fn build_codex_root(root: &Path, count: usize) {
        std::fs::write(root.join("auth.json"), "{}").unwrap();
        for index in 0..count {
            let day = format!("sessions/2026/{:02}/{:02}", index % 12 + 1, index % 28 + 1);
            let dir = root.join(day);
            std::fs::create_dir_all(&dir).unwrap();
            let session_id = uuid_at(index);
            let body = format!(
                "{{\"type\":\"session_meta\",\"payload\":{{\"meta\":{{\"id\":\"{session_id}\"}}}}}}\n"
            );
            std::fs::write(
                dir.join(format!("rollout-2026-01-01T00-00-00-{session_id}.jsonl")),
                body,
            )
            .unwrap();
        }
    }

    fn build_claude_root(root: &Path, count: usize) {
        std::fs::write(root.join("settings.json"), "{}").unwrap();
        for index in 0..count {
            let project = root.join(format!("projects/synthetic-project-{:03}", index % 50));
            std::fs::create_dir_all(&project).unwrap();
            std::fs::write(project.join(format!("{}.jsonl", uuid_at(index))), "{}\n").unwrap();
        }
    }

    #[test]
    fn the_shipped_discovery_path_stays_within_a_peak_memory_budget() {
        let tree = TempTree::new("both");
        let codex_root = tree.0.join(".codex");
        let claude_root = tree.0.join(".claude");
        std::fs::create_dir_all(&codex_root).unwrap();
        std::fs::create_dir_all(&claude_root).unwrap();
        build_codex_root(&codex_root, SESSIONS_PER_PROVIDER);
        build_claude_root(&claude_root, SESSIONS_PER_PROVIDER);

        let policy = RetentionPolicy {
            days: 1,
            keep_latest: 0,
            tool: ToolScope::All,
            allow_running: true,
        };
        let trust = builtin_provider_trust();
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());
        let clock = FrozenClock::at(4_000_000_000);
        let fs = SystemFsObserver;
        let claude_provider = ClaudeProvider::new(&claude_root, true);
        let codex_provider = CodexProvider::new(&codex_root, true);

        let claude = resolve_claude(
            &claude_root,
            |p: &Path| claude_provider.protection(p),
            &policy,
            &process,
            &clock,
            trust,
        );
        let codex = resolve_codex(
            &codex_root,
            |p: &Path| codex_provider.protection(p),
            &policy,
            &fs,
            &process,
            &clock,
            trust,
        );

        // The `CR-TE-02` lesson, made into an assertion again: a benchmark measuring an empty
        // tree looks exactly like a lean one.
        assert_eq!(
            claude.observed().len(),
            SESSIONS_PER_PROVIDER,
            "the Claude discovery path stopped finding the sessions this benchmark planted; the \
             memory reading below would be measuring nothing"
        );
        assert_eq!(
            codex.observed().len(),
            SESSIONS_PER_PROVIDER,
            "the Codex discovery path stopped finding the rollouts this benchmark planted"
        );

        let peak_rss = perf_support::peak_rss_bytes()
            .expect("VmHWM must be readable on Linux, the only platform this module compiles on");
        println!(
            "cancellai-cli shipped-path peak RSS: {:.1} MiB, budget {:.0} MiB",
            peak_rss as f64 / (1024.0 * 1024.0),
            MAX_PEAK_RSS_BYTES as f64 / (1024.0 * 1024.0)
        );
        assert!(
            peak_rss <= MAX_PEAK_RSS_BYTES,
            "resolving {} artifacts per provider through the shipped path peaked at {} bytes, \
             exceeding the {MAX_PEAK_RSS_BYTES}-byte regression budget",
            SESSIONS_PER_PROVIDER,
            peak_rss
        );
    }
}

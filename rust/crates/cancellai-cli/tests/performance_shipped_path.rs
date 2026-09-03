//! Performance regression gate for the discovery path the CLI actually executes (E21-S05).
//!
//! `cancellai-inventory`'s `performance_micro.rs`/`performance_scheduled.rs` measure
//! `scan_scope`. The 2026-09-03 target-engine review (`docs/audits/2026-09-03-CODE_REVIEW.md`,
//! `CR-TE-02`) found that `scan_scope` is not reachable from the shipped binary at all: the
//! CLI's OBSERVE stage is the provider adapters' own traversal, and
//! [ADR-0018](../../../../docs/adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md)
//! decided to keep it that way. A performance gate pointed at code the binary never runs is
//! green for reasons that have nothing to do with the product, so this file measures
//! `resolve_claude`/`resolve_codex` - the exact functions `cancellai-cli`'s `resolve_all`
//! calls - over a synthetic provider tree.
//!
//! Like the inventory microbenchmark, this is a *regression ceiling*, not an SLA: CI runner
//! hardware varies far too much for a throughput assertion to be anything but flaky. It exists
//! to catch an asymptotic regression - a re-scan per view, an accidental quadratic - and to
//! fail when the benchmark stops measuring anything at all.
//!
//! That last property is the one `CR-TE-02` teaches: a benchmark that silently measures an
//! empty tree is indistinguishable from a fast one. Every timing assertion below is therefore
//! paired with an assertion on what the resolution actually produced, so a discovery path that
//! stops finding artifacts fails here instead of reporting an excellent number.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cancellai_platform::{FrozenClock, SyntheticProcessObserver, SystemFsObserver};
use cancellai_policy::{
    RetentionPolicy, ToolScope, build_actions, builtin_provider_trust, resolve_claude,
    resolve_codex,
};
use cancellai_provider_api::ProtectionOutcome;
use cancellai_provider_claude::ClaudeProvider;
use cancellai_provider_codex::CodexProvider;

/// Small enough to stay fast and non-flaky on a shared runner, large enough that a quadratic
/// regression is unmistakable.
const SESSIONS_PER_PROVIDER: usize = 2_000;
const MAX_SECONDS: f64 = 20.0;

struct TempTree(PathBuf);

impl TempTree {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "cancellai-shipped-path-perf-{label}-{}",
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

/// A Codex root with `count` rollouts spread across date directories, mirroring the real
/// `sessions/<year>/<month>/<day>/rollout-*.jsonl` shape rather than one flat directory - the
/// traversal cost this measures is a tree walk, not a single listing.
fn build_codex_root(root: &Path, count: usize) {
    std::fs::write(root.join("auth.json"), "{}").unwrap();
    std::fs::write(root.join("config.toml"), "model = \"synthetic\"\n").unwrap();
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

/// A Claude root with `count` sessions spread across project directories.
fn build_claude_root(root: &Path, count: usize) {
    std::fs::write(root.join("settings.json"), "{}").unwrap();
    for index in 0..count {
        let project = root.join(format!("projects/synthetic-project-{:03}", index % 50));
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join(format!("{}.jsonl", uuid_at(index))), "{}\n").unwrap();
    }
}

fn policy() -> RetentionPolicy {
    RetentionPolicy {
        days: 1,
        keep_latest: 0,
        tool: ToolScope::All,
        // The benchmark measures traversal and classification, not the process probe: a real
        // `SystemProcessObserver` would spawn a subprocess and put its latency inside the
        // measurement, which is not what this gate is about.
        allow_running: true,
    }
}

#[test]
fn the_shipped_discovery_path_completes_within_budget() {
    let tree = TempTree::new("both");
    let codex_root = tree.0.join(".codex");
    let claude_root = tree.0.join(".claude");
    std::fs::create_dir_all(&codex_root).unwrap();
    std::fs::create_dir_all(&claude_root).unwrap();
    build_codex_root(&codex_root, SESSIONS_PER_PROVIDER);
    build_claude_root(&claude_root, SESSIONS_PER_PROVIDER);

    let policy = policy();
    let trust = builtin_provider_trust();
    let process = SyntheticProcessObserver::complete(Vec::<String>::new());
    let clock = FrozenClock::at(4_000_000_000);
    let fs = SystemFsObserver;
    let claude_provider = ClaudeProvider::new(&claude_root, true);
    let codex_provider = CodexProvider::new(&codex_root, true);

    let started = Instant::now();
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
    let elapsed = started.elapsed();

    // The `CR-TE-02` lesson, made into an assertion: a benchmark measuring an empty tree looks
    // exactly like a fast one. If the shipped discovery path stops finding what this fixture
    // planted, this gate fails instead of reporting an excellent number.
    assert_eq!(
        claude.observed().len(),
        SESSIONS_PER_PROVIDER,
        "the Claude discovery path stopped finding the sessions this benchmark planted; the \
         timing below would be measuring nothing"
    );
    assert_eq!(
        codex.observed().len(),
        SESSIONS_PER_PROVIDER,
        "the Codex discovery path stopped finding the rollouts this benchmark planted"
    );
    assert!(
        claude.scan_complete() && codex.scan_complete(),
        "a fully readable synthetic tree must resolve as complete; an incomplete scope here \
         would mean the walk failed and the timing is not comparable"
    );

    assert!(
        elapsed.as_secs_f64() < MAX_SECONDS,
        "resolving {} artifacts per provider through the shipped path took {:.2}s, budget is \
         {MAX_SECONDS}s - a regression-detection ceiling, not a tight SLA",
        SESSIONS_PER_PROVIDER,
        elapsed.as_secs_f64()
    );
}

#[test]
fn planning_does_not_re_walk_the_filesystem() {
    // E04-S02 proved this property for `scan_scope`'s report views. The shipped path needs its
    // own version: `plan` must be a pure function of an already-resolved inventory, or every
    // command pays the traversal again. Measured rather than asserted structurally, because the
    // failure mode is a re-scan hidden inside a helper, which no type signature would reveal.
    let tree = TempTree::new("replan");
    let codex_root = tree.0.join(".codex");
    std::fs::create_dir_all(&codex_root).unwrap();
    build_codex_root(&codex_root, SESSIONS_PER_PROVIDER);

    let policy = policy();
    let process = SyntheticProcessObserver::complete(Vec::<String>::new());
    let clock = FrozenClock::at(4_000_000_000);
    let fs = SystemFsObserver;
    let provider = CodexProvider::new(&codex_root, true);

    let resolve_started = Instant::now();
    let codex = resolve_codex(
        &codex_root,
        |p: &Path| provider.protection(p),
        &policy,
        &fs,
        &process,
        &clock,
        builtin_provider_trust(),
    );
    let resolve_elapsed = resolve_started.elapsed();
    assert_eq!(codex.observed().len(), SESSIONS_PER_PROVIDER);

    let plan_started = Instant::now();
    let actions = build_actions(std::slice::from_ref(&codex.planning_view()));
    let plan_elapsed = plan_started.elapsed();
    assert_eq!(actions.len(), SESSIONS_PER_PROVIDER);

    // Planning is CPU-only over data already in memory. A generous multiple, not a tight one:
    // the point is to catch a re-traversal, which would put `plan_elapsed` in the same order of
    // magnitude as `resolve_elapsed`, not to police allocation.
    assert!(
        plan_elapsed < resolve_elapsed || plan_elapsed.as_millis() < 250,
        "planning took {plan_elapsed:?} against a resolve of {resolve_elapsed:?}; planning must \
         read the resolved inventory, never walk the filesystem again"
    );
}

/// The protection closure both resolvers take is called once per artifact. This exists so a
/// future change that makes it do real I/O per call is visible as a benchmark regression
/// rather than as a mysterious slowdown in the field.
#[test]
fn the_protection_probe_is_called_once_per_artifact() {
    let tree = TempTree::new("protection-calls");
    let codex_root = tree.0.join(".codex");
    std::fs::create_dir_all(&codex_root).unwrap();
    build_codex_root(&codex_root, 200);

    let calls = std::cell::Cell::new(0usize);
    let codex = resolve_codex(
        &codex_root,
        |_: &Path| {
            calls.set(calls.get() + 1);
            ProtectionOutcome::Clear
        },
        &policy(),
        &SystemFsObserver,
        &SyntheticProcessObserver::complete(Vec::<String>::new()),
        &FrozenClock::at(4_000_000_000),
        builtin_provider_trust(),
    );

    assert_eq!(codex.observed().len(), 200);
    assert_eq!(
        calls.get(),
        200,
        "the protection probe must run exactly once per discovered artifact"
    );
}

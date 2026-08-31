//! CLI contract tests (E06-S01): read-only default, JSON schema conformance, exit code
//! taxonomy, and the constitutional non-negotiables SI-007/SI-008/SI-009 apply to at the
//! command-line layer specifically. Every test spawns the real built binary
//! (`CARGO_BIN_EXE_cancellai-cli`, set automatically by Cargo for integration tests) against
//! an isolated, synthetic `$HOME` - never the real user's `~/.claude`/`~/.codex` (AGENTS.md:
//! "tests must never target real `~/.claude`/`~/.codex` data").
//!
//! One deliberate test boundary: this suite cannot exercise the "a real `claude`/`codex`
//! process is running" branch, because `SystemProcessObserver` shells out to the real `ps` on
//! whatever machine runs this suite - `cancellai-policy::retention`'s own unit tests already
//! cover that branch with a `SyntheticProcessObserver`. Every test here passes
//! `--allow-running` for exactly this reason, not because the flag is expected to matter in a
//! clean CI sandbox.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct TempHome(PathBuf);

impl TempHome {
    fn new(label: &str) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "cancellai-cli-test-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp home");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// A stale Claude session file (mtime far in the past), matching `SI-008/SI-009`-relevant
    /// fixtures elsewhere in this workspace: synthetic, never a real transcript.
    fn write_stale_claude_session(&self, project: &str, session_id: &str) -> PathBuf {
        let dir = self.0.join("claude/projects").join(project);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{session_id}.jsonl"));
        std::fs::write(&path, "{}").unwrap();
        set_old_mtime(&path);
        path
    }

    fn write_stale_codex_session(&self, session_id: &str) -> PathBuf {
        let dir = self.0.join("codex/sessions/2020/01/01");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("rollout-{session_id}.jsonl"));
        std::fs::write(
            &path,
            format!(
                "{}\n",
                serde_json::json!({
                    "type": "session_meta",
                    "payload": {"meta": {"id": session_id}}
                })
            ),
        )
        .unwrap();
        set_old_mtime(&path);
        path
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn set_old_mtime(path: &Path) {
    let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(0))
        .unwrap();
}

fn run(home: &TempHome, args: &[&str]) -> Output {
    let bin = std::env::var("CARGO_BIN_EXE_cancellai-cli")
        .expect("cargo sets this for integration tests");
    Command::new(bin)
        .args(args)
        .env("HOME", home.path())
        .env("CLAUDE_CONFIG_DIR", home.path().join("claude"))
        .env("CODEX_HOME", home.path().join("codex"))
        .output()
        .expect("spawn cancellai-cli")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn no_arguments_defaults_to_read_only_status_and_never_mutates() {
    let home = TempHome::new("default-status");
    let session = home.write_stale_claude_session("proj-a", "11111111-1111-4111-8111-111111111111");

    let output = run(&home, &[]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert!(
        session.exists(),
        "the default (no-subcommand) invocation must never delete anything - SI-007"
    );
}

#[test]
fn plan_is_read_only_and_produces_a_schema_conformant_document() {
    let home = TempHome::new("plan-json");
    let session = home.write_stale_claude_session("proj-a", "22222222-2222-4222-8222-222222222222");

    let output = run(
        &home,
        &["plan", "--json", "--allow-running", "--keep-latest", "0"],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    let doc: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("valid JSON");
    assert_eq!(doc["document_type"], "plan");
    assert_eq!(doc["schema_version"], 1);
    let actions = doc["actions"].as_array().expect("actions array");
    assert!(
        actions.iter().any(|a| a["action_class"] == "delete"),
        "a stale, unpinned, unprotected session must be a delete candidate: {actions:?}"
    );
    assert!(
        session.exists(),
        "plan must never delete anything - it only proposes"
    );
}

#[test]
fn clean_dry_run_never_deletes_anything() {
    let home = TempHome::new("clean-dry-run");
    let session = home.write_stale_claude_session("proj-a", "33333333-3333-4333-8333-333333333333");

    let output = run(
        &home,
        &[
            "clean",
            "--dry-run",
            "--allow-running",
            "--keep-latest",
            "0",
        ],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    assert!(session.exists(), "--dry-run must never delete anything");
}

#[test]
fn clean_yes_deletes_a_stale_unprotected_session_and_reports_it_in_the_result_document() {
    let home = TempHome::new("clean-yes");
    let claude_session =
        home.write_stale_claude_session("proj-a", "44444444-4444-4444-8444-444444444444");
    let codex_session = home.write_stale_codex_session("55555555-5555-4555-8555-555555555555");

    let output = run(
        &home,
        &[
            "clean",
            "--yes",
            "--json",
            "--allow-running",
            "--keep-latest",
            "0",
        ],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    let doc: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("valid JSON");
    assert_eq!(doc["document_type"], "result");
    assert_eq!(doc["summary"]["succeeded"], 2);
    assert_eq!(doc["summary"]["failed"], 0);
    assert!(
        !claude_session.exists(),
        "the stale claude session must actually be deleted"
    );
    assert!(
        !codex_session.exists(),
        "the stale codex session must actually be deleted"
    );
}

#[test]
fn clean_without_confirmation_or_dry_run_declines_and_deletes_nothing() {
    let home = TempHome::new("clean-no-confirm");
    let session = home.write_stale_claude_session("proj-a", "66666666-6666-4666-8666-666666666666");

    // No `--yes`/`--dry-run`, and stdin is closed (`/dev/null`-equivalent via no input piped),
    // so the interactive confirmation prompt reads EOF and must decline, not proceed.
    let bin = std::env::var("CARGO_BIN_EXE_cancellai-cli").unwrap();
    let output = Command::new(bin)
        .args(["clean", "--allow-running", "--keep-latest", "0"])
        .env("HOME", home.path())
        .env("CLAUDE_CONFIG_DIR", home.path().join("claude"))
        .env("CODEX_HOME", home.path().join("codex"))
        .stdin(std::process::Stdio::null())
        .output()
        .expect("spawn cancellai-cli");

    assert_eq!(
        output.status.code(),
        Some(1),
        "a declined/uninitiated confirmation must exit 1 (EXIT_CANCELLED): {}",
        stdout(&output)
    );
    assert!(
        session.exists(),
        "a declined confirmation must never delete anything"
    );
}

#[test]
fn keep_latest_protects_the_most_recent_session_from_a_json_clean_run() {
    let home = TempHome::new("clean-keep-latest");
    let old = home.write_stale_claude_session("proj-a", "77777777-7777-4777-8777-777777777777");
    let recent = home.write_stale_claude_session("proj-a", "88888888-8888-4888-8888-888888888888");
    // Make "recent" newer than "old" but still older than the default 7-day cutoff, so both
    // are stale - only `keep_latest` distinguishes them.
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&recent)
        .unwrap();
    file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1))
        .unwrap();

    let output = run(
        &home,
        &["clean", "--yes", "--allow-running", "--keep-latest", "1"],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    assert!(
        !old.exists(),
        "the older, unprotected session must be deleted"
    );
    assert!(
        recent.exists(),
        "the most recently modified session must stay protected"
    );
}

#[test]
fn an_unrecognized_flag_is_refused_with_exit_code_2_and_never_partially_runs() {
    let home = TempHome::new("invalid-flag");
    let output = run(&home, &["--this-flag-does-not-exist"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn clean_json_without_yes_or_dry_run_is_refused_before_touching_anything() {
    let home = TempHome::new("json-no-intent");
    let session = home.write_stale_claude_session("proj-a", "99999999-9999-4999-8999-999999999999");

    let output = run(&home, &["clean", "--json", "--allow-running"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "a machine-readable destructive run must state --yes/--dry-run explicitly (SI-007)"
    );
    assert!(session.exists());
}

#[test]
fn configure_writes_the_native_claude_retention_setting_and_preserves_other_keys() {
    let home = TempHome::new("configure");
    let claude_dir = home.path().join("claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        serde_json::json!({"someOtherKey": true}).to_string(),
    )
    .unwrap();

    let output = run(&home, &["configure", "--claude-retention", "30"]);
    assert!(output.status.success(), "{}", stdout(&output));

    let written = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(value["cleanupPeriodDays"], 30);
    assert_eq!(value["someOtherKey"], true);
}

#[test]
fn version_prints_something_and_exits_zero() {
    let home = TempHome::new("version");
    let output = run(&home, &["version"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("cancellai-cli"));
}

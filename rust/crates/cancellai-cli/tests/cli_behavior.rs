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
        // Canonicalize once, here, on a path this test harness itself just created - not a
        // security-relevant resolution. Without it, macOS's `/tmp`/`/var` compatibility
        // symlinks (`std::env::temp_dir()` returns a `/var/folders/...` path there, and `/var`
        // is itself `-> private/var`) would make `configure`'s `SealedRoot::establish` refuse
        // every test HOME here as "reached through an intermediate symlink" (E07-S09) - a
        // false positive from an OS-level symlink no attacker in this test's threat model
        // controls, not the attacker-planted symlinks these tests construct deliberately below.
        let dir = std::fs::canonicalize(&dir).expect("canonicalize temp home");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// A stale Claude session file (mtime far in the past), matching `SI-008/SI-009`-relevant
    /// fixtures elsewhere in this workspace: synthetic, never a real transcript. Written under
    /// `.claude` (not `claude`) so this is the real OS-*default* root once a test's `run()`
    /// sets only `HOME` - see that function's own docs for why this distinction now matters.
    fn write_stale_claude_session(&self, project: &str, session_id: &str) -> PathBuf {
        let dir = self.0.join(".claude/projects").join(project);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{session_id}.jsonl"));
        std::fs::write(&path, "{}").unwrap();
        set_old_mtime(&path);
        path
    }

    // Only used by this file's real-deletion tests, which are `#[cfg(unix)]`-only (see the
    // note above `clean_yes_deletes_a_stale_unprotected_session_and_reports_it_in_the_result_
    // document`) - without this, a Windows build sees no caller at all and `-D warnings` turns
    // that into a hard `dead_code` error.
    #[cfg(unix)]
    fn write_stale_codex_session(&self, session_id: &str) -> PathBuf {
        let dir = self.0.join(".codex/sessions/2020/01/01");
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

/// Runs the CLI against `home` as the real OS-*default* root (`$HOME/.claude`/`$HOME/.codex`,
/// via `HOME` alone - no `CLAUDE_CONFIG_DIR`/`CODEX_HOME` override). This is deliberate, not
/// incidental: most of this suite's fixtures exist to prove a real deletion happens under
/// legitimate conditions, and only the *default* root is ever mutation-eligible (ADR-0013,
/// `withhold_for_root_authority`) - a test that instead points `CLAUDE_CONFIG_DIR` at the
/// fixture (as an earlier version of this harness did for every test) exercises the *custom*-
/// root path unconditionally and could never have caught the E06 verifier review round 1
/// defect where that path was silently treated as default anyway. `env_remove` guards against
/// either override leaking in from whatever shell actually runs this suite.
fn run(home: &TempHome, args: &[&str]) -> Output {
    let bin = std::env::var("CARGO_BIN_EXE_cancellai-cli")
        .expect("cargo sets this for integration tests");
    Command::new(bin)
        .args(args)
        .env("HOME", home.path())
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
        .output()
        .expect("spawn cancellai-cli")
}

/// Runs the CLI against a *custom* Claude root (`$CLAUDE_CONFIG_DIR`), with `HOME` pointed
/// somewhere unrelated so the custom root can never coincidentally equal the real default path.
fn run_custom_claude_root(claude_root: &Path, unrelated_home: &Path, args: &[&str]) -> Output {
    let bin = std::env::var("CARGO_BIN_EXE_cancellai-cli")
        .expect("cargo sets this for integration tests");
    Command::new(bin)
        .args(args)
        .env("HOME", unrelated_home)
        .env("CLAUDE_CONFIG_DIR", claude_root)
        .env_remove("CODEX_HOME")
        .output()
        .expect("spawn cancellai-cli")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The exact, committed byte content of one golden CLI snapshot under `tests/golden/`
/// (E22-S03 verifier review round 1: the previous version of this suite only asserted a
/// usage-prefix substring, which a real output regression could still pass). Read from disk
/// rather than inlined as a string literal so the reviewed fixture - not a hand-transcribed
/// copy of it - is what every platform in the tier-1 matrix compares against.
fn golden(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read golden file {path:?}: {e}"))
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

// `cancellai-platform::identity::SystemIdentityObserver` reports `Unsupported` unconditionally
// on non-Unix platforms today (E03-S01's own disclosed residual risk, `#[cfg(not(unix))]` in
// `identity.rs`) - `ApprovedRoot::establish`/`bind` therefore always fails closed on Windows,
// so a real deletion can never succeed there yet regardless of anything E06 changed (E20-S01
// "Windows native backend" tracks closing this - moved from E07 into a dedicated epic once it
// became clear this work needs a real Windows/WSL environment to verify against, see E07.json's
// own objective note). First observed as an actual Windows CI failure on 2026-09-01 - not a
// regression, but the first time this crate's mutation-path integration tests were ever reached
// on Windows CI (an unrelated pre-existing clippy failure had aborted the job before them on
// every prior run, the same pattern E07-S05/E20-S04 already document).
#[cfg(unix)]
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
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
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

// See the identical `#[cfg(unix)]` note above `clean_yes_deletes_a_stale_unprotected_session_
// and_reports_it_in_the_result_document` - this test also requires a real deletion to succeed.
#[cfg(unix)]
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

// `configure`'s write path (`cancellai-sealedfs::SealedRoot`) now has a verified handle-relative
// implementation on both Unix and Windows (E20-S05 gave Windows its own `NtCreateFile`-based
// walk, `windows_sealed.rs`) - this test is cross-platform for that reason. It used to be
// Unix-only, with a separate Windows counterpart asserting the (then-correct) disclosed refusal;
// that gap is exactly what real Windows CI surfaced originally (E07-S09 verification session,
// 2026-09-02) - not caught locally since this workspace's own executor environment is macOS.
#[test]
fn configure_writes_the_native_claude_retention_setting_and_preserves_other_keys() {
    let home = TempHome::new("configure");
    let claude_dir = home.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        serde_json::json!({"someOtherKey": true}).to_string(),
    )
    .unwrap();

    let output = run(&home, &["configure", "--claude-retention", "30"]);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        stdout(&output),
        stderr(&output)
    );

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

#[test]
fn version_rejects_unrecognized_arguments() {
    let home = TempHome::new("version-invalid");
    let output = run(&home, &["version", "--definitely-invalid"]);
    assert_eq!(output.status.code(), Some(2));
}

/// E06 verifier review round 1's exact reproduction: a stale session under a root supplied
/// through `CLAUDE_CONFIG_DIR`, containing only a low-confidence `projects/` marker, must never
/// be deleted - the Python reference refuses every custom root regardless of confidence
/// (ADR-0013), and this proves the Rust CLI now matches that instead of reporting the custom
/// root as `origin=default` and deleting it.
#[test]
fn clean_refuses_to_mutate_a_custom_claude_config_dir_root_even_with_yes() {
    let unrelated_home = TempHome::new("custom-root-unrelated-home");
    let custom_root = TempHome::new("custom-root-claude-config-dir");
    let project = custom_root.path().join("projects/proj-a");
    std::fs::create_dir_all(&project).unwrap();
    let session = project.join("11111111-1111-4111-8111-111111111111.jsonl");
    std::fs::write(&session, "{}").unwrap();
    set_old_mtime(&session);

    let output = run_custom_claude_root(
        custom_root.path(),
        unrelated_home.path(),
        &[
            "clean",
            "--yes",
            "--json",
            "--tool",
            "claude",
            "--allow-running",
            "--keep-latest",
            "0",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(4),
        "a withheld custom-root run must exit SAFETY_BLOCK(4): {}",
        stdout(&output)
    );
    assert!(
        session.exists(),
        "a stale session under a custom, unverified root must never be deleted"
    );
}

/// The same reproduction via `plan`, proving the preview agrees with what `clean` actually
/// does (SI-007) instead of advertising a delete candidate the real run would refuse.
#[test]
fn plan_reports_a_custom_root_as_not_mutation_eligible_and_withholds_the_delete_candidate() {
    let unrelated_home = TempHome::new("custom-root-plan-unrelated-home");
    let custom_root = TempHome::new("custom-root-plan-claude-config-dir");
    let project = custom_root.path().join("projects/proj-a");
    std::fs::create_dir_all(&project).unwrap();
    let session = project.join("22222222-2222-4222-8222-222222222222.jsonl");
    std::fs::write(&session, "{}").unwrap();
    set_old_mtime(&session);

    let output = run_custom_claude_root(
        custom_root.path(),
        unrelated_home.path(),
        &[
            "plan",
            "--json",
            "--tool",
            "claude",
            "--allow-running",
            "--keep-latest",
            "0",
        ],
    );

    assert_eq!(output.status.code(), Some(4), "{}", stdout(&output));
    let doc: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("valid JSON");
    let claude_root_doc = doc["provider_roots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["provider_id"] == "claude-code")
        .expect("claude-code root entry");
    assert_eq!(claude_root_doc["origin"], "custom");
    assert_eq!(claude_root_doc["mutation_eligible"], false);
    let actions = doc["actions"].as_array().expect("actions array");
    assert!(
        actions.iter().all(|a| a["action_class"] != "delete"),
        "no delete candidate may be proposed for a non-default root: {actions:?}"
    );
}

#[test]
fn configure_refuses_a_custom_claude_config_dir_root() {
    let unrelated_home = TempHome::new("configure-custom-root-unrelated-home");
    let custom_root = TempHome::new("configure-custom-root");
    std::fs::write(
        custom_root.path().join("settings.json"),
        serde_json::json!({"cleanupPeriodDays": 7}).to_string(),
    )
    .unwrap();

    let output = run_custom_claude_root(
        custom_root.path(),
        unrelated_home.path(),
        &["configure", "--claude-retention", "30"],
    );

    assert_eq!(output.status.code(), Some(4), "{}", stdout(&output));
    let written = std::fs::read_to_string(custom_root.path().join("settings.json")).unwrap();
    assert!(
        written.contains("\"cleanupPeriodDays\": 7") || written.contains("\"cleanupPeriodDays\":7"),
        "a custom root must never be written to: {written}"
    );
}

#[test]
fn configure_refuses_malformed_settings_json_instead_of_silently_replacing_it() {
    let home = TempHome::new("configure-malformed");
    let claude_dir = home.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("settings.json"), "{not valid json").unwrap();

    let output = run(&home, &["configure", "--claude-retention", "30"]);

    assert_eq!(
        output.status.code(),
        Some(4),
        "stdout: {}\nstderr: {}",
        stdout(&output),
        stderr(&output)
    );
    let unchanged = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
    assert_eq!(
        unchanged, "{not valid json",
        "invalid settings.json must be refused, never silently discarded and replaced"
    );
}

#[cfg(unix)]
#[test]
fn configure_never_writes_through_a_preexisting_settings_json_symlink_to_an_outside_file() {
    // E06 verifier review round 1's exact reproduction: `settings.json` itself is a symlink
    // pointing outside the approved root. The fix is unpredictable-tmp-name + `create_new`
    // (O_EXCL) for the write, plus `rename`'s POSIX guarantee that it replaces the symlink
    // itself rather than following it - this proves the outside file is never touched and the
    // symlink is replaced by a real file.
    let home = TempHome::new("configure-symlink-outside-home");
    let claude_dir = home.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    let outside_dir = TempHome::new("configure-symlink-outside-target");
    let outside_file = outside_dir.path().join("outside-settings.json");
    // Valid JSON (the read side legitimately follows the symlink, same as
    // `cancellai.py`'s own `settings.read_text()` would) - the property under test is the
    // *write* side: this content must be byte-for-byte unchanged afterward, proving the
    // O_EXCL-unique-tmp-file write never opened (and therefore never wrote through) this
    // symlink's target.
    let outside_sentinel = serde_json::json!({"outsideSentinel": "must-never-change"}).to_string();
    std::fs::write(&outside_file, &outside_sentinel).unwrap();
    std::os::unix::fs::symlink(&outside_file, claude_dir.join("settings.json")).unwrap();

    let output = run(&home, &["configure", "--claude-retention", "30"]);
    assert!(output.status.success(), "{}", stdout(&output));

    let outside_after = std::fs::read_to_string(&outside_file).unwrap();
    assert_eq!(
        outside_after, outside_sentinel,
        "the outside file the pre-existing symlink pointed to must never be written through"
    );
    let settings_path = claude_dir.join("settings.json");
    assert!(
        !settings_path
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink(),
        "settings.json must be replaced by a real file, not left as (or written through as) a symlink"
    );
    let written = std::fs::read_to_string(&settings_path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(value["cleanupPeriodDays"], 30);
}

// E06 verifier review round 2's exact reproduction: no CLAUDE_CONFIG_DIR override at all -
// $HOME/.claude is itself a symlink to an unrelated directory. Authority must never come from
// the lexical "$HOME/.claude" name alone (SI-002/ADR-0013); a stale session reachable only
// through that symlink must not be deleted, and configure must not write through it either.
#[cfg(unix)]
#[test]
fn clean_refuses_to_mutate_when_home_dot_claude_is_itself_a_symlink() {
    let home = TempHome::new("symlink-default-root-home");
    let outside = TempHome::new("symlink-default-root-outside");
    let project = outside.path().join("projects/proj-a");
    std::fs::create_dir_all(&project).unwrap();
    let session = project.join("11111111-1111-4111-8111-111111111111.jsonl");
    std::fs::write(&session, "{}").unwrap();
    set_old_mtime(&session);
    std::os::unix::fs::symlink(outside.path(), home.path().join(".claude")).unwrap();

    let bin = std::env::var("CARGO_BIN_EXE_cancellai-cli").unwrap();
    let output = Command::new(bin)
        .args([
            "clean",
            "--tool",
            "claude",
            "--days",
            "7",
            "--keep-latest",
            "0",
            "--allow-running",
            "--yes",
            "--json",
        ])
        .env("HOME", home.path())
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
        .output()
        .expect("spawn cancellai-cli");

    assert_eq!(
        output.status.code(),
        Some(4),
        "a symlinked $HOME/.claude must never be treated as the default, mutation-eligible \
         root: {}",
        stdout(&output)
    );
    assert!(
        session.exists(),
        "a stale session reachable only through a symlinked default-named root must never be \
         deleted"
    );
}

// E07-S09 round-1 independent verifier review's exact native reproduction: unlike the test
// above, `$HOME/.claude` itself is a *real* directory - `$HOME` is the symlink, one component
// up. `roots::is_symlink` alone (which only ever inspected the leaf) cannot catch this;
// `ApprovedRoot::establish`'s own `canonicalize()` would otherwise silently resolve through it.
#[cfg(unix)]
#[test]
fn clean_refuses_to_mutate_when_home_itself_is_a_symlink_to_a_real_dot_claude() {
    let home_target = TempHome::new("intermediate-symlink-home-target");
    let home_like = home_target.path().parent().unwrap().join(format!(
        "cancellai-cli-test-intermediate-symlink-home-link-{}",
        std::process::id()
    ));
    std::os::unix::fs::symlink(home_target.path(), &home_like).unwrap();

    let project = home_target.path().join(".claude/projects/proj-a");
    std::fs::create_dir_all(&project).unwrap();
    let session = project.join("11111111-1111-4111-8111-111111111111.jsonl");
    std::fs::write(&session, "{}").unwrap();
    set_old_mtime(&session);

    let bin = std::env::var("CARGO_BIN_EXE_cancellai-cli").unwrap();
    let output = Command::new(bin)
        .args([
            "clean",
            "--tool",
            "claude",
            "--days",
            "7",
            "--keep-latest",
            "0",
            "--allow-running",
            "--yes",
            "--json",
        ])
        .env("HOME", &home_like)
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
        .output()
        .expect("spawn cancellai-cli");

    std::fs::remove_file(&home_like).ok();

    assert_eq!(
        output.status.code(),
        Some(4),
        "a default root reached through an intermediate $HOME symlink must never be treated as \
         mutation-eligible, even when $HOME/.claude itself is a real directory: {}",
        stdout(&output)
    );
    assert!(
        session.exists(),
        "a stale session reachable only through an intermediate-symlinked $HOME must never be \
         deleted"
    );
}

#[cfg(unix)]
#[test]
fn configure_refuses_when_home_dot_claude_is_itself_a_symlink() {
    let home = TempHome::new("configure-symlink-default-root-home");
    let outside = TempHome::new("configure-symlink-default-root-outside");
    std::fs::write(
        outside.path().join("settings.json"),
        serde_json::json!({"cleanupPeriodDays": 7}).to_string(),
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.path(), home.path().join(".claude")).unwrap();

    let bin = std::env::var("CARGO_BIN_EXE_cancellai-cli").unwrap();
    let output = Command::new(bin)
        .args(["configure", "--claude-retention", "30"])
        .env("HOME", home.path())
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
        .output()
        .expect("spawn cancellai-cli");

    assert_eq!(output.status.code(), Some(4), "{}", stdout(&output));
    let written = std::fs::read_to_string(outside.path().join("settings.json")).unwrap();
    assert!(
        written.contains("\"cleanupPeriodDays\": 7") || written.contains("\"cleanupPeriodDays\":7"),
        "a symlinked default-named root must never be written to: {written}"
    );
}

// Windows counterparts of the two Unix symlink-authority-bypass tests above (E07-S07). Rust's
// cross-platform `FileType::is_symlink()` - what `roots::is_symlink` actually calls - reports
// `true` for a Windows directory symlink created via `std::os::windows::fs::symlink_dir`, the
// same std-only mechanism this crate's own dependency policy prefers over a new FFI/`windows-sys`
// dependency for reparse-point creation (`AGENTS.md` "do not add a dependency merely to reduce
// implementation effort"). This does not by itself prove NTFS *junction* reparse points (a
// distinct reparse tag, `IO_REPARSE_TAG_MOUNT_POINT`, created only via `DeviceIoControl` - no std
// API creates one) are refused identically; that remains this story's disclosed residual scope,
// not claimed as covered here. Requires `SeCreateSymbolicLinkPrivilege` (Developer Mode or an
// elevated process), which this repo's own Windows CI runners carry - see
// `.github/workflows/rust.yml`.
#[cfg(windows)]
#[test]
fn clean_refuses_to_mutate_when_home_dot_claude_is_itself_a_symlink() {
    let home = TempHome::new("symlink-default-root-home");
    let outside = TempHome::new("symlink-default-root-outside");
    let project = outside.path().join("projects/proj-a");
    std::fs::create_dir_all(&project).unwrap();
    let session = project.join("11111111-1111-4111-8111-111111111111.jsonl");
    std::fs::write(&session, "{}").unwrap();
    set_old_mtime(&session);
    std::os::windows::fs::symlink_dir(outside.path(), home.path().join(".claude")).unwrap();

    let bin = std::env::var("CARGO_BIN_EXE_cancellai-cli").unwrap();
    let output = Command::new(bin)
        .args([
            "clean",
            "--tool",
            "claude",
            "--days",
            "7",
            "--keep-latest",
            "0",
            "--allow-running",
            "--yes",
            "--json",
        ])
        .env("HOME", home.path())
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
        .output()
        .expect("spawn cancellai-cli");

    assert_eq!(
        output.status.code(),
        Some(4),
        "a symlinked $HOME/.claude must never be treated as the default, mutation-eligible \
         root: {}",
        stdout(&output)
    );
    assert!(
        session.exists(),
        "a stale session reachable only through a symlinked default-named root must never be \
         deleted"
    );
}

#[cfg(windows)]
#[test]
fn configure_refuses_when_home_dot_claude_is_itself_a_symlink() {
    let home = TempHome::new("configure-symlink-default-root-home");
    let outside = TempHome::new("configure-symlink-default-root-outside");
    std::fs::write(
        outside.path().join("settings.json"),
        serde_json::json!({"cleanupPeriodDays": 7}).to_string(),
    )
    .unwrap();
    std::os::windows::fs::symlink_dir(outside.path(), home.path().join(".claude")).unwrap();

    let bin = std::env::var("CARGO_BIN_EXE_cancellai-cli").unwrap();
    let output = Command::new(bin)
        .args(["configure", "--claude-retention", "30"])
        .env("HOME", home.path())
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
        .output()
        .expect("spawn cancellai-cli");

    assert_eq!(output.status.code(), Some(4), "{}", stdout(&output));
    let written = std::fs::read_to_string(outside.path().join("settings.json")).unwrap();
    assert!(
        written.contains("\"cleanupPeriodDays\": 7") || written.contains("\"cleanupPeriodDays\":7"),
        "a symlinked default-named root must never be written to: {written}"
    );
}

// ----------------------------------------------------------------------------------------
// E21 round-1 independent review: the native regression the verifier required. A `projects/`
// directory that exists and cannot be read must withhold and exit 4, not report a clean empty
// scan and exit 0. Reproduced by Codex against the round-1 implementation; pinned here so the
// escape cannot reopen.
// ----------------------------------------------------------------------------------------

/// chmod(0o000) denies a non-root reader only. If this process can still read such a directory
/// the assertions below would pass for the wrong reason, so skip loudly instead.
#[cfg(unix)]
fn can_deny_reads(path: &Path) -> bool {
    std::fs::read_dir(path).is_err()
}

#[cfg(unix)]
#[test]
fn an_unreadable_claude_projects_root_withholds_and_exits_four() {
    use std::os::unix::fs::PermissionsExt;

    let home = TempHome::new("unreadable-projects-root");
    let session = home.write_stale_claude_session("proj-a", "11111111-1111-4111-8111-111111111111");
    let projects = home.path().join(".claude/projects");
    std::fs::set_permissions(&projects, std::fs::Permissions::from_mode(0o000)).unwrap();
    if !can_deny_reads(&projects) {
        std::fs::set_permissions(&projects, std::fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skipped: this process can read a 0o000 directory (running as root?)");
        return;
    }

    let output = run(
        &home,
        &[
            "clean",
            "--yes",
            "--allow-running",
            "--days",
            "1",
            "--keep-latest",
            "0",
            "--tool",
            "claude",
        ],
    );
    std::fs::set_permissions(&projects, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(
        output.status.code(),
        Some(4),
        "an unobservable provider root must exit 4 (SAFETY_BLOCK), not 0: stdout={} stderr={}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        session.exists(),
        "nothing may be deleted from a scope this run could not observe (SI-008/SI-009/C-02)"
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_claude_projects_root_is_reported_incomplete_with_a_real_count() {
    use std::os::unix::fs::PermissionsExt;

    let home = TempHome::new("unreadable-projects-inspect");
    home.write_stale_claude_session("proj-a", "22222222-2222-4222-8222-222222222222");
    let projects = home.path().join(".claude/projects");
    std::fs::set_permissions(&projects, std::fs::Permissions::from_mode(0o000)).unwrap();
    if !can_deny_reads(&projects) {
        std::fs::set_permissions(&projects, std::fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skipped: this process can read a 0o000 directory (running as root?)");
        return;
    }

    let output = run(
        &home,
        &["inspect", "--json", "--allow-running", "--tool", "claude"],
    );
    std::fs::set_permissions(&projects, std::fs::Permissions::from_mode(0o755)).unwrap();

    let doc: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("inspect emits JSON");
    let claude = doc["scan_completeness"]
        .as_array()
        .expect("scan_completeness is an array")
        .iter()
        .find(|s| s["scope"] == "claude-code")
        .expect("claude scope present")
        .clone();
    assert_eq!(claude["complete"], serde_json::json!(false));
    assert_eq!(
        claude["error_count"],
        serde_json::json!(1),
        "error_count must be the real number of unobserved paths"
    );
}

/// E20-S02/E20-S03 round-1 independent verifier review: `RuntimeEnvironment`/
/// `FilesystemContextObserver` existed but had no production caller anywhere in this
/// workspace - unit-tested in isolation, never proven to reach the real CLI binary's actual
/// output. This spawns the real built binary (not a unit test of `documents.rs` alone) and
/// asserts both new fields are genuinely present in its `--json` output.
#[test]
fn inspect_json_surfaces_runtime_environment_and_filesystem_context() {
    let home = TempHome::new("wsl-facts-surfaced");
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();

    let output = run(
        &home,
        &["inspect", "--json", "--allow-running", "--tool", "claude"],
    );
    let doc: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("inspect emits JSON");

    let runtime_environment = doc["runtime_environment"]
        .as_str()
        .expect("runtime_environment must be a string");
    assert!(
        matches!(runtime_environment, "wsl2" | "native"),
        "got {runtime_environment:?}"
    );

    let claude_root_doc = doc["provider_roots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["provider_id"] == "claude-code")
        .expect("claude-code root entry");
    let filesystem_context = claude_root_doc["filesystem_context"]
        .as_str()
        .expect("filesystem_context must be a string");
    assert!(
        filesystem_context == "linux"
            || filesystem_context == "windows_mounted"
            || filesystem_context.starts_with("other:")
            || filesystem_context.starts_with("unsupported:"),
        "got {filesystem_context:?}"
    );
}

/// The counterexample that keeps the two tests above from being satisfied by "always withhold":
/// a Claude home with no `projects/` at all is a structurally empty install, and must stay
/// non-destructive *and* non-withholding.
#[test]
fn a_claude_home_without_projects_is_complete_not_withheld() {
    let home = TempHome::new("no-projects-dir");
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();
    std::fs::write(home.path().join(".claude/settings.json"), "{}").unwrap();

    let output = run(
        &home,
        &["inspect", "--json", "--allow-running", "--tool", "claude"],
    );
    let doc: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("inspect emits JSON");
    let claude = doc["scan_completeness"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["scope"] == "claude-code")
        .unwrap()
        .clone();
    assert_eq!(
        claude["complete"],
        serde_json::json!(true),
        "a provider that is simply not installed is a known-empty state, not missing evidence"
    );
    assert_eq!(output.status.code(), Some(0));
}

#[cfg(unix)]
#[test]
fn an_unreadable_home_withholds_rather_than_reporting_nothing_to_clean() {
    use std::os::unix::fs::PermissionsExt;

    // Found during E21's own post-review self-check, one level above the verifier's finding and
    // in the same class: `resolve_*` gated on `root.exists()`, and `Path::exists()` answers
    // `false` for both "not installed" and "not allowed to look". With an unreadable `$HOME`
    // the engine reported a clean empty scan and exited 0 while the reference exits 4.
    let home = TempHome::new("unreadable-home");
    home.write_stale_claude_session("proj-a", "33333333-3333-4333-8333-333333333333");
    std::fs::set_permissions(home.path(), std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(home.path()).is_ok() {
        std::fs::set_permissions(home.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skipped: this process can read a 0o000 directory (running as root?)");
        return;
    }

    let output = run(
        &home,
        &[
            "clean",
            "--yes",
            "--allow-running",
            "--days",
            "1",
            "--keep-latest",
            "0",
        ],
    );
    std::fs::set_permissions(home.path(), std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(
        output.status.code(),
        Some(4),
        "an unreadable home must withhold, not report nothing to clean: stdout={} stderr={}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The counterexample that keeps the test above from being satisfied by "always withhold": a
/// home with neither provider installed is a known-empty state and must stay at exit 0.
#[test]
fn a_home_with_no_provider_installed_is_complete_and_exits_zero() {
    let home = TempHome::new("no-provider-installed");
    let output = run(
        &home,
        &[
            "clean",
            "--yes",
            "--allow-running",
            "--days",
            "1",
            "--keep-latest",
            "0",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "a machine with no provider installed has nothing to observe and nothing to withhold: {}",
        stdout(&output)
    );
}

// E22-S03 (ADR-0019, CR-TE-07): the help/version/per-command-help surface this crate lacked
// entirely before switching from a hand-rolled parser to `clap`, plus the SI-007-relevant
// properties that must hold identically under the new parser - an unknown subcommand and a
// flag irrelevant to the chosen command must both still be refused with exit code 2, never
// silently accepted or guessed toward `clean`. These four tests are this story's own "golden
// CLI snapshot for help/version output" (the Verification Contract's own words) - run on
// every tier-1 platform because this file runs under `cargo test --workspace` in `rust.yml`'s
// `quality` job matrix (macOS/Linux/Windows) and in `release.yml`'s `verify-rust` job.

#[test]
fn top_level_help_matches_the_committed_golden_snapshot() {
    let home = TempHome::new("top-level-help");
    let output = run(&home, &["--help"]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(stdout(&output), golden("top_level_help.txt"));
}

#[test]
fn top_level_short_help_flag_behaves_like_the_long_form() {
    let home = TempHome::new("top-level-short-help");
    let long = run(&home, &["--help"]);
    let short = run(&home, &["-h"]);
    assert!(short.status.success(), "{}", stdout(&short));
    assert_eq!(stdout(&long), stdout(&short));
}

#[test]
fn top_level_version_flag_prints_the_crate_version_and_exits_zero() {
    let home = TempHome::new("top-level-version-flag");
    let output = run(&home, &["--version"]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(
        stdout(&output),
        format!("cancellai-cli {}\n", env!("CARGO_PKG_VERSION")),
        "the version line's exact shape - not merely one substring of it - is the golden \
         contract; only the version number itself is expected to vary release to release"
    );
}

#[test]
fn every_subcommand_help_matches_its_committed_golden_snapshot() {
    let home = TempHome::new("per-command-help");
    for (command, golden_file) in [
        ("status", "status_help.txt"),
        ("inspect", "inspect_help.txt"),
        ("plan", "plan_help.txt"),
        ("clean", "clean_help.txt"),
        ("configure", "configure_help.txt"),
        ("version", "version_help.txt"),
    ] {
        let output = run(&home, &[command, "--help"]);
        assert!(
            output.status.success(),
            "{command} --help should exit 0: {}",
            stdout(&output)
        );
        assert_eq!(
            stdout(&output),
            golden(golden_file),
            "{command} --help output drifted from the committed golden snapshot"
        );
    }
}

#[test]
fn an_unrecognized_subcommand_is_refused_with_exit_code_2_and_never_runs_anything() {
    let home = TempHome::new("unrecognized-subcommand");
    let session = home.write_stale_claude_session("proj-a", "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
    let output = run(&home, &["frobnicate"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "SI-007: an unrecognized subcommand must never be guessed at - exit INVALID_INPUT: {}",
        stderr(&output)
    );
    assert!(session.exists());
}

#[test]
fn a_flag_irrelevant_to_the_chosen_command_is_refused_not_silently_accepted() {
    // Before E22-S03, `status --dry-run` was accepted and simply had no effect - the
    // pre-`clap` parser recognized every flag across every command. `--dry-run` only has
    // meaning for `clean`; `status` must now refuse it outright (AC3).
    let home = TempHome::new("irrelevant-flag");
    let output = run(&home, &["status", "--dry-run"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));

    let output = run(&home, &["plan", "--yes"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));

    let output = run(&home, &["configure", "--json"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
}

/// E22-S03 verifier review round 1: `status --help --dry-run` exits 0 and prints help,
/// silently not validating `--dry-run` at all - `clap`'s help/version actions always
/// short-circuit the remaining argument list the moment they are matched, the same
/// convention `git`/`cargo`/most `clap`-based CLIs already follow (`git commit --help
/// --bogus` shows help too). AC3's "flags irrelevant to a command are rejected" describes
/// ordinary argument validation, not this precedence; docs/CLI_RUST.md's "Argument parsing"
/// section states the exception explicitly, and it is safe by construction rather than by
/// convention alone: `cli::parse` returns an `Invocation` only when `clap` neither printed
/// help/version nor errored, so no code path from `--help`/`-h`/`--version` ever reaches
/// `main.rs`'s dispatch - it always exits before an `Invocation` (in particular
/// `Invocation::Clean`) could be constructed at all (SI-007 stays about which mutation an
/// *executed* invocation resolves to, not about this exit).
#[test]
fn help_short_circuits_remaining_argument_validation_by_design() {
    let home = TempHome::new("help-short-circuits");
    let output = run(&home, &["status", "--help", "--dry-run"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "help must still win when it appears before an otherwise-irrelevant flag: {}",
        stderr(&output)
    );
    assert_eq!(stdout(&output), golden("status_help.txt"));
}

/// The mirror case: an irrelevant flag *before* `--help` is still refused. `clap` parses
/// left to right and only short-circuits once it actually reaches the help action, so
/// ordering - not merely presence - determines which error wins; both orderings are pinned
/// here so a future parser change cannot silently flip either one.
#[test]
fn an_irrelevant_flag_before_help_is_still_refused() {
    let home = TempHome::new("irrelevant-flag-before-help");
    let output = run(&home, &["status", "--dry-run", "--help"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
}

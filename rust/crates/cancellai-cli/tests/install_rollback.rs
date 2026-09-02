//! Install/rollback smoke tests (E06-S03: "Define side-by-side invocation, data
//! compatibility, and rollback from Rust candidate to Python reference during beta").
//!
//! The real "installer" this story's verification contract can exercise is narrower than
//! Epic E17's future canonical release factory (`docs/RELEASING.md` "Target Rust release
//! factory" - cross-platform packages, SBOM, signed provenance - is explicitly out of scope
//! until E17): during the beta/side-by-side period there is no packaged distribution for
//! `cancellai-cli` at all, only a binary built from source. What this suite proves instead is
//! the two properties the story's ACs actually name:
//!
//! - a beta user can tell which engine/version they are running (`version`'s output);
//! - "rollback" during beta is not a migration to undo - `cancellai-cli` shares neither an
//!   install path nor any local state file with `cancellai` (the Python reference's installed
//!   command name, `pyproject.toml`), so a user who stops invoking `cancellai-cli` has changed
//!   nothing about their existing Python install. This suite proves that concretely: every
//!   command this binary offers, including a real `clean`, touches nothing outside the
//!   provider roots it was explicitly pointed at (C-10: cancellAI's local state is disposable
//!   and rebuildable - here demonstrated by there being none to begin with).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct TempHome(PathBuf);

impl TempHome {
    fn new(label: &str) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "cancellai-cli-rollback-test-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // Canonicalize once, here, on a path this test harness itself just created - not a
        // security-relevant resolution. Without it, macOS's `/var -> private/var` compatibility
        // symlink (`std::env::temp_dir()` returns a `/var/folders/...` path there) would make
        // `clean`'s new whole-path `verify_no_intermediate_links` check (E07-S09) refuse every
        // test HOME here as "reached through an intermediate symlink" - a false positive from
        // an OS-level symlink no attacker in this test's threat model controls.
        let dir = std::fs::canonicalize(&dir).expect("canonicalize temp home");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn binary_path() -> PathBuf {
    PathBuf::from(
        std::env::var("CARGO_BIN_EXE_cancellai-cli")
            .expect("cargo sets this for integration tests"),
    )
}

/// Resolves the real OS-*default* root via `HOME` alone - see `cli_behavior.rs::run`'s own
/// docs for why a `CLAUDE_CONFIG_DIR`/`CODEX_HOME` override (as an earlier version of this
/// helper always set) exercises the wrong, never-mutation-eligible path instead (ADR-0013).
fn run_in(home: &TempHome, cwd: &Path, args: &[&str]) -> Output {
    Command::new(binary_path())
        .args(args)
        .current_dir(cwd)
        .env("HOME", home.path())
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
        .output()
        .expect("spawn cancellai-cli")
}

fn run(home: &TempHome, args: &[&str]) -> Output {
    run_in(home, home.path(), args)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Every path under `home`, relative to it, sorted - a filesystem snapshot for before/after
/// comparison.
fn snapshot(home: &Path) -> BTreeSet<PathBuf> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeSet<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            out.insert(path.strip_prefix(root).unwrap().to_path_buf());
            if path.is_dir() {
                walk(&path, root, out);
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(home, home, &mut out);
    out
}

fn write_stale_claude_session(home: &TempHome, session_id: &str) -> PathBuf {
    let dir = home.path().join(".claude/projects/proj-a");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{session_id}.jsonl"));
    std::fs::write(&path, "{}").unwrap();
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(0))
        .unwrap();
    path
}

#[test]
fn version_output_identifies_this_as_the_rust_engine_with_a_concrete_version() {
    let home = TempHome::new("version-identity");
    let output = run(&home, &["version"]);
    assert!(output.status.success());
    let text = stdout(&output);
    // Distinct from `cancellai.py`'s own installed command name (`pyproject.toml`: `cancellai`)
    // - a beta user reading this cannot mistake it for the Python reference.
    assert!(
        text.contains("cancellai-cli"),
        "version output must name the engine explicitly, got: {text:?}"
    );
    let version_token = text.trim().rsplit(' ').next().unwrap_or("");
    assert!(
        version_token
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit()),
        "version output must include a concrete version number, got: {text:?}"
    );
}

#[test]
fn every_read_only_command_leaves_no_trace_anywhere_under_home() {
    let home = TempHome::new("read-only-stateless");
    let session = write_stale_claude_session(&home, "11111111-1111-4111-8111-111111111111");
    let before = snapshot(home.path());

    for args in [
        vec!["status"],
        vec!["status", "--json"],
        vec!["inspect", "--json"],
        vec!["plan", "--json", "--allow-running"],
        vec![
            "clean",
            "--dry-run",
            "--allow-running",
            "--keep-latest",
            "0",
        ],
    ] {
        let output = run(&home, &args);
        assert!(output.status.success(), "{args:?}: {}", stdout(&output));
    }

    let after = snapshot(home.path());
    assert_eq!(
        before, after,
        "a read-only command (including clean --dry-run) must never create, remove, or touch \
         any path under $HOME - there is no cancellAI-owned local state to leave a trace in"
    );
    assert!(session.exists(), "the session itself must be untouched");
}

// `cancellai-platform::identity::SystemIdentityObserver` reports `Unsupported` unconditionally
// on non-Unix platforms today (E03-S01's own disclosed residual risk) - `ApprovedRoot::
// establish`/`bind` therefore always fails closed on Windows, so a real deletion can never
// succeed there yet (E20-S01 "Windows native backend" tracks closing this) - see the identical
// note in `cli_behavior.rs` above its own two real-deletion tests.
#[cfg(unix)]
#[test]
fn a_real_clean_touches_only_the_provider_artifact_it_deletes_nothing_else_anywhere() {
    let home = TempHome::new("clean-no-side-state");
    let session = write_stale_claude_session(&home, "22222222-2222-4222-8222-222222222222");
    let before = snapshot(home.path());

    let output = run(
        &home,
        &["clean", "--yes", "--allow-running", "--keep-latest", "0"],
    );
    assert!(output.status.success(), "{}", stdout(&output));

    let after = snapshot(home.path());
    assert!(
        !session.exists(),
        "the stale session must actually be deleted"
    );

    let mut expected = before;
    expected.remove(session.strip_prefix(home.path()).unwrap());
    // The now-empty parent project directory is still present (deletion removes the file, not
    // its parent) - both sides describe exactly the same set otherwise.
    assert_eq!(
        expected, after,
        "clean must remove exactly the one artifact it planned to delete and create/touch \
         nothing else anywhere under $HOME - no hidden database, log, or cache file"
    );
}

#[test]
fn the_built_binary_runs_correctly_regardless_of_its_invocation_directory() {
    // A minimal install smoke test: the binary must not assume anything about its own working
    // directory (a relative config/data path, a cwd-relative asset) - the property a real
    // installed binary (run from wherever the user's shell happens to be) actually needs.
    // Full packaged-install verification (checksums, SBOM, signed provenance) is Epic E17's
    // canonical release factory, not this beta-period story.
    let home = TempHome::new("cwd-independence");
    write_stale_claude_session(&home, "33333333-3333-4333-8333-333333333333");
    let elsewhere = std::env::temp_dir();

    let output = run_in(&home, &elsewhere, &["status", "--json"]);
    assert!(
        output.status.success(),
        "invoked from an unrelated cwd: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let doc: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("valid JSON");
    assert_eq!(doc["document_type"], "inventory");
}

#[test]
fn the_rust_and_python_commands_never_collide_on_path() {
    // `cancellai` (Python, `pyproject.toml`) and `cancellai-cli` (Rust, this crate's own
    // `Cargo.toml` package name) are different binary names - installing/building one can
    // never shadow the other on `$PATH`, which is the concrete mechanism "rollback" relies on
    // during beta (a user simply stops invoking the one they don't want).
    assert_ne!(binary_path().file_stem().unwrap(), "cancellai");
    assert_eq!(binary_path().file_stem().unwrap(), "cancellai-cli");
}

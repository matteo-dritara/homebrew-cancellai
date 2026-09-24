//! E06-S04's verification contract on Windows: a native reproduction, on the E21-S02 partial-scan
//! fixtures and in both root-origin scenarios, showing the engine withholds where the frozen
//! reference withholds (E06 review round 14).
//!
//! `scripts/rust_python_parity.py` runs the reference itself, and only on macOS and Linux: the
//! reference's fixture recipes lock a directory with `chmod 0o000`, which Windows does not honour.
//! Here the same four trees are built natively and locked with an NTFS deny ACE (`icacls /deny`),
//! and the engine's behaviour is asserted against what the committed characterization records
//! for each fixture (`tests/fixtures/characterization/<id>.characterization.json`): no action for
//! the tool, because the scan was incomplete. Every case also runs once unlocked, so a refusal is
//! shown to come from the lock and not from something else - a nightly build, a custom root.
//!
//! Runs in `rust.yml`'s stable-channel job on `windows-latest`; elsewhere it does not compile in.
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The well-known SID for Everyone: a deny ACE for it applies to this process whatever account
/// the runner uses, administrators included - a deny ACE is evaluated before any allow.
const EVERYONE: &str = "*S-1-1-0";

struct Tree(PathBuf);

impl Tree {
    fn new(label: &str) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "cancellai-win-partial-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(std::fs::canonicalize(&dir).unwrap())
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn write_old(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
    let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_modified(std::time::UNIX_EPOCH).unwrap();
}

fn icacls(args: &[&str]) {
    let status = Command::new("icacls").args(args).output().unwrap();
    assert!(
        status.status.success(),
        "icacls {args:?}: {}",
        String::from_utf8_lossy(&status.stdout)
    );
}

/// Denies listing a directory, or reading a file, to everyone - the NTFS equivalent of the
/// recipes' `chmod 0o000`. Refuses to go on if the lock does not actually hold, so a test can
/// never pass for the wrong reason.
fn lock(path: &Path) {
    let target = path.to_str().unwrap();
    if path.is_dir() {
        icacls(&[target, "/deny", &format!("{EVERYONE}:(RD)")]);
        assert!(
            std::fs::read_dir(path).is_err(),
            "the deny ACE did not stop listing {target}"
        );
    } else {
        icacls(&[target, "/deny", &format!("{EVERYONE}:(R)")]);
        assert!(
            std::fs::read(path).is_err(),
            "the deny ACE did not stop reading {target}"
        );
    }
}

fn unlock(path: &Path) {
    icacls(&[path.to_str().unwrap(), "/remove:d", EVERYONE]);
}

/// One E21-S02 fixture: the provider tree, the path the recipe locks, and the sessions that
/// must survive while it is locked.
struct Fixture {
    tool: &'static str,
    scope: &'static str,
    locked: PathBuf,
    sessions: Vec<PathBuf>,
}

fn claude_markers(root: &Path) {
    write_old(&root.join("settings.json"), "{}");
    write_old(&root.join("keybindings.json"), "{}");
}

fn codex_markers(root: &Path) {
    write_old(&root.join("auth.json"), "{}");
    write_old(&root.join("config.toml"), "model = \"synthetic\"\n");
}

fn claude_session(root: &Path, project: &str, id: &str) -> PathBuf {
    let path = root
        .join("projects")
        .join(project)
        .join(format!("{id}.jsonl"));
    write_old(&path, "{}\n");
    path
}

fn codex_rollout(root: &Path, id: &str, day: &str) -> PathBuf {
    let (year, rest) = day.split_at(4);
    let (month, dom) = (&rest[1..3], &rest[4..6]);
    let meta = serde_json::json!({"type": "session_meta", "payload": {"meta": {"id": id}}});
    let path = root
        .join("sessions")
        .join(year)
        .join(month)
        .join(dom)
        .join(format!("rollout-{day}T09-00-00-{id}.jsonl"));
    write_old(&path, &format!("{meta}\n"));
    path
}

/// `build_claude_partial_tree`: a session whose companion payload directory cannot be listed.
fn claude_partial_tree(root: &Path) -> Fixture {
    claude_markers(root);
    let project = "synthetic-project-c";
    let a = claude_session(root, project, "55555555-5555-4555-8555-555555555551");
    let b = claude_session(root, project, "55555555-5555-4555-8555-555555555552");
    let id = "55555555-5555-4555-8555-555555555553";
    let c = claude_session(root, project, id);
    let payload = root.join("projects").join(project).join(id);
    write_old(&payload.join("tool-result.txt"), "synthetic\n");
    Fixture {
        tool: "claude",
        scope: "claude-code",
        locked: payload,
        sessions: vec![a, b, c],
    }
}

/// `build_claude_partial_project`: a second project directory that cannot be listed.
fn claude_partial_project(root: &Path) -> Fixture {
    claude_markers(root);
    let a = claude_session(
        root,
        "synthetic-project-d",
        "99999999-9999-4999-8999-999999999991",
    );
    let b = claude_session(
        root,
        "synthetic-project-d",
        "99999999-9999-4999-8999-999999999992",
    );
    let c = claude_session(
        root,
        "synthetic-project-e",
        "99999999-9999-4999-8999-999999999993",
    );
    Fixture {
        tool: "claude",
        scope: "claude-code",
        locked: root.join("projects").join("synthetic-project-e"),
        sessions: vec![a, b, c],
    }
}

/// `build_codex_partial_tree`: a date directory under `sessions/` that cannot be listed.
fn codex_partial_tree(root: &Path) -> Fixture {
    codex_markers(root);
    let a = codex_rollout(root, "88888888-8888-4888-8888-888888888881", "2026-05-01");
    let b = codex_rollout(root, "88888888-8888-4888-8888-888888888882", "2026-05-01");
    let c = codex_rollout(root, "88888888-8888-4888-8888-888888888883", "2026-05-02");
    Fixture {
        tool: "codex",
        scope: "codex-cli",
        locked: root.join("sessions").join("2026").join("05").join("02"),
        sessions: vec![a, b, c],
    }
}

/// `build_codex_unreadable_rollout`: a rollout that lists and stats but cannot be opened.
fn codex_unreadable_rollout(root: &Path) -> Fixture {
    codex_markers(root);
    let a = codex_rollout(root, "88888888-8888-4888-8888-888888888891", "2026-05-01");
    let b = codex_rollout(root, "88888888-8888-4888-8888-888888888892", "2026-05-01");
    let c = codex_rollout(root, "88888888-8888-4888-8888-888888888893", "2026-05-01");
    Fixture {
        tool: "codex",
        scope: "codex-cli",
        locked: c.clone(),
        sessions: vec![a, b, c],
    }
}

/// Builds one fixture under a provider root.
type Recipe = fn(&Path) -> Fixture;

const FIXTURES: [(&str, Recipe); 4] = [
    ("claude-partial-tree", claude_partial_tree),
    ("claude-partial-project", claude_partial_project),
    ("codex-partial-tree", codex_partial_tree),
    ("codex-unreadable-rollout", codex_unreadable_rollout),
];

/// The two root-origin scenarios `rust_python_parity.py` runs: the provider's default directory
/// under `HOME`, or a custom root named by `CLAUDE_CONFIG_DIR`/`CODEX_HOME`.
#[derive(Clone, Copy, PartialEq)]
enum Origin {
    Default,
    Custom,
}

fn provider_root(tree: &Tree, tool: &str, origin: Origin) -> PathBuf {
    match (origin, tool) {
        (Origin::Default, "claude") => tree.0.join("home").join(".claude"),
        (Origin::Default, _) => tree.0.join("home").join(".codex"),
        (Origin::Custom, "claude") => tree.0.join("custom-claude"),
        (Origin::Custom, _) => tree.0.join("custom-codex"),
    }
}

fn run(tree: &Tree, tool: &str, origin: Origin, root: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cancellai-cli"));
    command
        .args(args)
        .env("HOME", tree.0.join("home"))
        .env("USERPROFILE", tree.0.join("home"))
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME");
    if origin == Origin::Custom {
        command.env(
            if tool == "claude" {
                "CLAUDE_CONFIG_DIR"
            } else {
                "CODEX_HOME"
            },
            root,
        );
    }
    command.output().unwrap()
}

fn scope_completeness(output: &Output, scope: &str) -> serde_json::Value {
    let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "inspect emits JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    doc["scan_completeness"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["scope"] == scope)
        .unwrap_or_else(|| panic!("{scope} missing from {doc}"))
        .clone()
}

fn clean_args(tool: &str) -> [&str; 11] {
    [
        "clean",
        "--yes",
        "--json",
        "--allow-running",
        "--days",
        "30",
        "--keep-latest",
        "0",
        "--tool",
        tool,
        "--verbose",
    ]
}

#[test]
fn the_e21_s02_partial_scan_fixtures_withhold_natively_on_windows() {
    let stable = matches!(
        option_env!("CANCELLAI_CHANNEL")
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("stable" | "beta")
    );
    for (id, build) in FIXTURES {
        for origin in [Origin::Default, Origin::Custom] {
            let label = format!(
                "{id}/{}",
                if origin == Origin::Default {
                    "default"
                } else {
                    "custom"
                }
            );
            let tree = Tree::new(id);
            let tool = if id.starts_with("claude") {
                "claude"
            } else {
                "codex"
            };
            let root = provider_root(&tree, tool, origin);
            let fixture = build(&root);
            assert_eq!(fixture.tool, tool);

            lock(&fixture.locked);
            let inspect = run(
                &tree,
                tool,
                origin,
                &root,
                &["inspect", "--json", "--allow-running", "--tool", tool],
            );
            let clean = run(&tree, tool, origin, &root, &clean_args(tool));
            let survivors: Vec<bool> = fixture.sessions.iter().map(|s| s.exists()).collect();
            unlock(&fixture.locked);

            // What the frozen reference records for every one of these fixtures: the scan is
            // incomplete and the tool gets no action.
            let scope = scope_completeness(&inspect, fixture.scope);
            assert_eq!(
                scope["complete"],
                serde_json::json!(false),
                "{label}: {scope}"
            );
            assert!(
                scope["error_count"].as_u64().unwrap_or(0) >= 1,
                "{label}: {scope}"
            );
            assert_eq!(
                clean.status.code(),
                Some(4),
                "{label}: {}",
                String::from_utf8_lossy(&clean.stdout)
            );
            assert!(
                survivors.iter().all(|s| *s),
                "{label}: a locked scope deleted a session"
            );

            // The control: unlocked, the same default-root tree deletes on a stable build, so the
            // refusal above came from the lock.
            if origin == Origin::Default && stable {
                let unlocked = run(&tree, tool, origin, &root, &clean_args(tool));
                assert_eq!(
                    unlocked.status.code(),
                    Some(0),
                    "{label} unlocked: {}",
                    String::from_utf8_lossy(&unlocked.stdout)
                );
                assert!(
                    fixture.sessions.iter().any(|s| !s.exists()),
                    "{label} unlocked: nothing was deleted, so the locked run proves nothing"
                );
            }
        }
    }
}

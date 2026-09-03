//! Differential fixture parity against the committed Python characterization corpus (E05-S04
//! AC1-equivalent verification: "Differential parity and native-delete fake CLI integration
//! tests"). See `cancellai-provider-claude`'s `tests/claude_fixture_parity.rs` for the same
//! approach applied to Claude fixtures and the full rationale for why this is fixture-recipe
//! parity rather than a `scripts/diff_harness.py` JSON-document comparison (a documented,
//! narrower residual - see this story's evidence packet).

use std::fs;
use std::path::{Path, PathBuf};

use cancellai_inventory::ScopeObservation;
use cancellai_provider_codex::{CodexProvider, RolloutCategory};

struct FixtureRoot(PathBuf);

impl FixtureRoot {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "cancellai-codex-fixture-parity-{label}-{}",
            std::process::id()
        ));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).expect("create fixture root");
        Self(dir)
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

/// Mirrors `recipes.py::_codex_markers`.
fn write_codex_markers(root: &Path) {
    fs::write(root.join("auth.json"), "{}").unwrap();
    fs::write(root.join("config.toml"), "model = \"synthetic\"\n").unwrap();
}

/// Mirrors `recipes.py::_codex_rollout` (age is irrelevant to this crate's CLASSIFY-stage
/// output, so it is not reproduced).
fn write_codex_rollout(root: &Path, session_id: &str, day: &str, parent: Option<&str>) -> PathBuf {
    let mut parts = day.split('-');
    let year = parts.next().unwrap();
    let month = parts.next().unwrap();
    let dom = parts.next().unwrap();
    let path = root
        .join("sessions")
        .join(year)
        .join(month)
        .join(dom)
        .join(format!("rollout-{day}T09-00-00-{session_id}.jsonl"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let meta = serde_json::json!({
        "type": "session_meta",
        "payload": {"meta": {"id": session_id, "parent_thread_id": parent}}
    });
    fs::write(&path, format!("{meta}\n")).unwrap();
    path
}

/// Mirrors `recipes.py::_write_protected_entry`.
fn write_protected_entry(root: &Path, name: &str) {
    let path = root.join(name);
    if name.ends_with(".json") {
        fs::write(&path, "{}").unwrap();
    } else if name.ends_with(".toml") {
        fs::write(&path, "key = \"synthetic\"\n").unwrap();
    } else {
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join(".synthetic-keep"), "synthetic").unwrap();
    }
}

/// tests/fixtures/recipes.py::build_codex_normal_session, checked against
/// tests/fixtures/characterization/codex-normal-session.characterization.json's
/// `plan_summary.roots.codex.markers` (auth.json, config.toml, sessions).
#[test]
fn codex_normal_session_matches_the_committed_characterization() {
    let fixture = FixtureRoot::new("normal-session");
    write_codex_markers(&fixture.0);
    write_codex_rollout(
        &fixture.0,
        "22222222-2222-4222-8222-222222222222",
        "2026-08-20",
        None,
    );

    let provider = CodexProvider::new(&fixture.0, true);
    assert_eq!(
        provider.fingerprint().markers,
        vec!["auth.json", "config.toml", "sessions"]
    );

    let discovered = provider.discover_sessions();
    assert_eq!(
        discovered.observation,
        ScopeObservation::complete(),
        "this fixture is fully readable: any recorded completeness reason means the walk failed \
         to observe something it should have (E21-S03)"
    );
    let sessions = discovered.sessions;
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].category, RolloutCategory::Session);
    assert_eq!(
        sessions[0].session_id,
        "22222222-2222-4222-8222-222222222222"
    );
    assert_eq!(sessions[0].parent_session_id, None);
}

/// tests/fixtures/recipes.py::build_codex_subagent_tree, checked against
/// tests/fixtures/characterization/codex-subagent-tree.characterization.json (3 sessions
/// discovered/selected; `by_category` shows `codex:session` actions=3 - the root plus its two
/// subagent children). This story's own AC1 ("Root/subagent trees are preserved as graph
/// relationships") is what `group_into_subagent_trees` proves beyond what the Python
/// characterization directly records: not just that 3 rollouts exist, but that they resolve to
/// one shared root via `parent_thread_id`.
#[test]
fn codex_subagent_tree_matches_the_committed_characterization() {
    let fixture = FixtureRoot::new("subagent-tree");
    write_codex_markers(&fixture.0);
    let root_id = "33333333-3333-4333-8333-333333333333";
    write_codex_rollout(&fixture.0, root_id, "2026-05-01", None);
    write_codex_rollout(
        &fixture.0,
        "33333333-3333-4333-8333-333333333334",
        "2026-05-01",
        Some(root_id),
    );
    write_codex_rollout(
        &fixture.0,
        "33333333-3333-4333-8333-333333333335",
        "2026-05-01",
        Some(root_id),
    );

    let provider = CodexProvider::new(&fixture.0, true);
    assert_eq!(
        provider.fingerprint().markers,
        vec!["auth.json", "config.toml", "sessions"]
    );

    let discovered = provider.discover_sessions();
    assert_eq!(
        discovered.observation,
        ScopeObservation::complete(),
        "this fixture is fully readable: any recorded completeness reason means the walk failed \
         to observe something it should have (E21-S03)"
    );
    let sessions = discovered.sessions;
    assert_eq!(sessions.len(), 3, "all three rollouts must be discovered");

    let trees = provider.subagent_trees();
    assert_eq!(
        trees.len(),
        1,
        "the three rollouts must resolve to one tree"
    );
    assert_eq!(trees[0].root_id, root_id);
    assert_eq!(trees[0].members.len(), 3);
}

/// tests/fixtures/recipes.py::build_codex_protected_state, checked against
/// tests/fixtures/characterization/codex-protected-state.characterization.json's
/// `plan_summary.roots.codex.markers` (auth.json, config.toml, memories, plugins, rules,
/// skills) and `coverage.protected.names` (all six CODEX_PROTECTED_NAMES entries).
#[test]
fn codex_protected_state_matches_the_committed_characterization() {
    let fixture = FixtureRoot::new("protected-state");
    write_codex_markers(&fixture.0);
    let mut names: Vec<&str> = cancellai_provider_codex::CODEX_PROTECTED_NAMES.to_vec();
    names.sort_unstable();
    for name in &names {
        let path = fixture.0.join(name);
        if !path.exists() {
            write_protected_entry(&fixture.0, name);
        }
    }

    let provider = CodexProvider::new(&fixture.0, true);
    assert_eq!(
        provider.fingerprint().markers,
        vec![
            "auth.json",
            "config.toml",
            "memories",
            "plugins",
            "rules",
            "skills"
        ]
    );

    for name in cancellai_provider_codex::CODEX_PROTECTED_NAMES {
        let outcome = provider.protection(&fixture.0.join(name));
        assert!(
            outcome.is_protected(),
            "{name} should be reported protected"
        );
    }
}

/// tests/fixtures/recipes.py::build_codex_symlink_escape, checked against
/// tests/fixtures/characterization/codex-symlink-escape.characterization.json's `actions: 1`
/// (only the real rollout is discovered; the symlink is never followed for accounting). The
/// fixture's own symlink filename ("escape.jsonl") is not UUID-shaped, so `discover_codex_sessions`
/// excludes it via the same UUID-extraction check that would exclude any non-rollout-shaped
/// name, independent of its symlink status - `cancellai-provider-codex`'s own
/// `a_symlinked_directory_is_never_descended_into_but_a_symlinked_file_still_is_a_rollout` unit
/// test is what separately proves a *UUID-named* symlinked file is still discovered, matching
/// `cancellai.py`'s `iter_files` exactly.
#[test]
fn codex_symlink_escape_matches_the_committed_characterization() {
    let fixture = FixtureRoot::new("symlink-escape");
    write_codex_markers(&fixture.0);
    write_codex_rollout(
        &fixture.0,
        "66666666-6666-4666-8666-666666666666",
        "2026-01-01",
        None,
    );
    let outside = fixture.0.parent().unwrap().join(format!(
        "outside-codex-root-{}",
        fixture.0.file_name().unwrap().to_string_lossy()
    ));
    fs::create_dir_all(&outside).unwrap();
    let outside_file = outside.join("not-a-rollout.jsonl");
    fs::write(&outside_file, "synthetic content outside the approved root").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_file, fixture.0.join("sessions/escape.jsonl")).unwrap();

    let provider = CodexProvider::new(&fixture.0, true);
    let discovered = provider.discover_sessions();
    assert_eq!(
        discovered.observation,
        ScopeObservation::complete(),
        "this fixture is fully readable: any recorded completeness reason means the walk failed \
         to observe something it should have (E21-S03)"
    );
    let sessions = discovered.sessions;
    assert_eq!(
        sessions.len(),
        1,
        "the symlink must never be counted as a second session"
    );
    assert_eq!(
        sessions[0].session_id,
        "66666666-6666-4666-8666-666666666666"
    );

    fs::remove_dir_all(&outside).ok();
}

/// tests/fixtures/recipes.py::build_codex_layout_drift, checked against
/// tests/fixtures/characterization/codex-layout-drift.characterization.json's `actions: 1`
/// (the one real rollout) and `coverage.unknown.names` (`plugin_cache_v2`, reported unknown,
/// never treated as cleanable) - this crate does not port the `coverage_state` reporting
/// bucket (see `cancellai-provider-claude`'s evidence packet residual for the same point
/// applied to Claude), so the check here is that the unrecognized entry does not disrupt
/// ordinary rollout discovery, and is not itself a recognized root marker.
#[test]
fn codex_layout_drift_matches_the_committed_characterization() {
    let fixture = FixtureRoot::new("layout-drift");
    write_codex_markers(&fixture.0);
    write_codex_rollout(
        &fixture.0,
        "77777777-7777-4777-8777-777777777777",
        "2026-07-01",
        None,
    );
    fs::create_dir_all(fixture.0.join("plugin_cache_v2")).unwrap();
    fs::write(
        fixture.0.join("plugin_cache_v2/index.bin"),
        "synthetic-unknown-layout",
    )
    .unwrap();

    let provider = CodexProvider::new(&fixture.0, true);
    assert_eq!(
        provider.fingerprint().markers,
        vec!["auth.json", "config.toml", "sessions"]
    );
    assert!(!provider.fingerprint().markers.contains(&"plugin_cache_v2"));

    let discovered = provider.discover_sessions();
    assert_eq!(
        discovered.observation,
        ScopeObservation::complete(),
        "this fixture is fully readable: any recorded completeness reason means the walk failed \
         to observe something it should have (E21-S03)"
    );
    let sessions = discovered.sessions;
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].session_id,
        "77777777-7777-4777-8777-777777777777"
    );
}

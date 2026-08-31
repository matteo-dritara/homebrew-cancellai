//! Differential fixture parity against the committed Python characterization corpus (E05-S03
//! AC1: "Reference fixtures match normative Python contract").
//!
//! `tests/fixtures/recipes.py`'s `build_claude_*` functions and
//! `tests/fixtures/characterization/claude-*.characterization.json` are this repository's
//! normative record of what `cancellai.py` actually does on each fixture
//! (`docs/development/VERIFICATION_STRATEGY.md#python-reference-contract`). No cross-language
//! test runner exists yet to drive the Rust adapter against those Python-generated trees
//! directly (that plumbing - a JSON_CONTRACTS-conformant inventory document assembled from a
//! full OBSERVE+CLASSIFY pipeline, fed through `scripts/diff_harness.py` - is E06/CLI-parity
//! scope, not this CLASSIFY-stage-only adapter story). Instead, each test below reproduces one
//! `build_claude_*` recipe's exact tree by hand (the recipe function name and the exact
//! characterization values asserted against are cited in each test), and checks this crate's
//! output against the values the committed characterization JSON records for that same
//! recipe - the same synthetic-filesystem reality, independently checked by both languages.
//!
//! This is a documented, narrower form of "differential" than the eventual JSON-document
//! comparison `scripts/diff_harness.py` will run once a Rust CLI exists (residual risk,
//! recorded in this story's evidence packet) - but every value asserted here is copied
//! character-for-character from a committed characterization file, not invented, so a change
//! to either side that breaks parity is caught the same way an unexplained NORMATIVE
//! divergence would be (AGENTS.md: "unexplained Python/Rust differential behavior is a
//! failure").

use std::fs;
use std::path::{Path, PathBuf};

use cancellai_provider_claude::{ClaudeProvider, RootConfidence};

struct FixtureRoot(PathBuf);

impl FixtureRoot {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "cancellai-claude-fixture-parity-{label}-{}",
            std::process::id()
        ));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).expect("create fixture root");
        Self(dir)
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        // Best-effort: a partial-tree fixture may still hold a 0o000 directory that a plain
        // recursive remove cannot enter: restore permissions first, matching
        // tests/fixtures/README.md's documented cleanup convention for the Python recipes.
        #[cfg(unix)]
        restore_permissions(&self.0);
        fs::remove_dir_all(&self.0).ok();
    }
}

#[cfg(unix)]
fn restore_permissions(root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fn walk(dir: &Path) {
        let Ok(entries) = fs::read_dir(dir) else {
            let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o755));
                walk(&path);
            }
        }
    }
    walk(root);
}

/// Mirrors `recipes.py::_claude_markers`.
fn write_claude_markers(root: &Path) {
    fs::write(root.join("settings.json"), "{}").unwrap();
    fs::write(root.join("keybindings.json"), "{}").unwrap();
}

/// Mirrors `recipes.py::_claude_session` (age is irrelevant to this crate's CLASSIFY-stage
/// output, so it is not reproduced - nothing here reads mtimes for a retention decision).
fn write_claude_session(root: &Path, project: &str, session_id: &str) -> PathBuf {
    let path = root
        .join("projects")
        .join(project)
        .join(format!("{session_id}.jsonl"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "{\"type\": \"user\"}\n").unwrap();
    path
}

/// Mirrors `recipes.py::_claude_session_with_payload`. Only used by the `#[cfg(unix)]`
/// partial-tree test below (which locks a directory via Unix permission bits) - gated the
/// same way so it is not flagged dead code on non-Unix targets (found via Windows CI, once
/// an unrelated pre-existing clippy failure elsewhere in the workspace stopped masking this
/// crate from ever actually being clippy-checked there).
#[cfg(unix)]
fn write_claude_session_with_payload(
    root: &Path,
    project: &str,
    session_id: &str,
) -> (PathBuf, PathBuf) {
    let session_path = write_claude_session(root, project, session_id);
    let payload_dir = root.join("projects").join(project).join(session_id);
    fs::create_dir_all(payload_dir.join("tool-results")).unwrap();
    fs::write(
        payload_dir.join("tool-results/large.txt"),
        "synthetic payload",
    )
    .unwrap();
    (session_path, payload_dir)
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

/// tests/fixtures/recipes.py::build_claude_normal_session, checked against
/// tests/fixtures/characterization/claude-normal-session.characterization.json's
/// `plan_summary.roots.claude` (origin=default, confidence=default,
/// markers=[keybindings.json, projects, settings.json]).
#[test]
fn claude_normal_session_matches_the_committed_characterization() {
    let fixture = FixtureRoot::new("normal-session");
    write_claude_markers(&fixture.0);
    write_claude_session(
        &fixture.0,
        "synthetic-project-a",
        "11111111-1111-4111-8111-111111111111",
    );

    let provider = ClaudeProvider::new(&fixture.0, true);
    let fp = provider.fingerprint();
    assert_eq!(fp.confidence, RootConfidence::Default);
    assert_eq!(
        fp.markers,
        vec!["keybindings.json", "projects", "settings.json"]
    );

    let sessions = provider.discover_sessions();
    assert_eq!(sessions.sessions.len(), 1);
    assert_eq!(sessions.sessions[0].project, "synthetic-project-a");
    assert_eq!(
        sessions.sessions[0].session_id,
        "11111111-1111-4111-8111-111111111111"
    );
    assert!(sessions.degraded_companions.is_empty());
}

/// tests/fixtures/recipes.py::build_claude_active_data, checked against
/// tests/fixtures/characterization/claude-active-data.characterization.json (same markers as
/// normal-session; one session in a different project).
#[test]
fn claude_active_data_matches_the_committed_characterization() {
    let fixture = FixtureRoot::new("active-data");
    write_claude_markers(&fixture.0);
    write_claude_session(
        &fixture.0,
        "synthetic-project-b",
        "44444444-4444-4444-8444-444444444444",
    );

    let provider = ClaudeProvider::new(&fixture.0, true);
    assert_eq!(
        provider.fingerprint().markers,
        vec!["keybindings.json", "projects", "settings.json"]
    );

    let sessions = provider.discover_sessions();
    assert_eq!(sessions.sessions.len(), 1);
    assert_eq!(
        sessions.sessions[0].session_id,
        "44444444-4444-4444-8444-444444444444"
    );
}

/// tests/fixtures/recipes.py::build_claude_protected_state, checked against
/// tests/fixtures/characterization/claude-protected-state.characterization.json's
/// `plan_summary.roots.claude.markers` (agent-memory, keybindings.json, plugins,
/// settings.json - the four CLAUDE_PROTECTED_NAMES entries that also happen to be
/// ROOT_MARKERS entries) and `coverage.protected.names` (all ten CLAUDE_PROTECTED_NAMES,
/// including the six that are protected but not root markers).
#[test]
fn claude_protected_state_matches_the_committed_characterization() {
    let fixture = FixtureRoot::new("protected-state");
    write_claude_markers(&fixture.0);
    let mut names: Vec<&str> = cancellai_provider_claude::CLAUDE_PROTECTED_NAMES.to_vec();
    names.sort_unstable();
    for name in &names {
        let path = fixture.0.join(name);
        if !path.exists() {
            write_protected_entry(&fixture.0, name);
        }
    }

    let provider = ClaudeProvider::new(&fixture.0, true);
    assert_eq!(
        provider.fingerprint().markers,
        vec![
            "agent-memory",
            "keybindings.json",
            "plugins",
            "settings.json"
        ]
    );

    for name in cancellai_provider_claude::CLAUDE_PROTECTED_NAMES {
        let outcome = provider.protection(&fixture.0.join(name));
        assert!(
            outcome.is_protected(),
            "{name} should be reported protected"
        );
    }
}

/// tests/fixtures/recipes.py::build_claude_partial_tree, checked against
/// tests/fixtures/characterization/claude-partial-tree.characterization.json's
/// `plan_summary.scan` (`complete: false`, one incomplete scope) - "the scan must record the
/// scope as incomplete, not as empty" (the fixture's own description). This crate has no
/// aggregate `Scan`/`complete` concept of its own (that is E04's `ScopeCompleteness`, not yet
/// wired to provider-adapter session discovery - a residual this story's evidence packet
/// records); the equivalent signal here is `degraded_companions` being non-empty while
/// `sessions` still reports all three sessions, never silently dropping the one whose
/// companion could not be read.
#[cfg(unix)]
#[test]
fn claude_partial_tree_matches_the_committed_characterization() {
    let fixture = FixtureRoot::new("partial-tree");
    write_claude_markers(&fixture.0);
    let project = "synthetic-project-c";
    write_claude_session(&fixture.0, project, "55555555-5555-4555-8555-555555555551");
    write_claude_session(&fixture.0, project, "55555555-5555-4555-8555-555555555552");
    let (_, locked_payload) = write_claude_session_with_payload(
        &fixture.0,
        project,
        "55555555-5555-4555-8555-555555555553",
    );
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&locked_payload, fs::Permissions::from_mode(0o000)).unwrap();

    let provider = ClaudeProvider::new(&fixture.0, true);
    assert_eq!(
        provider.fingerprint().markers,
        vec!["keybindings.json", "projects", "settings.json"]
    );

    let sessions = provider.discover_sessions();
    fs::set_permissions(&locked_payload, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(
        sessions.sessions.len(),
        3,
        "all three sessions must be reported, including the one whose companion is locked"
    );
    assert_eq!(sessions.degraded_companions, vec![locked_payload]);
}

/// tests/fixtures/recipes.py::build_claude_symlink_protected_name, checked against
/// tests/fixtures/characterization/claude-symlink-protected-name.characterization.json's
/// `plan_summary.roots.claude.markers` (keybindings.json, settings.json only - no "projects"
/// since it was never created, no "plugins" since "Plugins" is a case-variant symlink to a
/// plain file, and `_is_dir` rejects any symlink outright regardless of its target) and
/// `coverage.protected.names` (keybindings.json, settings.json) - the case-variant "Plugins"
/// symlink is reported `unknown` by `cancellai.py`'s literal-name coverage bucket (a
/// reporting-only quirk this story does not reproduce - see the evidence packet), but *is*
/// still caught by the actual safety-relevant barrier, `protected_component`, which is what
/// this test asserts: "protection must survive case/decomposition variants" (the fixture's own
/// description).
#[test]
fn claude_symlink_protected_name_matches_the_committed_characterization() {
    let fixture = FixtureRoot::new("symlink-protected-name");
    write_claude_markers(&fixture.0);
    let outside = fixture.0.parent().unwrap().join(format!(
        "outside-claude-root-{}",
        fixture.0.file_name().unwrap().to_string_lossy()
    ));
    fs::create_dir_all(&outside).unwrap();
    let outside_file = outside.join("payload.txt");
    fs::write(&outside_file, "synthetic content outside the approved root").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_file, fixture.0.join("Plugins")).unwrap();

    let provider = ClaudeProvider::new(&fixture.0, true);
    assert_eq!(
        provider.fingerprint().markers,
        vec!["keybindings.json", "settings.json"]
    );

    let sessions = provider.discover_sessions();
    assert!(
        matches!(
            sessions.scope,
            cancellai_provider_claude::SessionDiscoveryScope::Unavailable
        ),
        "no projects/ directory exists in this fixture"
    );

    #[cfg(unix)]
    {
        let outcome = provider.protection(&fixture.0.join("Plugins"));
        assert!(
            outcome.is_protected(),
            "a case-variant symlink of a protected name must still be reported protected"
        );
    }

    fs::remove_dir_all(&outside).ok();
}

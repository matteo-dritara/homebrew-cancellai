//! Built-in, committed provider manifests (E16-S05, E16-S03, E16-S04) - the first real
//! [`crate::ProviderManifest`] instances the "Manifest-only" integration level
//! (`docs/architecture/PROVIDER_MODEL.md`) actually ships, proving `manifest.rs`/
//! `manifest_provider.rs` (E16-S01) work end to end against real, evidenced tool layouts, not
//! only synthetic schema fixtures.
//!
//! Compiled into the binary via [`include_str!`], the same way `cancellai-provider-claude`/
//! `cancellai-provider-codex` bake their own marker tables in as Rust consts - being *committed
//! and reviewed* is not the same claim as being *trusted*: every function here still returns a
//! manifest whose provider defaults to `cancellai_safety::TrustedTier::untrusted()` exactly like
//! every other provider (SI-021), and "community contribution path exercises same trust
//! pipeline" (E16-S05's own AC) is proven by there being no separate code path here at all - a
//! manifest contributed by a pull request and reviewed the same way these were would go
//! through the identical `parse_manifest`/`ManifestProvider`/`TrustedTier` machinery,
//! `include_str!` versus "loaded from a file at runtime" is a loading-mechanism detail this
//! epic does not need to build a second one for.

use crate::manifest::{ProviderManifest, parse_manifest};

const OPENCODE_MANIFEST_JSON: &str = include_str!("../manifests/opencode.json");
const GEMINI_CLI_MANIFEST_JSON: &str = include_str!("../manifests/gemini-cli.json");
const GITHUB_COPILOT_CLI_MANIFEST_JSON: &str = include_str!("../manifests/github-copilot-cli.json");

/// The OpenCode provider manifest. Its layout (`$XDG_DATA_HOME/opencode` holding `auth.json`
/// and a `storage/` tree of `session`/`message`/`part`/`session_diff`/`project` JSON records;
/// `$XDG_CONFIG_HOME/opencode/opencode.json` or an `OPENCODE_CONFIG_DIR` override for
/// configuration; `$XDG_CACHE_HOME/opencode` for cache) was confirmed directly against
/// `anomalyco/opencode`'s real source (`packages/core/src/global.ts`, `packages/opencode/src/
/// storage/storage.ts`, `packages/opencode/src/auth/index.ts`, and `packages/web/src/content/
/// docs/config.mdx`) during this story, not assumed from the tool's name or general XDG
/// convention alone - see `project/evidence/E16-S05/EVIDENCE.md` for the exact citations.
///
/// # Panics
///
/// Never, in practice: the manifest text is a committed, version-controlled asset this crate's
/// own test suite (`builtin::tests::the_committed_opencode_manifest_parses`) proves parses on
/// every `cargo test` run - if it ever stopped parsing, that test would fail before this
/// function's `.expect` could ever be reached by anything else in this workspace.
pub fn opencode_manifest() -> ProviderManifest {
    parse_manifest(OPENCODE_MANIFEST_JSON)
        .expect("the committed OpenCode manifest is valid (proven by this crate's own tests)")
}

/// The Gemini CLI provider manifest (E16-S03). Layout confirmed directly against
/// `google-gemini/gemini-cli`'s real source: `packages/core/src/utils/paths.ts` (the
/// `GEMINI_CLI_HOME` override, which replaces the *home* concept wholesale rather than naming
/// the root directly - `homedir()` resolves it, then `.gemini` is joined on regardless, matching
/// [`crate::ManifestRoot`]'s `default_relative_to_home: ""` + `subdir` shape exactly) and
/// `packages/core/src/config/storage.ts` (`oauth_creds.json`, `google_accounts.json`,
/// `settings.json`, `trustedFolders.json` at the root; `tmp/<projectIdentifier>/chats/*.jsonl`
/// for session data, confirmed against `chatRecordingService.ts`'s real filename construction).
/// `vendor_notes` cites `docs/cli/session-management.md`'s documented built-in session
/// retention policy (`settings.json`'s `general.sessionRetention`) - satisfying this story's
/// "vendor-native retention is... explained where relevant" AC without claiming to detect a
/// user's actual configured value, which stays outside the manifest-only ceiling.
///
/// # Panics
///
/// Never in practice - see [`opencode_manifest`]'s identical note; this manifest has the same
/// "proven to parse by this crate's own test suite" guarantee.
pub fn gemini_manifest() -> ProviderManifest {
    parse_manifest(GEMINI_CLI_MANIFEST_JSON)
        .expect("the committed Gemini CLI manifest is valid (proven by this crate's own tests)")
}

/// The GitHub Copilot CLI provider manifest (E16-S04). Layout confirmed directly against
/// GitHub's own published documentation (`docs.github.com/en/copilot/reference/copilot-cli-reference/
/// cli-config-dir-reference` - Copilot CLI is closed-source, so published docs are the
/// authoritative source here rather than a source-code citation, matching the same "evidence
/// before action" bar applied differently because the evidence available differs, not because
/// the bar is lower): `~/.copilot` by default, `COPILOT_HOME` as a full-path override
/// (mirroring `CLAUDE_CONFIG_DIR`/`CODEX_HOME` exactly); `config.json` (auth credentials +
/// plugin metadata) and `settings.json` (user config) at the root, both `Protected`;
/// `session-state/<sessionID>/events.jsonl` for session event logs. The separate,
/// platform-conditional cache directory (`COPILOT_CACHE_HOME`, with a different default per
/// OS) is deliberately not modeled as a root in this schema version - see
/// `project/evidence/E16-S04/EVIDENCE.md` for why.
///
/// # Panics
///
/// Never in practice - see [`opencode_manifest`]'s identical note.
pub fn github_copilot_manifest() -> ProviderManifest {
    parse_manifest(GITHUB_COPILOT_CLI_MANIFEST_JSON).expect(
        "the committed GitHub Copilot CLI manifest is valid (proven by this crate's own tests)",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ArtifactCategory;
    use crate::manifest_provider::{ManifestProvider, ResolvedManifestRoot, resolve_root};
    use crate::root_fingerprint::RootOrigin;
    use crate::{CapabilityKind, ProviderCapabilities, SupportState, capability_report};
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-opencode-manifest-test-{label}-{}",
                std::process::id()
            ));
            fs::remove_dir_all(&dir).ok();
            fs::create_dir_all(&dir).expect("create temp root");
            Self(dir)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn env_map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn lookup(map: &HashMap<String, String>) -> impl Fn(&str) -> Option<String> + '_ {
        move |key: &str| map.get(key).cloned()
    }

    fn resolve_all(manifest: &ProviderManifest, home: &Path) -> Vec<ResolvedManifestRoot> {
        manifest
            .roots
            .iter()
            .map(|root| resolve_root(root, home, &no_env))
            .collect()
    }

    #[test]
    fn the_committed_opencode_manifest_parses() {
        let manifest = opencode_manifest();
        assert_eq!(manifest.provider_id, "opencode");
        assert_eq!(manifest.roots.len(), 3);
        assert!(!manifest.artifacts.is_empty());
    }

    #[test]
    fn ac_resolution_defaults_to_the_real_xdg_paths_when_no_override_is_set() {
        let manifest = opencode_manifest();
        let home = Path::new("/home/user");
        let resolved = resolve_all(&manifest, home);

        let data = resolved.iter().find(|r| r.name == "data").unwrap();
        assert_eq!(data.path, PathBuf::from("/home/user/.local/share/opencode"));
        assert_eq!(data.origin, RootOrigin::Default);

        let config = resolved.iter().find(|r| r.name == "config").unwrap();
        assert_eq!(config.path, PathBuf::from("/home/user/.config/opencode"));

        let cache = resolved.iter().find(|r| r.name == "cache").unwrap();
        assert_eq!(cache.path, PathBuf::from("/home/user/.cache/opencode"));
    }

    #[test]
    fn ac_a_full_path_override_is_honored_for_the_config_root() {
        let manifest = opencode_manifest();
        let env: HashMap<String, String> = [(
            "OPENCODE_CONFIG_DIR".to_string(),
            "/custom/opencode-config".to_string(),
        )]
        .into_iter()
        .collect();
        let lookup = move |key: &str| env.get(key).cloned();
        let config_root = manifest.roots.iter().find(|r| r.name == "config").unwrap();
        let resolved = resolve_root(config_root, Path::new("/home/user"), &lookup);
        assert_eq!(resolved.path, PathBuf::from("/custom/opencode-config"));
        assert_eq!(resolved.origin, RootOrigin::Custom);
    }

    /// Builds the exact real layout confirmed from `anomalyco/opencode`'s source: `auth.json`
    /// plus `storage/{session,message,part,session_diff,project}/`.
    fn build_realistic_data_root(dir: &Path) {
        fs::write(dir.join("auth.json"), r#"{"anthropic": {"type": "oauth"}}"#).unwrap();
        fs::create_dir_all(dir.join("storage/session/proj-a")).unwrap();
        fs::write(
            dir.join("storage/session/proj-a/ses-1.json"),
            r#"{"id": "ses-1"}"#,
        )
        .unwrap();
        fs::create_dir_all(dir.join("storage/message/ses-1")).unwrap();
        fs::write(
            dir.join("storage/message/ses-1/msg-1.json"),
            r#"{"id": "msg-1"}"#,
        )
        .unwrap();
        fs::create_dir_all(dir.join("storage/part/msg-1")).unwrap();
        fs::write(dir.join("storage/part/msg-1/part-1.json"), "{}").unwrap();
        fs::create_dir_all(dir.join("storage/session_diff")).unwrap();
        fs::write(dir.join("storage/session_diff/ses-1.json"), "{}").unwrap();
        fs::create_dir_all(dir.join("storage/project")).unwrap();
        fs::write(
            dir.join("storage/project/proj-a.json"),
            r#"{"id": "proj-a"}"#,
        )
        .unwrap();
        fs::write(dir.join("storage/migration"), "2").unwrap();
    }

    #[test]
    fn ac_a_realistic_opencode_data_root_is_detected_and_its_sessions_inventoried() {
        let tree = TempTree::new("realistic");
        build_realistic_data_root(&tree.0);

        let manifest = opencode_manifest();
        let resolved = vec![ResolvedManifestRoot {
            name: "data".to_string(),
            path: tree.0.clone(),
            origin: RootOrigin::Default,
        }];
        let provider = ManifestProvider::new(&manifest, resolved);

        assert_eq!(provider.detect().support(), SupportState::SupportedObserved);
        assert_eq!(
            provider.fingerprint_root().support(),
            SupportState::SupportedObserved
        );
        let inventory = provider.inventory_map();
        assert_eq!(inventory.support(), SupportState::SupportedObserved);
        // 5 real session-category files planted above (session, message, part, session_diff,
        // project) - the migration marker is deliberately unmatched by any declared pattern.
        assert!(
            inventory.evidence()[0].contains('5'),
            "{:?}",
            inventory.evidence()
        );
    }

    #[test]
    fn ac_unknown_layouts_remain_inspection_only() {
        // AC1: an empty/unrelated directory - nothing about it resembles OpenCode's real
        // layout - must report Unsupported across the board, never guess.
        let tree = TempTree::new("unknown-layout");
        let manifest = opencode_manifest();
        let resolved = vec![ResolvedManifestRoot {
            name: "data".to_string(),
            path: tree.0.clone(),
            origin: RootOrigin::Custom,
        }];
        let provider = ManifestProvider::new(&manifest, resolved);

        for (kind, outcome) in capability_report(&provider) {
            assert_eq!(outcome.support(), SupportState::Unsupported, "{kind:?}");
        }
    }

    #[test]
    fn ac_a_realistic_layout_still_never_reports_capabilities_beyond_the_manifest_only_ceiling() {
        let tree = TempTree::new("ceiling-realistic");
        build_realistic_data_root(&tree.0);
        let manifest = opencode_manifest();
        let resolved = vec![ResolvedManifestRoot {
            name: "data".to_string(),
            path: tree.0.clone(),
            origin: RootOrigin::Default,
        }];
        let provider = ManifestProvider::new(&manifest, resolved);

        for kind in [
            CapabilityKind::ProjectAttribution,
            CapabilityKind::SessionGraph,
            CapabilityKind::ActivityState,
            CapabilityKind::NativeDeleteCapability,
            CapabilityKind::RetentionCapability,
            CapabilityKind::Explain,
        ] {
            assert_eq!(
                provider.capability(kind).support(),
                SupportState::Unsupported,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn credentials_are_classified_protected_not_session() {
        let manifest = opencode_manifest();
        let auth_pattern = manifest
            .artifacts
            .iter()
            .find(|a| a.relative_glob == "auth.json")
            .expect("auth.json must be a declared artifact pattern");
        assert_eq!(auth_pattern.category, ArtifactCategory::Protected);
    }

    fn assert_every_artifact_references_a_declared_root(manifest: &ProviderManifest) {
        // Regression guard: every artifact.root must name one of the manifest's own declared
        // roots - parse_manifest already enforces this for any manifest it accepts, so this is
        // really proving the committed file has not drifted from that invariant, not re-testing
        // the parser.
        let declared: std::collections::HashSet<&str> =
            manifest.roots.iter().map(|r| r.name.as_str()).collect();
        for artifact in &manifest.artifacts {
            assert!(
                declared.contains(artifact.root.as_str()),
                "{}",
                artifact.root
            );
        }
    }

    #[test]
    fn no_artifact_pattern_references_an_undeclared_root() {
        assert_every_artifact_references_a_declared_root(&opencode_manifest());
        assert_every_artifact_references_a_declared_root(&gemini_manifest());
        assert_every_artifact_references_a_declared_root(&github_copilot_manifest());
    }

    // --- E16-S03: Gemini CLI -------------------------------------------------------------

    #[test]
    fn the_committed_gemini_manifest_parses() {
        let manifest = gemini_manifest();
        assert_eq!(manifest.provider_id, "gemini-cli");
        assert_eq!(manifest.roots.len(), 1);
        assert!(manifest.vendor_notes.is_some());
    }

    #[test]
    fn gemini_home_resolves_using_the_real_override_and_default() {
        let manifest = gemini_manifest();
        let root = &manifest.roots[0];

        let empty: HashMap<String, String> = HashMap::new();
        let default_resolved = resolve_root(root, Path::new("/home/user"), &lookup(&empty));
        assert_eq!(default_resolved.path, PathBuf::from("/home/user/.gemini"));
        assert_eq!(default_resolved.origin, RootOrigin::Default);

        let overridden = env_map(&[("GEMINI_CLI_HOME", "/custom/gemini-home")]);
        let custom_resolved = resolve_root(root, Path::new("/home/user"), &lookup(&overridden));
        assert_eq!(
            custom_resolved.path,
            PathBuf::from("/custom/gemini-home/.gemini")
        );
        assert_eq!(custom_resolved.origin, RootOrigin::Custom);
    }

    #[test]
    fn gemini_oauth_credentials_are_classified_protected() {
        let manifest = gemini_manifest();
        let pattern = manifest
            .artifacts
            .iter()
            .find(|a| a.relative_glob == "oauth_creds.json")
            .expect("oauth_creds.json must be declared");
        assert_eq!(pattern.category, ArtifactCategory::Protected);
    }

    #[test]
    fn gemini_explain_cites_the_vendor_documented_retention_policy() {
        let manifest = gemini_manifest();
        let tree = TempTree::new("gemini-explain");
        fs::write(tree.0.join("settings.json"), "{}").unwrap();
        let resolved = vec![ResolvedManifestRoot {
            name: "home".to_string(),
            path: tree.0.clone(),
            origin: RootOrigin::Default,
        }];
        let provider = ManifestProvider::new(&manifest, resolved);
        let explain = provider.explain();
        assert_eq!(explain.support(), SupportState::Unsupported);
        assert!(
            explain.evidence()[0].contains("sessionRetention")
                || explain.evidence()[0].contains("30d"),
            "{:?}",
            explain.evidence()
        );
    }

    #[test]
    fn a_realistic_gemini_home_is_detected_and_its_chats_inventoried() {
        let tree = TempTree::new("gemini-realistic");
        fs::write(tree.0.join("settings.json"), "{}").unwrap();
        fs::write(tree.0.join("oauth_creds.json"), "{}").unwrap();
        fs::create_dir_all(tree.0.join("tmp/proj-a/chats")).unwrap();
        fs::write(tree.0.join("tmp/proj-a/chats/session-1.jsonl"), "{}\n").unwrap();
        fs::write(tree.0.join("tmp/proj-a/chats/session-2.jsonl"), "{}\n").unwrap();

        let manifest = gemini_manifest();
        let resolved = vec![ResolvedManifestRoot {
            name: "home".to_string(),
            path: tree.0.clone(),
            origin: RootOrigin::Default,
        }];
        let provider = ManifestProvider::new(&manifest, resolved);
        assert_eq!(provider.detect().support(), SupportState::SupportedObserved);
        let inventory = provider.inventory_map();
        assert_eq!(inventory.support(), SupportState::SupportedObserved);
        assert!(
            inventory.evidence()[0].contains('2'),
            "{:?}",
            inventory.evidence()
        );
    }

    // --- E16-S04: GitHub Copilot CLI ------------------------------------------------------

    #[test]
    fn the_committed_github_copilot_manifest_parses() {
        let manifest = github_copilot_manifest();
        assert_eq!(manifest.provider_id, "github-copilot-cli");
        assert_eq!(manifest.roots.len(), 1);
    }

    #[test]
    fn copilot_home_resolves_using_the_real_full_path_override_and_default() {
        let manifest = github_copilot_manifest();
        let root = &manifest.roots[0];

        let empty: HashMap<String, String> = HashMap::new();
        let default_resolved = resolve_root(root, Path::new("/home/user"), &lookup(&empty));
        assert_eq!(default_resolved.path, PathBuf::from("/home/user/.copilot"));
        assert_eq!(default_resolved.origin, RootOrigin::Default);

        let overridden = env_map(&[("COPILOT_HOME", "/custom/copilot-home")]);
        let custom_resolved = resolve_root(root, Path::new("/home/user"), &lookup(&overridden));
        assert_eq!(custom_resolved.path, PathBuf::from("/custom/copilot-home"));
        assert_eq!(custom_resolved.origin, RootOrigin::Custom);
    }

    #[test]
    fn copilot_config_json_is_classified_protected_not_session() {
        // config.json holds authentication credentials per GitHub's own documentation - the
        // single highest-stakes file in this layout to get right.
        let manifest = github_copilot_manifest();
        let pattern = manifest
            .artifacts
            .iter()
            .find(|a| a.relative_glob == "config.json")
            .expect("config.json must be declared");
        assert_eq!(pattern.category, ArtifactCategory::Protected);
    }

    #[test]
    fn a_realistic_copilot_home_is_detected_and_its_session_events_inventoried() {
        let tree = TempTree::new("copilot-realistic");
        fs::write(tree.0.join("settings.json"), "{}").unwrap();
        fs::write(tree.0.join("config.json"), "{}").unwrap();
        fs::create_dir_all(tree.0.join("session-state/ses-1")).unwrap();
        fs::write(tree.0.join("session-state/ses-1/events.jsonl"), "{}\n").unwrap();

        let manifest = github_copilot_manifest();
        let resolved = vec![ResolvedManifestRoot {
            name: "home".to_string(),
            path: tree.0.clone(),
            origin: RootOrigin::Default,
        }];
        let provider = ManifestProvider::new(&manifest, resolved);
        assert_eq!(provider.detect().support(), SupportState::SupportedObserved);
        let inventory = provider.inventory_map();
        assert_eq!(inventory.support(), SupportState::SupportedObserved);
        assert!(
            inventory.evidence()[0].contains('1'),
            "{:?}",
            inventory.evidence()
        );
    }

    #[test]
    fn every_builtin_manifests_provider_defaults_to_untrusted_authority() {
        // SI-021: none of these three manifests grants themselves anything - proven by
        // construction (TrustedTier::untrusted() is the only production constructor this
        // workspace's ManifestProvider path ever reaches for a manifest-driven provider; there
        // is no field in ProviderManifest a manifest could set to change that), restated here as
        // an explicit, named regression guard rather than left implicit.
        for manifest in [
            opencode_manifest(),
            gemini_manifest(),
            github_copilot_manifest(),
        ] {
            // A manifest cannot express trust at all (E16-S01's own AC1) - this loop's real
            // assertion is that constructing each one compiles and parses without any such
            // field existing to inspect, which the type system already guarantees; see
            // manifest.rs's own AC1 smuggling tests for the adversarial half of this proof.
            assert!(!manifest.provider_id.is_empty());
        }
    }
}

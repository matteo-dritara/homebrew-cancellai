//! Built-in, committed provider manifests (E16-S05) - the first real
//! [`crate::ProviderManifest`] instances the "Manifest-only" integration level
//! (`docs/architecture/PROVIDER_MODEL.md`) actually ships, proving `manifest.rs`/
//! `manifest_provider.rs` (E16-S01) work end to end against a real, evidenced tool layout, not
//! only synthetic schema fixtures.
//!
//! Compiled into the binary via [`include_str!`], the same way `cancellai-provider-claude`/
//! `cancellai-provider-codex` bake their own marker tables in as Rust consts - being *committed
//! and reviewed* is not the same claim as being *trusted*: [`opencode_manifest`] still returns a
//! manifest whose provider defaults to `cancellai_safety::TrustedTier::untrusted()` exactly like
//! every other provider (SI-021), and "community contribution path exercises same trust
//! pipeline" (E16-S05's own AC) is proven by there being no separate code path here at all - a
//! manifest contributed by a pull request and reviewed the same way this one was would go
//! through the identical `parse_manifest`/`ManifestProvider`/`TrustedTier` machinery,
//! `include_str!` versus "loaded from a file at runtime" is a loading-mechanism detail this
//! story does not need to build a second one for.

use crate::manifest::{ProviderManifest, parse_manifest};

const OPENCODE_MANIFEST_JSON: &str = include_str!("../manifests/opencode.json");

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

    #[test]
    fn no_artifact_pattern_references_an_undeclared_root() {
        // Regression guard: every artifact.root must name one of the three declared roots -
        // parse_manifest already enforces this for any manifest it accepts, so this is really
        // proving the committed file has not drifted from that invariant, not re-testing the
        // parser.
        let manifest = opencode_manifest();
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
}

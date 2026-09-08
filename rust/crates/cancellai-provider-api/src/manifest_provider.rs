//! Turns a parsed [`crate::ProviderManifest`] into real filesystem observations: root
//! resolution, fingerprinting, and a [`crate::ProviderCapabilities`] implementation (E16-S01,
//! E16-S05).
//!
//! [`ManifestProvider`] only ever answers `DISCOVERY`/`FINGERPRINT_ROOT`/`INVENTORY_MAP` from
//! real evidence - every other [`crate::CapabilityKind`] is unconditionally `Unsupported`,
//! citing PROVIDER_MODEL.md's "Manifest-only" integration level by name in its evidence string
//! (`SESSION_GRAPH`/`PROJECT_ATTRIBUTION`/`ACTIVITY_DETECTION`/`NATIVE_DELETE`/
//! `RETENTION_CONFIG` all require real adapter code this crate does not have and a manifest
//! cannot supply - see `manifest.rs`'s module doc for why the schema has no field that could
//! claim otherwise). This is the same "the ceiling is structural, not merely reported" pattern
//! the schema itself uses, applied to the capability layer this schema feeds.

use std::path::{Path, PathBuf};

use crate::capability::{CapabilityKind, CapabilityOutcome, ProviderCapabilities, SupportState};
use crate::manifest::{ArtifactCategory, ManifestRoot, MarkerProbe, ProviderManifest};
use crate::root_fingerprint::{RootConfidence, RootOrigin, derive_root_confidence};
use crate::root_probe::{is_dir, is_json_object, is_jsonl_of_objects, is_nonempty_file};
use cancellai_model::KnowledgeConfidence;

/// Like `cancellai_provider_api::RootFingerprint`, but owning its marker names as [`String`]
/// rather than borrowing `&'static str` - that type's markers come from compile-time adapter
/// const tables (`cancellai-provider-claude`/`cancellai-provider-codex`'s own `fingerprint.rs`),
/// while a manifest's markers are parsed at runtime from JSON and have no `'static` borrow to
/// offer. Carries the identical two other fields (`origin`, `confidence`) and is derived by the
/// identical [`derive_root_confidence`] rule, so the two types agree on what "High"/"Low"/
/// "Unknown" mean even though neither can literally be the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestRootFingerprint {
    pub origin: RootOrigin,
    pub confidence: RootConfidence,
    pub markers: Vec<String>,
}

/// A [`ManifestRoot`] resolved against a real (or synthetic-for-tests) `$HOME` and environment -
/// mirrors `cancellai-cli::roots::ResolvedRoot` (path + whether this is the default location),
/// built independently here because `cancellai-provider-api` sits below `cancellai-cli` in the
/// dependency graph and cannot depend on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedManifestRoot {
    pub name: String,
    pub path: PathBuf,
    pub origin: RootOrigin,
}

/// Resolves one [`ManifestRoot`] using `home` and an environment lookup (`env`, deliberately a
/// closure rather than reading `std::env::var` directly - mirrors `cancellai-cli::roots`'s own
/// "isolated from reading real environment variables" testability rationale: every branch below
/// is exercisable with a synthetic map, no process-wide global state, no `unsafe` `set_var`).
///
/// - `env_var` set and `env_var_is_full_path`: the variable's value *is* the root.
/// - `env_var` set and not full-path: the variable names a *base* directory; `subdir` (if any)
///   is joined onto it - the XDG-base-directory shape (`root.manifest.rs`'s own docs).
/// - `env_var` unset (or not present in `env`): `$HOME/default_relative_to_home[/subdir]`.
///
/// A root is [`RootOrigin::Default`] only when no override applied - matching every other
/// adapter's rule that override authority and default authority are mutually exclusive facts
/// about *this* resolution, not a property of the path string alone.
pub fn resolve_root(
    root: &ManifestRoot,
    home: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> ResolvedManifestRoot {
    let override_value = root.env_var.as_deref().and_then(env);

    let (base, is_override) = match override_value {
        Some(value) => (PathBuf::from(value), true),
        None => (home.join(&root.default_relative_to_home), false),
    };

    let path = if is_override && root.env_var_is_full_path {
        base
    } else if let Some(subdir) = &root.subdir {
        base.join(subdir)
    } else {
        base
    };

    ResolvedManifestRoot {
        name: root.name.clone(),
        path,
        origin: if is_override {
            RootOrigin::Custom
        } else {
            RootOrigin::Default
        },
    }
}

fn run_probe(probe: MarkerProbe, path: &Path) -> bool {
    match probe {
        MarkerProbe::Dir => is_dir(path),
        MarkerProbe::JsonObject => is_json_object(path),
        MarkerProbe::JsonlOfObjects => is_jsonl_of_objects(path),
        MarkerProbe::NonemptyFile => is_nonempty_file(path),
    }
}

/// Fingerprints `resolved` against `root`'s own marker table, reusing
/// [`crate::derive_root_confidence`] - the identical rule every hand-written adapter's own
/// `fingerprint_<tool>_root` uses (`cancellai-provider-claude`/`cancellai-provider-codex`), so a
/// manifest-driven provider's confidence is not a second, drifting notion of "how sure are we."
pub fn fingerprint_manifest_root(
    root: &ManifestRoot,
    resolved: &ResolvedManifestRoot,
) -> ManifestRootFingerprint {
    let mut found: Vec<String> = Vec::new();
    let mut identifying = 0usize;
    for marker in &root.markers {
        if run_probe(marker.probe, &resolved.path.join(&marker.relative_path)) {
            found.push(marker.relative_path.clone());
            if marker.identifying {
                identifying += 1;
            }
        }
    }
    found.sort_unstable();

    let is_default = resolved.origin == RootOrigin::Default;
    ManifestRootFingerprint {
        origin: resolved.origin,
        confidence: derive_root_confidence(is_default, identifying, found.len()),
        markers: found,
    }
}

/// Whether one path segment matches one pattern segment, where `*` within the pattern segment
/// matches zero or more characters (ordinary shell-glob-within-a-segment semantics: `*.json`
/// matches `one.json`, `session-*` matches `session-42`) but never crosses a `/` boundary - `*`
/// cannot match into a different path segment, only within the one it appears in.
fn segment_matches(pattern: &str, actual: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == actual;
    }
    let mut pos = 0usize;
    if !parts[0].is_empty() {
        if !actual.starts_with(parts[0]) {
            return false;
        }
        pos = parts[0].len();
    }
    let last = parts[parts.len() - 1];
    if !last.is_empty() && (actual.len() < pos || !actual[pos..].ends_with(last)) {
        return false;
    }
    let end = actual.len().saturating_sub(last.len());
    if end < pos {
        return false;
    }
    let mut cursor = pos;
    for part in &parts[1..parts.len() - 1] {
        if part.is_empty() {
            continue;
        }
        match actual[cursor..end].find(part) {
            Some(idx) => cursor += idx + part.len(),
            None => return false,
        }
    }
    true
}

/// Whether `relative`'s path segments match `pattern`'s: the same number of `/`-separated
/// segments, each matching per [`segment_matches`] - the bounded, non-recursive glob
/// `manifest.rs`'s own docs describe (not a general glob engine). `**` is deliberately not
/// supported: a pattern this crate cannot express is a pattern a manifest author cannot use to
/// match more than they enumerated, matching the same "the ceiling is what the schema can
/// express, not what a clever pattern can reach" posture the rest of this module keeps.
fn matches_pattern(relative: &Path, pattern: &str) -> bool {
    let pattern_segments: Vec<&str> = pattern.split('/').collect();
    let relative_segments: Vec<_> = relative.iter().map(|s| s.to_string_lossy()).collect();
    if pattern_segments.len() != relative_segments.len() {
        return false;
    }
    pattern_segments
        .iter()
        .zip(relative_segments.iter())
        .all(|(pat, seg)| segment_matches(pat, seg.as_ref()))
}

/// Bounded walk of `root_path` (mirrors `root_probe::contains_uuid_named_jsonl`'s own bound and
/// no-symlink-descent rule), returning every file whose root-relative path matches
/// `relative_glob`.
fn find_matching_files(root_path: &Path, relative_glob: &str) -> Vec<PathBuf> {
    const MAX_ENTRIES: usize = 20_000;
    let mut matches = Vec::new();
    let mut seen = 0usize;
    let mut stack = vec![root_path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if std::fs::symlink_metadata(&path)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(true)
            {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            seen += 1;
            if seen > MAX_ENTRIES {
                return matches;
            }
            if let Ok(relative) = path.strip_prefix(root_path) {
                if matches_pattern(relative, relative_glob) {
                    matches.push(path);
                }
            }
        }
    }
    matches
}

/// A [`crate::ProviderCapabilities`] implementation driven entirely by a
/// [`crate::ProviderManifest`] and a set of already-resolved roots - no per-provider Rust code.
pub struct ManifestProvider<'a> {
    manifest: &'a ProviderManifest,
    resolved: Vec<ResolvedManifestRoot>,
}

impl<'a> ManifestProvider<'a> {
    /// `resolved` should be [`resolve_root`]'s output for every root `manifest` declares, in any
    /// order - callers that omit a root simply get `Unsupported`/no matches for patterns
    /// referencing it, never a panic (matching this crate's "absence is first-class" contract).
    pub fn new(manifest: &'a ProviderManifest, resolved: Vec<ResolvedManifestRoot>) -> Self {
        Self { manifest, resolved }
    }

    fn resolved_root(&self, name: &str) -> Option<&ResolvedManifestRoot> {
        self.resolved.iter().find(|r| r.name == name)
    }

    fn declared_root(&self, name: &str) -> Option<&ManifestRoot> {
        self.manifest.roots.iter().find(|r| r.name == name)
    }

    fn best_fingerprint(&self) -> Option<(String, ManifestRootFingerprint)> {
        self.manifest
            .roots
            .iter()
            .filter_map(|root| {
                let resolved = self.resolved_root(&root.name)?;
                Some((root.name.clone(), fingerprint_manifest_root(root, resolved)))
            })
            .max_by_key(|(_, fp)| confidence_rank(fp.confidence))
    }
}

fn confidence_rank(confidence: RootConfidence) -> u8 {
    match confidence {
        RootConfidence::Unknown => 0,
        RootConfidence::Low => 1,
        RootConfidence::High => 2,
        RootConfidence::Default => 3,
    }
}

/// Manifest-only ceiling: `Unknown` confidence never claims more than `LowUnknown`
/// [`KnowledgeConfidence`], matching SI-004 ("unknown provider layout/version reduces
/// capability") - the same floor a hand-written adapter's `RootConfidence::Unknown` maps to.
fn support_and_confidence_for(
    fingerprint_confidence: RootConfidence,
) -> (SupportState, KnowledgeConfidence) {
    match fingerprint_confidence {
        RootConfidence::Default | RootConfidence::High => (
            SupportState::SupportedObserved,
            KnowledgeConfidence::Observed,
        ),
        RootConfidence::Low => (
            SupportState::SupportedObserved,
            KnowledgeConfidence::Inferred,
        ),
        RootConfidence::Unknown => (SupportState::Unsupported, KnowledgeConfidence::LowUnknown),
    }
}

const MANIFEST_ONLY_UNSUPPORTED: &str = "manifest-only integration: this capability requires native adapter code, which no manifest can supply (docs/architecture/PROVIDER_MODEL.md \"Manifest-only\")";

impl ProviderCapabilities for ManifestProvider<'_> {
    fn provider_id(&self) -> &str {
        &self.manifest.provider_id
    }

    fn capability(&self, kind: CapabilityKind) -> CapabilityOutcome {
        match kind {
            CapabilityKind::Detect | CapabilityKind::FingerprintRoot => match self
                .best_fingerprint()
            {
                Some((root_name, fingerprint)) => {
                    let (support, confidence) = support_and_confidence_for(fingerprint.confidence);
                    CapabilityOutcome::new(
                        support,
                        confidence,
                        format!(
                            "root {root_name:?}: {:?} confidence, markers: [{}]",
                            fingerprint.confidence,
                            fingerprint.markers.join(", ")
                        ),
                        Vec::new(),
                        None,
                    )
                }
                None => CapabilityOutcome::new(
                    SupportState::Unsupported,
                    KnowledgeConfidence::LowUnknown,
                    "no root could be resolved for this manifest",
                    Vec::new(),
                    None,
                ),
            },
            CapabilityKind::InventoryMap => {
                let mut total_matches = 0usize;
                let mut any_root_confident = false;
                for artifact in &self.manifest.artifacts {
                    if artifact.category != ArtifactCategory::Session {
                        continue;
                    }
                    let Some(declared) = self.declared_root(&artifact.root) else {
                        continue;
                    };
                    let Some(resolved) = self.resolved_root(&artifact.root) else {
                        continue;
                    };
                    let fingerprint = fingerprint_manifest_root(declared, resolved);
                    if fingerprint.confidence == RootConfidence::Unknown {
                        continue;
                    }
                    any_root_confident = true;
                    total_matches +=
                        find_matching_files(&resolved.path, &artifact.relative_glob).len();
                }
                if !any_root_confident {
                    CapabilityOutcome::new(
                        SupportState::Unsupported,
                        KnowledgeConfidence::LowUnknown,
                        "no root reached at least Low confidence; inventory mapping stays inspection-only for an unrecognized layout",
                        Vec::new(),
                        None,
                    )
                } else {
                    CapabilityOutcome::new(
                        SupportState::SupportedObserved,
                        KnowledgeConfidence::Observed,
                        format!(
                            "{total_matches} session artifact(s) matched against declared patterns"
                        ),
                        Vec::new(),
                        None,
                    )
                }
            }
            CapabilityKind::ProjectAttribution
            | CapabilityKind::SessionGraph
            | CapabilityKind::ActivityState
            | CapabilityKind::NativeDeleteCapability
            | CapabilityKind::RetentionCapability => CapabilityOutcome::new(
                SupportState::Unsupported,
                KnowledgeConfidence::LowUnknown,
                MANIFEST_ONLY_UNSUPPORTED,
                Vec::new(),
                None,
            ),
            // E16-S03: still unconditionally Unsupported (the manifest-only ceiling is
            // unaffected), but the evidence cites the manifest's own `vendor_notes` when
            // present - "vendor-native retention is detected/explained where relevant" is
            // satisfied by the "explained" half: this crate cites a vendor's *documented*
            // behavior, it never claims to have detected the tool's actual configured value.
            CapabilityKind::Explain => match &self.manifest.vendor_notes {
                Some(notes) => CapabilityOutcome::new(
                    SupportState::Unsupported,
                    KnowledgeConfidence::LowUnknown,
                    format!(
                        "{MANIFEST_ONLY_UNSUPPORTED}; vendor-documented behavior (not detected or configured by this adapter): {notes}"
                    ),
                    Vec::new(),
                    None,
                ),
                None => CapabilityOutcome::new(
                    SupportState::Unsupported,
                    KnowledgeConfidence::LowUnknown,
                    MANIFEST_ONLY_UNSUPPORTED,
                    Vec::new(),
                    None,
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::parse_manifest;
    use std::collections::HashMap;
    use std::fs;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-manifest-provider-test-{label}-{}",
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

    fn env_map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn lookup(map: &HashMap<String, String>) -> impl Fn(&str) -> Option<String> + '_ {
        move |key: &str| map.get(key).cloned()
    }

    fn example_manifest() -> ProviderManifest {
        parse_manifest(
            r#"{
                "schema_version": 1,
                "provider_id": "example-tool",
                "display_name": "Example Tool",
                "roots": [
                    {
                        "name": "data",
                        "env_var": "EXAMPLE_TOOL_HOME",
                        "env_var_is_full_path": true,
                        "subdir": null,
                        "default_relative_to_home": ".example-tool",
                        "markers": [
                            {"relative_path": "config.json", "probe": "json_object", "identifying": true},
                            {"relative_path": "sessions", "probe": "dir", "identifying": true}
                        ]
                    }
                ],
                "artifacts": [
                    {"root": "data", "category": "session", "relative_glob": "sessions/*.json"},
                    {"root": "data", "category": "protected", "relative_glob": "auth.json"}
                ]
            }"#,
        )
        .expect("valid fixture manifest")
    }

    #[test]
    fn resolve_root_uses_the_full_path_override_when_set() {
        let manifest = example_manifest();
        let root = &manifest.roots[0];
        let env = env_map(&[("EXAMPLE_TOOL_HOME", "/custom/location")]);
        let resolved = resolve_root(root, Path::new("/home/user"), &lookup(&env));
        assert_eq!(resolved.path, PathBuf::from("/custom/location"));
        assert_eq!(resolved.origin, RootOrigin::Custom);
    }

    #[test]
    fn resolve_root_falls_back_to_the_home_relative_default_when_unset() {
        let manifest = example_manifest();
        let root = &manifest.roots[0];
        let env: HashMap<String, String> = HashMap::new();
        let resolved = resolve_root(root, Path::new("/home/user"), &lookup(&env));
        assert_eq!(resolved.path, PathBuf::from("/home/user/.example-tool"));
        assert_eq!(resolved.origin, RootOrigin::Default);
    }

    #[test]
    fn resolve_root_joins_subdir_onto_a_base_directory_env_var() {
        let text = r#"{
            "schema_version": 1, "provider_id": "x", "display_name": "X",
            "roots": [{"name": "data", "env_var": "XDG_DATA_HOME", "env_var_is_full_path": false, "subdir": "example-tool", "default_relative_to_home": ".local/share", "markers": []}],
            "artifacts": []
        }"#;
        let manifest = parse_manifest(text).unwrap();
        let env = env_map(&[("XDG_DATA_HOME", "/xdg/data")]);
        let resolved = resolve_root(&manifest.roots[0], Path::new("/home/user"), &lookup(&env));
        assert_eq!(resolved.path, PathBuf::from("/xdg/data/example-tool"));
        assert_eq!(resolved.origin, RootOrigin::Custom);

        let empty: HashMap<String, String> = HashMap::new();
        let default_resolved =
            resolve_root(&manifest.roots[0], Path::new("/home/user"), &lookup(&empty));
        assert_eq!(
            default_resolved.path,
            PathBuf::from("/home/user/.local/share/example-tool")
        );
        assert_eq!(default_resolved.origin, RootOrigin::Default);
    }

    #[test]
    fn ac1_an_unrecognized_layout_reports_unsupported_and_stays_inspection_only() {
        let tree = TempTree::new("unknown-layout");
        let manifest = example_manifest();
        let resolved = vec![ResolvedManifestRoot {
            name: "data".to_string(),
            path: tree.0.clone(),
            origin: RootOrigin::Custom,
        }];
        let provider = ManifestProvider::new(&manifest, resolved);

        assert_eq!(provider.detect().support(), SupportState::Unsupported);
        assert_eq!(
            provider.fingerprint_root().support(),
            SupportState::Unsupported
        );
        assert_eq!(
            provider.inventory_map().support(),
            SupportState::Unsupported
        );
    }

    #[test]
    fn a_recognized_layout_reports_supported_observed_and_lists_session_artifacts() {
        let tree = TempTree::new("recognized-layout");
        fs::write(tree.0.join("config.json"), "{}").unwrap();
        fs::create_dir_all(tree.0.join("sessions")).unwrap();
        fs::write(tree.0.join("sessions/one.json"), "{}").unwrap();
        fs::write(tree.0.join("sessions/two.json"), "{}").unwrap();
        fs::write(tree.0.join("auth.json"), "{}").unwrap();

        let manifest = example_manifest();
        let resolved = vec![ResolvedManifestRoot {
            name: "data".to_string(),
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

    #[test]
    fn every_capability_beyond_the_manifest_only_three_is_unconditionally_unsupported() {
        let tree = TempTree::new("ceiling");
        fs::write(tree.0.join("config.json"), "{}").unwrap();
        let manifest = example_manifest();
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
            let outcome = provider.capability(kind);
            assert_eq!(outcome.support(), SupportState::Unsupported, "{kind:?}");
            assert!(
                outcome.evidence()[0].contains("manifest-only"),
                "{kind:?}: {:?}",
                outcome.evidence()
            );
        }
    }

    #[test]
    fn matches_pattern_requires_the_same_segment_count() {
        // A wildcard never crosses a '/' boundary - one extra path level is a non-match, not a
        // greedier match.
        assert!(!matches_pattern(
            Path::new("storage/session/proj-a/nested/s1.json"),
            "storage/session/*/*.json"
        ));
    }

    #[test]
    fn matches_pattern_matches_a_within_segment_wildcard() {
        assert!(matches_pattern(
            Path::new("sessions/one.json"),
            "sessions/*.json"
        ));
        assert!(matches_pattern(
            Path::new("sessions/one.json"),
            "sessions/*"
        ));
        assert!(matches_pattern(
            Path::new("storage/session/proj-a/s1.json"),
            "storage/session/*/*.json"
        ));
        assert!(!matches_pattern(
            Path::new("sessions/one.txt"),
            "sessions/*.json"
        ));
    }

    #[test]
    fn find_matching_files_does_not_descend_into_a_symlinked_directory() {
        let tree = TempTree::new("symlink-inventory");
        let outside = tree.0.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.json"), "{}").unwrap();
        fs::create_dir_all(tree.0.join("root/sessions")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, tree.0.join("root/sessions/link")).unwrap();

        #[cfg(unix)]
        {
            let matches = find_matching_files(&tree.0.join("root"), "sessions/*/secret.json");
            assert!(matches.is_empty());
        }
    }
}

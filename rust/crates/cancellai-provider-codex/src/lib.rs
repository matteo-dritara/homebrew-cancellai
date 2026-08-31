//! Codex CLI provider adapter: implements `cancellai_provider_api`'s capability contract
//! for Codex's on-disk layout, including the subagent/rollout graph
//! (`docs/architecture/AS_IS.md` - Codex subagent graph). Provider-specific knowledge lives
//! here, not in `cancellai-model`/`cancellai-safety` (`docs/architecture/TARGET.md`).
//!
//! E05-S04 adds the first real logic: root fingerprinting ([`fingerprint`]), the protected-name
//! barrier ([`protected_names`]), rollout discovery with parent-lineage parsing ([`session`]),
//! subagent-tree grouping ([`graph`]), native-delete detection ([`native_delete`]), and
//! [`CodexProvider`], the [`ProviderCapabilities`] implementation tying them together. See
//! `cancellai-provider-claude` for the Claude counterpart this crate deliberately does not
//! share session/graph logic with - Codex's parent/child rollout graph has no Claude analogue,
//! and Claude's flat project grouping has no Codex analogue.

mod fingerprint;
mod graph;
mod native_delete;
mod protected_names;
mod session;

use std::path::{Path, PathBuf};

use cancellai_model::{AuthorityLevel, KnowledgeConfidence};
use cancellai_provider_api::{
    CapabilityKind, CapabilityOutcome, ProtectionOutcome, ProviderCapabilities, RootConfidence,
    RootFingerprint, SupportState,
};

pub use fingerprint::fingerprint_codex_root;
pub use graph::{SubagentTree, group_into_subagent_trees};
pub use native_delete::{NativeDeleteSupport, codex_delete_supported};
pub use protected_names::{CODEX_PROTECTED_NAMES, codex_protected_component};
pub use session::{CodexSession, RolloutCategory, discover_codex_sessions};

/// A Codex CLI provider adapter bound to one candidate root.
pub struct CodexProvider {
    root: PathBuf,
    is_default_root: bool,
    /// An explicit `codex` binary path for native-delete probing, overriding this process's
    /// `PATH` resolution - mirrors `cancellai.py`'s optional `codex_bin` argument.
    codex_bin: Option<PathBuf>,
}

impl CodexProvider {
    pub fn new(root: impl Into<PathBuf>, is_default_root: bool) -> Self {
        Self {
            root: root.into(),
            is_default_root,
            codex_bin: None,
        }
    }

    pub fn with_codex_bin(mut self, codex_bin: impl Into<PathBuf>) -> Self {
        self.codex_bin = Some(codex_bin.into());
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn fingerprint(&self) -> RootFingerprint {
        fingerprint_codex_root(&self.root, self.is_default_root)
    }

    pub fn discover_sessions(&self) -> Vec<CodexSession> {
        discover_codex_sessions(&self.root)
    }

    pub fn subagent_trees(&self) -> Vec<SubagentTree> {
        group_into_subagent_trees(&self.discover_sessions())
    }

    pub fn native_delete_support(&self) -> NativeDeleteSupport {
        codex_delete_supported(self.codex_bin.as_deref())
    }

    pub fn protection(&self, path: &Path) -> ProtectionOutcome {
        codex_protected_component(path, &self.root)
    }

    fn explain_root(&self) -> String {
        let fp = self.fingerprint();
        let markers = if fp.markers.is_empty() {
            "none".to_string()
        } else {
            fp.markers.join(", ")
        };
        match fp.confidence {
            RootConfidence::Default => format!(
                "{} is the default Codex root (markers: {markers})",
                self.root.display()
            ),
            RootConfidence::High => format!(
                "{} is a custom root with high-confidence structural evidence (markers: {markers})",
                self.root.display()
            ),
            RootConfidence::Low => format!(
                "{} is a custom root with only low-confidence structural evidence (markers: {markers})",
                self.root.display()
            ),
            RootConfidence::Unknown => format!(
                "{} does not look like a Codex root (no recognized markers); inspection-only",
                self.root.display()
            ),
        }
    }
}

impl ProviderCapabilities for CodexProvider {
    fn provider_id(&self) -> &str {
        "codex-cli"
    }

    fn capability(&self, kind: CapabilityKind) -> CapabilityOutcome {
        match kind {
            CapabilityKind::Detect | CapabilityKind::FingerprintRoot => {
                let fp = self.fingerprint();
                let markers = if fp.markers.is_empty() {
                    "no markers found".to_string()
                } else {
                    format!("markers: {}", fp.markers.join(", "))
                };
                match fp.confidence {
                    RootConfidence::Default => CapabilityOutcome::new(
                        SupportState::Verified,
                        KnowledgeConfidence::Verified,
                        format!("default Codex root ({markers})"),
                        Vec::new(),
                        None,
                    ),
                    RootConfidence::High => CapabilityOutcome::new(
                        SupportState::SupportedObserved,
                        KnowledgeConfidence::Observed,
                        format!("custom root, high-confidence structural evidence ({markers})"),
                        Vec::new(),
                        None,
                    ),
                    RootConfidence::Low => CapabilityOutcome::new(
                        SupportState::SupportedObserved,
                        KnowledgeConfidence::Inferred,
                        format!("custom root, only low-confidence structural evidence ({markers})"),
                        Vec::new(),
                        Some(AuthorityLevel::Recommend),
                    ),
                    // AC3-equivalent to E05-S03's Claude adapter: an unknown layout downgrades
                    // to inspection-only (SI-004).
                    RootConfidence::Unknown => CapabilityOutcome::new(
                        SupportState::Unsupported,
                        KnowledgeConfidence::LowUnknown,
                        "no recognized Codex markers found under this root; inspection-only",
                        Vec::new(),
                        Some(AuthorityLevel::Observe),
                    ),
                }
            }
            CapabilityKind::ProjectAttribution => CapabilityOutcome::new(
                SupportState::Unsupported,
                KnowledgeConfidence::LowUnknown,
                "Codex sessions carry no project attribution in the reference implementation; \
                 only session/subagent identity is discoverable",
                Vec::new(),
                None,
            ),
            CapabilityKind::SessionGraph => {
                let trees = self.subagent_trees();
                let session_count: usize = trees.iter().map(|tree| tree.members.len()).sum();
                let multi_member_trees = trees.iter().filter(|tree| tree.members.len() > 1).count();
                CapabilityOutcome::new(
                    SupportState::SupportedObserved,
                    KnowledgeConfidence::Observed,
                    format!(
                        "{session_count} session(s) observed across {} root-rooted tree(s) ({multi_member_trees} with subagent children)",
                        trees.len()
                    ),
                    Vec::new(),
                    None,
                )
            }
            CapabilityKind::InventoryMap => CapabilityOutcome::new(
                SupportState::SupportedObserved,
                KnowledgeConfidence::Observed,
                "top-level Codex layout entries are classified via the protected-name list \
                 ported from cancellai.py; per-entry retention/legacy category mapping is \
                 deferred to a later story",
                Vec::new(),
                None,
            ),
            CapabilityKind::ActivityState => CapabilityOutcome::new(
                SupportState::Unsupported,
                KnowledgeConfidence::LowUnknown,
                "process activity detection (cancellai.py's active_processes) is not ported to this adapter yet",
                Vec::new(),
                None,
            ),
            CapabilityKind::NativeDeleteCapability => match self.native_delete_support() {
                NativeDeleteSupport::Supported { codex_bin } => CapabilityOutcome::new(
                    SupportState::Verified,
                    KnowledgeConfidence::Verified,
                    format!(
                        "{} advertises --force-capable delete (probed via `delete --help`)",
                        codex_bin.display()
                    ),
                    Vec::new(),
                    None,
                ),
                NativeDeleteSupport::Unsupported { codex_bin } => CapabilityOutcome::new(
                    SupportState::Unsupported,
                    KnowledgeConfidence::Observed,
                    format!(
                        "{} ran but does not advertise --force-capable delete; native delete is not assumed \
                         equivalent to filesystem deletion (SI-004, TM-09)",
                        codex_bin.display()
                    ),
                    Vec::new(),
                    None,
                ),
                NativeDeleteSupport::BinaryNotFound => CapabilityOutcome::new(
                    SupportState::Unsupported,
                    KnowledgeConfidence::LowUnknown,
                    "no codex binary found on PATH; native delete capability could not be probed",
                    Vec::new(),
                    None,
                ),
                NativeDeleteSupport::ProbeFailed { codex_bin, reason } => CapabilityOutcome::new(
                    SupportState::ErrorPartial,
                    KnowledgeConfidence::LowUnknown,
                    format!(
                        "probing {} for native delete support failed: {reason}",
                        codex_bin.display()
                    ),
                    Vec::new(),
                    None,
                ),
            },
            CapabilityKind::RetentionCapability => CapabilityOutcome::new(
                SupportState::Unsupported,
                KnowledgeConfidence::Verified,
                "Codex has no retention-configuration write path in the reference implementation",
                Vec::new(),
                None,
            ),
            CapabilityKind::Explain => CapabilityOutcome::new(
                SupportState::SupportedObserved,
                KnowledgeConfidence::Observed,
                self.explain_root(),
                Vec::new(),
                None,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_provider_api::capability_report;
    use std::fs;
    use std::path::PathBuf;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-codex-provider-test-{label}-{}",
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

    #[test]
    fn ac_an_unknown_layout_downgrades_detection_to_inspection_only() {
        let tree = TempTree::new("unknown-layout");
        let provider = CodexProvider::new(&tree.0, false);
        let outcome = provider.capability(CapabilityKind::Detect);
        assert_eq!(outcome.support(), SupportState::Unsupported);
        assert_eq!(outcome.authority_ceiling(), Some(AuthorityLevel::Observe));
    }

    #[test]
    fn ac2_native_delete_capability_is_never_inferred_from_root_detection_alone() {
        let tree = TempTree::new("native-delete-independent");
        fs::write(tree.0.join("auth.json"), "{}").unwrap();
        fs::write(tree.0.join("installation_id"), "x").unwrap();
        let provider = CodexProvider::new(&tree.0, false)
            .with_codex_bin("/definitely/does/not/exist/cancellai-test-codex-binary");
        assert_eq!(
            provider.capability(CapabilityKind::Detect).support(),
            SupportState::SupportedObserved
        );
        // Root detection succeeding must not by itself grant NativeDeleteCapability - the two
        // are answered independently (E05-S01 AC1: one capability's support never implies
        // another's). A nonexistent explicit codex_bin surfaces as either BinaryNotFound
        // (`Unsupported`) or a spawn failure (`ErrorPartial`, this platform's actual outcome -
        // `Command::spawn` on a nonexistent path is a spawn failure, not merely "not found"),
        // never as `Verified`/`SupportedObserved`.
        assert!(!matches!(
            provider
                .capability(CapabilityKind::NativeDeleteCapability)
                .support(),
            SupportState::Verified | SupportState::SupportedObserved
        ));
    }

    #[test]
    fn ac2_every_capability_report_entry_carries_evidence_and_confidence() {
        let tree = TempTree::new("full-report");
        fs::write(tree.0.join("auth.json"), "{}").unwrap();
        fs::write(tree.0.join("config.toml"), "x").unwrap();
        let provider = CodexProvider::new(&tree.0, true)
            .with_codex_bin("/definitely/does/not/exist/cancellai-test-codex-binary");
        for (kind, outcome) in capability_report(&provider) {
            assert!(
                !outcome.evidence().is_empty(),
                "{} had no evidence",
                kind.code()
            );
        }
    }

    #[test]
    fn protection_is_reachable_through_the_provider_not_only_the_free_function() {
        let root = Path::new("/root");
        let provider = CodexProvider::new(root, true);
        assert!(provider.protection(&root.join("auth.json")).is_protected());
        assert_eq!(
            provider.protection(&root.join("sessions/2026/08/20/rollout-x.jsonl")),
            ProtectionOutcome::Clear
        );
    }
}

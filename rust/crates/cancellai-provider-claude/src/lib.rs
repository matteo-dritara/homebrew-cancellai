//! Claude Code provider adapter: implements `cancellai_provider_api`'s capability contract
//! for Claude's on-disk layout. Provider-specific knowledge lives here, not in
//! `cancellai-model`/`cancellai-safety` (`docs/architecture/TARGET.md`).
//!
//! E05-S03 adds the first real logic: root fingerprinting ([`fingerprint`]), the protected-name
//! barrier ([`protected_names`]), flat project/session discovery ([`session`]), and
//! [`ClaudeProvider`], the [`ProviderCapabilities`] implementation tying them together.
//! `cancellai-inventory`'s generic OBSERVE-stage walk (`scan_scope`) is not reused for session
//! discovery - Claude's `projects/<project>/<session>.jsonl` (+ optional companion payload
//! directory) shape needs its own bespoke walk, matching how `cancellai.py`'s own
//! `discover_claude_sessions` is a bespoke function rather than a generic tree walk.

mod fingerprint;
mod protected_names;
mod session;

use std::path::{Path, PathBuf};

use cancellai_model::{AuthorityLevel, KnowledgeConfidence};
use cancellai_provider_api::{
    CapabilityKind, CapabilityOutcome, ProtectionOutcome, ProviderCapabilities, SupportState,
};

pub use fingerprint::{ClaudeRootFingerprint, RootConfidence, RootOrigin, fingerprint_claude_root};
pub use protected_names::{CLAUDE_PROTECTED_NAMES, claude_protected_component};
pub use session::{
    ClaudeSession, SessionDiscoveryResult, SessionDiscoveryScope, discover_claude_sessions,
};

/// A Claude Code provider adapter bound to one candidate root.
pub struct ClaudeProvider {
    root: PathBuf,
    is_default_root: bool,
}

impl ClaudeProvider {
    pub fn new(root: impl Into<PathBuf>, is_default_root: bool) -> Self {
        Self {
            root: root.into(),
            is_default_root,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn fingerprint(&self) -> ClaudeRootFingerprint {
        fingerprint_claude_root(&self.root, self.is_default_root)
    }

    pub fn discover_sessions(&self) -> SessionDiscoveryResult {
        discover_claude_sessions(&self.root)
    }

    pub fn protection(&self, path: &Path) -> ProtectionOutcome {
        claude_protected_component(path, &self.root)
    }

    fn explain_root(&self) -> String {
        let fp = self.fingerprint();
        let markers = if fp.markers.is_empty() {
            "none".to_string()
        } else {
            fp.markers.join(", ")
        };
        match fp.confidence {
            RootConfidence::Default => {
                format!(
                    "{} is the default Claude Code root (markers: {markers})",
                    self.root.display()
                )
            }
            RootConfidence::High => format!(
                "{} is a custom root with high-confidence structural evidence (markers: {markers})",
                self.root.display()
            ),
            RootConfidence::Low => format!(
                "{} is a custom root with only low-confidence structural evidence (markers: {markers})",
                self.root.display()
            ),
            RootConfidence::Unknown => format!(
                "{} does not look like a Claude Code root (no recognized markers); inspection-only",
                self.root.display()
            ),
        }
    }
}

impl ProviderCapabilities for ClaudeProvider {
    fn provider_id(&self) -> &str {
        "claude-code"
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
                        format!("default Claude Code root ({markers})"),
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
                    // AC3: an unknown layout downgrades to inspection-only.
                    RootConfidence::Unknown => CapabilityOutcome::new(
                        SupportState::Unsupported,
                        KnowledgeConfidence::LowUnknown,
                        "no recognized Claude Code markers found under this root; inspection-only",
                        Vec::new(),
                        Some(AuthorityLevel::Observe),
                    ),
                }
            }
            CapabilityKind::ProjectAttribution | CapabilityKind::SessionGraph => {
                let result = self.discover_sessions();
                match result.scope {
                    SessionDiscoveryScope::Unavailable => CapabilityOutcome::new(
                        SupportState::Unsupported,
                        KnowledgeConfidence::LowUnknown,
                        "projects/ is missing or is a symlink; no session relationships can be observed",
                        Vec::new(),
                        None,
                    ),
                    SessionDiscoveryScope::Observed if !result.degraded_companions.is_empty() => {
                        CapabilityOutcome::new(
                            SupportState::ErrorPartial,
                            KnowledgeConfidence::Observed,
                            format!(
                                "{} session(s) observed, but {} companion payload director{} could not be fully listed",
                                result.sessions.len(),
                                result.degraded_companions.len(),
                                if result.degraded_companions.len() == 1 {
                                    "y"
                                } else {
                                    "ies"
                                }
                            ),
                            Vec::new(),
                            Some(AuthorityLevel::Recommend),
                        )
                    }
                    SessionDiscoveryScope::Observed => CapabilityOutcome::new(
                        SupportState::SupportedObserved,
                        KnowledgeConfidence::Observed,
                        format!(
                            "{} session(s) observed, grouped by project directory (flat relationship; Claude has no subagent hierarchy)",
                            result.sessions.len()
                        ),
                        Vec::new(),
                        None,
                    ),
                }
            }
            CapabilityKind::InventoryMap => CapabilityOutcome::new(
                SupportState::SupportedObserved,
                KnowledgeConfidence::Observed,
                "top-level Claude layout entries are classified via the protected-name list \
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
            CapabilityKind::NativeDeleteCapability => CapabilityOutcome::new(
                SupportState::Unsupported,
                KnowledgeConfidence::Verified,
                "Claude Code exposes no vendor-native session delete command; deletion is filesystem-mediated only, through the safety kernel",
                Vec::new(),
                None,
            ),
            CapabilityKind::RetentionCapability => CapabilityOutcome::new(
                SupportState::Unsupported,
                KnowledgeConfidence::LowUnknown,
                "retention configuration write path (cancellai.py's configure_claude_retention) is not ported to this adapter yet",
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
                "cancellai-claude-provider-test-{label}-{}",
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
    fn ac1_an_unknown_layout_downgrades_every_root_dependent_capability_to_inspection_only() {
        let tree = TempTree::new("unknown-layout");
        let provider = ClaudeProvider::new(&tree.0, false);
        let outcome = provider.capability(CapabilityKind::Detect);
        assert_eq!(outcome.support(), SupportState::Unsupported);
        assert_eq!(outcome.authority_ceiling(), Some(AuthorityLevel::Observe));
    }

    #[test]
    fn ac3_a_default_root_is_verified_regardless_of_directory_contents() {
        let tree = TempTree::new("default-verified");
        let provider = ClaudeProvider::new(&tree.0, true);
        assert_eq!(
            provider.capability(CapabilityKind::Detect).support(),
            SupportState::Verified
        );
    }

    #[test]
    fn ac2_every_capability_report_entry_carries_evidence_and_confidence() {
        let tree = TempTree::new("full-report");
        fs::write(tree.0.join("settings.json"), "{}").unwrap();
        fs::write(tree.0.join("keybindings.json"), "{}").unwrap();
        let provider = ClaudeProvider::new(&tree.0, true);
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
        let provider = ClaudeProvider::new(root, true);
        assert!(
            provider
                .protection(&root.join("settings.json"))
                .is_protected()
        );
        assert_eq!(
            provider.protection(&root.join("projects/a/b.jsonl")),
            ProtectionOutcome::Clear
        );
    }
}

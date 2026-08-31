//! Claude Code's fixed protected-name barrier (ported verbatim from `cancellai.py`'s
//! `CLAUDE_PROTECTED_NAMES`, SI-006 "Protected-name/category barriers are defense in depth").
//! Memory, settings, and plugin state are never deleted by this build in normal operation.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use cancellai_provider_api::{ProtectionOutcome, protected_component};

/// Verbatim, unreordered copy of `cancellai.py`'s `CLAUDE_PROTECTED_NAMES` - settings/
/// keybindings config, and every category of auto-memory/skills/agents/commands/rules/
/// workflows/output-styles/plugin state.
pub const CLAUDE_PROTECTED_NAMES: &[&str] = &[
    "settings.json",
    "keybindings.json",
    "plugins",
    "skills",
    "agents",
    "commands",
    "rules",
    "workflows",
    "output-styles",
    "agent-memory",
];

static CLAUDE_PROTECTED_NAMES_SET: LazyLock<BTreeSet<&'static str>> =
    LazyLock::new(|| CLAUDE_PROTECTED_NAMES.iter().copied().collect());

/// Whether `path` is, or lives under, a Claude Code protected entry relative to `root` (AC:
/// "Memory/settings/plugin protected classes are explicit artifacts or exclusions with
/// evidence").
pub fn claude_protected_component(path: &Path, root: &Path) -> ProtectionOutcome {
    protected_component(path, root, &CLAUDE_PROTECTED_NAMES_SET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_documented_protected_name_is_actually_protected_at_the_top_level() {
        let root = Path::new("/root");
        for name in CLAUDE_PROTECTED_NAMES {
            let outcome = claude_protected_component(&root.join(name), root);
            assert!(outcome.is_protected(), "{name} was not reported protected");
        }
    }

    #[test]
    fn an_ordinary_session_path_is_never_protected() {
        let root = Path::new("/root");
        let outcome = claude_protected_component(
            &root.join("projects/synthetic-project-a/11111111-1111-4111-8111-111111111111.jsonl"),
            root,
        );
        assert_eq!(outcome, ProtectionOutcome::Clear);
    }
}

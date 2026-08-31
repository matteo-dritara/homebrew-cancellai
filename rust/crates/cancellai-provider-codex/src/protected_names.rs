//! Codex CLI's fixed protected-name barrier (ported verbatim from `cancellai.py`'s
//! `CODEX_PROTECTED_NAMES`, SI-006). Auth/config, skills, rules, memories, and installed
//! plugin state are never deleted by this build in normal operation.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use cancellai_provider_api::{ProtectionOutcome, protected_component};

/// Verbatim, unreordered copy of `cancellai.py`'s `CODEX_PROTECTED_NAMES`. `plugins` mirrors
/// `CLAUDE_PROTECTED_NAMES`'s own entry as a deliberate guard against a future broader
/// top-level scan treating installed Codex plugin state as disposable, even though no current
/// discovery path sweeps it.
pub const CODEX_PROTECTED_NAMES: &[&str] = &[
    "auth.json",
    "config.toml",
    "skills",
    "rules",
    "memories",
    "plugins",
];

static CODEX_PROTECTED_NAMES_SET: LazyLock<BTreeSet<&'static str>> =
    LazyLock::new(|| CODEX_PROTECTED_NAMES.iter().copied().collect());

/// Whether `path` is, or lives under, a Codex protected entry relative to `root` (AC:
/// "SQLite/config/auth/plugin state stays protected" - `sqlite/` itself is a `ROOT_MARKERS`
/// fingerprint marker, not a `CODEX_PROTECTED_NAMES` entry in `cancellai.py`; this function is
/// a verbatim port of the protected-name list as documented, not an expansion of it).
pub fn codex_protected_component(path: &Path, root: &Path) -> ProtectionOutcome {
    protected_component(path, root, &CODEX_PROTECTED_NAMES_SET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_documented_protected_name_is_actually_protected_at_the_top_level() {
        let root = Path::new("/root");
        for name in CODEX_PROTECTED_NAMES {
            let outcome = codex_protected_component(&root.join(name), root);
            assert!(outcome.is_protected(), "{name} was not reported protected");
        }
    }

    #[test]
    fn an_ordinary_rollout_path_is_never_protected() {
        let root = Path::new("/root");
        let outcome = codex_protected_component(
            &root.join("sessions/2026/08/20/rollout-2026-08-20T09-00-00-11111111-1111-4111-8111-111111111111.jsonl"),
            root,
        );
        assert_eq!(outcome, ProtectionOutcome::Clear);
    }
}

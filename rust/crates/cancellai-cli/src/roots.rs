//! Default provider root resolution, matching `cancellai.py`'s `get_claude_home`/
//! `get_codex_home` exactly: an explicit environment override when present, `$HOME/.claude` or
//! `$HOME/.codex` otherwise.
//!
//! Unix-only for now (`$HOME`) - this mirrors `cancellai-platform::identity`'s own precedent of
//! an honest, typed gap rather than a plausible-but-unverified Windows path (`%USERPROFILE%`)
//! this workspace has no Windows CI to exercise yet; see that module's docs for the same
//! rationale applied to identity instead of home-directory resolution.

use std::path::PathBuf;

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Resolves the Codex CLI home directory: `$CODEX_HOME`, or `$HOME/.codex`.
pub fn codex_home() -> Option<PathBuf> {
    if let Some(raw) = std::env::var_os("CODEX_HOME") {
        return Some(PathBuf::from(raw));
    }
    home_dir().map(|home| home.join(".codex"))
}

/// Resolves the Claude Code home directory: `$CLAUDE_CONFIG_DIR`, or `$HOME/.claude`.
pub fn claude_home() -> Option<PathBuf> {
    if let Some(raw) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(PathBuf::from(raw));
    }
    home_dir().map(|home| home.join(".claude"))
}

//! Default provider root resolution, matching `cancellai.py`'s `get_claude_home`/
//! `get_codex_home`/`fingerprint_root`'s own origin derivation exactly: an explicit
//! environment override when present, `$HOME/.claude` or `$HOME/.codex` otherwise - and
//! `origin = "default"` only when the *resolved* path is literally the provider's own default
//! directory, not merely "no override env var was set" (E06 verifier review round 1: an
//! earlier version of this module let a caller separately decide `is_default_root`, and every
//! call site got it wrong by hard-coding `true` regardless of `CLAUDE_CONFIG_DIR`/`CODEX_HOME`,
//! violating SI-002/SI-004: "a custom or low-confidence provider root cannot gain destructive
//! authority"). Deriving `is_default` here, from the same comparison Python performs, removes
//! that whole class of caller mistake instead of trusting every caller to get it right.
//!
//! Unix-only for now (`$HOME`) - this mirrors `cancellai-platform::identity`'s own precedent of
//! an honest, typed gap rather than a plausible-but-unverified Windows path (`%USERPROFILE%`)
//! this workspace has no Windows CI to exercise yet; see that module's docs for the same
//! rationale applied to identity instead of home-directory resolution.

use std::path::{Path, PathBuf};

/// A resolved provider root, plus whether it is the provider's own OS-default directory
/// (`cancellai.py`'s `RootAuthority.origin == "default"`) - the only origin the safety kernel
/// permits destructive work against (ADR-0013).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoot {
    pub path: PathBuf,
    pub is_default: bool,
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Best-effort path equality matching `cancellai.py`'s `path.resolve(strict=False)` comparison:
/// canonicalize both sides when they exist (resolving symlinks, matching Python exactly), and
/// fall back to plain path equality when canonicalization is unavailable (a path that does not
/// exist yet, e.g. on a fresh machine, cannot be canonicalized by either language - `resolve
/// (strict=False)` tolerates that by construction, `std::fs::canonicalize` does not, so this
/// falls back rather than treating a nonexistent root as an error here).
fn paths_match(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

/// The pure resolution rule, isolated from reading real environment variables so tests never
/// have to mutate process-wide global state (`std::env::set_var` is `unsafe` and this
/// workspace forbids unsafe code outright - see `rust/deny.toml`/clippy config) to exercise
/// every branch.
fn resolve_from(
    env_override: Option<PathBuf>,
    home: Option<PathBuf>,
    default_suffix: &str,
) -> Option<ResolvedRoot> {
    let default_path = home.map(|home| home.join(default_suffix));
    match env_override {
        Some(custom) => {
            let is_default = default_path
                .as_deref()
                .is_some_and(|d| paths_match(&custom, d));
            Some(ResolvedRoot {
                path: custom,
                is_default,
            })
        }
        None => default_path.map(|path| ResolvedRoot {
            path,
            is_default: true,
        }),
    }
}

/// Resolves the Codex CLI home directory: `$CODEX_HOME`, or `$HOME/.codex`. `None` when neither
/// an override nor `$HOME` is available - callers must fail closed rather than guess at a
/// fallback root (E06 verifier review round 1: an earlier caller silently fell back to `"."`,
/// the current working directory, which is not a positively-identified provider root at all).
pub fn codex_home() -> Option<ResolvedRoot> {
    resolve_from(
        std::env::var_os("CODEX_HOME").map(PathBuf::from),
        home_dir(),
        ".codex",
    )
}

/// Resolves the Claude Code home directory: `$CLAUDE_CONFIG_DIR`, or `$HOME/.claude`. See
/// [`codex_home`] for why this is `None`, not a silent fallback, when it cannot be determined.
pub fn claude_home() -> Option<ResolvedRoot> {
    resolve_from(
        std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
        home_dir(),
        ".claude",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_override_resolves_to_home_dot_claude_as_the_default_origin() {
        let resolved = resolve_from(None, Some(PathBuf::from("/synthetic/home")), ".claude")
            .expect("home is set");
        assert_eq!(resolved.path, PathBuf::from("/synthetic/home/.claude"));
        assert!(resolved.is_default);
    }

    #[test]
    fn an_override_pointing_elsewhere_is_a_custom_non_default_root() {
        let resolved = resolve_from(
            Some(PathBuf::from("/synthetic/elsewhere")),
            Some(PathBuf::from("/synthetic/home")),
            ".claude",
        )
        .expect("override is set");
        assert_eq!(resolved.path, PathBuf::from("/synthetic/elsewhere"));
        assert!(
            !resolved.is_default,
            "an override pointing anywhere but the real default must never be reported as default"
        );
    }

    #[test]
    fn an_override_pointing_literally_at_the_default_path_is_still_the_default_origin() {
        // Matches `cancellai.py::fingerprint_root`'s own comparison exactly: origin is decided
        // by where the resolved path *is*, not by which code path produced it.
        let resolved = resolve_from(
            Some(PathBuf::from("/synthetic/home/.claude")),
            Some(PathBuf::from("/synthetic/home")),
            ".claude",
        )
        .expect("override is set");
        assert!(resolved.is_default);
    }

    #[test]
    fn an_absent_home_with_no_override_resolves_to_nothing_rather_than_a_guessed_fallback() {
        assert_eq!(
            resolve_from(None, None, ".claude"),
            None,
            "an unresolvable root must never silently become the current working directory"
        );
    }

    #[test]
    fn an_absent_home_with_an_override_is_a_custom_root_not_an_error() {
        // Cannot prove "default" without a real $HOME to compare against, so this must fail
        // toward Custom (the more restrictive origin), never toward Default.
        let resolved = resolve_from(Some(PathBuf::from("/synthetic/elsewhere")), None, ".claude")
            .expect("override is set");
        assert!(!resolved.is_default);
    }

    #[test]
    fn codex_home_uses_its_own_suffix() {
        let resolved = resolve_from(None, Some(PathBuf::from("/synthetic/home")), ".codex")
            .expect("home is set");
        assert_eq!(resolved.path, PathBuf::from("/synthetic/home/.codex"));
        assert!(resolved.is_default);
    }

    #[test]
    fn low_confidence_custom_root_containing_only_a_low_confidence_marker_is_still_non_default() {
        // Regression for the review's exact reproduction: origin alone (not confidence) is
        // what `withhold_for_root_authority` gates on - a custom root must never read as
        // `is_default` regardless of how much or how little marker evidence it later turns out
        // to carry.
        let resolved = resolve_from(
            Some(PathBuf::from("/synthetic/custom-root-with-only-projects")),
            Some(PathBuf::from("/synthetic/home")),
            ".claude",
        )
        .expect("override is set");
        assert!(!resolved.is_default);
    }
}

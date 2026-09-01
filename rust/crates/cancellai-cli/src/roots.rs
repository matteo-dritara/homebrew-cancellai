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
//! `is_default` also requires the leaf path itself (`$HOME/.claude`/`$HOME/.codex`, or the
//! override value) not to be a symlink/reparse point (E06 verifier review round 2): a default
//! root's *authority* is grounded in the well-known path belonging to the operator, not merely
//! in whatever it happens to resolve to - `$HOME/.claude -> <attacker-controlled-or-just-wrong
//! -directory>` must never be treated as authoritative purely because no override env var was
//! set. This is deliberately checked here (classification time, "before planning") *and*
//! re-checked independently immediately before every mutation/configuration rewrite by this
//! module's callers (`main.rs::delete_one`/`cmd_configure`) via [`is_symlink`], not trusted from
//! a cached classification alone - the same defense-in-depth shape already used for root origin
//! and the process-liveness guard.
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

/// Whether `path` itself - not a descendant, the path exactly - is a symlink (or, on a platform
/// where `std`'s `FileType::is_symlink` reports it, a reparse point/junction). A nonexistent
/// path is never a symlink (a fresh machine's absent `$HOME/.claude` is legitimately, positively
/// default - `cancellai.py::fingerprint_root`'s own "authoritative by definition, including when
/// empty or absent" comment), so this only ever *removes* default authority, never blocks
/// resolution outright the way an I/O error would.
pub fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

/// The pure resolution rule, isolated from reading real environment variables/the filesystem so
/// tests never have to mutate process-wide global state (`std::env::set_var` is `unsafe` and
/// this workspace forbids unsafe code outright - see `rust/deny.toml`/clippy config) or touch
/// real paths to exercise every branch. `default_path`/`default_is_symlink` are the caller's own
/// answers for the provider's default location - see [`is_symlink`]'s docs for why the symlink
/// check applies uniformly regardless of whether an override was supplied (E06 verifier review
/// round 2: authority must never come from the lexical `$HOME/.claude` name alone).
fn resolve_from(
    env_override: Option<PathBuf>,
    default_path: Option<PathBuf>,
    default_is_symlink: bool,
) -> Option<ResolvedRoot> {
    match env_override {
        Some(custom) => {
            let is_default = !default_is_symlink
                && default_path
                    .as_deref()
                    .is_some_and(|d| paths_match(&custom, d));
            Some(ResolvedRoot {
                path: custom,
                is_default,
            })
        }
        None => default_path.map(|path| ResolvedRoot {
            path,
            is_default: !default_is_symlink,
        }),
    }
}

/// Resolves the Codex CLI home directory: `$CODEX_HOME`, or `$HOME/.codex`. `None` when neither
/// an override nor `$HOME` is available - callers must fail closed rather than guess at a
/// fallback root (E06 verifier review round 1: an earlier caller silently fell back to `"."`,
/// the current working directory, which is not a positively-identified provider root at all).
pub fn codex_home() -> Option<ResolvedRoot> {
    let default_path = home_dir().map(|home| home.join(".codex"));
    let default_is_symlink = default_path.as_deref().is_some_and(is_symlink);
    resolve_from(
        std::env::var_os("CODEX_HOME").map(PathBuf::from),
        default_path,
        default_is_symlink,
    )
}

/// Resolves the Claude Code home directory: `$CLAUDE_CONFIG_DIR`, or `$HOME/.claude`. See
/// [`codex_home`] for why this is `None`, not a silent fallback, when it cannot be determined.
pub fn claude_home() -> Option<ResolvedRoot> {
    let default_path = home_dir().map(|home| home.join(".claude"));
    let default_is_symlink = default_path.as_deref().is_some_and(is_symlink);
    resolve_from(
        std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
        default_path,
        default_is_symlink,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_override_resolves_to_home_dot_claude_as_the_default_origin() {
        let resolved = resolve_from(None, Some(PathBuf::from("/synthetic/home/.claude")), false)
            .expect("home is set");
        assert_eq!(resolved.path, PathBuf::from("/synthetic/home/.claude"));
        assert!(resolved.is_default);
    }

    #[test]
    fn an_override_pointing_elsewhere_is_a_custom_non_default_root() {
        let resolved = resolve_from(
            Some(PathBuf::from("/synthetic/elsewhere")),
            Some(PathBuf::from("/synthetic/home/.claude")),
            false,
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
            Some(PathBuf::from("/synthetic/home/.claude")),
            false,
        )
        .expect("override is set");
        assert!(resolved.is_default);
    }

    #[test]
    fn an_absent_home_with_no_override_resolves_to_nothing_rather_than_a_guessed_fallback() {
        assert_eq!(
            resolve_from(None, None, false),
            None,
            "an unresolvable root must never silently become the current working directory"
        );
    }

    #[test]
    fn an_absent_home_with_an_override_is_a_custom_root_not_an_error() {
        // Cannot prove "default" without a real $HOME to compare against, so this must fail
        // toward Custom (the more restrictive origin), never toward Default.
        let resolved = resolve_from(Some(PathBuf::from("/synthetic/elsewhere")), None, false)
            .expect("override is set");
        assert!(!resolved.is_default);
    }

    #[test]
    fn codex_home_uses_its_own_suffix() {
        let resolved = resolve_from(None, Some(PathBuf::from("/synthetic/home/.codex")), false)
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
            Some(PathBuf::from("/synthetic/home/.claude")),
            false,
        )
        .expect("override is set");
        assert!(!resolved.is_default);
    }

    #[test]
    fn a_default_path_that_is_itself_a_symlink_is_never_the_default_origin() {
        // E06 verifier review round 2's exact reproduction: $HOME/.claude -> <outside>, no
        // CLAUDE_CONFIG_DIR override at all. Authority must never come from the lexical
        // "$HOME/.claude" name alone - a symlinked leaf loses default status regardless of
        // where it happens to point.
        let resolved = resolve_from(None, Some(PathBuf::from("/synthetic/home/.claude")), true)
            .expect("home is set");
        assert!(
            !resolved.is_default,
            "a symlinked default-named root must never be treated as the default origin"
        );
    }

    #[test]
    fn an_override_literally_naming_the_default_path_is_still_refused_when_that_path_is_a_symlink()
    {
        // Same principle, reached via the override branch: an operator (or an attacker) setting
        // CLAUDE_CONFIG_DIR to the exact string "$HOME/.claude" must not launder a symlinked
        // default location into default authority either.
        let resolved = resolve_from(
            Some(PathBuf::from("/synthetic/home/.claude")),
            Some(PathBuf::from("/synthetic/home/.claude")),
            true,
        )
        .expect("override is set");
        assert!(!resolved.is_default);
    }

    #[test]
    fn a_nonexistent_default_path_is_not_a_symlink() {
        // A fresh machine with no ~/.claude yet must stay positively default (`cancellai.py::
        // fingerprint_root`'s own "authoritative by definition, including when empty or absent")
        // - `is_symlink` must answer false for a path with nothing there, not fail closed here.
        let missing = std::env::temp_dir().join(format!(
            "cancellai-roots-test-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        assert!(!is_symlink(&missing));
    }

    #[cfg(unix)]
    #[test]
    fn is_symlink_detects_a_real_symlink_but_not_a_real_directory() {
        let base = std::env::temp_dir().join(format!(
            "cancellai-roots-test-symlink-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let real_dir = base.join("real");
        let link = base.join("link");
        std::fs::create_dir_all(&real_dir).unwrap();
        std::os::unix::fs::symlink(&real_dir, &link).unwrap();

        assert!(!is_symlink(&real_dir));
        assert!(is_symlink(&link));

        std::fs::remove_dir_all(&base).ok();
    }

    // Windows counterpart of the Unix case above (E07-S07). `FileType::is_symlink()` reports
    // `true` for a directory symlink created via `std::os::windows::fs::symlink_dir` - the same
    // reparse-point machinery this module's own docs rely on - proving `is_symlink` rejects a
    // symlinked default-named root on Windows too, not only Unix. Requires
    // `SeCreateSymbolicLinkPrivilege` (Developer Mode or an elevated process), which this repo's
    // Windows CI runners carry.
    #[cfg(windows)]
    #[test]
    fn is_symlink_detects_a_real_symlink_but_not_a_real_directory() {
        let base = std::env::temp_dir().join(format!(
            "cancellai-roots-test-symlink-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let real_dir = base.join("real");
        let link = base.join("link");
        std::fs::create_dir_all(&real_dir).unwrap();
        std::os::windows::fs::symlink_dir(&real_dir, &link).unwrap();

        assert!(!is_symlink(&real_dir));
        assert!(is_symlink(&link));

        std::fs::remove_dir_all(&base).ok();
    }
}

//! Tool-agnostic protected-name barrier (ported from `cancellai.py`'s `canonical_name`/
//! `protected_component`, `docs/architecture/AS_IS.md`'s safety-critical core item 5, SI-006
//! "Protected-name/category barriers are defense in depth").
//!
//! Every provider adapter (Claude, Codex, ...) supplies its own fixed protected-name set; the
//! comparison and containment logic does not vary by provider (`cancellai.py`'s own
//! `protected_component` already takes `protected_names` as a parameter rather than hardcoding
//! either tool's list), so it lives here rather than being duplicated per adapter crate.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

/// Unicode canonical caseless form: NFD, lowercase, NFD again - the same two-pass shape
/// `cancellai.py`'s `canonical_name` uses (NFD, `casefold()`, NFD) so a decomposed-vs-
/// precomposed or case-variant spelling of a protected name is recognized as the same name.
///
/// `to_lowercase` is Rust's Unicode-aware *simple* case mapping, not Python's full Unicode
/// case *folding* table (`str.casefold()`) - the two differ only for a small set of exotic
/// characters (e.g. German ß). Every name in every protected-name set this workspace defines
/// today (`CLAUDE_PROTECTED_NAMES`/`CODEX_PROTECTED_NAMES`) is plain ASCII, where simple
/// lowercase and full casefold agree exactly, so this is a documented, narrow, currently-inert
/// divergence from the Python reference rather than a silent one - see this story's evidence
/// packet residual risks.
pub fn canonical_name(name: &str) -> String {
    let lowered: String = name.nfd().collect::<String>().to_lowercase();
    lowered.nfd().collect()
}

/// Collapses `.`/`..`/repeated separators textually, without touching the filesystem or
/// requiring the path to exist - Rust's `Path`/`Component` iteration already merges repeated
/// separators and drops bare `.` components, but leaves `..` unresolved; this additionally
/// pops the previous `Normal` component for each `..`, mirroring Python's
/// `os.path.normpath`. A leading `..` past the root (nothing left to pop) is kept as-is,
/// matching `normpath`'s own behavior for a path that walks above its starting point.
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut stack: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match stack.last() {
                Some(Component::Normal(_)) => {
                    stack.pop();
                }
                _ => stack.push(component),
            },
            other => stack.push(other),
        }
    }
    stack.into_iter().collect()
}

/// The outcome of checking whether `path` is, or lives under, a protected entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectionOutcome {
    /// No component of `path` (lexically, or after resolving symlinks when that resolution
    /// succeeds) canonically matches a protected name.
    Clear,
    /// `path` is or lives under a protected entry, canonically matching `matched_name` (one
    /// of the caller-supplied protected names, in its original spelling).
    Protected { matched_name: String },
}

impl ProtectionOutcome {
    pub fn is_protected(&self) -> bool {
        matches!(self, ProtectionOutcome::Protected { .. })
    }
}

/// Whether `path` is, or lives under, one of `protected_names` relative to `root`. Checked
/// both lexically (normalized, not resolved) and after resolving symlinks - a protected entry
/// that is itself a symlink pointing outside `root` must not lose its protection merely
/// because resolution moves it outside the relative-path computation
/// (`cancellai.py::protected_component`'s doc comment; the exact scenario the
/// `claude-symlink-protected-name` fixture exercises). The name is checked lexically *first*
/// for exactly that reason: resolving before checking would let a protected symlink escape
/// detection by falling outside the relative-path computation entirely.
///
/// Scope note (documented, narrow divergence from `cancellai.py`): when resolving `path` or
/// `root` fails, this function silently skips the resolved view rather than returning
/// Python's `"<unresolvable>"` fail-closed sentinel. Python's `resolve(strict=False)` tolerates
/// a nonexistent path and only raises `OSError` for a genuine resolution failure (a symlink
/// loop, a permission error walking an intermediate component) - a rare case none of this
/// corpus's fixtures trigger. The lexical view (always checked, first) is unaffected either
/// way; this only narrows the *additional* resolved-view check. This module is a
/// classification/evidence signal, not itself the mutation safety boundary - path-escape
/// prevention for real mutation is `cancellai-safety`'s `ApprovedRoot`/`BoundedPath` (SI-002,
/// SI-003), which this module does not replace.
pub fn protected_component(
    path: &Path,
    root: &Path,
    protected_names: &BTreeSet<&str>,
) -> ProtectionOutcome {
    if protected_names.is_empty() {
        return ProtectionOutcome::Clear;
    }
    let folded: Vec<(String, &str)> = protected_names
        .iter()
        .map(|name| (canonical_name(name), *name))
        .collect();

    let mut views: Vec<(PathBuf, PathBuf)> =
        vec![(normalize_lexically(path), normalize_lexically(root))];
    if let (Ok(resolved_path), Ok(resolved_root)) = (path.canonicalize(), root.canonicalize()) {
        views.push((resolved_path, resolved_root));
    }

    for (candidate, base) in &views {
        let Ok(relative) = candidate.strip_prefix(base) else {
            continue;
        };
        for part in relative.components() {
            if let Component::Normal(part) = part {
                let canonical = canonical_name(&part.to_string_lossy());
                if let Some((_, original)) = folded.iter().find(|(f, _)| *f == canonical) {
                    return ProtectionOutcome::Protected {
                        matched_name: (*original).to_string(),
                    };
                }
            }
        }
    }
    ProtectionOutcome::Clear
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&'static str]) -> BTreeSet<&'static str> {
        list.iter().copied().collect()
    }

    #[test]
    fn canonical_name_folds_ascii_case() {
        assert_eq!(canonical_name("Plugins"), canonical_name("plugins"));
    }

    #[test]
    fn canonical_name_folds_decomposed_and_precomposed_forms() {
        // "café" precomposed (é = U+00E9) vs decomposed (e + U+0301).
        let precomposed = "caf\u{00e9}";
        let decomposed = "cafe\u{0301}";
        assert_eq!(canonical_name(precomposed), canonical_name(decomposed));
    }

    #[test]
    fn ac_a_top_level_protected_entry_is_protected() {
        let root = Path::new("/root");
        let path = Path::new("/root/settings.json");
        let outcome = protected_component(path, root, &names(&["settings.json", "plugins"]));
        assert_eq!(
            outcome,
            ProtectionOutcome::Protected {
                matched_name: "settings.json".to_string()
            }
        );
    }

    #[test]
    fn ac_a_path_nested_under_a_protected_directory_is_protected() {
        let root = Path::new("/root");
        let path = Path::new("/root/plugins/cache/index.bin");
        let outcome = protected_component(path, root, &names(&["plugins"]));
        assert!(outcome.is_protected());
    }

    #[test]
    fn an_unrelated_path_is_clear() {
        let root = Path::new("/root");
        let path = Path::new("/root/projects/a/session.jsonl");
        let outcome = protected_component(path, root, &names(&["settings.json", "plugins"]));
        assert_eq!(outcome, ProtectionOutcome::Clear);
    }

    #[test]
    fn an_empty_protected_set_never_protects_anything() {
        let root = Path::new("/root");
        let path = Path::new("/root/settings.json");
        let outcome = protected_component(path, root, &BTreeSet::new());
        assert_eq!(outcome, ProtectionOutcome::Clear);
    }

    #[test]
    fn si006_a_case_variant_of_a_protected_name_is_still_protected() {
        // The exact claude-symlink-protected-name fixture scenario: "Plugins" (capital P)
        // must still match the protected "plugins" entry.
        let root = Path::new("/root");
        let path = Path::new("/root/Plugins");
        let outcome = protected_component(path, root, &names(&["plugins"]));
        assert_eq!(
            outcome,
            ProtectionOutcome::Protected {
                matched_name: "plugins".to_string()
            }
        );
    }

    #[test]
    fn a_path_outside_root_lexically_is_not_matched_against_root_relative_components() {
        let root = Path::new("/root");
        let path = Path::new("/elsewhere/plugins");
        let outcome = protected_component(path, root, &names(&["plugins"]));
        assert_eq!(outcome, ProtectionOutcome::Clear);
    }

    #[test]
    fn normalize_lexically_collapses_parent_dir_components_textually() {
        let normalized = normalize_lexically(Path::new("/root/a/../b"));
        assert_eq!(normalized, PathBuf::from("/root/b"));
    }
}

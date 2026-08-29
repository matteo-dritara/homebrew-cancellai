//! Path canonicalization/normalization as its own OS capability seam (E03-S03,
//! `docs/architecture/PLATFORM_MODEL.md`'s "Required platform capabilities": "path
//! canonicalization/normalization" is listed there separately from filesystem object
//! identity, and this mirrors that split - resolving a path and observing what sits at it
//! are two different OS operations with two different failure modes).
//!
//! Mirrors [`crate::clock::Clock`]/[`crate::fs_observer::FsObserver`]/
//! [`crate::identity::IdentityObserver`]'s seam shape: a real, OS-backed
//! [`SystemPathResolver`] and a test-only [`SyntheticPathResolver`] that injects a
//! canonicalization result without touching the real filesystem.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A source of path canonicalization facts.
pub trait PathResolver: Send + Sync {
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, String>;
}

/// The real, OS-backed resolver.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPathResolver;

impl PathResolver for SystemPathResolver {
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, String> {
        std::fs::canonicalize(path).map_err(|e| e.to_string())
    }
}

/// Test-only seam: synthesize a canonicalization result for specific paths without touching
/// the real filesystem. A path with no fact explicitly `set` reports a distinct "not
/// configured" error, never a silently invented one, since a synthetic resolver that guesses
/// would be lying about what the test actually configured.
#[derive(Debug, Default)]
pub struct SyntheticPathResolver {
    facts: BTreeMap<PathBuf, Result<PathBuf, String>>,
}

impl SyntheticPathResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, path: impl Into<PathBuf>, result: Result<PathBuf, String>) -> &mut Self {
        self.facts.insert(path.into(), result);
        self
    }
}

impl PathResolver for SyntheticPathResolver {
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, String> {
        self.facts.get(path).cloned().unwrap_or_else(|| {
            Err(format!(
                "no synthetic canonicalization configured for {}",
                path.display()
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_resolver_canonicalizes_a_real_path() {
        let dir = std::env::temp_dir();
        let resolver = SystemPathResolver;
        let canonical = resolver
            .canonicalize(&dir)
            .expect("canonicalize a real directory");
        assert!(canonical.is_absolute());
    }

    #[test]
    fn system_resolver_reports_a_missing_path_as_an_error() {
        let missing = std::env::temp_dir().join("cancellai-path-resolver-does-not-exist");
        let resolver = SystemPathResolver;
        assert!(resolver.canonicalize(&missing).is_err());
    }

    #[test]
    fn synthetic_resolver_reports_exactly_what_was_set() {
        let mut resolver = SyntheticPathResolver::new();
        resolver.set("/synthetic/in", Ok(PathBuf::from("/synthetic/canonical")));
        resolver.set("/synthetic/broken", Err("dangling symlink".into()));

        assert_eq!(
            resolver.canonicalize(Path::new("/synthetic/in")),
            Ok(PathBuf::from("/synthetic/canonical"))
        );
        assert_eq!(
            resolver.canonicalize(Path::new("/synthetic/broken")),
            Err("dangling symlink".to_string())
        );
    }

    #[test]
    fn synthetic_resolver_never_silently_invents_a_result_for_an_unset_path() {
        let resolver = SyntheticPathResolver::new();
        let err = resolver
            .canonicalize(Path::new("/never/configured"))
            .expect_err("an unconfigured path must be a distinct error, not a guess");
        assert!(err.contains("no synthetic canonicalization configured"));
    }
}

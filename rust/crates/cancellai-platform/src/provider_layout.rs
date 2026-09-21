//! Real, non-forgeable observation of a provider root's top-level layout (E14-S04 round 5,
//! ADR-0036).
//!
//! Four independent review rounds on `cancellai-safety`'s authority-boundary design (ADR-0034,
//! ADR-0035, `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND3.md`,
//! `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND4.md`) found the same structural defect in
//! three different shapes: a *caller-supplied* layout fact - a ceiling, then the raw
//! `known_signatures`/`observed` markers themselves - can always be asserted honestly once and
//! then, in a second, independently constructed value, asserted differently or omitted
//! entirely. No amount of making the *field* mandatory closes this, because the type doing the
//! asserting is a plain, publicly constructible value with no binding to the real object it
//! claims to describe.
//!
//! [`BoundLayoutObservation`] closes the "supplied" half of that gap by removing it: its only
//! constructor, [`BoundLayoutObservation::observe`], performs real directory I/O against a real
//! path and records the root's own [`IdentityToken`] alongside what it actually found. There is
//! no way to construct one from caller-asserted marker strings, in this crate or any other -
//! unlike `cancellai-safety`'s former `ProviderLayoutAssessment::Observed`, which any caller
//! could construct directly. `cancellai_safety::resolve_provider_execution_authority` (round 5)
//! is the only place this observation feeds into an authority decision, and it requires one by
//! value, not an enum variant a caller could choose to omit.

use std::path::Path;

use crate::identity::{IdentityObserver, IdentityToken};

/// A directory listing or identity read failed. Never treated as an empty/clean layout by any
/// caller of [`BoundLayoutObservation::observe`] - an unobservable root is missing evidence, not
/// absence of drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutObservationError(pub String);

/// A real observation of one provider root: the root's own identity (so a later authority
/// decision can be bound to *this specific object*, not merely a path string that could since
/// have been swapped - the same reasoning `cancellai-platform::mutation`'s confirmed-delete path
/// already applies to a file), and the marker names actually present as its direct children.
/// Directory markers carry a trailing `/`, matching
/// `cancellai_guardian::structural::LayoutSignature`'s own convention, so the two crates'
/// independently maintained comparisons agree on the same real-world layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundLayoutObservation {
    root_identity: IdentityToken,
    markers: Vec<String>,
}

impl BoundLayoutObservation {
    /// The only constructor. `identity_observer` is a capability, not a shortcut: production
    /// callers pass [`crate::SystemIdentityObserver`]; nothing here reads `root`'s identity by
    /// any other path, so this cannot be satisfied with a fabricated [`IdentityToken`] the way a
    /// plain struct literal could be.
    pub fn observe(
        root: &Path,
        identity_observer: &dyn IdentityObserver,
    ) -> Result<Self, LayoutObservationError> {
        let root_identity = match identity_observer.observe(root) {
            crate::identity::IdentityObservation::Identity(identity) => identity,
            crate::identity::IdentityObservation::Absent => {
                return Err(LayoutObservationError(format!(
                    "{} does not exist; cannot bind a layout observation to a root that is not \
                     there",
                    root.display()
                )));
            }
            crate::identity::IdentityObservation::Unreadable { reason } => {
                return Err(LayoutObservationError(format!(
                    "could not observe {}'s identity: {reason}",
                    root.display()
                )));
            }
            crate::identity::IdentityObservation::Unsupported { reason } => {
                return Err(LayoutObservationError(format!(
                    "{}'s identity evidence is not trusted for a safety decision: {reason}",
                    root.display()
                )));
            }
        };

        let entries = std::fs::read_dir(root).map_err(|e| {
            LayoutObservationError(format!("could not list {}: {e}", root.display()))
        })?;
        let mut markers = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| {
                LayoutObservationError(format!(
                    "could not read a directory entry under {}: {e}",
                    root.display()
                ))
            })?;
            let file_type = entry.file_type().map_err(|e| {
                LayoutObservationError(format!(
                    "could not read the file type of {}: {e}",
                    entry.path().display()
                ))
            })?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if file_type.is_dir() {
                markers.push(format!("{name}/"));
            } else {
                markers.push(name);
            }
        }

        Ok(Self {
            root_identity,
            markers,
        })
    }

    /// The observed root's own identity, captured at observation time - the binding a future
    /// mutation-boundary consumer needs to refuse a permit whose root has since changed
    /// (ADR-0036's disclosed residual: this story mints the permit, it does not yet wire that
    /// check into `cancellai-safety::mutation_executor`).
    pub fn root_identity(&self) -> &IdentityToken {
        &self.root_identity
    }

    /// The marker names actually found, in the order `std::fs::read_dir` returned them - a
    /// caller that wants order-independent comparison normalizes on its own side, exactly as
    /// `cancellai_guardian::structural::LayoutSignature::new`/`cancellai_safety::LayoutSignature::
    /// new` already do.
    pub fn markers(&self) -> &[String] {
        &self.markers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::SystemIdentityObserver;

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let base = std::fs::canonicalize(std::env::temp_dir())
                .unwrap_or_else(|_| std::env::temp_dir());
            let dir = base.join(format!(
                "cancellai-provider-layout-test-{label}-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn observes_real_markers_with_trailing_slash_for_directories() {
        let dir = TempDir::new("markers");
        std::fs::create_dir_all(dir.0.join("sessions")).unwrap();
        std::fs::write(dir.0.join("config.json"), b"{}").unwrap();

        let observation = BoundLayoutObservation::observe(&dir.0, &SystemIdentityObserver)
            .expect("a real, existing directory must observe cleanly");

        let mut markers = observation.markers().to_vec();
        markers.sort();
        assert_eq!(
            markers,
            vec!["config.json".to_string(), "sessions/".to_string()]
        );
    }

    #[test]
    fn refuses_an_absent_root() {
        let dir = TempDir::new("absent-parent");
        let missing = dir.0.join("does-not-exist");
        let err = BoundLayoutObservation::observe(&missing, &SystemIdentityObserver)
            .expect_err("an absent root must not observe as an empty, clean layout");
        assert!(!err.0.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_an_unreadable_root() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new("unreadable");
        std::fs::set_permissions(&dir.0, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read_dir(&dir.0).is_ok() {
            std::fs::set_permissions(&dir.0, std::fs::Permissions::from_mode(0o755)).unwrap();
            eprintln!("skipped: this process can read a 0o000 directory (running as root?)");
            return;
        }

        let result = BoundLayoutObservation::observe(&dir.0, &SystemIdentityObserver);
        std::fs::set_permissions(&dir.0, std::fs::Permissions::from_mode(0o755)).unwrap();

        let err = result.expect_err("an unreadable root must not observe as an empty layout");
        assert!(!err.0.is_empty());
    }

    #[test]
    fn two_observations_of_the_same_real_root_carry_the_same_identity() {
        let dir = TempDir::new("stable-identity");
        std::fs::write(dir.0.join("config.json"), b"{}").unwrap();

        let first = BoundLayoutObservation::observe(&dir.0, &SystemIdentityObserver).unwrap();
        let second = BoundLayoutObservation::observe(&dir.0, &SystemIdentityObserver).unwrap();

        assert_eq!(first.root_identity(), second.root_identity());
    }
}

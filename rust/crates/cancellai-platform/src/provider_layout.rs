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
//!
//! **Round 5's own independent review, first pass** (`project/evidence/E14-S04-VERIFIER-REVIEW-
//! ROUND5.md`) found the first version of `observe` still forgeable: it took `identity_observer:
//! &dyn IdentityObserver` as a public parameter, and [`crate::identity::SyntheticIdentityObserver`]
//! is itself public API (needed elsewhere for legitimate testing). A caller could observe a
//! real, unrelated, favorably-shaped directory's markers while pairing them with a fabricated
//! [`IdentityToken`] equal to some *other*, genuinely drifted root's real identity.
//!
//! **Round 5's own independent review, second pass** (`project/evidence/E14-S04-VERIFIER-REVIEW-
//! ROUND5-PASS2.md`) found the repair for that still insufficient: identity
//! (`SystemIdentityObserver`, `symlink_metadata`) and markers (`std::fs::read_dir`) were two
//! separate path-based syscalls against `root`, admitting the identical class of defect via a
//! filesystem-level TOCTOU race instead of a public-API one - a same-user actor could swap the
//! real, drifted directory at `root` for a recognized one between the two calls, or plant a
//! symlink `read_dir` would silently follow while identity observed the link itself. "Read from
//! the same call" was true of the source text, not of the syscalls it issued.
//!
//! `observe` now binds `root` exactly once, via [`cancellai_sealedfs::SealedRoot::bind_existing`]
//! (ADR-0017): a handle-relative, `O_NOFOLLOW`-at-every-component walk that refuses a symlinked
//! root outright rather than following it, and holds one open directory descriptor for the
//! lifetime of the call. Both the identity (`SealedRoot::metadata`, `fstat` on the held
//! descriptor) and the marker listing (`SealedRoot::list_child_names`, `fdopendir`/`readdir` on
//! a duplicate of the same descriptor) are read from that one bound object - there is no path
//! lookup left, at any point after the initial bind, for a concurrent actor to redirect. This is
//! the same "retained handle, not a re-checked path" shape `cancellai-platform::mutation`'s own
//! confirmed-delete path already relies on for its unlink race.

use std::path::Path;

use crate::identity::IdentityToken;

#[cfg(unix)]
fn identity_from_sealed_metadata(
    meta: &std::fs::Metadata,
) -> Result<IdentityToken, LayoutObservationError> {
    use crate::fs_observer::modification_timestamp;
    use crate::identity::FileKind;
    use std::os::unix::fs::MetadataExt;

    let kind = if meta.is_dir() {
        FileKind::Directory
    } else if meta.is_file() {
        FileKind::File
    } else {
        FileKind::Other
    };
    match modification_timestamp(meta.modified()) {
        Ok(modified) => Ok(IdentityToken::Unix {
            device: meta.dev(),
            inode: meta.ino(),
            kind,
            modified,
            modified_nanos: meta.mtime_nsec() as u32,
        }),
        Err(reason) => Err(LayoutObservationError(format!(
            "could not represent the bound root's modification time: {reason}"
        ))),
    }
}

#[cfg(not(unix))]
fn identity_from_sealed_metadata(
    _meta: &std::fs::Metadata,
) -> Result<IdentityToken, LayoutObservationError> {
    // Unreachable in practice: `SealedRoot::metadata` itself fails closed with
    // `SealError::Unsupported` on every non-Unix platform today (no verified handle-bound
    // implementation yet, mirroring this crate's own `IdentityObservation::Unsupported`
    // precedent), so `observe` below never reaches this function on those platforms. Kept as a
    // real, honest refusal rather than an `unreachable!()` in case that stops being true for one
    // non-Unix platform before another.
    Err(LayoutObservationError(
        "no verified handle-bound identity conversion exists for this platform yet".to_string(),
    ))
}

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
    /// The only constructor, and it takes no observer parameter (round 5 independent review:
    /// accepting one, even as a trait object, let a caller pair real markers from one path with
    /// a fabricated identity for another - see the module doc). Identity and markers are always
    /// read from the same real `root`, via [`crate::identity::SystemIdentityObserver`]
    /// internally, so a caller has no seam to supply either fact independently.
    ///
    /// ```compile_fail
    /// // An external crate cannot substitute an observer to bind a fabricated identity to
    /// // real markers - there is no parameter for one.
    /// use cancellai_platform::provider_layout::BoundLayoutObservation;
    /// use std::path::Path;
    /// let _ = BoundLayoutObservation::observe(
    ///     Path::new("/tmp"),
    ///     &cancellai_platform::SyntheticIdentityObserver::new(),
    /// );
    /// ```
    pub fn observe(root: &Path) -> Result<Self, LayoutObservationError> {
        // No `canonicalize()` here, deliberately: resolving symlinks in `root` itself before
        // binding would silently follow exactly what `SealedRoot::bind_existing`'s own
        // component-by-component `O_NOFOLLOW` walk exists to refuse. `root` must already be
        // absolute and normalized (no `.`/`..`) - `bind_existing` reports `NotAbsolute`/
        // `PathNotNormalized` clearly if it is not, rather than this function silently
        // resolving it on the caller's behalf.
        let sealed = cancellai_sealedfs::SealedRoot::bind_existing(root).map_err(|e| {
            LayoutObservationError(format!("could not bind {}: {e}", root.display()))
        })?;

        let metadata = sealed.metadata().map_err(|e| {
            LayoutObservationError(format!(
                "could not read {}'s bound identity: {e}",
                root.display()
            ))
        })?;
        let root_identity = identity_from_sealed_metadata(&metadata)?;

        let children = sealed.list_child_names().map_err(|e| {
            LayoutObservationError(format!("could not list {}: {e}", root.display()))
        })?;
        let markers = children
            .into_iter()
            .map(|(name, is_dir)| if is_dir { format!("{name}/") } else { name })
            .collect();

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

        let observation = BoundLayoutObservation::observe(&dir.0)
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
        let err = BoundLayoutObservation::observe(&missing)
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

        let result = BoundLayoutObservation::observe(&dir.0);
        std::fs::set_permissions(&dir.0, std::fs::Permissions::from_mode(0o755)).unwrap();

        let err = result.expect_err("an unreadable root must not observe as an empty layout");
        assert!(!err.0.is_empty());
    }

    #[test]
    fn two_observations_of_the_same_real_root_carry_the_same_identity() {
        let dir = TempDir::new("stable-identity");
        std::fs::write(dir.0.join("config.json"), b"{}").unwrap();

        let first = BoundLayoutObservation::observe(&dir.0).unwrap();
        let second = BoundLayoutObservation::observe(&dir.0).unwrap();

        assert_eq!(first.root_identity(), second.root_identity());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_root_rather_than_following_it() {
        let dir = TempDir::new("symlinked-root");
        let real = dir.0.join("real");
        std::fs::create_dir_all(&real).unwrap();
        let link = dir.0.join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let err = BoundLayoutObservation::observe(&link)
            .expect_err("a symlinked root must be refused, not silently followed");
        assert!(!err.0.is_empty());
    }

    // `observe`'s actual immunity to a mid-call root-swap race (the exact defect E14-S04 round
    // 5's second independent review found) is proven at the primitive it now depends on:
    // `cancellai-sealedfs`'s own `metadata_and_list_child_names_survive_a_root_rename_after_
    // binding` reproduces the interleaving with a real rename between binding and reading, since
    // that crate holds the descriptor and this one does not expose a hook to interleave mid-call.
}

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

/// The bound root's identity, read from the handle `sealed` holds (`fstat` on Unix).
#[cfg(unix)]
fn bound_root_identity(
    sealed: &cancellai_sealedfs::SealedRoot,
    root: &Path,
) -> Result<IdentityToken, LayoutObservationError> {
    let metadata = sealed.metadata().map_err(|e| {
        LayoutObservationError(format!(
            "could not read {}'s bound identity: {e}",
            root.display()
        ))
    })?;
    identity_from_sealed_metadata(&metadata)
}

/// The bound root's identity on Windows, from `GetFileInformationByHandle` on the handle
/// `sealed` holds (E06-S13) - the same facts `SystemIdentityObserver` reports for a path, but
/// with no path lookup after the bind. Until E06-S13 this failed closed, so no Windows deletion
/// could pass the provider-layout check.
#[cfg(windows)]
fn bound_root_identity(
    sealed: &cancellai_sealedfs::SealedRoot,
    root: &Path,
) -> Result<IdentityToken, LayoutObservationError> {
    use crate::identity::{FileKind, windows_filetime_to_unix_timestamp};

    let facts = sealed.windows_facts().map_err(|e| {
        LayoutObservationError(format!(
            "could not read {}'s bound identity: {e}",
            root.display()
        ))
    })?;
    let kind = if facts.is_reparse_point {
        FileKind::Symlink
    } else if facts.is_directory {
        FileKind::Directory
    } else {
        FileKind::File
    };
    let (modified, modified_ticks) =
        windows_filetime_to_unix_timestamp(facts.last_write_time_ticks).ok_or_else(|| {
            LayoutObservationError(format!(
                "{}'s modification time predates the Unix epoch",
                root.display()
            ))
        })?;
    Ok(IdentityToken::Windows {
        volume_serial_number: facts.volume_serial_number,
        file_index: facts.file_index,
        kind,
        modified,
        modified_ticks: modified_ticks.into(),
    })
}

#[cfg(not(any(unix, windows)))]
fn bound_root_identity(
    _sealed: &cancellai_sealedfs::SealedRoot,
    _root: &Path,
) -> Result<IdentityToken, LayoutObservationError> {
    Err(LayoutObservationError(
        "no verified handle-bound identity exists for this platform".to_string(),
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
        let sealed =
            cancellai_sealedfs::SealedRoot::bind_existing_for_listing(root).map_err(|e| {
                LayoutObservationError(format!("could not bind {}: {e}", root.display()))
            })?;

        let root_identity = bound_root_identity(&sealed, root)?;

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

    /// Test-only, crate-private construction of an arbitrary observation - never exposed
    /// outside this crate, so nothing beyond [`SyntheticProviderLayoutObserver`] (same crate)
    /// can fabricate one; mirrors `cancellai_safety::trust_promotion::TrustedTier::for_tests`'s
    /// identical `pub(crate)` shape for the same reason (module docs above).
    pub(crate) fn for_tests(root_identity: IdentityToken, markers: Vec<String>) -> Self {
        Self {
            root_identity,
            markers,
        }
    }
}

/// Capability seam for obtaining a [`BoundLayoutObservation`] (E14-S05, SI-004, SI-013's own
/// "revalidate immediately before mutation" principle applied to the provider root, not only
/// the target artifact) - mirrors [`crate::IdentityObserver`]/[`crate::ProcessObserver`]:
/// production code is injected [`SystemProviderLayoutObserver`], the only implementation
/// `cancellai_safety::mutation_executor::execute_with_system_capabilities` is allowed to use;
/// tests use [`SyntheticProviderLayoutObserver`].
pub trait ProviderLayoutObserver: Send + Sync {
    fn observe(&self, root: &Path) -> Result<BoundLayoutObservation, LayoutObservationError>;
}

/// The real, OS-backed implementation - calls [`BoundLayoutObservation::observe`] and nothing
/// else, so every non-forgeability property that constructor's own module docs establish holds
/// unchanged through this seam.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemProviderLayoutObserver;

impl ProviderLayoutObserver for SystemProviderLayoutObserver {
    fn observe(&self, root: &Path) -> Result<BoundLayoutObservation, LayoutObservationError> {
        BoundLayoutObservation::observe(root)
    }
}

/// Test double: a fixed, path-keyed table of canned results, configured via [`Self::set`] -
/// the only way any crate outside this one can obtain a [`BoundLayoutObservation`] value at
/// all, since [`BoundLayoutObservation::for_tests`] itself stays `pub(crate)`. A path with no
/// configured response fails closed (`LayoutObservationError`), never a silent empty/clean
/// layout - the same "unconfigured is not evidence of absence" posture
/// [`crate::SyntheticIdentityObserver`] does not need (its own unset default,
/// `IdentityObservation::Absent`, is itself a legitimate real-world outcome; an unconfigured
/// layout observation has no such honest default).
#[derive(Debug, Default)]
pub struct SyntheticProviderLayoutObserver {
    responses: std::collections::BTreeMap<
        std::path::PathBuf,
        Result<(IdentityToken, Vec<String>), String>,
    >,
}

impl SyntheticProviderLayoutObserver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure `path` to observe as a clean, real reading: `root_identity` paired with
    /// `markers`, exactly what [`BoundLayoutObservation::observe`] would have produced for a
    /// real directory with that identity and those children.
    pub fn set_observed(
        &mut self,
        path: impl Into<std::path::PathBuf>,
        root_identity: IdentityToken,
        markers: impl IntoIterator<Item = impl Into<String>>,
    ) -> &mut Self {
        self.responses.insert(
            path.into(),
            Ok((root_identity, markers.into_iter().map(Into::into).collect())),
        );
        self
    }

    /// Configure `path` to fail observation (an absent root, a permission error, a platform
    /// with no verified handle-bound implementation, ...) - the caller-facing shape
    /// [`BoundLayoutObservation::observe`] itself would return.
    pub fn set_unobservable(
        &mut self,
        path: impl Into<std::path::PathBuf>,
        reason: impl Into<String>,
    ) -> &mut Self {
        self.responses.insert(path.into(), Err(reason.into()));
        self
    }
}

impl ProviderLayoutObserver for SyntheticProviderLayoutObserver {
    fn observe(&self, root: &Path) -> Result<BoundLayoutObservation, LayoutObservationError> {
        match self.responses.get(root) {
            Some(Ok((identity, markers))) => Ok(BoundLayoutObservation::for_tests(
                identity.clone(),
                markers.clone(),
            )),
            Some(Err(reason)) => Err(LayoutObservationError(reason.clone())),
            None => Err(LayoutObservationError(format!(
                "no synthetic provider-layout observation configured for {}",
                root.display()
            ))),
        }
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

    #[cfg(any(unix, windows))]
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

    #[cfg(any(unix, windows))]
    #[test]
    fn two_observations_of_the_same_real_root_carry_the_same_identity() {
        let dir = TempDir::new("stable-identity");
        std::fs::write(dir.0.join("config.json"), b"{}").unwrap();

        let first = BoundLayoutObservation::observe(&dir.0).unwrap();
        let second = BoundLayoutObservation::observe(&dir.0).unwrap();

        assert_eq!(first.root_identity(), second.root_identity());
    }

    /// E06-S13: a listing larger than one 64 KiB `GetFileInformationByHandleEx` buffer on
    /// Windows (and an ordinary large `readdir` on Unix) returns every entry exactly once.
    #[cfg(any(unix, windows))]
    #[test]
    fn a_listing_larger_than_one_buffer_returns_every_entry_once() {
        let dir = TempDir::new("large-listing");
        let expected: Vec<String> = (0..1500)
            .map(|index| format!("entry-with-a-deliberately-long-name-{index:05}.jsonl"))
            .collect();
        for name in &expected {
            std::fs::write(dir.0.join(name), b"").unwrap();
        }
        let observation = BoundLayoutObservation::observe(&dir.0).unwrap();
        let mut markers = observation.markers().to_vec();
        markers.sort();
        assert_eq!(markers, expected);
    }

    /// E06-S13: a directory symlink child is reported as a plain name, not as a directory -
    /// the Windows listing's reparse handling matches the Unix listing's `DT_LNK`.
    #[cfg(windows)]
    #[test]
    fn a_directory_symlink_child_is_not_reported_as_a_directory() {
        let dir = TempDir::new("reparse-child");
        let target = TempDir::new("reparse-target");
        if std::os::windows::fs::symlink_dir(&target.0, dir.0.join("linked")).is_err() {
            eprintln!("skipped: this process may not create directory symlinks");
            return;
        }
        let observation = BoundLayoutObservation::observe(&dir.0).unwrap();
        assert_eq!(observation.markers(), ["linked".to_string()]);
    }

    #[cfg(not(any(unix, windows)))]
    #[test]
    fn fails_closed_on_a_platform_with_no_verified_handle_bound_observation() {
        let dir = TempDir::new("non-unix-unsupported");
        std::fs::write(dir.0.join("config.json"), b"{}").unwrap();

        let err = BoundLayoutObservation::observe(&dir.0).expect_err(
            "a platform with no verified handle-bound observation must refuse, not silently \
             report an empty layout",
        );
        assert!(!err.0.is_empty());
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

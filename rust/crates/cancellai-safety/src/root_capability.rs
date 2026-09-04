//! Approved provider roots and boundary-checked paths as explicit capabilities, not raw
//! paths (E03-S03, `docs/architecture/PLATFORM_MODEL.md` "Boundary rules", SI-002, SI-003,
//! SI-018).
//!
//! PLATFORM_MODEL.md's boundary rules: never mutate the provider root itself; never escape
//! the approved root capability; crossing a filesystem/volume boundary is explicit and
//! normally prohibited for recursive mutation/quarantine. [`ApprovedRoot::bind`] is the one
//! place all three are enforced, and the only way to obtain a [`BoundedPath`] - there is no
//! public constructor for it otherwise. A future mutation API that takes `BoundedPath`
//! instead of `&Path`/`PathBuf` therefore cannot be called with an unconstrained raw path
//! (AC1/AC2): the type itself is the proof, not a runtime assertion a call site could forget.
//!
//! This layers with, rather than replaces, SI-013 (E03-S02's `revalidate`): `bind` resolves
//! the candidate through [`PathResolver`] (`docs/architecture/PLATFORM_MODEL.md`'s
//! canonicalization capability, which does resolve symlinks), so a candidate that already
//! escapes through a symlink component is rejected here at bind time. A symlink/mount swap
//! that happens *after* a successful bind is `revalidate`'s job to catch immediately before
//! mutation (E03-S05 wires the two together); this story does not claim to close that later
//! window by itself. Like `SealedPlan`, this crate performs no filesystem I/O of its own -
//! `PathResolver`/`IdentityObserver` are the only OS-facing calls, both owned by
//! `cancellai-platform`.

use std::path::{Path, PathBuf};

use cancellai_platform::{IdentityObservation, IdentityObserver, IdentityToken, PathResolver};

/// Why a candidate path was refused a [`BoundedPath`], or why a root could not be
/// [`ApprovedRoot::establish`]ed. Every variant is a refusal - there is no success case in
/// this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryError {
    /// The root's own identity could not be established; without it, nothing can be
    /// verified as being inside or outside it (SI-002 fails closed).
    RootIdentityUnavailable(String),
    /// The candidate, once canonicalized, targets the root object itself - never mutate the
    /// root (PLATFORM_MODEL.md).
    TargetsRootItself,
    /// The candidate, once canonicalized (symlinks resolved), is not inside the approved
    /// root at all (SI-003) - including a symlink inside the root that points outside it.
    EscapesRoot,
    /// The candidate path could not be resolved at all (e.g. a dangling symlink component).
    CandidatePathUnresolvable(String),
    /// The candidate does not exist.
    CandidateAbsent,
    /// The candidate could not be examined (permission/I/O failure).
    CandidateUnreadable(String),
    /// The platform cannot produce identity evidence strong enough to verify a boundary
    /// (SI-017) - refused rather than guessed.
    CandidateIdentityUnsupported(String),
    /// The candidate resolves onto a different filesystem/volume than the root (SI-018).
    CrossesFilesystemBoundary,
}

/// A provider root positively bound to the object identity observed for it (SI-002), not
/// merely a path string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedRoot {
    path: PathBuf,
    identity: IdentityToken,
}

impl ApprovedRoot {
    /// Establish an approved root capability. Fails closed if the root's own identity
    /// cannot be observed - there would be nothing to check candidates against otherwise.
    pub fn establish(
        path: &Path,
        resolver: &dyn PathResolver,
        observer: &dyn IdentityObserver,
    ) -> Result<Self, BoundaryError> {
        let canonical = resolver
            .canonicalize(path)
            .map_err(BoundaryError::RootIdentityUnavailable)?;
        match observer.observe(&canonical) {
            IdentityObservation::Identity(identity) => Ok(Self {
                path: canonical,
                identity,
            }),
            IdentityObservation::Absent => Err(BoundaryError::RootIdentityUnavailable(
                "root does not exist".to_string(),
            )),
            IdentityObservation::Unreadable { reason } => {
                Err(BoundaryError::RootIdentityUnavailable(reason))
            }
            IdentityObservation::Unsupported { reason } => {
                Err(BoundaryError::CandidateIdentityUnsupported(reason))
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn identity(&self) -> &IdentityToken {
        &self.identity
    }

    /// Bind a candidate path to this root, enforcing SI-002/SI-003/SI-018. The only way to
    /// obtain a [`BoundedPath`] under this root.
    pub fn bind(
        &self,
        candidate: &Path,
        resolver: &dyn PathResolver,
        observer: &dyn IdentityObserver,
    ) -> Result<BoundedPath, BoundaryError> {
        let canonical = resolver
            .canonicalize(candidate)
            .map_err(BoundaryError::CandidatePathUnresolvable)?;

        if canonical == self.path {
            return Err(BoundaryError::TargetsRootItself);
        }
        if !canonical.starts_with(&self.path) {
            return Err(BoundaryError::EscapesRoot);
        }

        match observer.observe(&canonical) {
            IdentityObservation::Identity(identity) => {
                if identity.device() != self.identity.device() {
                    return Err(BoundaryError::CrossesFilesystemBoundary);
                }
                Ok(BoundedPath {
                    path: canonical,
                    identity,
                    root_identity: self.identity.clone(),
                })
            }
            IdentityObservation::Absent => Err(BoundaryError::CandidateAbsent),
            IdentityObservation::Unreadable { reason } => {
                Err(BoundaryError::CandidateUnreadable(reason))
            }
            IdentityObservation::Unsupported { reason } => {
                Err(BoundaryError::CandidateIdentityUnsupported(reason))
            }
        }
    }
}

/// A path verified to lie inside an [`ApprovedRoot`], distinct from the root itself, and on
/// the same filesystem/volume as the root. The only public constructor is
/// [`ApprovedRoot::bind`].
///
/// Carries the *root's* identity at bind time (`root_identity`), not only the target's own
/// (`identity`) - E03 verifier review round 1 found nothing connected a `SealedPlan`'s
/// recorded root to the target it actually executed against, so a plan sealed for one root
/// could execute against a target bound under a completely different one.
/// `SealedPlan::seal`/`mutation_executor::execute` (E03-S02/E03-S05) compare this field
/// against the plan's own recorded root identity before ever considering a mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedPath {
    path: PathBuf,
    identity: IdentityToken,
    root_identity: IdentityToken,
}

impl BoundedPath {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn identity(&self) -> &IdentityToken {
        &self.identity
    }

    /// The identity of the [`ApprovedRoot`] this path was bound under (not the target's own
    /// identity - see the struct docs).
    pub fn root_identity(&self) -> &IdentityToken {
        &self.root_identity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_platform::{
        FileKind, SyntheticIdentityObserver, SystemIdentityObserver, SystemPathResolver, Timestamp,
    };

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-root-capability-test-{label}-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[cfg(unix)]
    #[test]
    fn bind_a_plain_child_succeeds() {
        let dir = TempDir::new("valid-child");
        let child = dir.path("file.txt");
        std::fs::write(&child, b"hello").expect("create file");

        let resolver = SystemPathResolver;
        let observer = SystemIdentityObserver;
        let root = ApprovedRoot::establish(&dir.0, &resolver, &observer).expect("establish root");
        let bound = root
            .bind(&child, &resolver, &observer)
            .expect("bind valid child");
        assert_eq!(bound.path(), std::fs::canonicalize(&child).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn bind_the_root_itself_is_rejected() {
        let dir = TempDir::new("root-self");
        let resolver = SystemPathResolver;
        let observer = SystemIdentityObserver;
        let root = ApprovedRoot::establish(&dir.0, &resolver, &observer).expect("establish root");
        let err = root
            .bind(&dir.0, &resolver, &observer)
            .expect_err("must reject the root itself");
        assert_eq!(err, BoundaryError::TargetsRootItself);
    }

    #[cfg(unix)]
    #[test]
    fn bind_a_path_outside_the_root_is_rejected() {
        let dir = TempDir::new("outside-a");
        let outside_dir = TempDir::new("outside-b");
        let outside_file = outside_dir.path("elsewhere.txt");
        std::fs::write(&outside_file, b"hello").expect("create outside file");

        let resolver = SystemPathResolver;
        let observer = SystemIdentityObserver;
        let root = ApprovedRoot::establish(&dir.0, &resolver, &observer).expect("establish root");
        let err = root
            .bind(&outside_file, &resolver, &observer)
            .expect_err("must reject a path outside the root");
        assert_eq!(err, BoundaryError::EscapesRoot);
    }

    #[cfg(unix)]
    #[test]
    fn bind_a_symlink_that_escapes_the_root_is_rejected() {
        // The adversarial case SI-003 exists for: a path that is lexically under the root
        // but, once resolved, points somewhere else entirely.
        let dir = TempDir::new("symlink-escape-root");
        let outside = TempDir::new("symlink-escape-target");
        let escape = dir.path("escape");
        std::os::unix::fs::symlink(&outside.0, &escape).expect("create escaping symlink");

        let resolver = SystemPathResolver;
        let observer = SystemIdentityObserver;
        let root = ApprovedRoot::establish(&dir.0, &resolver, &observer).expect("establish root");
        let err = root
            .bind(&escape, &resolver, &observer)
            .expect_err("a symlink escaping the root must be rejected, not silently followed");
        assert_eq!(err, BoundaryError::EscapesRoot);
    }

    #[test]
    fn establish_fails_when_the_root_does_not_exist() {
        let dir = TempDir::new("missing-root-parent");
        let missing = dir.path("does-not-exist");
        let resolver = SystemPathResolver;
        let observer = SystemIdentityObserver;
        let err = ApprovedRoot::establish(&missing, &resolver, &observer)
            .expect_err("missing root must fail closed");
        assert!(matches!(err, BoundaryError::RootIdentityUnavailable(_)));
    }

    #[cfg(unix)]
    #[test]
    fn bind_fails_when_the_candidate_no_longer_exists() {
        // A racy candidate: canonicalize needs the path to resolve, so simulate the
        // "vanished between listing and binding" case with a dangling symlink target
        // instead of a plain missing path (which canonicalize would already reject above).
        let dir = TempDir::new("candidate-vanished");
        let dangling = dir.path("dangling");
        std::os::unix::fs::symlink(dir.path("never-created"), &dangling)
            .expect("create dangling symlink");

        let resolver = SystemPathResolver;
        let observer = SystemIdentityObserver;
        let root = ApprovedRoot::establish(&dir.0, &resolver, &observer).expect("establish root");
        let err = root
            .bind(&dangling, &resolver, &observer)
            .expect_err("a candidate that cannot be resolved must fail closed");
        assert!(matches!(err, BoundaryError::CandidatePathUnresolvable(_)));
    }

    // --- Identity-observation branches and the mount-boundary case are exercised through a
    // synthetic observer: real files back the canonicalize step (which must succeed), but
    // the identity the boundary check reasons about is injected, exactly as E03-S01/E03-S02
    // use SyntheticIdentityObserver for scenarios a sandbox cannot construct for real (a
    // mount swap needs root privileges this test does not have).

    fn synthetic_token(device: u64) -> IdentityToken {
        IdentityToken::Unix {
            device,
            inode: 1,
            kind: FileKind::Directory,
            modified: Timestamp(0),
            modified_nanos: 0,
        }
    }

    #[test]
    fn bind_rejects_a_candidate_on_a_different_device_via_synthetic_identity() {
        let dir = TempDir::new("cross-device");
        let child = dir.path("child");
        std::fs::create_dir(&child).expect("create child dir");
        let root_canonical = std::fs::canonicalize(&dir.0).unwrap();
        let child_canonical = std::fs::canonicalize(&child).unwrap();

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(
            &root_canonical,
            IdentityObservation::Identity(synthetic_token(1)),
        );
        observer.set(
            &child_canonical,
            IdentityObservation::Identity(synthetic_token(2)), // a different device: a mount swap
        );

        let resolver = SystemPathResolver;
        let root = ApprovedRoot::establish(&dir.0, &resolver, &observer).expect("establish root");
        let err = root
            .bind(&child, &resolver, &observer)
            .expect_err("a different device must be rejected as a filesystem boundary crossing");
        assert_eq!(err, BoundaryError::CrossesFilesystemBoundary);
    }

    fn synthetic_windows_token(volume_serial_number: u32, file_index: u64) -> IdentityToken {
        IdentityToken::Windows {
            volume_serial_number,
            file_index,
            kind: FileKind::Directory,
            modified: Timestamp(0),
            modified_ticks: 0,
        }
    }

    #[test]
    fn bind_rejects_a_candidate_on_a_different_windows_volume_via_synthetic_identity() {
        // E20-S01 round-1 independent verifier review: the only cross-device boundary test
        // constructed Unix tokens, leaving SI-018's Windows arm
        // (`IdentityToken::device()`'s `Windows { volume_serial_number, .. }` case) completely
        // unexercised. This mirrors `bind_rejects_a_candidate_on_a_different_device_via_
        // synthetic_identity` above, but with Windows identity tokens on two different volume
        // serial numbers - the same synthetic-observer technique, since a real multi-volume
        // Windows machine is not available to this executor either.
        let dir = TempDir::new("cross-windows-volume");
        let child = dir.path("child");
        std::fs::create_dir(&child).expect("create child dir");
        let root_canonical = std::fs::canonicalize(&dir.0).unwrap();
        let child_canonical = std::fs::canonicalize(&child).unwrap();

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(
            &root_canonical,
            IdentityObservation::Identity(synthetic_windows_token(0x1111_2222, 1)),
        );
        observer.set(
            &child_canonical,
            // A different volume serial number: a Windows drive-letter/volume boundary.
            IdentityObservation::Identity(synthetic_windows_token(0x3333_4444, 2)),
        );

        let resolver = SystemPathResolver;
        let root = ApprovedRoot::establish(&dir.0, &resolver, &observer).expect("establish root");
        let err = root
            .bind(&child, &resolver, &observer)
            .expect_err("a different Windows volume must be rejected as a boundary crossing");
        assert_eq!(err, BoundaryError::CrossesFilesystemBoundary);
    }

    #[test]
    fn bind_accepts_a_candidate_on_the_same_windows_volume_via_synthetic_identity() {
        // The positive counterpart: two different Windows objects (different `file_index`) on
        // the *same* volume must not be rejected as a boundary crossing.
        let dir = TempDir::new("same-windows-volume");
        let child = dir.path("child");
        std::fs::create_dir(&child).expect("create child dir");
        let root_canonical = std::fs::canonicalize(&dir.0).unwrap();
        let child_canonical = std::fs::canonicalize(&child).unwrap();

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(
            &root_canonical,
            IdentityObservation::Identity(synthetic_windows_token(0x1111_2222, 1)),
        );
        observer.set(
            &child_canonical,
            IdentityObservation::Identity(synthetic_windows_token(0x1111_2222, 2)),
        );

        let resolver = SystemPathResolver;
        let root = ApprovedRoot::establish(&dir.0, &resolver, &observer).expect("establish root");
        root.bind(&child, &resolver, &observer)
            .expect("the same Windows volume must not be rejected as a boundary crossing");
    }

    #[test]
    fn bind_fails_closed_when_candidate_identity_is_unsupported() {
        let dir = TempDir::new("unsupported-candidate");
        let child = dir.path("child");
        std::fs::create_dir(&child).expect("create child dir");
        let root_canonical = std::fs::canonicalize(&dir.0).unwrap();
        let child_canonical = std::fs::canonicalize(&child).unwrap();

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(
            &root_canonical,
            IdentityObservation::Identity(synthetic_token(1)),
        );
        observer.set(
            &child_canonical,
            IdentityObservation::Unsupported {
                reason: "no verified Windows identity yet".into(),
            },
        );

        let resolver = SystemPathResolver;
        let root = ApprovedRoot::establish(&dir.0, &resolver, &observer).expect("establish root");
        let err = root
            .bind(&child, &resolver, &observer)
            .expect_err("Unsupported must never be treated as bindable");
        assert_eq!(
            err,
            BoundaryError::CandidateIdentityUnsupported("no verified Windows identity yet".into())
        );
    }

    #[test]
    fn establish_fails_closed_when_root_identity_is_unsupported() {
        let dir = TempDir::new("unsupported-root");
        let root_canonical = std::fs::canonicalize(&dir.0).unwrap();

        let mut observer = SyntheticIdentityObserver::new();
        observer.set(
            &root_canonical,
            IdentityObservation::Unsupported {
                reason: "no verified Windows identity yet".into(),
            },
        );

        let resolver = SystemPathResolver;
        let err = ApprovedRoot::establish(&dir.0, &resolver, &observer)
            .expect_err("Unsupported root must never be approved");
        assert!(matches!(
            err,
            BoundaryError::CandidateIdentityUnsupported(_)
        ));
    }
}

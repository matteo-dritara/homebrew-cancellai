//! A sealed, handle-relative directory capability (E07-S07 round-1 repair,
//! [ADR-0017](../../../docs/adrs/0017-sealed-root-handle-for-configuration-writes.md)).
//!
//! `cancellai-cli`'s `configure` command writes a vendor settings file inside a provider's
//! default root without going through `cancellai-safety`'s `ApprovedRoot`/`IdentityObserver`
//! machinery (SI-019's own docs explain why: it edits one JSON key in the provider's own
//! settings file, not a cancellAI-tracked artifact deletion). Before this crate existed, that
//! path checked `roots::is_symlink(&root)` once and then performed every read/write/rename
//! against the *path* `root.join("settings.json")` directly - `std::fs::create_dir_all`,
//! `read_to_string`, `OpenOptions::open`, `rename`. E07-S07's round-1 independent verifier
//! review found the gap that shape always has: the symlink check and the first write-side
//! path lookup are two separate syscalls, so a root that is a real directory at check time and
//! is atomically replaced with a symlink immediately afterward (a same-user attacker racing
//! the CLI, not a hypothetical) causes every following path-based operation to silently follow
//! the new link and write outside the approved root - violating SI-002/SI-003/SI-013/SI-019.
//!
//! The fix a re-check before use cannot provide, and only a *retained* capability can: open
//! the root exactly once, with `O_NOFOLLOW`, and perform every subsequent operation via
//! `openat(2)`/`renameat(2)` against that one held file descriptor rather than the original
//! path. The kernel resolves an `*at()` call relative to the directory the descriptor already
//! refers to, not whatever name currently sits at the path that produced it - so a rename or
//! symlink-swap of the root's own path, at any point after [`SealedRoot::establish`] returns,
//! cannot redirect a single byte this crate reads or writes. This is the same "retained
//! handle, not a re-checked path" shape `cancellai-platform::mutation`'s own module docs
//! describe wanting for its unlink race and explicitly could not build (no `unsafe`, no
//! reviewed FFI dependency existed yet) - that residual is unrelated to `configure` and is not
//! closed by this crate; see this crate's own repository docs for the cross-reference.
//!
//! `std` has no safe API for `openat`/`renameat` (only for opening a path directly, optionally
//! with `O_NOFOLLOW` via `OpenOptionsExt::custom_flags` - which *is* enough, unsafe-free, for
//! the initial directory open below). Handle-relative child operations need real libc FFI,
//! which needs `unsafe`. ADR-0015 anticipated exactly this ("OS-specific identity/reparse-point
//! handling in `cancellai-platform`, E07 ... isolated in a small, dedicated crate whose only
//! job is that unsafe boundary") without naming it in advance; this crate is that boundary.
//! [`libc`](https://crates.io/crates/libc) is used rather than hand-written `extern "C"`
//! syscall declarations - the ABI details it encodes (`mode_t`'s width differs between Linux
//! and macOS, for one) are exactly the kind of platform-specific detail a hand-rolled
//! declaration would risk getting subtly wrong in a security-boundary crate; `libc` is the
//! Rust project's own zero-dependency, MIT OR Apache-2.0 crate for this, already inside
//! `rust/deny.toml`'s license allow-list.
//!
//! Non-Unix platforms have no verified reparse-safe handle-relative implementation yet
//! (mirroring `cancellai-platform::identity`'s own `IdentityObservation::Unsupported`
//! precedent for the identical reason: a plausible-but-unverified safety-critical equality/
//! containment check is worse than an honest refusal, SI-017). [`SealedRoot::establish`] on
//! those platforms always fails closed rather than silently falling back to the unprotected
//! path-based operations this crate exists to replace.

use std::ffi::CString;
use std::io;

/// Why establishing or using a [`SealedRoot`] failed.
#[derive(Debug)]
pub enum SealError {
    /// The target's final path component is a symlink or platform reparse point - refused
    /// rather than followed, at every point this crate ever inspects it, not merely once at
    /// an earlier validation step.
    IsSymlinkOrReparsePoint,
    /// The target exists but is not a directory.
    NotADirectory,
    /// A child name violates this crate's own contract: empty, `.`/`..`, containing `/`, or
    /// containing an embedded NUL. Defense in depth - every current caller passes a
    /// constant/generated literal - so a future caller can never accidentally build a path
    /// that escapes the sealed directory through a crafted child name.
    InvalidChildName,
    /// No verified no-follow/handle-relative directory capability exists for this platform
    /// yet (see module docs); callers must fail closed rather than fall back to unprotected
    /// path-based operations.
    Unsupported(&'static str),
    Io(io::Error),
}

impl std::fmt::Display for SealError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SealError::IsSymlinkOrReparsePoint => {
                write!(
                    f,
                    "target is a symlink or reparse point; refusing to bind or traverse it"
                )
            }
            SealError::NotADirectory => write!(f, "target exists but is not a directory"),
            SealError::InvalidChildName => {
                write!(
                    f,
                    "invalid child name (must be a bare filename with no path separators)"
                )
            }
            SealError::Unsupported(reason) => write!(f, "unsupported on this platform: {reason}"),
            SealError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SealError {}

impl From<io::Error> for SealError {
    fn from(e: io::Error) -> Self {
        SealError::Io(e)
    }
}

/// A bare filename - no `/`, not `.`/`..`, not empty, no embedded NUL - ready for an `*at()`
/// call. See [`SealError::InvalidChildName`].
fn validate_child_name(name: &str) -> Result<CString, SealError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return Err(SealError::InvalidChildName);
    }
    CString::new(name).map_err(|_| SealError::InvalidChildName)
}

#[cfg(unix)]
pub use unix_impl::SealedRoot;

#[cfg(not(unix))]
pub use fallback_impl::SealedRoot;

#[cfg(unix)]
mod unix_impl {
    use super::{SealError, validate_child_name};
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::io::{AsRawFd, FromRawFd};
    use std::path::Path;

    /// A directory opened with `O_NOFOLLOW`, retained for the lifetime of every operation
    /// performed against it. See the crate module docs for why holding this descriptor -
    /// not re-checking the path before each use - is what actually closes the E07-S07
    /// round-1 TOCTOU.
    #[derive(Debug)]
    pub struct SealedRoot {
        dir: File,
    }

    impl SealedRoot {
        /// Binds `path` as a sealed root: creates it if absent, then opens it with
        /// `O_NOFOLLOW | O_DIRECTORY`. Creation is safe against a symlink pre-planted at
        /// `path` - `create_dir_all`'s underlying `mkdir(2)` fails outright (`EEXIST`)
        /// against anything already there, including a symlink; it never follows one to
        /// create inside it. The subsequent open is the actual authority boundary: even a
        /// caller that saw a real directory here microseconds ago gets nothing but a
        /// refusal if the name now resolves to a link.
        pub fn establish(path: &Path) -> Result<Self, SealError> {
            Self::establish_with_hook(path, || {})
        }

        /// `before_open` runs after the create-if-absent step and immediately before the
        /// authoritative open - solely so tests can deterministically reproduce "swapped
        /// after final validation, before the bind" without relying on real thread-timing
        /// luck, mirroring `cancellai-platform::mutation`'s own
        /// `confirmed_delete_file_inner` test hook for the analogous unlink race.
        fn establish_with_hook(path: &Path, before_open: impl FnOnce()) -> Result<Self, SealError> {
            match std::fs::symlink_metadata(path) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(SealError::IsSymlinkOrReparsePoint);
                }
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    std::fs::create_dir_all(path)?;
                }
                Err(e) => return Err(SealError::Io(e)),
            }

            before_open();

            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
                .open(path)
                .map_err(|e| {
                    // `O_NOFOLLOW | O_DIRECTORY` against a symlink is reported as `ELOOP` on
                    // Linux but as `ENOTDIR` on macOS/BSD (verified empirically - the kernel
                    // checks "is this a directory" before "was the final component followed",
                    // so refusing the follow makes it look like a non-directory instead of a
                    // link). `ENOTDIR` is otherwise genuinely ambiguous (a plain file also
                    // produces it), so on that code only, a follow-up `symlink_metadata`
                    // disambiguates which one this actually is - purely for accurate error
                    // classification: the open above has already unconditionally refused
                    // either way, this cannot reopen the race it closed.
                    match e.raw_os_error() {
                        Some(code) if code == libc::ELOOP => SealError::IsSymlinkOrReparsePoint,
                        Some(code) if code == libc::ENOTDIR => {
                            if std::fs::symlink_metadata(path)
                                .is_ok_and(|m| m.file_type().is_symlink())
                            {
                                SealError::IsSymlinkOrReparsePoint
                            } else {
                                SealError::NotADirectory
                            }
                        }
                        _ => SealError::Io(e),
                    }
                })?;
            if !file.metadata()?.is_dir() {
                return Err(SealError::NotADirectory);
            }
            Ok(SealedRoot { dir: file })
        }

        /// Reads a direct child file by name, relative to the held directory descriptor -
        /// never the root's original path. `Ok(None)` for a missing child. Deliberately
        /// *not* `O_NOFOLLOW`: a pre-existing `settings.json` that is itself a symlink is a
        /// separate, already-verified case (E06 verifier review round 1's
        /// `configure_never_writes_through_a_preexisting_settings_json_symlink_to_an_outside_
        /// file`) - the read side legitimately follows it, matching
        /// `std::fs::read_to_string`'s own behavior and `cancellai.py::configure_claude_
        /// retention`'s `settings.read_text()`; the safety property that matters is on the
        /// *write* side ([`write_new_child_atomically`]'s `O_EXCL` + `renameat`, which never
        /// follows a symlink at either name regardless of what this read did). Following a
        /// child symlink here is orthogonal to the E07-S07 round-1 root-directory TOCTOU this
        /// crate exists to close - it is still bound to the held `dirfd`, not the root's
        /// original path, which is the property that actually matters for that fix.
        pub fn read_child_to_string(&self, name: &str) -> Result<Option<String>, SealError> {
            let cname = validate_child_name(name)?;
            let dirfd = self.dir.as_raw_fd();
            // SAFETY: `dirfd` is a valid, open directory descriptor for the duration of this
            // call (borrowed from `self.dir`, which outlives it). `cname` is a
            // NUL-terminated bare filename (no `/`, not `.`/`..` - validated above), so this
            // cannot resolve outside the directory `dirfd` refers to, regardless of what its
            // original path now names.
            let fd =
                unsafe { libc::openat(dirfd, cname.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
            if fd < 0 {
                let e = io::Error::last_os_error();
                return if e.kind() == io::ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(SealError::Io(e))
                };
            }
            // SAFETY: `openat` returned a non-negative fd, which on success is a newly
            // allocated, exclusively-owned file descriptor; wrapping it here gives it
            // exactly one owning destructor.
            let mut file = unsafe { File::from_raw_fd(fd) };
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            Ok(Some(contents))
        }

        /// Writes `contents` to a new child `tmp_name` (`O_CREAT | O_EXCL`, refusing
        /// anything already present there - including a pre-planted symlink, exactly like
        /// the path-based `OpenOptions::create_new` this replaces), then atomically renames
        /// it to `final_name`. Both operations are issued against the held directory
        /// descriptor: a rename/symlink-swap of the root's own path, at any point during
        /// this call, cannot redirect either the create or the rename.
        pub fn write_new_child_atomically(
            &self,
            tmp_name: &str,
            final_name: &str,
            contents: &[u8],
        ) -> Result<(), SealError> {
            let tmp_c = validate_child_name(tmp_name)?;
            let final_c = validate_child_name(final_name)?;
            let dirfd = self.dir.as_raw_fd();

            // SAFETY: see `read_child_to_string` above for the `dirfd`/name-shape argument.
            // `O_CREAT | O_EXCL` refuses to open anything - regular file or symlink -
            // already present at `tmp_name`.
            let fd = unsafe {
                libc::openat(
                    dirfd,
                    tmp_c.as_ptr(),
                    libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd < 0 {
                return Err(SealError::Io(io::Error::last_os_error()));
            }
            // SAFETY: newly allocated, exclusively-owned fd on success, as above.
            let mut file = unsafe { File::from_raw_fd(fd) };
            let write_result = file.write_all(contents).and_then(|()| file.sync_all());
            drop(file);
            write_result?;

            // SAFETY: `dirfd` is held open and used as both the source and destination
            // directory for two names that are both direct children of the same sealed
            // directory. `renameat(2)`, like `rename(2)`, never follows a symlink at either
            // name - it replaces/creates the directory entry itself.
            let rc = unsafe { libc::renameat(dirfd, tmp_c.as_ptr(), dirfd, final_c.as_ptr()) };
            if rc != 0 {
                return Err(SealError::Io(io::Error::last_os_error()));
            }
            Ok(())
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
                let dir = std::env::temp_dir().join(format!(
                    "cancellai-sealedfs-test-{label}-{}-{unique}",
                    std::process::id()
                ));
                std::fs::create_dir_all(&dir).expect("create temp dir");
                Self(dir)
            }

            fn path(&self, name: &str) -> std::path::PathBuf {
                self.0.join(name)
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).ok();
            }
        }

        #[test]
        fn establish_binds_a_real_directory_and_round_trips_a_child_write_and_read() {
            let base = TempDir::new("round-trip");
            let root_path = base.path("root");
            let root = SealedRoot::establish(&root_path).expect("establish real directory");

            assert_eq!(root.read_child_to_string("settings.json").unwrap(), None);

            root.write_new_child_atomically("settings.json.tmp", "settings.json", b"{\"a\":1}")
                .expect("write should succeed");

            assert_eq!(
                root.read_child_to_string("settings.json").unwrap(),
                Some("{\"a\":1}".to_string())
            );
            assert!(
                !root_path.join("settings.json.tmp").exists(),
                "the temp name must not survive the rename"
            );
        }

        #[test]
        fn establish_creates_an_absent_root_before_binding_it() {
            let base = TempDir::new("create-if-absent");
            let root_path = base.path("does-not-exist-yet");
            assert!(!root_path.exists());

            let root = SealedRoot::establish(&root_path).expect("establish should create it");
            root.write_new_child_atomically("t", "f", b"x").unwrap();
            assert_eq!(std::fs::read_to_string(root_path.join("f")).unwrap(), "x");
        }

        #[test]
        fn establish_refuses_a_root_that_is_already_a_symlink() {
            let base = TempDir::new("already-symlink");
            let real = base.path("real");
            let link = base.path("link");
            std::fs::create_dir_all(&real).unwrap();
            std::os::unix::fs::symlink(&real, &link).unwrap();

            let err = SealedRoot::establish(&link).expect_err("a symlinked root must be refused");
            assert!(matches!(err, SealError::IsSymlinkOrReparsePoint));
        }

        #[test]
        fn establish_refuses_a_root_that_is_a_regular_file() {
            let base = TempDir::new("regular-file");
            let path = base.path("not-a-dir");
            std::fs::write(&path, b"hello").unwrap();

            let err = SealedRoot::establish(&path).expect_err("a plain file must be refused");
            assert!(matches!(err, SealError::NotADirectory));
        }

        #[test]
        fn establish_rejects_a_root_swapped_to_a_symlink_after_final_validation_but_before_the_bind()
         {
            // Exact reproduction of the E07-S07 round-1 independent verifier rejection: the
            // root exists as a real, empty directory when `establish`'s own pre-check runs
            // (matching the "classification saw a real default root" precondition), and is
            // then atomically replaced with a symlink to a directory outside the approved
            // root immediately before the authoritative bind - the interval the old
            // check-then-path-write code had no defense against at all. `establish` must
            // still refuse, and nothing may ever reach the outside sentinel.
            let base = TempDir::new("root-swap");
            let root_path = base.path("approved-root");
            std::fs::create_dir_all(&root_path).unwrap();
            let outside = base.path("outside");
            std::fs::create_dir_all(&outside).unwrap();
            let sentinel = outside.join("settings.json");

            let result = SealedRoot::establish_with_hook(&root_path, || {
                std::fs::remove_dir(&root_path)
                    .expect("remove the real (still-empty) root to make way for the swap");
                std::os::unix::fs::symlink(&outside, &root_path)
                    .expect("install the attacker symlink in its place");
            });

            assert!(
                result.is_err(),
                "a root swapped to a symlink immediately before the bind must be refused, not \
                 silently bound to the link target"
            );
            assert!(
                matches!(result.unwrap_err(), SealError::IsSymlinkOrReparsePoint),
                "the refusal reason must be the link itself, not an unrelated I/O failure"
            );
            assert!(
                !sentinel.exists(),
                "the outside sentinel must never be created - no write may follow the swapped \
                 link"
            );
        }

        #[test]
        fn write_new_child_atomically_refuses_a_pre_planted_symlink_at_the_temp_name() {
            let base = TempDir::new("tmp-symlink-race");
            let root_path = base.path("root");
            let root = SealedRoot::establish(&root_path).unwrap();
            let outside = base.path("outside-target");
            std::fs::write(&outside, b"do not touch").unwrap();
            std::os::unix::fs::symlink(&outside, root_path.join("t")).unwrap();

            let err = root
                .write_new_child_atomically("t", "f", b"payload")
                .expect_err("O_EXCL must refuse a name that already exists, symlink or not");
            assert!(matches!(err, SealError::Io(_)));
            assert_eq!(
                std::fs::read_to_string(&outside).unwrap(),
                "do not touch",
                "the pre-planted symlink's target must never be written through"
            );
        }

        #[test]
        fn read_child_to_string_follows_a_preexisting_symlink_child_matching_prior_behavior() {
            // Matches the already-verified `cancellai-cli` behavior (E06 verifier review
            // round 1): a pre-existing `settings.json` that is itself a symlink is read
            // through - only the write side refuses to follow it (see
            // `write_new_child_atomically_refuses_a_pre_planted_symlink_at_the_temp_name`
            // and `read_child_to_string`'s own doc comment for why this split is safe).
            let base = TempDir::new("read-symlink-child");
            let root_path = base.path("root");
            let root = SealedRoot::establish(&root_path).unwrap();
            let outside = base.path("secret");
            std::fs::write(&outside, b"secret contents").unwrap();
            std::os::unix::fs::symlink(&outside, root_path.join("settings.json")).unwrap();

            assert_eq!(
                root.read_child_to_string("settings.json").unwrap(),
                Some("secret contents".to_string())
            );
        }

        #[test]
        fn validate_child_name_rejects_escaping_and_malformed_names() {
            let base = TempDir::new("invalid-names");
            let root = SealedRoot::establish(&base.path("root")).unwrap();
            for bad in ["", ".", "..", "a/b", "/etc/passwd"] {
                assert!(
                    matches!(
                        root.read_child_to_string(bad),
                        Err(SealError::InvalidChildName)
                    ),
                    "expected InvalidChildName for {bad:?}"
                );
                assert!(
                    matches!(
                        root.write_new_child_atomically(bad, "f", b"x"),
                        Err(SealError::InvalidChildName)
                    ),
                    "expected InvalidChildName for {bad:?} as tmp_name"
                );
            }
        }
    }
}

#[cfg(not(unix))]
mod fallback_impl {
    use super::SealError;
    use std::path::Path;

    /// No verified no-follow/handle-relative directory capability exists for this platform
    /// yet - mirroring `cancellai-platform::identity`'s own `Unsupported` precedent for the
    /// identical reason (SI-017: an unverified safety-critical implementation is worse than
    /// an honest refusal). [`establish`](SealedRoot::establish) always fails closed; no
    /// instance of this type is ever constructed, so the remaining methods are unreachable
    /// by construction, not merely by convention.
    #[derive(Debug)]
    pub struct SealedRoot {
        _unreachable: std::convert::Infallible,
    }

    impl SealedRoot {
        pub fn establish(_path: &Path) -> Result<Self, SealError> {
            Err(SealError::Unsupported(
                "no verified no-follow/handle-relative directory capability exists for this \
                 platform yet (E07-S07 residual)",
            ))
        }

        pub fn read_child_to_string(&self, _name: &str) -> Result<Option<String>, SealError> {
            match self._unreachable {}
        }

        pub fn write_new_child_atomically(
            &self,
            _tmp_name: &str,
            _final_name: &str,
            _contents: &[u8],
        ) -> Result<(), SealError> {
            match self._unreachable {}
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn establish_always_fails_closed_on_a_platform_with_no_verified_handle_capability() {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-sealedfs-fallback-test-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let err = SealedRoot::establish(&dir)
                .expect_err("a platform with no verified capability must refuse, not proceed");
            assert!(matches!(err, SealError::Unsupported(_)));
            std::fs::remove_dir_all(&dir).ok();
        }
    }
}

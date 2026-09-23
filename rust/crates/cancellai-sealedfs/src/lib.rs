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
//!
//! ## Intermediate-component containment (E07-S09)
//!
//! E07-S07 round-1 closed the *final*-component race: the leaf was opened with `O_NOFOLLOW`, so
//! a symlink swap of the leaf itself is refused. E07-S07 round-2 independent verifier review
//! found that this left every *intermediate* path component unprotected: `establish`'s
//! pre-check (`std::fs::symlink_metadata(path)`) and the leaf's `OpenOptions::open(path)` both
//! resolve the *whole* path through the kernel's normal (link-following) name resolution before
//! `O_NOFOLLOW` is ever applied to the final component - so `$HOME` (or any directory between
//! the trusted anchor and the leaf) being itself a symlink to an attacker- or
//! operator-mistaken directory was silently followed, and a real, non-symlink `.claude`
//! directory underneath it was then sealed and mutated as if it were the approved root.
//!
//! `establish` now walks every path component handle-relatively from the filesystem root: open
//! `/` (which cannot itself be a symlink), then `openat` each subsequent component against the
//! descriptor already held for its parent, with `O_NOFOLLOW | O_DIRECTORY`, refusing the moment
//! any component - intermediate or final - turns out to be a link. Only the final component may
//! be created if absent, via `mkdirat` against the already-held parent descriptor (never
//! `create_dir_all`'s path-based, potentially link-following recursive creation). No component
//! in the chain is ever looked up twice through two different mechanisms the way the old
//! `symlink_metadata` + `OpenOptions::open(path)` pair was - each is opened exactly once,
//! relative to a descriptor already proven safe.

#[cfg(unix)]
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
    /// The child name still exists, but no longer refers to the object the caller confirmed.
    /// Refused rather than removed: deleting whatever happens to sit at a name is precisely the
    /// failure `cancellai-platform::mutation`'s identity confirmation exists to prevent.
    IdentityMismatch,
    /// A rename's destination name already refers to something. Refused rather than replaced:
    /// a quarantine move must never silently clobber an existing object at the destination
    /// (E12-S01).
    DestinationAlreadyExists,
    /// `establish` was given a relative path. There is no safe trusted anchor to walk a
    /// relative path from (it would resolve against the process's current directory, which
    /// this crate has no basis to trust) - refused rather than silently resolved against CWD.
    NotAbsolute,
    /// The path contains a `.` or `..` component. Resolving these safely would require the
    /// same kind of path-based, potentially link-following lookup this crate exists to avoid
    /// (`..` in particular cannot be walked handle-relatively without re-deriving a parent from
    /// a name, which is exactly the TOCTOU shape this crate closes elsewhere) - refused rather
    /// than resolved.
    PathNotNormalized,
    /// The child still names the confirmed object, but that object has other names too.
    /// Refused (E06-S13): a second hard link cannot be told apart from a decoy link planted
    /// beside a swapped-out directory, so only a single-name file is removed.
    MultipleLinks(u64),
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
            SealError::IdentityMismatch => write!(
                f,
                "the child no longer refers to the confirmed object; refusing to remove a \
                 different one"
            ),
            SealError::DestinationAlreadyExists => write!(
                f,
                "the destination name already refers to an existing object; refusing to \
                 replace it"
            ),
            SealError::NotAbsolute => write!(f, "root path must be absolute"),
            SealError::PathNotNormalized => {
                write!(f, "root path must not contain '.' or '..' components")
            }
            SealError::MultipleLinks(links) => write!(
                f,
                "the child has {links} links; refusing to remove one name of a hard-linked file"
            ),
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
/// call. See [`SealError::InvalidChildName`]. Unix-only: `fallback_impl`'s methods are all
/// unreachable (`match self._unreachable {}`), so this would be genuine dead code on a
/// platform where `unix_impl` never compiles in - found by real Windows CI (E07-S09), not
/// hypothesized.
#[cfg(unix)]
fn validate_child_name(name: &str) -> Result<CString, SealError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return Err(SealError::InvalidChildName);
    }
    CString::new(name).map_err(|_| SealError::InvalidChildName)
}

#[cfg(unix)]
pub use unix_impl::SealedRoot;

#[cfg(unix)]
pub use unix_impl::VerifiedPath;

#[cfg(windows)]
pub use windows_sealed::SealedRoot;

#[cfg(windows)]
pub use windows_sealed::VerifiedPath;

#[cfg(not(any(unix, windows)))]
pub use fallback_impl::SealedRoot;

#[cfg(not(any(unix, windows)))]
pub use fallback_impl::VerifiedPath;

#[cfg(unix)]
pub use unix_impl::verify_no_intermediate_links;

#[cfg(windows)]
pub use windows_sealed::verify_no_intermediate_links;

#[cfg(not(any(unix, windows)))]
pub use fallback_impl::verify_no_intermediate_links;

#[cfg(windows)]
mod windows_identity;

#[cfg(windows)]
pub use windows_identity::{WindowsFileFacts, observe_identity, open_and_observe_identity};

#[cfg(windows)]
mod windows_allocation;

#[cfg(windows)]
pub use windows_allocation::observe_allocated_size;

#[cfg(windows)]
mod windows_process;

#[cfg(windows)]
pub use windows_process::list_running_process_names;

#[cfg(windows)]
mod windows_sealed;

#[cfg(target_os = "macos")]
mod macos_filesystem;

#[cfg(target_os = "macos")]
pub use macos_filesystem::observe_filesystem_name;

#[cfg(unix)]
mod unix_impl {
    use super::{SealError, validate_child_name};
    use std::ffi::CString;
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
    use std::path::{Component, Path};

    /// A directory opened with `O_NOFOLLOW`, retained for the lifetime of every operation
    /// performed against it. See the crate module docs for why holding this descriptor -
    /// not re-checking the path before each use - is what actually closes the E07-S07
    /// round-1 TOCTOU.
    #[derive(Debug)]
    pub struct SealedRoot {
        dir: File,
    }

    /// Splits an absolute path into bare component names ready for handle-relative `*at()`
    /// calls, refusing anything that would need path-based (link-following) resolution to
    /// interpret safely: a relative path (no trusted anchor to walk it from) or a `.`/`..`
    /// component (resolving `..` handle-relatively would need to ask the parent's parent for a
    /// name, which is exactly the re-lookup shape this module exists to avoid). Built from raw
    /// bytes ([`OsStrExt::as_bytes`]), not `&str`, so a component need not be valid UTF-8 -
    /// Unix filenames are just byte strings.
    fn decompose_absolute_path(path: &Path) -> Result<Vec<CString>, SealError> {
        if !path.is_absolute() {
            return Err(SealError::NotAbsolute);
        }
        let mut out = Vec::new();
        for component in path.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(name) => {
                    out.push(
                        CString::new(name.as_bytes()).map_err(|_| SealError::InvalidChildName)?,
                    );
                }
                Component::CurDir | Component::ParentDir => {
                    return Err(SealError::PathNotNormalized);
                }
                Component::Prefix(_) => return Err(SealError::NotAbsolute),
            }
        }
        Ok(out)
    }

    /// Opens the filesystem root. `/` cannot itself be a symlink, so this is the one open in
    /// the walk with nothing upstream of it to have been swapped.
    fn open_root_dir() -> Result<File, SealError> {
        let root = CString::new("/").expect("literal has no embedded NUL");
        // SAFETY: `root` is a valid NUL-terminated string naming the filesystem root, which
        // always exists and is always a directory.
        let fd = unsafe {
            libc::open(
                root.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(SealError::Io(io::Error::last_os_error()));
        }
        // SAFETY: non-negative fd on success is newly allocated and exclusively owned here.
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    /// A pointer to the calling thread's `errno` storage - the platform-specific location the C
    /// library actually keeps it (glibc: `__errno_location`; the BSD family, including macOS:
    /// `__error`). Needed only by [`list_child_names`](SealedRoot::list_child_names): `readdir`
    /// returns a null pointer both at end-of-directory and on a real read failure, and POSIX's
    /// own contract for telling them apart is "the caller clears `errno` before the call and
    /// checks it after a null result", not a distinct return value.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    unsafe fn errno_location() -> *mut libc::c_int {
        // SAFETY: `__errno_location` takes no arguments and always returns a valid pointer to
        // the calling thread's own `errno` storage - it cannot fail or alias another thread's.
        unsafe { libc::__errno_location() }
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))]
    unsafe fn errno_location() -> *mut libc::c_int {
        // SAFETY: `__error` takes no arguments and always returns a valid pointer to the calling
        // thread's own `errno` storage - it cannot fail or alias another thread's.
        unsafe { libc::__error() }
    }

    /// `true` if `name`, looked up directly under `parent_fd` without following it, is a
    /// symlink. Used only to classify an ambiguous `ENOTDIR` error for accurate reporting
    /// (see [`open_child_dir_nofollow`]) - never as the containment check itself, which is the
    /// no-follow `openat` call that already ran.
    fn is_symlink_at(parent_fd: RawFd, name: &CString) -> bool {
        // SAFETY: `libc::stat` is a plain C aggregate of integers and fixed arrays with no
        // niche and no validity invariant, so the all-zero bit pattern is a valid value; the
        // `fstatat` call below overwrites it before anything reads it.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: `parent_fd` is a valid open directory descriptor for the call's duration;
        // `stat` is a valid, appropriately-sized out-parameter; `AT_SYMLINK_NOFOLLOW` makes
        // this itself a no-follow lookup, consistent with every other check in this module.
        let rc = unsafe {
            libc::fstatat(
                parent_fd,
                name.as_ptr(),
                &mut stat,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        rc == 0 && (stat.st_mode & libc::S_IFMT) == libc::S_IFLNK
    }

    /// Opens `name` as a subdirectory of the already-held `parent`, with `O_NOFOLLOW`: refuses
    /// outright if `name` is a symlink/reparse point rather than following it, regardless of
    /// whether it is an intermediate component or the final one. This is the one primitive
    /// [`SealedRoot::establish`]'s whole-path walk is built from.
    fn open_child_dir_nofollow(parent: &File, name: &CString) -> Result<File, SealError> {
        let parent_fd = parent.as_raw_fd();
        // SAFETY: `parent_fd` is a valid open directory descriptor for the call's duration
        // (borrowed from `parent`); `name` is a NUL-terminated bare component name. Resolution
        // happens relative to `parent_fd`'s own bound object, not any path, and `O_NOFOLLOW`
        // refuses rather than follows if `name` is itself a link.
        let fd = unsafe {
            libc::openat(
                parent_fd,
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd >= 0 {
            // SAFETY: non-negative fd on success is newly allocated and exclusively owned.
            return Ok(unsafe { File::from_raw_fd(fd) });
        }
        let e = io::Error::last_os_error();
        match e.raw_os_error() {
            // `O_NOFOLLOW | O_DIRECTORY` against a symlink is reported as `ELOOP` on Linux but
            // as `ENOTDIR` on macOS/BSD (verified empirically - the kernel checks "is this a
            // directory" before "was the final component followed", so refusing the follow
            // makes it look like a non-directory instead of a link). `ENOTDIR` is otherwise
            // genuinely ambiguous (a plain file also produces it), so on that code only, a
            // handle-relative `fstatat` disambiguates which one this actually is - purely for
            // accurate error classification: the `openat` above has already unconditionally
            // refused either way, this cannot reopen the race it closed.
            Some(code) if code == libc::ELOOP => Err(SealError::IsSymlinkOrReparsePoint),
            Some(code) if code == libc::ENOTDIR => {
                if is_symlink_at(parent_fd, name) {
                    Err(SealError::IsSymlinkOrReparsePoint)
                } else {
                    Err(SealError::NotADirectory)
                }
            }
            _ => Err(SealError::Io(e)),
        }
    }

    /// Walks every component of `path` handle-relatively from the filesystem root, refusing if
    /// any component - intermediate or the leaf itself - is a symlink/reparse point. Unlike
    /// [`SealedRoot::establish`], a missing component (including the leaf) is not created and
    /// is not itself an error: this exists purely to *prove a path contains no link
    /// indirection* before a different capability that does not hold a retained descriptor
    /// (e.g. `cancellai-safety::ApprovedRoot`, whose own `canonicalize()` step would otherwise
    /// silently resolve through one - the exact E07-S09 round-1 independent verifier review
    /// finding: `configure`'s repair did not extend to `clean`'s root establishment) takes over.
    /// The returned [`VerifiedPath`] retains the final directory descriptor. A caller that
    /// must subsequently canonicalize the path can compare that result's native identity with
    /// the held descriptor, so a component swap between this walk and that canonicalization
    /// is refused rather than accepted as a merely narrow revalidate-then-use window.
    #[derive(Debug)]
    pub struct VerifiedPath {
        dir: Option<File>,
    }

    impl VerifiedPath {
        /// Confirms that a separately-observed Unix identity still names the exact directory
        /// this no-follow walk bound. Keeping the descriptor alive until after the comparison
        /// closes the walk-then-canonicalize swap window: a replacement path cannot inherit
        /// the held object's device/inode pair merely by occupying the same name.
        pub fn matches_unix_identity(&self, device: u64, inode: u64) -> Result<bool, SealError> {
            use std::os::unix::fs::MetadataExt;

            let Some(dir) = &self.dir else {
                return Ok(false);
            };
            let metadata = dir.metadata()?;
            Ok(metadata.dev() == device && metadata.ino() == inode)
        }
    }

    pub fn verify_no_intermediate_links(path: &Path) -> Result<VerifiedPath, SealError> {
        let components = decompose_absolute_path(path)?;
        let mut current = open_root_dir()?;
        for name in &components {
            match open_child_dir_nofollow(&current, name) {
                Ok(dir) => current = dir,
                // A missing component means the leaf cannot exist either - nothing to protect,
                // and the caller's own subsequent establishment step will report the absence
                // with its own clear error.
                Err(SealError::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
                    return Ok(VerifiedPath { dir: None });
                }
                Err(e) => return Err(e),
            }
        }
        Ok(VerifiedPath { dir: Some(current) })
    }

    /// Renames `name` (a direct child of the directory `dirfd` refers to) to `dest_name` (a
    /// direct child of `dest_dirfd`), atomically refusing rather than replacing whatever
    /// already exists at the destination name (E12-S02 round-1 repair).
    ///
    /// Plain `renameat(2)` has no such flag - a caller wanting "rename only if the destination
    /// is absent" has always had to check absence first and rename second, and that pair is
    /// exactly the window an object created at the destination name in between defeats. Each
    /// platform this crate targets has its own atomic no-replace primitive instead
    /// (`renameat2`/`RENAME_NOREPLACE` on Linux, `renameatx_np`/`RENAME_EXCL` on macOS); where
    /// this platform has neither, this refuses outright rather than falling back to the unsafe
    /// check-then-rename shape - "refuse where the guarantee cannot be established," matching
    /// this crate's and `cancellai-platform`'s existing posture for every other unverified
    /// capability.
    #[cfg(target_os = "linux")]
    fn rename_no_replace(
        dirfd: RawFd,
        name: &CString,
        dest_dirfd: RawFd,
        dest_name: &CString,
    ) -> Result<(), SealError> {
        // SAFETY: `dirfd`/`dest_dirfd` are valid, held-open directory descriptors for the
        // call's duration; `name`/`dest_name` are validated bare filenames. `RENAME_NOREPLACE`
        // makes the kernel itself perform the absence check and the move as one atomic
        // operation, failing with `EEXIST` rather than replacing an object created at the
        // destination name after any earlier check a caller might otherwise have done.
        let rc = unsafe {
            libc::renameat2(
                dirfd,
                name.as_ptr(),
                dest_dirfd,
                dest_name.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if rc == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EEXIST) => Err(SealError::DestinationAlreadyExists),
            Some(libc::ENOSYS) | Some(libc::EINVAL) => Err(SealError::Unsupported(
                "atomic no-replace rename (renameat2/RENAME_NOREPLACE) is not available on \
                 this kernel or filesystem; refusing to move without that guarantee",
            )),
            _ => Err(SealError::Io(err)),
        }
    }

    /// macOS counterpart of the Linux `rename_no_replace` above: `renameatx_np` with
    /// `RENAME_EXCL` is Darwin's atomic no-replace rename, present since macOS 10.12.
    #[cfg(target_os = "macos")]
    fn rename_no_replace(
        dirfd: RawFd,
        name: &CString,
        dest_dirfd: RawFd,
        dest_name: &CString,
    ) -> Result<(), SealError> {
        // SAFETY: same argument as the Linux `renameat2` call above - valid, held-open
        // directory descriptors and validated bare filenames for the call's duration.
        // `RENAME_EXCL` is Darwin's equivalent atomic "fail if destination exists" flag.
        let rc = unsafe {
            libc::renameatx_np(
                dirfd,
                name.as_ptr(),
                dest_dirfd,
                dest_name.as_ptr(),
                libc::RENAME_EXCL,
            )
        };
        if rc == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EEXIST) => Err(SealError::DestinationAlreadyExists),
            Some(libc::ENOTSUP) | Some(libc::EINVAL) => Err(SealError::Unsupported(
                "atomic no-replace rename (renameatx_np/RENAME_EXCL) is not available on this \
                 filesystem; refusing to move without that guarantee",
            )),
            _ => Err(SealError::Io(err)),
        }
    }

    /// Every other Unix this crate might compile on (none currently CI-tested - see
    /// `docs/architecture/PLATFORM_MODEL.md`'s tier list) has no verified atomic no-replace
    /// rename here yet; refused rather than assumed, matching this module's own precedent for
    /// every other unverified platform capability.
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
    fn rename_no_replace(
        _dirfd: RawFd,
        _name: &CString,
        _dest_dirfd: RawFd,
        _dest_name: &CString,
    ) -> Result<(), SealError> {
        Err(SealError::Unsupported(
            "no verified atomic no-replace rename exists for this Unix platform yet",
        ))
    }

    impl SealedRoot {
        /// Binds `path` as a sealed root: walks every component handle-relatively from the
        /// filesystem root, creates the final component if absent, then holds it open with
        /// `O_NOFOLLOW | O_DIRECTORY`. See the crate module docs' "Intermediate-component
        /// containment" section for why every component - not only the leaf - must be walked
        /// this way.
        pub fn establish(path: &Path) -> Result<Self, SealError> {
            Self::establish_with_hook(path, || {})
        }

        /// `before_open` runs after every component up to and including the leaf's parent has
        /// been walked and held, and immediately before the leaf itself is opened/created -
        /// solely so tests can deterministically reproduce "swapped after the walk reached the
        /// parent, before the leaf is bound" without relying on real thread-timing luck,
        /// mirroring `cancellai-platform::mutation`'s own `confirmed_delete_file_inner` test
        /// hook for the analogous unlink race.
        fn establish_with_hook(path: &Path, before_open: impl FnOnce()) -> Result<Self, SealError> {
            let components = decompose_absolute_path(path)?;

            let mut current = open_root_dir()?;
            let Some((leaf, parents)) = components.split_last() else {
                // `path == "/"` itself. Not a real provider-root shape, but handled rather
                // than panicking: the already-open root descriptor is itself a valid sealed
                // root (it was opened directly, with no path-based lookup at all).
                return Ok(SealedRoot { dir: current });
            };

            for name in parents {
                current = open_child_dir_nofollow(&current, name)?;
            }

            before_open();

            match open_child_dir_nofollow(&current, leaf) {
                Ok(dir) => Ok(SealedRoot { dir }),
                Err(SealError::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
                    // Leaf absent: create it beneath the already-held parent descriptor - never
                    // `create_dir_all`, whose path-based recursive creation would re-open the
                    // exact link-following resolution this walk exists to avoid. `EEXIST` here
                    // means something was planted at this name after the lookup above and
                    // before this call (e.g. a concurrently-created real directory, or an
                    // attacker's symlink) - fall through to the no-follow open, which accepts
                    // the former and refuses the latter, rather than treating either as this
                    // call's own failure.
                    // SAFETY: `current` is a live open directory descriptor borrowed for this call, and
                    // `leaf` is a NUL-terminated bare filename validated upstream, so the relative lookup
                    // cannot leave that directory. `mkdirat` reads both and retains neither past the call.
                    let rc = unsafe { libc::mkdirat(current.as_raw_fd(), leaf.as_ptr(), 0o700) };
                    if rc != 0 {
                        let create_err = io::Error::last_os_error();
                        if create_err.raw_os_error() != Some(libc::EEXIST) {
                            return Err(SealError::Io(create_err));
                        }
                    }
                    open_child_dir_nofollow(&current, leaf).map(|dir| SealedRoot { dir })
                }
                Err(e) => Err(e),
            }
        }

        /// Binds an *existing* directory as a sealed root, walking every component
        /// handle-relatively exactly as [`establish`](Self::establish) does but never creating
        /// the leaf (E21-S07).
        ///
        /// `establish`'s create-if-absent behaviour is right for `configure`, which may
        /// legitimately need to create `~/.claude`. It is wrong for a deletion path: bringing a
        /// directory into existence as a side effect of removing a file inside it is not an
        /// operation anything should be able to perform by accident.
        pub fn bind_existing(path: &Path) -> Result<Self, SealError> {
            let components = decompose_absolute_path(path)?;
            let mut current = open_root_dir()?;
            for name in &components {
                current = open_child_dir_nofollow(&current, name)?;
            }
            Ok(SealedRoot { dir: current })
        }

        /// [`Self::bind_existing`]: on Unix a held directory descriptor can always be listed, so
        /// the Windows-specific need for a listing right on the leaf (E06-S13) does not arise.
        pub fn bind_existing_for_listing(path: &Path) -> Result<Self, SealError> {
            Self::bind_existing(path)
        }

        /// Removes a direct child by name, relative to the held directory descriptor, but only
        /// if that name still resolves - without following links - to the exact
        /// `(device, inode)` the caller confirmed (E21-S07, SI-013).
        ///
        /// This is the prevention half of `cancellai-platform::mutation`'s delete path. Its
        /// three checks used to be a held *file* descriptor plus two *path* lookups, so the
        /// object being unlinked was identified by a name resolved afresh through every
        /// intermediate directory, any of which could be swapped between the check and the
        /// call. Here both the identity check and the removal are issued against one directory
        /// descriptor that was opened once, with `O_NOFOLLOW` at every component: a rename or
        /// symlink-swap of any part of the path after `bind_existing` returned cannot redirect
        /// either of them.
        ///
        /// **Residual, stated rather than implied.** POSIX has no "unlink this entry only if it
        /// still points at this inode" primitive, so `fstatat` and `unlinkat` remain two
        /// syscalls. What this closes is the *directory* being swapped; what remains is an
        /// attacker with write access to that specific directory replacing the entry in the
        /// window between them. That is a strictly smaller surface than the path-based version,
        /// and `cancellai-platform::mutation` still holds its own open descriptor to the target
        /// file and re-checks its link count afterwards, so such a swap is detected even where
        /// it cannot be prevented.
        pub fn unlink_child_matching_unix_identity(
            &self,
            name: &str,
            device: u64,
            inode: u64,
        ) -> Result<(), SealError> {
            let cname = validate_child_name(name)?;
            let dirfd = self.dir.as_raw_fd();

            // SAFETY: `libc::stat` is a plain C aggregate of integers and fixed arrays with no
            // niche and no validity invariant, so the all-zero bit pattern is a valid value; the
            // `fstatat` call below overwrites it before anything reads it.
            let mut stat: libc::stat = unsafe { std::mem::zeroed() };
            // SAFETY: `dirfd` is a valid open directory descriptor for this call's duration
            // (borrowed from `self.dir`); `cname` is a NUL-terminated bare filename validated
            // above, so it cannot resolve outside the bound directory; `stat` is a valid,
            // appropriately-sized out-parameter. `AT_SYMLINK_NOFOLLOW` keeps this a no-follow
            // lookup, so a symlink planted at `name` is measured as the link itself and will
            // not match the caller's expected regular-file identity.
            let rc = unsafe {
                libc::fstatat(dirfd, cname.as_ptr(), &mut stat, libc::AT_SYMLINK_NOFOLLOW)
            };
            if rc != 0 {
                return Err(SealError::Io(io::Error::last_os_error()));
            }
            if stat.st_dev as u64 != device || stat.st_ino as u64 != inode {
                return Err(SealError::IdentityMismatch);
            }
            // E06-S13: the link count read in the same `fstatat` that confirms identity, as
            // late as this seam can read it. A link added after this and before `unlinkat` is
            // the same window as the owner-accepted leaf-name race.
            #[allow(clippy::unnecessary_cast)] // `st_nlink` is u16 on macOS, u64 on Linux.
            let links = stat.st_nlink as u64;
            if links != 1 {
                return Err(SealError::MultipleLinks(links));
            }

            // SAFETY: same invariants as the `fstatat` above - a valid borrowed directory
            // descriptor and a validated bare filename. `0` (no `AT_REMOVEDIR`) removes a
            // non-directory entry only, so this cannot remove a directory even if one appeared
            // at this name.
            let rc = unsafe { libc::unlinkat(dirfd, cname.as_ptr(), 0) };
            if rc != 0 {
                return Err(SealError::Io(io::Error::last_os_error()));
            }
            Ok(())
        }

        /// Moves a direct child by name to a direct child of `destination`, relative to both
        /// held directory descriptors, but only if the source name still resolves - without
        /// following links - to the exact `(device, inode)` the caller confirmed, and only if
        /// nothing already exists at the destination name (E12-S01).
        ///
        /// Mirrors [`Self::unlink_child_matching_unix_identity`]'s shape, extended to a second
        /// directory: both the source identity check and the destination-side move resolve
        /// their names relative to the directory descriptors that name them, never through a
        /// path - a rename or symlink-swap of either directory's own original path, at any
        /// point after `self`/`destination` were bound, cannot redirect this call.
        ///
        /// **E12-S02 round-1 verifier review** found that an earlier version of this function
        /// performed the destination-absence check and the rename as two separate syscalls
        /// (`fstatat` then `renameat`), which is not what this function's own doc claimed: for
        /// `Restore`, the destination is the artifact's *original provider location* - not
        /// cancellAI's own private store - so an object created there in the window between
        /// those two syscalls was silently replaced (SI-013). [`rename_no_replace`] closes that
        /// window with one atomic no-clobber rename syscall per platform
        /// (`renameat2`/`RENAME_NOREPLACE` on Linux, `renameatx_np`/`RENAME_EXCL` on macOS)
        /// instead of a check-then-act pair, and refuses outright - rather than falling back to
        /// the unsafe two-syscall shape - wherever that platform guarantee turns out to be
        /// unavailable (an old kernel, or a filesystem that does not implement the flag).
        pub fn rename_child_matching_unix_identity(
            &self,
            name: &str,
            destination: &SealedRoot,
            destination_name: &str,
            device: u64,
            inode: u64,
        ) -> Result<(), SealError> {
            let cname = validate_child_name(name)?;
            let dest_cname = validate_child_name(destination_name)?;
            let dirfd = self.dir.as_raw_fd();
            let dest_dirfd = destination.dir.as_raw_fd();

            // SAFETY: `libc::stat` is a plain C aggregate of integers and fixed arrays with no
            // niche and no validity invariant, so the all-zero bit pattern is a valid value;
            // the `fstatat` call below overwrites it before anything reads it.
            let mut stat: libc::stat = unsafe { std::mem::zeroed() };
            // SAFETY: `dirfd` is a valid open directory descriptor for this call's duration
            // (borrowed from `self.dir`); `cname` is a NUL-terminated bare filename validated
            // above, so it cannot resolve outside the bound directory; `stat` is a valid,
            // appropriately-sized out-parameter. `AT_SYMLINK_NOFOLLOW` keeps this a no-follow
            // lookup, so a symlink planted at `name` is measured as the link itself.
            let rc = unsafe {
                libc::fstatat(dirfd, cname.as_ptr(), &mut stat, libc::AT_SYMLINK_NOFOLLOW)
            };
            if rc != 0 {
                return Err(SealError::Io(io::Error::last_os_error()));
            }
            if stat.st_dev as u64 != device || stat.st_ino as u64 != inode {
                return Err(SealError::IdentityMismatch);
            }

            rename_no_replace(dirfd, &cname, dest_dirfd, &dest_cname)
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
            // E12-S01/E12-S03 round-3 repair: fsync the directory itself, not only the file's
            // own content above - on POSIX, a rename's directory-entry update is not guaranteed
            // durable across a crash until the directory holding it is fsynced too. Without
            // this, a crash immediately after the `renameat` above could still lose the rename
            // itself even though the file's bytes are safely on disk, which would defeat the
            // "durable, restart-recoverable" property this call exists to provide.
            self.dir.sync_all().map_err(SealError::Io)?;
            Ok(())
        }

        /// Renames an existing direct child `from_name` to `to_name`, both within this same
        /// held directory. No identity confirmation, unlike [`Self::rename_child_matching_
        /// unix_identity`]: both names always refer to cancellAI's own already-durable metadata
        /// sidecar (never user artifact content), so there is no caller-supplied identity to
        /// protect here - the same trust distinction [`Self::write_new_child_atomically`]'s own
        /// tmp-to-final rename already makes. A plain, replacing rename (E12-S01/E12-S03
        /// round-3 repair): finalizing a pending sidecar that was written durably *before* a
        /// move, once that move's own completion has been independently confirmed, so a retry
        /// of an interrupted finalize is safe to repeat.
        pub fn finalize_own_child(&self, from_name: &str, to_name: &str) -> Result<(), SealError> {
            let from_c = validate_child_name(from_name)?;
            let to_c = validate_child_name(to_name)?;
            let dirfd = self.dir.as_raw_fd();
            // SAFETY: `dirfd` is a valid, held-open directory descriptor for the call's
            // duration; `from_c`/`to_c` are validated bare filenames, so this cannot resolve
            // outside the directory `dirfd` refers to.
            let rc = unsafe { libc::renameat(dirfd, from_c.as_ptr(), dirfd, to_c.as_ptr()) };
            if rc != 0 {
                return Err(SealError::Io(io::Error::last_os_error()));
            }
            self.dir.sync_all().map_err(SealError::Io)?;
            Ok(())
        }

        /// Removes a direct child by name, unconditionally - no identity confirmation, for the
        /// same reason [`Self::finalize_own_child`] needs none: the name is always one of
        /// cancellAI's own just-written metadata files, never user content. Cleanup of a
        /// pending sidecar when the move it was written ahead of never happened (E12-S01/
        /// E12-S03 round-3 repair).
        pub fn remove_own_child(&self, name: &str) -> Result<(), SealError> {
            let cname = validate_child_name(name)?;
            let dirfd = self.dir.as_raw_fd();
            // SAFETY: `dirfd` is a valid, held-open directory descriptor for the call's
            // duration; `cname` is a validated bare filename, so this cannot resolve outside
            // the directory `dirfd` refers to. `0` (no `AT_REMOVEDIR`) removes a non-directory
            // entry only.
            let rc = unsafe { libc::unlinkat(dirfd, cname.as_ptr(), 0) };
            if rc != 0 {
                return Err(SealError::Io(io::Error::last_os_error()));
            }
            Ok(())
        }

        /// The held descriptor's own metadata (`fstat`, via `std::fs::File::metadata`) - never
        /// a fresh path-based lookup, so this always describes the exact object `self` was
        /// bound to, even if the path that produced it has since been renamed or replaced.
        /// E14-S04 round-5 second independent review's own finding: identity and a directory
        /// listing obtained from two separate path-based calls (`symlink_metadata` then
        /// `read_dir`) admit a swap between them; both must come from the same held object.
        /// [`Self::list_child_names`] is this method's listing counterpart, for exactly that
        /// reason.
        pub fn metadata(&self) -> Result<std::fs::Metadata, SealError> {
            self.dir.metadata().map_err(SealError::Io)
        }

        /// The direct children's bare names and whether each is a directory (per `d_type`,
        /// falling back to a handle-relative, no-follow `fstatat` when the filesystem does not
        /// report it), enumerated via a *duplicate* of the held descriptor - never a fresh
        /// `opendir(path)` - so this cannot be redirected by a rename/symlink-swap of `self`'s
        /// own original path any more than [`Self::metadata`] can. The duplicate (not `self.dir`
        /// itself) is necessary because `fdopendir` takes ownership of the descriptor it is
        /// given; `closedir` closing a fd `self.dir` still owns would be a double-close.
        ///
        /// A `readdir` failure partway through a stream and reaching the genuine end of it both
        /// return a null entry from `readdir` itself; this method clears `errno` before each
        /// call and checks it after a null result, per POSIX's own contract for telling the two
        /// apart, and returns an error rather than the entries collected so far. E14-S04 round-6
        /// independent review's finding: an earlier version of this method treated any null as
        /// end-of-stream, so a mid-enumeration failure could return a truncated-but-`Ok` listing,
        /// and its only production consumer decides a layout is "recognized" by exact signature
        /// equality, so a truncated listing could coincidentally equal a known-good one. The
        /// `DT_UNKNOWN` `fstatat` fallback is held to the same standard: a failed lookup there is
        /// also a failed listing, never a silently-assumed "not a directory".
        pub fn list_child_names(&self) -> Result<Vec<(String, bool)>, SealError> {
            self.list_child_names_with_hook(|_count, _dup_fd| {})
        }

        /// `after_entry` runs once per real (non-`.`/`..`) entry collected, immediately after
        /// it is pushed and before the next `readdir` call, with the running count and the raw
        /// duplicated descriptor `readdir` is reading from - solely so tests can deterministically
        /// force the *next* `readdir` call to fail after a chosen number of real entries (e.g. one
        /// matching a known signature), reproducing the exact mid-enumeration failure E14-S04
        /// round 6/7 independent review required a regression for, without relying on real
        /// scheduler timing. In production ([`Self::list_child_names`]) this hook is a no-op.
        fn list_child_names_with_hook(
            &self,
            mut after_entry: impl FnMut(usize, RawFd),
        ) -> Result<Vec<(String, bool)>, SealError> {
            let duplicate = self.dir.try_clone().map_err(SealError::Io)?;
            let dup_fd = duplicate.into_raw_fd();

            // SAFETY: `dup_fd` was just obtained from `File::try_clone().into_raw_fd()`, so it
            // is a valid, exclusively-owned, open directory descriptor that no other code holds
            // a `File`/`RawFd` handle to. `fdopendir` takes ownership of it on success; on
            // failure ownership was never transferred, so it is closed explicitly below via
            // `File::from_raw_fd` (reconstructing and dropping a `File` runs its own `close`).
            let dirp = unsafe { libc::fdopendir(dup_fd) };
            if dirp.is_null() {
                let err = io::Error::last_os_error();
                // SAFETY: `dup_fd` is still exclusively owned here (ownership was never
                // transferred to a null `fdopendir` result); reconstructing a `File` from it
                // and letting it drop closes it exactly once.
                drop(unsafe { File::from_raw_fd(dup_fd) });
                return Err(SealError::Io(err));
            }

            struct DirGuard(*mut libc::DIR);
            impl Drop for DirGuard {
                fn drop(&mut self) {
                    // SAFETY: `self.0` is a non-null `DIR*` obtained from a successful
                    // `fdopendir` and not yet closed - `DirGuard` is the sole owner and this is
                    // its only `closedir` call.
                    unsafe {
                        libc::closedir(self.0);
                    }
                }
            }
            let guard = DirGuard(dirp);
            let dirfd = self.dir.as_raw_fd();

            let mut entries = Vec::new();
            loop {
                // SAFETY: `errno_location()` returns a valid pointer to this thread's own
                // `errno` storage. POSIX's `readdir` contract requires clearing `errno` before
                // the call to tell a real read failure (null result, `errno` left nonzero) apart
                // from genuine end-of-directory (null result, `errno` still zero) - `readdir`
                // itself has no other way to report the difference.
                unsafe {
                    *errno_location() = 0;
                }
                // SAFETY: `guard.0` is the live `DIR*` this loop owns for its duration;
                // `readdir` is safe to call repeatedly on it until it returns null.
                let entry = unsafe { libc::readdir(guard.0) };
                if entry.is_null() {
                    // SAFETY: same pointer as the clear immediately above this call.
                    let errno = unsafe { *errno_location() };
                    if errno != 0 {
                        return Err(SealError::Io(io::Error::from_raw_os_error(errno)));
                    }
                    break;
                }
                // SAFETY: `entry` was just returned non-null by `readdir` and is valid until the
                // next `readdir`/`closedir` call on `guard.0`, neither of which happens before
                // this read completes. `d_name` is a NUL-terminated array per POSIX.
                let name_cstr = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) };
                let name = name_cstr.to_string_lossy().into_owned();
                if name == "." || name == ".." {
                    continue;
                }
                // SAFETY: same `entry` validity as above; `d_type` is a plain byte field.
                let d_type = unsafe { (*entry).d_type };
                let is_dir = if d_type == libc::DT_DIR {
                    true
                } else if d_type == libc::DT_UNKNOWN {
                    // SAFETY: `libc::stat` is a plain C aggregate with no niche/validity
                    // invariant; `fstatat` overwrites it before anything reads it.
                    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
                    // SAFETY: `dirfd` is `self.dir`'s own borrowed descriptor, live for this
                    // call; `name_cstr` names a child this same `readdir` just reported, so the
                    // lookup cannot resolve outside the bound directory. `AT_SYMLINK_NOFOLLOW`
                    // keeps this a no-follow lookup, matching every other identity check in this
                    // crate - a symlink is reported as itself, not its target's kind.
                    let rc = unsafe {
                        libc::fstatat(
                            dirfd,
                            name_cstr.as_ptr(),
                            &mut stat,
                            libc::AT_SYMLINK_NOFOLLOW,
                        )
                    };
                    if rc != 0 {
                        // The filesystem left this entry's kind unclassified (`DT_UNKNOWN`) and
                        // the fallback lookup meant to resolve it also failed - report that
                        // honestly as a failed listing (round-6 independent review's finding: a
                        // silently-defaulted `false`/"not a directory" here let an incomplete
                        // enumeration still return `Ok`, which its caller could not distinguish
                        // from a genuinely complete one). A concurrent removal of this exact
                        // entry between `readdir` reporting it and this lookup is the most likely
                        // real cause; that is itself missing evidence, not a fact to guess at.
                        return Err(SealError::Io(io::Error::last_os_error()));
                    }
                    (stat.st_mode & libc::S_IFMT) == libc::S_IFDIR
                } else {
                    false
                };
                entries.push((name, is_dir));
                after_entry(entries.len(), dup_fd);
            }

            Ok(entries)
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
                // Canonicalize once, here, on a path this test harness itself just created -
                // not a security-relevant resolution. Without it, macOS's `/tmp`/`/var`
                // compatibility symlinks (`/var` -> `/private/var`, and `std::env::temp_dir()`
                // returns a `/var/folders/...` path there) would make every test below fail:
                // `establish`'s handle-relative walk correctly refuses *any* symlink component,
                // including OS-level ones no attacker in this test's threat model controls.
                let dir = std::fs::canonicalize(&dir).expect("canonicalize temp dir");
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
        fn establish_refuses_a_root_reached_through_an_intermediate_symlink_component() {
            // E07-S07 round-2 independent verifier reproduction, tracked as E07-S09: the
            // *leaf* is a real directory, but an intermediate component - standing in for
            // `$HOME` in the real `configure --claude-retention` counterexample - is itself a
            // symlink to an outside location. The round-1 fix bound only the leaf with
            // `O_NOFOLLOW`; it never inspected anything above it, so the leaf opened
            // successfully and the outside directory it actually lived in was sealed and
            // written through. The whole-path handle-relative walk must refuse this before
            // ever reaching the leaf.
            let base = TempDir::new("intermediate-symlink");
            let outside = base.path("outside");
            let outside_leaf = outside.join("leaf");
            std::fs::create_dir_all(&outside_leaf).unwrap();
            let sentinel = outside_leaf.join("settings.json");

            let home_like = base.path("home-like");
            std::os::unix::fs::symlink(&outside, &home_like).unwrap();
            let root_path = home_like.join("leaf");

            let err = SealedRoot::establish(&root_path)
                .expect_err("a root reached through an intermediate symlink must be refused");
            assert!(
                matches!(err, SealError::IsSymlinkOrReparsePoint),
                "the refusal reason must be the intermediate link itself, got {err:?}"
            );
            assert!(
                !sentinel.exists(),
                "no write may ever reach the outside directory through the intermediate link"
            );
        }

        #[test]
        fn establish_refuses_a_relative_path() {
            let err = SealedRoot::establish(Path::new("relative/path"))
                .expect_err("a relative path has no trusted anchor to walk from");
            assert!(matches!(err, SealError::NotAbsolute));
        }

        #[test]
        fn establish_refuses_a_path_containing_dot_dot() {
            let base = TempDir::new("dot-dot");
            let root_path = base.path("real").join("..").join("real");
            let err = SealedRoot::establish(&root_path)
                .expect_err("a `..` component must be refused, not resolved");
            assert!(matches!(err, SealError::PathNotNormalized));
        }

        #[test]
        fn verify_no_intermediate_links_refuses_an_intermediate_symlink() {
            // The E07-S09 round-1 independent verifier reproduction, one layer up: `clean`
            // calls this (not `SealedRoot::establish`) ahead of `ApprovedRoot::establish`,
            // which would otherwise silently canonicalize through the same intermediate link.
            let base = TempDir::new("verify-intermediate-symlink");
            let outside = base.path("outside");
            let outside_leaf = outside.join("leaf");
            std::fs::create_dir_all(&outside_leaf).unwrap();

            let home_like = base.path("home-like");
            std::os::unix::fs::symlink(&outside, &home_like).unwrap();
            let root_path = home_like.join("leaf");

            let err = verify_no_intermediate_links(&root_path)
                .expect_err("an intermediate symlink must be refused");
            assert!(matches!(err, SealError::IsSymlinkOrReparsePoint));
        }

        #[test]
        fn verify_no_intermediate_links_refuses_a_symlinked_leaf_too() {
            let base = TempDir::new("verify-symlink-leaf");
            let real = base.path("real");
            let link = base.path("link");
            std::fs::create_dir_all(&real).unwrap();
            std::os::unix::fs::symlink(&real, &link).unwrap();

            let err = verify_no_intermediate_links(&link)
                .expect_err("a symlinked leaf must be refused, not only intermediate links");
            assert!(matches!(err, SealError::IsSymlinkOrReparsePoint));
        }

        #[test]
        fn verify_no_intermediate_links_accepts_a_real_path_and_creates_nothing() {
            let base = TempDir::new("verify-real-path");
            let root_path = base.path("real-root");
            std::fs::create_dir_all(&root_path).unwrap();

            verify_no_intermediate_links(&root_path).expect("a real, link-free path must pass");
        }

        #[test]
        fn verify_no_intermediate_links_treats_a_missing_leaf_as_ok_and_creates_nothing() {
            // Unlike `establish`, this must never create anything - `clean` has no business
            // materializing a provider root that does not exist. The caller's own subsequent
            // `ApprovedRoot::establish` reports the absence with its own clear error.
            let base = TempDir::new("verify-missing-leaf");
            let root_path = base.path("does-not-exist");

            verify_no_intermediate_links(&root_path)
                .expect("a missing leaf is not this function's concern");
            assert!(
                !root_path.exists(),
                "verify_no_intermediate_links must never create the path it checks"
            );
        }

        #[test]
        fn verified_path_detects_a_component_swapped_after_the_walk() {
            use std::os::unix::fs::MetadataExt;

            let base = TempDir::new("verify-post-walk-swap");
            let home = base.path("home");
            let root_path = home.join(".claude");
            std::fs::create_dir_all(&root_path).unwrap();
            let verified =
                verify_no_intermediate_links(&root_path).expect("the initial real path must bind");

            let original_home = base.path("original-home");
            std::fs::rename(&home, &original_home).unwrap();
            let outside = base.path("outside");
            std::fs::create_dir_all(outside.join(".claude")).unwrap();
            std::os::unix::fs::symlink(&outside, &home).unwrap();

            let replacement = std::fs::metadata(&root_path).unwrap();
            assert!(
                !verified
                    .matches_unix_identity(replacement.dev(), replacement.ino())
                    .unwrap(),
                "the held no-follow directory must not match the replacement reached by a \
                 component symlink planted after the walk"
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

        fn unix_identity(path: &std::path::Path) -> (u64, u64) {
            use std::os::unix::fs::MetadataExt;
            let meta = std::fs::symlink_metadata(path).expect("stat path for test identity");
            (meta.dev(), meta.ino())
        }

        #[test]
        fn rename_child_matching_unix_identity_moves_across_directories() {
            let base = TempDir::new("rename-cross-dir");
            let source_root = SealedRoot::establish(&base.path("source")).unwrap();
            let dest_root = SealedRoot::establish(&base.path("dest")).unwrap();
            let source_path = base.path("source").join("artifact.txt");
            std::fs::write(&source_path, b"hello").unwrap();
            let (device, inode) = unix_identity(&source_path);

            source_root
                .rename_child_matching_unix_identity(
                    "artifact.txt",
                    &dest_root,
                    "quarantined",
                    device,
                    inode,
                )
                .expect("cross-directory rename should succeed");

            assert!(
                !source_path.exists(),
                "the source name must no longer exist"
            );
            assert_eq!(
                std::fs::read_to_string(base.path("dest").join("quarantined")).unwrap(),
                "hello"
            );
        }

        #[test]
        fn rename_child_matching_unix_identity_refuses_when_source_identity_changed() {
            let base = TempDir::new("rename-stale-identity");
            let source_root = SealedRoot::establish(&base.path("source")).unwrap();
            let dest_root = SealedRoot::establish(&base.path("dest")).unwrap();
            let source_path = base.path("source").join("artifact.txt");
            std::fs::write(&source_path, b"hello").unwrap();
            let (device, _inode) = unix_identity(&source_path);

            let err = source_root
                .rename_child_matching_unix_identity(
                    "artifact.txt",
                    &dest_root,
                    "quarantined",
                    device,
                    999_999, // wrong inode: stands in for "identity changed since confirmation"
                )
                .expect_err("a mismatched expected identity must refuse the move");
            assert!(matches!(err, SealError::IdentityMismatch));
            assert!(
                source_path.exists(),
                "the source must survive a refused move"
            );
        }

        #[test]
        fn rename_child_matching_unix_identity_refuses_to_clobber_an_existing_destination() {
            let base = TempDir::new("rename-destination-exists");
            let source_root = SealedRoot::establish(&base.path("source")).unwrap();
            let dest_root = SealedRoot::establish(&base.path("dest")).unwrap();
            let source_path = base.path("source").join("artifact.txt");
            std::fs::write(&source_path, b"hello").unwrap();
            std::fs::write(base.path("dest").join("quarantined"), b"already here").unwrap();
            let (device, inode) = unix_identity(&source_path);

            let err = source_root
                .rename_child_matching_unix_identity(
                    "artifact.txt",
                    &dest_root,
                    "quarantined",
                    device,
                    inode,
                )
                .expect_err("an existing destination name must never be silently replaced");
            assert!(matches!(err, SealError::DestinationAlreadyExists));
            assert!(
                source_path.exists(),
                "the source must survive a refused move"
            );
            assert_eq!(
                std::fs::read_to_string(base.path("dest").join("quarantined")).unwrap(),
                "already here",
                "the pre-existing destination object must be untouched"
            );
        }

        #[test]
        fn list_child_names_reports_real_direct_children_and_kinds() {
            let base = TempDir::new("list-child-names");
            std::fs::create_dir_all(base.path("root/inner")).unwrap();
            std::fs::write(base.path("root/config.json"), b"{}").unwrap();
            let root = SealedRoot::establish(&base.path("root")).unwrap();

            let mut entries = root
                .list_child_names()
                .expect("list a real, real directory");
            entries.sort();
            assert_eq!(
                entries,
                vec![
                    ("config.json".to_string(), false),
                    ("inner".to_string(), true),
                ]
            );
        }

        #[test]
        fn list_child_names_excludes_dot_and_dotdot() {
            let base = TempDir::new("list-child-names-no-dots");
            let root = SealedRoot::establish(&base.path("root")).unwrap();
            let entries = root.list_child_names().unwrap();
            assert!(entries.iter().all(|(name, _)| name != "." && name != ".."));
        }

        #[test]
        fn list_child_names_fails_when_the_descriptor_is_already_broken_before_the_call() {
            // A narrower, earlier failure point than the mid-enumeration one below: the
            // descriptor is invalid before `list_child_names` ever calls `try_clone`. Kept
            // alongside `..._reports_a_readdir_failure_after_a_real_entry_not_a_truncated_ok`
            // (round-7 independent review's finding: this test alone does not reach `readdir`'s
            // own errno handling, since `try_clone` itself already fails first) because both are
            // real, distinct ways this method can be asked to read a broken directory.
            let base = TempDir::new("list-child-names-broken-fd");
            std::fs::create_dir_all(base.path("root")).unwrap();
            std::fs::write(base.path("root/marker"), b"x").unwrap();
            let root = SealedRoot::establish(&base.path("root")).unwrap();

            let raw_fd = root.dir.as_raw_fd();
            // SAFETY: `raw_fd` is `root.dir`'s own descriptor. Closing it directly, bypassing
            // `File`'s normal ownership, deliberately invalidates it for this test - the point
            // is to observe how `list_child_names` reacts to a descriptor that stops being
            // usable, matching a real failure mode `try_clone`/`fdopendir`/`readdir` can all
            // meet in production.
            unsafe {
                libc::close(raw_fd);
            }

            let result = root.list_child_names();
            // `root.dir` now wraps an already-closed fd; letting `root` drop normally would
            // make `std`'s I/O-safety check abort the process (a real double-close it correctly
            // refuses to perform silently). `root` is never used again after the assertion
            // below, so suppressing its `Drop` here via `ManuallyDrop` - test-only, and only
            // after the one call this test exists to exercise - is the honest way to end this
            // specific reproduction, not a resource leak this process would ever repeat outside
            // this test.
            let _ = std::mem::ManuallyDrop::new(root);
            assert!(
                result.is_err(),
                "a broken descriptor must be reported as a failure, never as a successful, \
                 possibly-truncated listing: got {result:?}"
            );
        }

        #[test]
        fn list_child_names_reports_a_readdir_failure_after_a_real_entry_not_a_truncated_ok() {
            // The exact class round 6/7 independent review required a regression for: a
            // directory whose enumeration starts successfully - `readdir` returns a real,
            // recognized-signature-shaped entry - and then fails, rather than reaching genuine
            // end-of-stream. The prior code could not distinguish that failure from EOF, so it
            // returned `Ok(vec![the one real entry])`, exactly the shape a caller comparing
            // against a known signature could mistake for a complete, matching layout.
            //
            // `list_child_names_with_hook`'s `after_entry` callback fires once per real entry,
            // with the exact raw descriptor `readdir` is reading from, letting this test close
            // that descriptor deterministically after the single entry this directory contains
            // - forcing the *next* `readdir` call (which would otherwise return null for
            // genuine end-of-stream) to instead fail with `EBADF`, a real, nonzero `errno`.
            let base = TempDir::new("list-child-names-mid-stream-failure");
            std::fs::create_dir_all(base.path("root")).unwrap();
            // `readdir`'s real implementations fill an internal buffer from the kernel in
            // batches, so closing the descriptor after only one or two entries is not
            // guaranteed to force a *new* syscall before this small a directory's true
            // end-of-stream is already known from the batch already read. A few hundred entries
            // reliably exceeds one batch on every platform this workspace targets, so the
            // descriptor closed partway through is guaranteed to be needed again before EOF.
            for i in 0..500 {
                std::fs::write(base.path(&format!("root/file-{i:04}")), b"").unwrap();
            }
            let root = SealedRoot::establish(&base.path("root")).unwrap();

            let mut closed = false;
            let result = root.list_child_names_with_hook(|_count, dup_fd| {
                if !closed {
                    closed = true;
                    // SAFETY: `dup_fd` is the raw descriptor `list_child_names_with_hook` is
                    // currently reading `readdir` entries from, handed to this hook for exactly
                    // this purpose. Closing it here, between two `readdir` calls, deterministically
                    // turns a later call into a real failure instead of relying on unrelated
                    // concurrent activity to race a close against it.
                    unsafe {
                        libc::close(dup_fd);
                    }
                }
            });

            assert!(
                result.is_err(),
                "a real readdir failure after a genuine entry must be reported as an error, \
                 never as a successful listing containing only the entries collected so far \
                 (which a caller could mistake for a complete, matching layout): got {result:?}"
            );
        }

        #[test]
        fn metadata_matches_a_real_fstat_of_the_same_object() {
            use std::os::unix::fs::MetadataExt;

            let base = TempDir::new("metadata");
            let root = SealedRoot::establish(&base.path("root")).unwrap();
            let via_sealed = root.metadata().expect("fstat the held descriptor");
            let via_path = std::fs::metadata(base.path("root")).unwrap();
            assert_eq!(via_sealed.dev(), via_path.dev());
            assert_eq!(via_sealed.ino(), via_path.ino());
            assert!(via_sealed.is_dir());
        }

        #[test]
        fn metadata_and_list_child_names_survive_a_root_rename_after_binding() {
            use std::os::unix::fs::MetadataExt;

            // The exact property E14-S04 round 5's second independent review required: identity
            // and a directory listing obtained after a root has been renamed away from its
            // original path must still describe the object `establish` actually bound, not
            // whatever a fresh path-based lookup would now find at the old (or a replacement)
            // path - the TOCTOU a path-based `symlink_metadata` + `read_dir` pair cannot close.
            let base = TempDir::new("rename-after-bind");
            std::fs::create_dir_all(base.path("root")).unwrap();
            std::fs::write(base.path("root/config.json"), b"{}").unwrap();
            let root = SealedRoot::establish(&base.path("root")).unwrap();
            let identity_before = root.metadata().unwrap();

            let moved_away = base.path("root-moved-away");
            std::fs::rename(base.path("root"), &moved_away).unwrap();
            std::fs::create_dir_all(base.path("root")).unwrap();
            std::fs::write(base.path("root/decoy.txt"), b"decoy").unwrap();

            let identity_after = root.metadata().unwrap();
            let entries = root.list_child_names().unwrap();

            assert_eq!(identity_before.ino(), identity_after.ino());
            assert_eq!(
                entries,
                vec![("config.json".to_string(), false)],
                "the sealed root must still list the real, original object's contents, not the \
                 replacement planted at its old path"
            );
        }
    }
}

#[cfg(not(any(unix, windows)))]
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

    #[derive(Debug)]
    pub struct VerifiedPath {
        _unreachable: std::convert::Infallible,
    }

    impl VerifiedPath {
        pub fn matches_unix_identity(&self, _device: u64, _inode: u64) -> Result<bool, SealError> {
            match self._unreachable {}
        }
    }

    impl SealedRoot {
        pub fn bind_existing(_path: &Path) -> Result<Self, SealError> {
            Err(SealError::Unsupported(
                "no verified no-follow, handle-relative directory binding exists for this \
                 platform yet (see the crate module docs)",
            ))
        }

        pub fn bind_existing_for_listing(path: &Path) -> Result<Self, SealError> {
            Self::bind_existing(path)
        }

        pub fn unlink_child_matching_unix_identity(
            &self,
            _name: &str,
            _device: u64,
            _inode: u64,
        ) -> Result<(), SealError> {
            Err(SealError::Unsupported(
                "no verified no-follow, handle-relative unlink exists for this platform yet",
            ))
        }

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

        pub fn metadata(&self) -> Result<std::fs::Metadata, SealError> {
            match self._unreachable {}
        }

        pub fn list_child_names(&self) -> Result<Vec<(String, bool)>, SealError> {
            match self._unreachable {}
        }
    }

    /// Mirrors [`SealedRoot::establish`]'s fail-closed posture: no verified handle-relative
    /// walk exists on this platform, so a caller cannot be told "no intermediate link found"
    /// with any real confidence - refuse rather than claim a check that was not actually
    /// performed.
    pub fn verify_no_intermediate_links(_path: &Path) -> Result<VerifiedPath, SealError> {
        Err(SealError::Unsupported(
            "no verified no-follow/handle-relative directory capability exists for this \
             platform yet (E07-S07 residual)",
        ))
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

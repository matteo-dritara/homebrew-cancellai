//! Windows handle-relative, no-follow directory establishment and identity-confirmed deletion
//! (E20-S05, extending ADR-0020) - the Windows counterpart of [`crate::unix_impl`].
//!
//! Mirrors `unix_impl`'s own reasoning exactly: open a trusted anchor once, then walk every
//! subsequent path component relative to the descriptor already held for its parent, refusing
//! outright the moment any component - intermediate or final - turns out to be a reparse point.
//! A rename or symlink/junction-swap of any path component after the walk completes cannot
//! redirect a single operation this module performs, because there is no path left to redirect
//! - exactly the property `unix_impl`'s `openat`/`O_NOFOLLOW` walk has.
//!
//! Ordinary `CreateFileW` cannot build this: `FILE_FLAG_OPEN_REPARSE_POINT` only stops it from
//! following a reparse point at the *final* path component - Windows' own documented path
//! resolution still transparently follows a reparse point at any *intermediate* component
//! regardless of that flag, the exact gap E07-S09 closed on Unix with `O_NOFOLLOW` at every
//! component, not only the leaf. No Win32-level flag closes that gap for Windows either; only
//! `NtCreateFile`'s `RootDirectory` field does, by resolving `ObjectName` relative to an
//! already-open, already-verified directory object rather than re-parsing a path string.
//!
//! `NtCreateFile` is `ntdll.dll`'s native NT API entry point, not a documented Win32 function -
//! `windows-sys`'s `Wdk` module is Microsoft's own generated binding for it (the same
//! code-generation pipeline as the rest of this dependency, per ADR-0020's own reasoning for
//! choosing `windows-sys` over a hand-transcribed `extern` declaration in the first place, now
//! extended to the `Wdk` half of the same crate rather than a second dependency).
//!
//! The trusted anchor is a drive root (e.g. `C:\`), opened with an ordinary, safe
//! `std::fs::OpenOptions` call: a drive root cannot itself be a reparse point on Windows (there
//! is no "symlinked volume root" the way there can be a bind-mounted `/` on Unix), mirroring
//! `unix_impl::open_root_dir`'s identical trust in `/`.

use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::path::{Component, Path, PathBuf, Prefix};

use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_FOR_BACKUP_INTENT,
    FILE_OPEN_IF, FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
};
use windows_sys::Win32::Foundation::{
    HANDLE, OBJ_CASE_INSENSITIVE, RtlNtStatusToDosError, STATUS_SUCCESS, UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES,
    FILE_READ_DATA, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO,
    FILE_TRAVERSE, FileDispositionInfo, FileStandardInfo, GetFileInformationByHandleEx,
    SYNCHRONIZE, SetFileInformationByHandle,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

use crate::SealError;
use crate::windows_identity::observe_identity_of_handle;

/// A bare filename - no `/`, `\`, `.`/`..`, empty, or embedded NUL. The Windows analogue of
/// `unix_impl::validate_child_name`, not shared with it (Windows forbids the *opposite* slash
/// too, and `NtCreateFile`'s `ObjectName` must never itself contain a separator - `RootDirectory`
/// is exactly what lets a single component resolve relative to the held parent instead).
fn validate_child_name(name: &str) -> Result<Vec<u16>, SealError> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        return Err(SealError::InvalidChildName);
    }
    Ok(name.encode_utf16().collect())
}

/// Splits an absolute Windows path into its drive-root anchor (e.g. `C:\`, opened directly and
/// safely - see module docs) and the bare component names between it and the leaf. Refuses a
/// relative path, a UNC/device path (`\\server\share`, `\\?\...` - out of scope: every real
/// provider root this workspace resolves is a local drive path, and those prefix kinds carry
/// materially different identity/boundary semantics this module has not verified), and any
/// `.`/`..` component (resolving one handle-relatively would need to ask the parent's parent
/// for a name, the exact re-lookup shape this module exists to avoid - matching
/// `unix_impl::decompose_absolute_path`'s identical refusal).
fn decompose_absolute_path(path: &Path) -> Result<(PathBuf, Vec<Vec<u16>>), SealError> {
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(SealError::NotAbsolute);
    };
    if !matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) {
        return Err(SealError::NotAbsolute);
    }
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(SealError::NotAbsolute);
    }
    let mut names = Vec::new();
    for component in components {
        match component {
            Component::Normal(name) => {
                let name = name.to_str().ok_or(SealError::InvalidChildName)?;
                names.push(validate_child_name(name)?);
            }
            Component::CurDir | Component::ParentDir => {
                return Err(SealError::PathNotNormalized);
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(SealError::PathNotNormalized);
            }
        }
    }
    let anchor = PathBuf::from(format!("{}\\", prefix.as_os_str().to_string_lossy()));
    Ok((anchor, names))
}

/// Opens the drive root named by `anchor` (e.g. `C:\`). Safe, path-based, and deliberately not
/// `unsafe`/handle-relative - see module docs for why a drive root needs neither.
fn open_anchor(anchor: &Path) -> Result<File, SealError> {
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(anchor)
        .map_err(SealError::Io)
}

/// One `NtCreateFile` call, relative to `parent`'s held handle, with `FILE_OPEN_REPARSE_POINT`
/// so a reparse point at `name` is opened as itself rather than followed. `directory` selects
/// `FILE_DIRECTORY_FILE` (this walk's normal case) or `FILE_NON_DIRECTORY_FILE` (opening a
/// leaf artifact file for deletion, never a traversal step). `disposition` is `FILE_OPEN`
/// (must already exist) or `FILE_OPEN_IF` (open-or-create, `SealedRoot::establish`'s leaf
/// only) - `NtCreateFile` supports "open or create" as one atomic disposition, unlike Unix's
/// separate `openat`-then-`mkdirat`-then-reopen dance.
///
/// Desired access is deliberately narrow: `FILE_READ_ATTRIBUTES` (every caller immediately
/// runs `observe_identity_of_handle`'s `GetFileInformationByHandle` on the result) plus
/// `FILE_TRAVERSE` (so the returned handle can itself serve as the next hop's `RootDirectory`)
/// and `SYNCHRONIZE` (required by `FILE_SYNCHRONOUS_IO_NONALERT`). This module's first real-
/// Windows-CI run (E20-S05) requested `FILE_LIST_DIRECTORY` here too - a "list this
/// directory's contents" right this crate never actually exercises (it only ever opens one
/// *named* child at a time, never enumerates) - and every intermediate hop through a directory
/// this process does not own (e.g. a CI runner's own workspace ancestor directories, not
/// created by this crate's own test fixtures) failed with `ERROR_ACCESS_DENIED`: unlike
/// `FILE_TRAVERSE` (bypassed for virtually every real-world token via the default-granted
/// "bypass traverse checking" privilege), `FILE_LIST_DIRECTORY` is a real, non-bypassed ACL
/// check against the object being opened, and ordinary path-based resolution (`std::fs`,
/// which this bug's own regression tests rely on to set up their fixtures) never requests it
/// on intermediate components at all - only this crate's own per-component walk did, because
/// nothing in this module actually needed it. Real Windows CI (not local cross-compilation)
/// is what caught this; `windows_sealed.rs`'s own unit tests happened to only ever walk
/// directories this same process created and therefore owns, masking the gap.
fn nt_open_child(
    parent: &File,
    name: &[u16],
    directory: bool,
    disposition: u32,
    extra_access: u32,
) -> Result<File, SealError> {
    let mut wide = name.to_vec();
    let byte_len = u16::try_from(wide.len() * 2).map_err(|_| SealError::InvalidChildName)?;
    let unicode_name = UNICODE_STRING {
        Length: byte_len,
        MaximumLength: byte_len,
        Buffer: wide.as_mut_ptr(),
    };
    let object_attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &unicode_name,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut handle: HANDLE = std::ptr::null_mut();
    let mut iosb: IO_STATUS_BLOCK = unsafe { std::mem::zeroed() };
    let create_options = if directory {
        FILE_DIRECTORY_FILE
    } else {
        FILE_NON_DIRECTORY_FILE
    } | FILE_SYNCHRONOUS_IO_NONALERT
        | FILE_OPEN_FOR_BACKUP_INTENT
        | FILE_OPEN_REPARSE_POINT;

    // SAFETY: `object_attributes.RootDirectory` is `parent`'s handle, valid and open for the
    // duration of this call (borrowed from `parent`, which outlives it); `ObjectName` points at
    // `unicode_name`, itself pointing at `wide`, both stack/local values kept alive across the
    // call. `name` is a single path component (validated by every caller via
    // `validate_child_name` - no separators), so `NtCreateFile` resolves it relative to
    // `RootDirectory` alone, never re-parsing a multi-component path. `handle`/`iosb` are
    // valid, correctly-sized out-parameters. `NtCreateFile` performs no allocation this code
    // must free and retains no pointer past the call.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            FILE_READ_ATTRIBUTES | FILE_TRAVERSE | SYNCHRONIZE | extra_access,
            &object_attributes,
            &mut iosb,
            std::ptr::null(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            disposition,
            create_options,
            std::ptr::null(),
            0,
        )
    };
    if status != STATUS_SUCCESS {
        // SAFETY: `status` is the exact `NTSTATUS` `NtCreateFile` just returned above; this
        // call performs a pure, allocation-free translation and has no other precondition.
        let win32_error = unsafe { RtlNtStatusToDosError(status) };
        return Err(SealError::Io(io::Error::from_raw_os_error(
            win32_error as i32,
        )));
    }
    // SAFETY: `status == STATUS_SUCCESS`, so `handle` is a newly allocated HANDLE this call
    // exclusively owns - wrapping it in a `File` gives it exactly one owning `Drop`.
    Ok(unsafe { File::from_raw_handle(handle as RawHandle) })
}

/// Opens `name` as a subdirectory of `parent`, refusing outright if it is a reparse point or
/// not a directory - the one primitive [`SealedRoot::establish`]'s/[`bind_existing`]'s whole-
/// path walks are built from, mirroring `unix_impl::open_child_dir_nofollow` exactly.
fn open_child_dir_nofollow(
    parent: &File,
    name: &[u16],
    disposition: u32,
) -> Result<File, SealError> {
    let file = nt_open_child(parent, name, true, disposition, 0)?;
    let facts = observe_identity_of_handle(file.as_raw_handle()).map_err(SealError::Io)?;
    if facts.is_reparse_point {
        return Err(SealError::IsSymlinkOrReparsePoint);
    }
    if !facts.is_directory {
        return Err(SealError::NotADirectory);
    }
    Ok(file)
}

/// A directory bound by a retained, handle-relative walk from the drive root - see module docs.
#[derive(Debug)]
pub struct SealedRoot {
    dir: File,
}

/// The Windows counterpart of `unix_impl::VerifiedPath` - see [`verify_no_intermediate_links`].
#[derive(Debug)]
pub struct VerifiedPath {
    dir: Option<File>,
}

impl VerifiedPath {
    /// Confirms that a separately-observed Windows identity still names the exact object this
    /// no-follow walk bound - the Windows counterpart of `unix_impl::VerifiedPath::
    /// matches_unix_identity`.
    pub fn matches_windows_identity(
        &self,
        volume_serial_number: u32,
        file_index: u64,
    ) -> Result<bool, SealError> {
        let Some(dir) = &self.dir else {
            return Ok(false);
        };
        let facts = observe_identity_of_handle(dir.as_raw_handle()).map_err(SealError::Io)?;
        Ok(facts.volume_serial_number == volume_serial_number && facts.file_index == file_index)
    }
}

/// Walks every component of `path` handle-relatively from the drive root, refusing if any
/// component is a reparse point. A missing component (including the leaf) is not an error -
/// see `unix_impl::verify_no_intermediate_links`'s identical contract and rationale.
pub fn verify_no_intermediate_links(path: &Path) -> Result<VerifiedPath, SealError> {
    let (anchor, names) = decompose_absolute_path(path)?;
    let mut current = open_anchor(&anchor)?;
    for name in &names {
        match open_child_dir_nofollow(&current, name, FILE_OPEN) {
            Ok(dir) => current = dir,
            Err(SealError::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(VerifiedPath { dir: None });
            }
            Err(e) => return Err(e),
        }
    }
    Ok(VerifiedPath { dir: Some(current) })
}

impl SealedRoot {
    /// Binds `path` as a sealed root, creating the final component if absent - the Windows
    /// counterpart of `unix_impl::SealedRoot::establish`.
    pub fn establish(path: &Path) -> Result<Self, SealError> {
        let (anchor, names) = decompose_absolute_path(path)?;
        let mut current = open_anchor(&anchor)?;
        let Some((leaf, parents)) = names.split_last() else {
            // `path` is the drive root itself - already-open, no path-based lookup involved.
            return Ok(SealedRoot { dir: current });
        };
        for name in parents {
            current = open_child_dir_nofollow(&current, name, FILE_OPEN)?;
        }
        let dir = open_child_dir_nofollow(&current, leaf, FILE_OPEN_IF)?;
        Ok(SealedRoot { dir })
    }

    /// Binds an *existing* directory as a sealed root, never creating the leaf (E21-S07's
    /// Windows counterpart) - see `unix_impl::SealedRoot::bind_existing`'s identical rationale.
    pub fn bind_existing(path: &Path) -> Result<Self, SealError> {
        let (anchor, names) = decompose_absolute_path(path)?;
        let mut current = open_anchor(&anchor)?;
        for name in &names {
            current = open_child_dir_nofollow(&current, name, FILE_OPEN)?;
        }
        Ok(SealedRoot { dir: current })
    }

    /// Removes a direct child file by name, relative to the held directory descriptor, but only
    /// if it still resolves - without following a reparse point - to the exact `(volume_serial_
    /// number, file_index)` the caller confirmed. The Windows counterpart of `unix_impl::
    /// SealedRoot::unlink_child_matching_unix_identity`; see that method's own docs for the full
    /// TOCTOU rationale, which applies identically here.
    ///
    /// Uses the classic `FileDispositionInfo{DeleteFile: true}` (not the newer POSIX-semantics
    /// variant, which needs a second struct/flag and a more recent Windows baseline this crate
    /// has not verified against) - the marked file is actually removed once every handle to it
    /// closes, which happens by the time this function returns (nothing else in this crate
    /// retains one), so the deletion is complete from this call's own caller's perspective.
    pub fn unlink_child_matching_windows_identity(
        &self,
        name: &str,
        volume_serial_number: u32,
        file_index: u64,
    ) -> Result<(), SealError> {
        let wide = validate_child_name(name)?;
        let file = nt_open_child(&self.dir, &wide, false, FILE_OPEN, DELETE)?;

        let facts = observe_identity_of_handle(file.as_raw_handle()).map_err(SealError::Io)?;
        if facts.is_reparse_point {
            return Err(SealError::IsSymlinkOrReparsePoint);
        }
        if facts.volume_serial_number != volume_serial_number || facts.file_index != file_index {
            return Err(SealError::IdentityMismatch);
        }

        let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
        // SAFETY: `file` is a valid, currently-open HANDLE for the duration of this call, just
        // confirmed above to name the exact object the caller expected. `disposition` is a
        // stack-allocated, correctly-sized `FILE_DISPOSITION_INFO`, passed as a valid in-pointer
        // alongside its exact byte size. `SetFileInformationByHandle` only reads `disposition`
        // and returns nonzero on success; it retains no pointer past the call.
        let ok = unsafe {
            SetFileInformationByHandle(
                file.as_raw_handle(),
                FileDispositionInfo,
                &disposition as *const FILE_DISPOSITION_INFO as *const core::ffi::c_void,
                size_of::<FILE_DISPOSITION_INFO>() as u32,
            )
        };
        if ok == 0 {
            return Err(SealError::Io(io::Error::last_os_error()));
        }
        Ok(())
    }

    /// `true` if the handle this `SealedRoot` retains for `name` is actually marked for
    /// deletion - used by `cancellai-platform::mutation`'s own post-delete corroboration step,
    /// the Windows counterpart of the Unix path's post-unlink link-count re-check. Takes an
    /// already-open handle (from the *original*, pre-delete open the caller retained) rather
    /// than reopening by name, since a reopen after the delete call above would itself be a
    /// fresh, unprotected path lookup - exactly what this crate exists to avoid.
    pub fn is_delete_pending(handle: &File) -> Result<bool, SealError> {
        let mut info: FILE_STANDARD_INFO = unsafe { std::mem::zeroed() };
        // SAFETY: `handle` is a valid, currently-open HANDLE for the duration of this call.
        // `info` is a stack-allocated, correctly-sized `FILE_STANDARD_INFO`, passed as a valid
        // out-pointer alongside its exact byte size; `GetFileInformationByHandleEx` only writes
        // into `info` and returns nonzero on success.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                handle.as_raw_handle(),
                FileStandardInfo,
                &mut info as *mut FILE_STANDARD_INFO as *mut core::ffi::c_void,
                size_of::<FILE_STANDARD_INFO>() as u32,
            )
        };
        if ok == 0 {
            return Err(SealError::Io(io::Error::last_os_error()));
        }
        Ok(info.DeletePending)
    }

    /// Reads a direct child file by name, relative to the held directory descriptor - the
    /// Windows counterpart of `unix_impl::SealedRoot::read_child_to_string`. Deliberately
    /// follows a reparse point at `name` itself (not `FILE_OPEN_REPARSE_POINT`), matching that
    /// method's own already-verified "the read side legitimately follows it" contract.
    pub fn read_child_to_string(&self, name: &str) -> Result<Option<String>, SealError> {
        use std::io::Read;

        let wide: Vec<u16> = OsStr::new(name).encode_wide().collect();
        if name.is_empty() || name.contains(['/', '\\', '\0']) {
            return Err(SealError::InvalidChildName);
        }
        let byte_len = u16::try_from(wide.len() * 2).map_err(|_| SealError::InvalidChildName)?;
        let mut wide = wide;
        let unicode_name = UNICODE_STRING {
            Length: byte_len,
            MaximumLength: byte_len,
            Buffer: wide.as_mut_ptr(),
        };
        let object_attributes = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: self.dir.as_raw_handle(),
            ObjectName: &unicode_name,
            Attributes: OBJ_CASE_INSENSITIVE,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null(),
        };
        let mut handle: HANDLE = std::ptr::null_mut();
        let mut iosb: IO_STATUS_BLOCK = unsafe { std::mem::zeroed() };
        // SAFETY: same invariants as `nt_open_child` above - `RootDirectory` is this
        // `SealedRoot`'s own held, open handle; `ObjectName` names a single bare component with
        // no separators, so resolution never escapes the directory `self.dir` refers to.
        // `FILE_NON_DIRECTORY_FILE` without `FILE_OPEN_REPARSE_POINT` deliberately allows
        // following a reparse point at this single, final component - the documented,
        // already-verified read-side contract this method mirrors from `unix_impl`.
        let status = unsafe {
            NtCreateFile(
                &mut handle,
                FILE_READ_DATA | SYNCHRONIZE,
                &object_attributes,
                &mut iosb,
                std::ptr::null(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                FILE_OPEN,
                FILE_NON_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT,
                std::ptr::null(),
                0,
            )
        };
        if status != STATUS_SUCCESS {
            // SAFETY: see `nt_open_child` - pure, allocation-free NTSTATUS translation.
            let win32_error = unsafe { RtlNtStatusToDosError(status) };
            let e = io::Error::from_raw_os_error(win32_error as i32);
            return if e.kind() == io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(SealError::Io(e))
            };
        }
        // SAFETY: `status == STATUS_SUCCESS`, so `handle` is newly allocated and exclusively
        // owned here.
        let mut file = unsafe { File::from_raw_handle(handle as RawHandle) };
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(Some(contents))
    }

    /// Writes `contents` to a new child `tmp_name`, refusing anything already present there,
    /// then atomically renames it to `final_name` - the Windows counterpart of `unix_impl::
    /// SealedRoot::write_new_child_atomically`.
    pub fn write_new_child_atomically(
        &self,
        tmp_name: &str,
        final_name: &str,
        contents: &[u8],
    ) -> Result<(), SealError> {
        use std::io::Write;

        let tmp_wide = validate_child_name(tmp_name)?;
        // `FILE_CREATE` (not `FILE_OPEN_IF`) refuses anything already present at `tmp_name` -
        // including a pre-planted reparse point - exactly like Unix's `O_CREAT | O_EXCL`.
        let mut file = nt_open_child(
            &self.dir,
            &tmp_wide,
            false,
            windows_sys::Wdk::Storage::FileSystem::FILE_CREATE,
            windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE,
        )?;
        let write_result = file.write_all(contents).and_then(|()| file.sync_all());
        drop(file);
        write_result.map_err(SealError::Io)?;

        rename_child(&self.dir, tmp_name, final_name)
    }
}

/// Renames `old_name` to `new_name`, both direct children of `dir` - relative to the held
/// handle, via `FILE_RENAME_INFO`/`SetFileInformationByHandle`, the handle-based analogue of
/// Unix's `renameat`. Opens `old_name` itself (not `dir`) because `FILE_RENAME_INFO` is applied
/// to the object being renamed, with the *new* name's parent given via `RootDirectory`.
fn rename_child(dir: &File, old_name: &str, new_name: &str) -> Result<(), SealError> {
    use windows_sys::Win32::Storage::FileSystem::{FILE_RENAME_INFO, FileRenameInfo};

    let old_wide = validate_child_name(old_name)?;
    let target = nt_open_child(
        dir,
        &old_wide,
        false,
        FILE_OPEN,
        DELETE | windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE,
    )?;

    let mut new_wide: Vec<u16> = OsStr::new(new_name).encode_wide().collect();
    new_wide.push(0);
    let name_byte_len = ((new_wide.len() - 1) * 2) as u32;

    // `FILE_RENAME_INFO` is a variable-length struct (a fixed header followed by the
    // destination name's own UTF-16 bytes) - built as raw bytes rather than a fixed-size Rust
    // struct, since `windows-sys`'s own generated type already models it as a 1-element
    // `FileName: [u16; 1]` trailing array for exactly this reason.
    let header_len = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let mut buffer = vec![0u8; header_len + new_wide.len() * 2];
    // SAFETY: `buffer` is at least `header_len` bytes (allocated above), so this points inside
    // it; `FILE_RENAME_INFO`'s layout has no padding/alignment requirement beyond `u32`/`HANDLE`
    // that a byte-aligned `Vec<u8>` cannot satisfy in practice on this target, and every field
    // is written before being read.
    unsafe {
        let info = buffer.as_mut_ptr() as *mut FILE_RENAME_INFO;
        (*info).Anonymous.ReplaceIfExists = true;
        (*info).RootDirectory = dir.as_raw_handle();
        (*info).FileNameLength = name_byte_len;
    }
    buffer[header_len..].copy_from_slice(&bytemuck_u16_to_u8(&new_wide));

    // SAFETY: `target` is a valid, currently-open HANDLE for the duration of this call, opened
    // with `DELETE` access (required for a rename). `buffer` is a correctly-sized, fully
    // initialized `FILE_RENAME_INFO` followed immediately by its destination name, matching the
    // variable-length layout `SetFileInformationByHandle` documents for `FileRenameInfo`.
    let ok = unsafe {
        SetFileInformationByHandle(
            target.as_raw_handle(),
            FileRenameInfo,
            buffer.as_ptr() as *const core::ffi::c_void,
            buffer.len() as u32,
        )
    };
    if ok == 0 {
        return Err(SealError::Io(io::Error::last_os_error()));
    }
    Ok(())
}

/// `[u16]` reinterpreted as its own little-endian byte representation - `FILE_RENAME_INFO`'s
/// trailing `FileName` field is documented as raw UTF-16LE bytes, and this target is always
/// little-endian (`x86_64`/`aarch64` Windows), so this is exact, not merely convenient.
fn bytemuck_u16_to_u8(input: &[u16]) -> Vec<u8> {
    input.iter().flat_map(|c| c.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-sealedfs-windows-sealed-test-{label}-{}",
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

    fn create_junction(link: &Path, target: &Path) -> io::Result<()> {
        let status = std::process::Command::new("cmd")
            .arg("/c")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!("mklink /J exited with {status}")))
        }
    }

    #[test]
    fn establish_binds_a_real_directory_and_round_trips_a_child_write_and_read() {
        let dir = TempDir::new("basic");
        let root_path = dir.path("root");
        std::fs::create_dir(&root_path).expect("create root");

        let root = SealedRoot::establish(&root_path).expect("establish");
        root.write_new_child_atomically("tmp", "final.txt", b"hello")
            .expect("write");
        let contents = root
            .read_child_to_string("final.txt")
            .expect("read")
            .expect("child exists");
        assert_eq!(contents, "hello");
    }

    #[test]
    fn establish_creates_an_absent_root_before_binding_it() {
        let dir = TempDir::new("create-absent");
        let root_path = dir.path("does-not-exist-yet");

        SealedRoot::establish(&root_path).expect("establish must create the missing leaf");
        assert!(root_path.is_dir());
    }

    #[test]
    fn bind_existing_refuses_a_missing_directory() {
        let dir = TempDir::new("bind-missing");
        let missing = dir.path("nope");
        let err = SealedRoot::bind_existing(&missing).expect_err("must refuse a missing root");
        assert!(matches!(err, SealError::Io(_)));
    }

    #[test]
    fn establish_refuses_a_root_that_is_a_real_symlink() {
        let dir = TempDir::new("symlink-root");
        let real = dir.path("real");
        std::fs::create_dir(&real).expect("create real dir");
        let link = dir.path("link");
        std::os::windows::fs::symlink_dir(&real, &link).expect("create symlink");

        let err = SealedRoot::establish(&link).expect_err("must refuse a symlinked root");
        assert!(matches!(err, SealError::IsSymlinkOrReparsePoint));
    }

    #[test]
    fn establish_refuses_a_root_reached_through_a_real_junction_intermediate_component() {
        let dir = TempDir::new("junction-intermediate");
        let real = dir.path("real");
        std::fs::create_dir(&real).expect("create real dir");
        let junction = dir.path("junction");
        create_junction(&junction, &real).expect("create junction");
        let leaf_via_junction = junction.join("leaf");

        let err = SealedRoot::establish(&leaf_via_junction)
            .expect_err("a junction anywhere in the path, not only the leaf, must be refused");
        assert!(matches!(err, SealError::IsSymlinkOrReparsePoint));
    }

    #[test]
    fn establish_refuses_a_relative_path() {
        let err = SealedRoot::establish(Path::new("relative\\path"))
            .expect_err("must refuse a relative path");
        assert!(matches!(err, SealError::NotAbsolute));
    }

    #[test]
    fn establish_refuses_a_path_containing_dot_dot() {
        let dir = TempDir::new("dot-dot");
        let path = dir.path("child").join("..").join("child");
        let err = SealedRoot::establish(&path).expect_err("must refuse a path containing '..'");
        assert!(matches!(err, SealError::PathNotNormalized));
    }

    #[test]
    fn write_new_child_atomically_refuses_a_pre_planted_reparse_point_at_the_temp_name() {
        let dir = TempDir::new("pre-planted");
        let root_path = dir.path("root");
        std::fs::create_dir(&root_path).expect("create root");
        let outside = TempDir::new("pre-planted-outside");
        std::os::windows::fs::symlink_file(outside.path("nonexistent"), root_path.join("tmp"))
            .expect("plant a symlink at the temp name");

        let root = SealedRoot::establish(&root_path).expect("establish");
        let err = root
            .write_new_child_atomically("tmp", "final.txt", b"hello")
            .expect_err("must refuse to write through a pre-planted reparse point");
        assert!(matches!(
            err,
            SealError::IsSymlinkOrReparsePoint | SealError::Io(_)
        ));
    }

    #[test]
    fn unlink_child_matching_windows_identity_deletes_only_on_a_real_identity_match() {
        let dir = TempDir::new("unlink");
        let root_path = dir.path("root");
        std::fs::create_dir(&root_path).expect("create root");
        let target = root_path.join("target.txt");
        std::fs::write(&target, b"hello").expect("create file");

        let facts =
            crate::windows_identity::observe_identity(&target).expect("observe real identity");

        let root = SealedRoot::bind_existing(&root_path).expect("bind");
        let err = root
            .unlink_child_matching_windows_identity(
                "target.txt",
                facts.volume_serial_number,
                facts.file_index.wrapping_add(1),
            )
            .expect_err("a mismatched file_index must refuse the delete");
        assert!(matches!(err, SealError::IdentityMismatch));
        assert!(target.exists(), "a refused delete must not touch the file");

        root.unlink_child_matching_windows_identity(
            "target.txt",
            facts.volume_serial_number,
            facts.file_index,
        )
        .expect("a matching identity must delete");
        assert!(!target.exists(), "the file must actually be gone");
    }

    #[test]
    fn unlink_child_matching_windows_identity_refuses_a_reparse_point_at_the_name() {
        let dir = TempDir::new("unlink-reparse");
        let root_path = dir.path("root");
        std::fs::create_dir(&root_path).expect("create root");
        let outside = TempDir::new("unlink-reparse-outside");
        let outside_file = outside.path("real.txt");
        std::fs::write(&outside_file, b"hello").expect("create outside file");
        let link = root_path.join("link.txt");
        std::os::windows::fs::symlink_file(&outside_file, &link).expect("create symlink");

        let root = SealedRoot::bind_existing(&root_path).expect("bind");
        let err = root
            .unlink_child_matching_windows_identity("link.txt", 0, 0)
            .expect_err("a reparse point at the child name must be refused, not followed");
        assert!(matches!(err, SealError::IsSymlinkOrReparsePoint));
        assert!(
            outside_file.exists(),
            "the real outside file must be untouched"
        );
    }

    #[test]
    fn verify_no_intermediate_links_refuses_an_intermediate_reparse_point() {
        let dir = TempDir::new("verify-reparse");
        let real = dir.path("real");
        std::fs::create_dir(&real).expect("create real dir");
        let junction = dir.path("junction");
        create_junction(&junction, &real).expect("create junction");
        let leaf_via_junction = junction.join("leaf");

        let err = verify_no_intermediate_links(&leaf_via_junction)
            .expect_err("an intermediate junction must be refused");
        assert!(matches!(err, SealError::IsSymlinkOrReparsePoint));
    }

    #[test]
    fn verify_no_intermediate_links_accepts_a_real_path_and_creates_nothing() {
        let dir = TempDir::new("verify-real");
        let root_path = dir.path("root");
        std::fs::create_dir(&root_path).expect("create real dir");

        let verified =
            verify_no_intermediate_links(&root_path).expect("a real, link-free path must pass");
        let facts = crate::windows_identity::observe_identity(&root_path).expect("observe");
        assert!(
            verified
                .matches_windows_identity(facts.volume_serial_number, facts.file_index)
                .expect("compare")
        );
    }

    #[test]
    fn verify_no_intermediate_links_treats_a_missing_leaf_as_ok_and_creates_nothing() {
        let dir = TempDir::new("verify-missing");
        let missing = dir.path("does-not-exist");

        verify_no_intermediate_links(&missing)
            .expect("a missing path is not itself a link and must not error");
        assert!(!missing.exists(), "must never create the path it checks");
    }
}

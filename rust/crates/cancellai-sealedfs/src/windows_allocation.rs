//! Real Windows allocated/physical size (E20-S05, extending ADR-0020).
//!
//! `std` has no stable way to read this at all on Windows (`MetadataExt::file_size()` is the
//! *logical* length only). `GetFileInformationByHandleEx` with `FileStandardInfo` gives the
//! real allocated size (`FILE_STANDARD_INFO::AllocationSize`) - handle-based, so it reuses this
//! crate's existing [`crate::windows_identity::open_no_follow`] open (`FILE_FLAG_OPEN_REPARSE_
//! POINT`), keeping allocated-size observation exactly as no-follow as identity observation
//! already is, rather than reaching for the simpler but path-based (and reparse-point-following)
//! `GetCompressedFileSizeW`.

use std::io;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use windows_sys::Win32::Storage::FileSystem::{
    FILE_STANDARD_INFO, FileStandardInfo, GetFileInformationByHandleEx,
};

use crate::windows_identity::open_no_follow;

/// Observe `path`'s real allocated size in bytes, without following a reparse point at the
/// final path component.
pub fn observe_allocated_size(path: &Path) -> io::Result<u64> {
    let file = open_no_follow(path)?;

    let mut info: FILE_STANDARD_INFO = unsafe { std::mem::zeroed() };
    // SAFETY: `file` is a valid, currently-open HANDLE for the entire duration of this call.
    // `info` is a stack-allocated, correctly-sized `FILE_STANDARD_INFO` (the struct
    // `windows-sys` generates directly from the same Win32 metadata that documents this call),
    // passed as a valid out-pointer alongside its exact byte size - `Get
    // FileInformationByHandleEx` only writes into `info` and returns nonzero on success; it
    // retains no pointer past the call and performs no allocation.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileStandardInfo,
            &mut info as *mut FILE_STANDARD_INFO as *mut core::ffi::c_void,
            size_of::<FILE_STANDARD_INFO>() as u32,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }

    // `AllocationSize` is documented non-negative for a real file/directory; a negative value
    // here would be a platform contract violation this observer has no honest way to represent
    // as a byte count, so it is refused rather than silently reinterpreted via `as u64`.
    u64::try_from(info.AllocationSize).map_err(|_| {
        io::Error::other(format!(
            "GetFileInformationByHandleEx reported a negative AllocationSize ({})",
            info.AllocationSize
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-sealedfs-windows-allocation-test-{label}-{}",
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

    #[test]
    fn observe_allocated_size_reports_a_nonzero_allocation_for_a_real_file() {
        let dir = TempDir::new("file");
        let target = dir.path("target.txt");
        std::fs::write(&target, vec![b'x'; 8192]).expect("create an 8KB file");

        let allocated = observe_allocated_size(&target).expect("observe a real file");
        assert!(
            allocated > 0,
            "an 8KB file must occupy at least one allocation unit"
        );
    }

    #[test]
    fn observe_allocated_size_reports_zero_for_an_empty_file() {
        let dir = TempDir::new("empty");
        let target = dir.path("empty.txt");
        std::fs::write(&target, b"").expect("create an empty file");

        let allocated = observe_allocated_size(&target).expect("observe an empty file");
        assert_eq!(allocated, 0);
    }

    #[test]
    fn observe_allocated_size_errors_for_a_missing_path() {
        let dir = TempDir::new("missing");
        let missing = dir.path("does-not-exist");
        assert!(observe_allocated_size(&missing).is_err());
    }
}

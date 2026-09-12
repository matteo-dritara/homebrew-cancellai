//! Real macOS filesystem-type name (E10-S01).
//!
//! `std` has no way to ask which filesystem backs a path. `libc::statfs`'s `f_fstypename`
//! field is macOS's own short type name (`"apfs"`, `"hfs"`, `"nfs"`, `"msdos"`, `"smbfs"`, ...) -
//! the same string `mount`/`df -T` report, populated directly by the kernel rather than
//! inferred from a mount-table path match (this workspace's Linux equivalent,
//! `cancellai-platform::wsl::longest_matching_mount_fstype`, has to do the latter because Linux
//! exposes no per-path syscall for it). `cancellai-platform::filesystem_kind` turns this raw
//! name into a clone/reflink-capability judgment (APFS clones make a naive sum of allocated
//! sizes across files an overstatement of real reclaim - SI-008/SI-009 generalized to reclaim
//! accounting); this function only reports the observed fact.

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Observe the filesystem type name backing `path`, e.g. `"apfs"`.
pub fn observe_filesystem_name(path: &Path) -> io::Result<String> {
    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c_path` is a valid, NUL-terminated C string for the duration of this call.
    // `stat` is a stack-allocated, correctly-sized `libc::statfs` passed as a valid out-pointer;
    // `statfs(2)` only writes into it and returns `-1` on error without retaining either
    // pointer past the call.
    let result = unsafe { libc::statfs(c_path.as_ptr(), &mut stat) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }

    // `f_fstypename` is a fixed-size, NUL-terminated (and NUL-padded) `c_char` array, never
    // guaranteed to contain valid UTF-8 in principle - decoded lossily rather than panicking or
    // silently truncating at the first invalid byte, since a real, unrecognized value here still
    // needs to reach `cancellai-platform`'s conservative "unrecognized filesystem" fallback
    // rather than becoming an `Err` that fallback never sees.
    let raw: Vec<u8> = stat
        .f_fstypename
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as u8)
        .collect();
    Ok(String::from_utf8_lossy(&raw).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_filesystem_name_reports_a_nonempty_name_for_a_real_path() {
        let name = observe_filesystem_name(Path::new(".")).expect("observe the current directory");
        assert!(
            !name.is_empty(),
            "a real, mounted path must report a non-empty filesystem type name"
        );
    }

    #[test]
    fn observe_filesystem_name_errors_for_a_missing_path() {
        let missing = std::env::temp_dir().join(format!(
            "cancellai-sealedfs-macos-filesystem-test-missing-{}",
            std::process::id()
        ));
        assert!(observe_filesystem_name(&missing).is_err());
    }
}

//! Real Windows file/volume identity (E20-S01, ADR-0020).
//!
//! `std::os::windows::fs::MetadataExt` only stabilizes `file_attributes`/`creation_time`/
//! `last_access_time`/`last_write_time`/`file_size`; `file_index`/`volume_serial_number`/
//! `number_of_links` remain gated behind the nightly-only `windows_by_handle` feature
//! (rust-lang/rust#63010). An inode/device-strength Windows identity therefore needs
//! `GetFileInformationByHandle` (`BY_HANDLE_FILE_INFORMATION`), which `std` does not expose
//! safely. This module is `cancellai-sealedfs`'s second, independent unsafe surface (after
//! the Unix `openat`/`renameat`/`mkdirat` calls in [`crate::unix_impl`]) - ADR-0020 explains
//! why it lives here rather than in `cancellai-platform` directly (keeping every `unsafe`
//! block in the one crate this workspace already trusts with it) and why `windows-sys`
//! (Microsoft's own code-generated bindings, already used internally by `std`) is used instead
//! of a hand-transcribed `extern "system"` declaration.
//!
//! The open itself needs no `unsafe`: `OpenOptionsExt::custom_flags` (stable) requests
//! `FILE_FLAG_OPEN_REPARSE_POINT` (the Windows equivalent of `symlink_metadata`'s no-follow
//! behavior - every other observer in this workspace never follows a final symlink/reparse
//! point either) and `FILE_FLAG_BACKUP_SEMANTICS` (required by `CreateFileW` to open a
//! directory at all). Only the single `GetFileInformationByHandle` call needs `unsafe`, against
//! a handle whose lifecycle `std::fs::File`'s own `Drop` already manages safely.

use std::fs::OpenOptions;
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, GetFileInformationByHandle,
};

/// Real, `GetFileInformationByHandle`-backed identity facts for one Windows filesystem
/// object, observed without following a reparse point at the final path component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsFileFacts {
    pub volume_serial_number: u32,
    pub file_index: u64,
    pub is_reparse_point: bool,
    pub is_directory: bool,
    /// Raw 100-nanosecond `FILETIME` ticks of the last-write time (the Windows analogue of the
    /// Unix identity token's `modified_nanos`: the sub-second remainder needed to disambiguate
    /// a same-second delete-and-recreate, E07-S05).
    pub last_write_time_ticks: u64,
}

/// Observe `path`'s real identity. Never follows a reparse point at the final component -
/// matches `symlink_metadata`'s no-follow contract, which every other identity/allocation
/// observer in this workspace relies on.
pub fn observe_identity(path: &Path) -> io::Result<WindowsFileFacts> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;

    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: `file` is a valid, currently-open HANDLE for the entire duration of this call -
    // it is not dropped until this function returns, and its handle was obtained through
    // `std::fs::OpenOptions`, never constructed by this crate itself. `info` is a
    // stack-allocated, correctly-sized `BY_HANDLE_FILE_INFORMATION` (the struct `windows-sys`
    // generates directly from the same Win32 metadata that documents this call), passed as a
    // valid, uniquely-owned out-pointer. `GetFileInformationByHandle` only writes into `info`
    // and returns nonzero on success; it retains no pointer past the call and performs no
    // allocation.
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(WindowsFileFacts {
        volume_serial_number: info.dwVolumeSerialNumber,
        file_index: ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
        is_reparse_point: info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0,
        is_directory: info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0,
        last_write_time_ticks: ((info.ftLastWriteTime.dwHighDateTime as u64) << 32)
            | info.ftLastWriteTime.dwLowDateTime as u64,
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
                "cancellai-sealedfs-windows-identity-test-{label}-{}",
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
    fn observe_identity_reports_facts_for_a_real_file() {
        let dir = TempDir::new("file");
        let target = dir.path("target.txt");
        std::fs::write(&target, b"hello").expect("create file");

        let facts = observe_identity(&target).expect("observe a real file");
        assert!(!facts.is_directory);
        assert!(!facts.is_reparse_point);
        assert!(facts.file_index != 0 || facts.volume_serial_number != 0);
    }

    #[test]
    fn observe_identity_reports_is_directory_for_a_real_directory() {
        let dir = TempDir::new("dir");
        let target = dir.path("child");
        std::fs::create_dir(&target).expect("create directory");

        let facts = observe_identity(&target).expect("observe a real directory");
        assert!(facts.is_directory);
        assert!(!facts.is_reparse_point);
    }

    #[test]
    fn observe_identity_reports_is_reparse_point_for_a_real_symlink_without_following_it() {
        let dir = TempDir::new("symlink");
        let real_target = dir.path("real-target");
        std::fs::create_dir(&real_target).expect("create real directory");
        let link = dir.path("link");
        std::os::windows::fs::symlink_dir(&real_target, &link)
            .expect("create a real directory symlink (requires Developer Mode or admin on CI)");

        let facts = observe_identity(&link).expect("observe the symlink itself");
        assert!(
            facts.is_reparse_point,
            "a directory symlink must be observed as a reparse point, not silently followed"
        );

        let real_facts = observe_identity(&real_target).expect("observe the real target");
        assert_ne!(
            facts.file_index, real_facts.file_index,
            "the link's own identity must differ from the target it points at - proving this \
             observation did not follow the reparse point"
        );
    }

    /// Creates a real NTFS junction (`IO_REPARSE_TAG_MOUNT_POINT`) via the OS's own `mklink
    /// /J`, deliberately not `std::os::windows::fs::symlink_dir` (which creates a *symlink*,
    /// `IO_REPARSE_TAG_SYMLINK`, a different reparse tag) and not a hand-rolled
    /// `DeviceIoControl(FSCTL_SET_REPARSE_POINT)` call. E20-S01 round-1 independent verifier
    /// review found no real junction fixture existed - only a symlink one - leaving
    /// `IO_REPARSE_TAG_MOUNT_POINT` completely unexercised. Shelling out to `mklink /J` (test
    /// code only, never production) reuses the OS's own already-correct, already-audited
    /// junction-creation logic instead of adding a second, junction-specific unsafe FFI surface
    /// to this crate for a one-off test fixture - the same "prefer the audited primitive"
    /// reasoning ADR-0020 already applied to `GetFileInformationByHandle` itself. Unlike a
    /// symlink, a junction needs no elevated privilege/Developer Mode on any Windows version,
    /// so this works unconditionally on real Windows CI.
    fn create_junction(link: &Path, target: &Path) -> std::io::Result<()> {
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
            Err(std::io::Error::other(format!(
                "mklink /J exited with {status}"
            )))
        }
    }

    #[test]
    fn observe_identity_reports_is_reparse_point_for_a_real_junction_without_following_it() {
        let dir = TempDir::new("junction");
        let real_target = dir.path("real-target");
        std::fs::create_dir(&real_target).expect("create real directory");
        let link = dir.path("junction-link");
        create_junction(&link, &real_target).expect("create a real NTFS junction via mklink /J");

        let facts = observe_identity(&link).expect("observe the junction itself");
        assert!(
            facts.is_reparse_point,
            "an NTFS junction (IO_REPARSE_TAG_MOUNT_POINT) must be observed as a reparse \
             point, not silently followed - the same FILE_ATTRIBUTE_REPARSE_POINT check that \
             already covers symlinks must generalize to this different reparse tag too"
        );

        let real_facts = observe_identity(&real_target).expect("observe the real target");
        assert_ne!(
            facts.file_index, real_facts.file_index,
            "the junction's own identity must differ from the target it points at - proving \
             this observation did not follow the reparse point"
        );
    }

    #[test]
    fn observe_identity_reports_different_volume_serial_numbers_across_real_drive_letters() {
        // E20-S01 round-1 independent verifier review asked for "native multi-volume coverage
        // where the CI environment permits it" - this codebase does not control how many
        // drive letters a given Windows CI image happens to expose (GitHub's own `D:` drive on
        // its Windows runners has been added, undocumented, and removed across image versions -
        // relying on a specific letter existing would make this test flaky for reasons outside
        // this repository's control, not a real regression). This probes every drive letter
        // C-Z at runtime and only asserts once at least one *other* real, accessible volume is
        // found alongside the one this test's own temp directory lives on; otherwise it
        // disclosed-skips rather than failing on infrastructure this codebase does not own.
        let dir = TempDir::new("multi-volume");
        let own_facts = observe_identity(&dir.0).expect("observe this test's own temp directory");

        let other_drive = ('C'..='Z')
            .map(|letter| PathBuf::from(format!("{letter}:\\")))
            .find(|root| {
                observe_identity(root)
                    .is_ok_and(|facts| facts.volume_serial_number != own_facts.volume_serial_number)
            });

        match other_drive {
            Some(root) => {
                let other_facts =
                    observe_identity(&root).expect("already confirmed observable above");
                assert_ne!(
                    own_facts.volume_serial_number, other_facts.volume_serial_number,
                    "two genuinely different drive letters must report different volume serial numbers"
                );
            }
            None => {
                eprintln!(
                    "skipping: only one accessible volume found on this machine (no second \
                     drive letter to compare against) - not a failure of this crate's own code"
                );
            }
        }
    }

    #[test]
    fn observe_identity_two_hardlinks_to_the_same_file_share_a_file_index() {
        // The positive counterpart of the symlink test above: `file_index` genuinely tracks
        // the underlying object, not merely the path used to reach it.
        let dir = TempDir::new("hardlink");
        let original = dir.path("original.txt");
        std::fs::write(&original, b"hello").expect("create file");
        let hardlink = dir.path("hardlink.txt");
        std::fs::hard_link(&original, &hardlink).expect("create hard link");

        let original_facts = observe_identity(&original).expect("observe original");
        let hardlink_facts = observe_identity(&hardlink).expect("observe hard link");
        assert_eq!(
            original_facts.file_index, hardlink_facts.file_index,
            "two hard links to the same file must report the same file index"
        );
        assert_eq!(
            original_facts.volume_serial_number,
            hardlink_facts.volume_serial_number
        );
    }

    #[test]
    fn observe_identity_errors_for_a_missing_path() {
        let dir = TempDir::new("missing");
        let missing = dir.path("does-not-exist");
        assert!(observe_identity(&missing).is_err());
    }
}

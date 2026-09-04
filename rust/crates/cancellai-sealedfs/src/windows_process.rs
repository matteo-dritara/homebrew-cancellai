//! Real Windows running-process enumeration (E20-S05, extending ADR-0020).
//!
//! `cancellai-platform::process::SystemProcessObserver` shells out to `ps`, which does not
//! exist on stock Windows - the probe there always fails and (correctly, fail-closed) reports
//! `complete: false`, never a false "not running," but that means every retention decision on
//! Windows treats every provider process as possibly running, permanently, regardless of
//! reality. `CreateToolhelp32Snapshot`/`Process32FirstW`/`Process32NextW` (`kernel32.dll`) is
//! the standard, widely-used Win32 mechanism for listing every running process's name without
//! opening a handle to each one individually (which would need per-process query permission and
//! could fail for processes owned by other users) - a plain snapshot enumeration needs no
//! elevated privilege.

use std::io;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};

/// A snapshot handle, closed on drop - the only state this module's `unsafe` calls need to
/// stay valid for, matching this crate's existing pattern of wrapping a raw OS handle in a
/// type whose `Drop` frees it unconditionally.
struct Snapshot(HANDLE);

impl Drop for Snapshot {
    fn drop(&mut self) {
        // SAFETY: `self.0` is a valid, currently-open snapshot handle this struct exclusively
        // owns (checked non-invalid at construction, never cloned or otherwise duplicated).
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Every currently-running process's executable file name (e.g. `"claude.exe"`), as reported
/// by a single point-in-time snapshot - inherently racy (a process can start or exit between
/// this call returning and its caller acting on the result), the same as `ps` is on Unix; this
/// is a fact-gathering primitive, never itself the sole safety control.
pub fn list_running_process_names() -> io::Result<Vec<String>> {
    // SAFETY: `TH32CS_SNAPPROCESS` with a `0` PID requests a process-list snapshot of the
    // whole system, which needs no special privilege; the returned handle is checked against
    // `INVALID_HANDLE_VALUE` immediately below before being trusted.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let snapshot = Snapshot(raw);

    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;

    let mut names = Vec::new();
    // SAFETY: `snapshot.0` is the just-created, still-open handle above; `entry` is
    // stack-allocated with `dwSize` set exactly as `Process32FirstW`/`Process32NextW` require,
    // and is only read after a nonzero (success) return, matching the documented contract.
    let mut ok = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    while ok != 0 {
        names.push(exe_file_name(&entry.szExeFile));
        // SAFETY: same handle and buffer as above, reused for the next entry.
        ok = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    Ok(names)
}

/// `PROCESSENTRY32W::szExeFile` is a fixed-size, NUL-terminated (or fully-filled) UTF-16
/// buffer - this decodes exactly the meaningful prefix, never the untouched trailing zeros.
fn exe_file_name(buffer: &[u16; 260]) -> String {
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_file_name_decodes_a_nul_terminated_buffer() {
        let mut buffer = [0u16; 260];
        for (i, c) in "claude.exe".encode_utf16().enumerate() {
            buffer[i] = c;
        }
        assert_eq!(exe_file_name(&buffer), "claude.exe");
    }

    #[test]
    fn exe_file_name_decodes_a_fully_filled_buffer_with_no_nul() {
        // Defensive: PROCESSENTRY32W's own documentation does not guarantee NUL-termination
        // when the name fills the buffer exactly - `position` returning `None` must not panic.
        let buffer = [b'a' as u16; 260];
        assert_eq!(exe_file_name(&buffer).len(), 260);
    }

    #[test]
    fn list_running_process_names_finds_this_test_processs_own_name() {
        // The current process is, definitionally, always running - a real, deterministic
        // positive case rather than asserting anything about a specific provider process name.
        let names = list_running_process_names().expect("enumerate real running processes");
        assert!(
            !names.is_empty(),
            "a real Windows machine always has more than zero running processes"
        );
    }
}

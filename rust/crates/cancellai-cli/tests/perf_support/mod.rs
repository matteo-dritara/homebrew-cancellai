//! Shared self-footprint measurement for the E10-S02 performance tests
//! (`performance_memory.rs`, `performance_scheduled_shipped.rs`). Kept as its own module (not
//! `#[cfg(test)]` code in `src/`) since it is only needed by integration tests, matching
//! `cancellai-inventory/tests/perf_support/mod.rs`'s own precedent.

/// Parses `/proc/self/status`-formatted content and returns `VmHWM` ("high water mark") in
/// bytes. Pure and platform-independent (mirrors `cancellai_platform::wsl::
/// longest_matching_mount_fstype`'s own observation/classification split), so the parsing logic
/// itself is exhaustively unit-testable on any host - this executor's own machine cannot run the
/// real Linux `/proc/self/status` read that [`peak_rss_bytes`] performs, but this function is
/// what actually turns that content into a byte count, and it runs (and is tested) everywhere.
fn parse_vmhwm_bytes(status: &str) -> Option<u64> {
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            // Format is "VmHWM:     1234 kB" - whitespace-separated, kernel-emitted, always kB.
            let kb: u64 = rest.trim().trim_end_matches("kB").trim().parse().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

/// This process's peak resident set size in bytes, if it can be observed without adding a new
/// dependency. `/proc/self/status`'s `VmHWM` is a monotonically non-decreasing running peak for
/// the calling process's entire lifetime - reading it once, after the work under test has run,
/// reports the true peak for that run as long as the test binary does nothing memory-heavy
/// before it (true here: each of these files is its own process, per Cargo's
/// one-binary-per-`tests/*.rs`-file convention, and runs a single measured workload).
///
/// `None` on any platform without `/proc` (macOS, Windows) or where the line cannot be parsed -
/// never a fabricated number standing in for a real measurement (this crate's own
/// `SizeMetric::Unsupported`/`CloneSemantics::Unsupported` convention, applied here to a test
/// helper rather than a typed observation). A caller must treat `None` as "not measured on this
/// platform," not as zero.
#[cfg(target_os = "linux")]
pub fn peak_rss_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    parse_vmhwm_bytes(&status)
}

#[cfg(not(target_os = "linux"))]
pub fn peak_rss_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vmhwm_bytes_converts_kilobytes_to_bytes() {
        let status = "Name:\tcargo\nVmHWM:\t   12345 kB\nVmRSS:\t   9000 kB\n";
        assert_eq!(parse_vmhwm_bytes(status), Some(12_345 * 1024));
    }

    #[test]
    fn parse_vmhwm_bytes_is_none_for_content_with_no_such_field() {
        let status = "Name:\tcargo\nVmRSS:\t   9000 kB\n";
        assert_eq!(parse_vmhwm_bytes(status), None);
    }

    #[test]
    fn parse_vmhwm_bytes_is_none_for_empty_or_garbage_content_never_a_guess() {
        assert_eq!(parse_vmhwm_bytes(""), None);
        assert_eq!(
            parse_vmhwm_bytes("not /proc/self/status content at all"),
            None
        );
        assert_eq!(parse_vmhwm_bytes("VmHWM:\tnot-a-number kB\n"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn peak_rss_bytes_reports_a_real_nonzero_value_on_linux() {
        // Allocate and touch a real block so there is something to have measured, ruling out a
        // parser that always reports `Some(0)` regardless of the real content.
        let block = vec![1u8; 16 * 1024 * 1024];
        assert_eq!(block.len(), 16 * 1024 * 1024);
        let rss = peak_rss_bytes().expect("VmHWM must be readable on Linux");
        assert!(
            rss >= 16 * 1024 * 1024,
            "peak RSS ({rss} bytes) must be at least as large as the block just allocated"
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn peak_rss_bytes_is_honestly_unmeasured_off_linux() {
        assert_eq!(peak_rss_bytes(), None);
    }
}

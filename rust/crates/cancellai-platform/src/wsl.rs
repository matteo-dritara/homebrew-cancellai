//! WSL runtime-environment detection and `/mnt`-style filesystem-context classification
//! (E20-S02, `docs/architecture/PLATFORM_MODEL.md`'s "WSL" section).
//!
//! WSL is a distinct environment, not an alias for generic Linux (C-12): a WSL2 guest runs a
//! real Linux kernel, so every existing Unix seam in this crate (`IdentityObserver`,
//! `AllocationObserver`, ...) already works correctly there without special-casing. What is
//! specific to WSL2 is (1) knowing the process is running inside one at all, and (2) that a
//! path reached through the guest's auto-mounted Windows drives (conventionally `/mnt/c`, via
//! the `drvfs` filesystem) carries materially different performance/permission/atomicity
//! semantics than the guest's own native Linux filesystem - "surfaced rather than abstracted
//! away," per this crate's own architecture document.
//!
//! Both detectors below split their real observation (a raw file read, `#[cfg(target_os =
//! "linux")]`-gated) from a pure classification function that is fully unit-testable on any
//! host, including this workspace's own macOS/Linux CI - this executor has no real WSL2 guest
//! to run against, so the fabricated-content tests below are what this story's "simulated path
//! fixtures" verification contract actually exercises.

use std::path::Path;

/// This process's runtime environment. Never inferred as [`Wsl2`](RuntimeEnvironment::Wsl2) by
/// default - absence of a positive signal, including any error observing it at all, is
/// [`Native`](RuntimeEnvironment::Native) (C-03: ambiguity never escalates classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEnvironment {
    /// A Windows host running a WSL2 Linux guest. `PLATFORM_MODEL.md`'s representation also
    /// names `host_os: windows` and `guest_os: linux` - both implied by this one variant
    /// rather than carried as separate fields, since neither varies independently of it today.
    Wsl2,
    /// Anything else this detector cannot positively identify as WSL2: native Linux, macOS,
    /// Windows, or a WSL guest whose kernel release string carries no recognizable marker.
    Native,
}

/// A source of runtime-environment facts. Mirrors this crate's other observer seams
/// (`IdentityObserver`, `AllocationObserver`): production code takes `&dyn
/// EnvironmentObserver` and uses [`SystemEnvironmentObserver`]; tests use
/// [`SyntheticEnvironmentObserver`] to exercise WSL2-specific behavior without a real WSL2
/// guest to run on.
pub trait EnvironmentObserver: Send + Sync {
    fn detect(&self) -> RuntimeEnvironment;
}

/// The real, OS-backed observer.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemEnvironmentObserver;

impl EnvironmentObserver for SystemEnvironmentObserver {
    fn detect(&self) -> RuntimeEnvironment {
        detect_runtime_environment()
    }
}

/// WSL2's own kernel (Microsoft's WSL2 kernel repository, built specifically for it) reports a
/// release string containing `microsoft-standard-wsl2` (e.g.
/// `5.15.153.1-microsoft-standard-WSL2`) - a marker unique to that specific kernel build.
///
/// WSL1 is architecturally not this crate's `Wsl2` variant at all: it has no real Linux kernel
/// underneath it, only a syscall-translation layer running directly on the Windows NT kernel
/// (`docs/architecture/PLATFORM_MODEL.md`'s "WSL" section describes specifically a Linux
/// *guest*, which WSL1 does not have) - `IdentityObserver`/`AllocationObserver`'s Unix code
/// paths that work correctly on a genuine Linux kernel are not known to behave the same way
/// there. E20-S02 round-1 independent verifier review found the original, looser
/// `contains("microsoft")` match folded WSL1 into `Wsl2` on the strength of an older,
/// differently-shaped kernel string some WSL1 builds report (`<version>-Microsoft`, no `wsl`
/// token at all) - conflating two architecturally different environments this codebase's own
/// documentation already distinguishes. Matching specifically on `wsl2`/`microsoft-standard`
/// excludes that string while still matching every real WSL2 kernel release documented by
/// Microsoft; a WSL1 host (or any other environment lacking this precise marker) reports
/// `Native` instead - not a wrong-but-plausible guess, an honest "not positively WSL2" (C-03).
// Reachable in production only on Linux (`detect_runtime_environment`'s cfg(linux) branch
// below), but kept available under `cfg(test)` on every host too so its unit tests - the only
// verification this executor can give it without a real WSL2 guest - run everywhere, not only
// on Linux CI.
#[cfg(any(test, target_os = "linux"))]
fn classify_osrelease(osrelease: &str) -> RuntimeEnvironment {
    let lowered = osrelease.to_lowercase();
    if lowered.contains("wsl2") || lowered.contains("microsoft-standard") {
        RuntimeEnvironment::Wsl2
    } else {
        RuntimeEnvironment::Native
    }
}

#[cfg(target_os = "linux")]
fn detect_runtime_environment() -> RuntimeEnvironment {
    match std::fs::read_to_string("/proc/sys/kernel/osrelease") {
        Ok(osrelease) => classify_osrelease(&osrelease),
        Err(_) => RuntimeEnvironment::Native,
    }
}

#[cfg(not(target_os = "linux"))]
fn detect_runtime_environment() -> RuntimeEnvironment {
    // Only a Linux kernel (native or a WSL2 guest) has `/proc/sys/kernel/osrelease` at all;
    // macOS and native Windows are never WSL2 by construction.
    RuntimeEnvironment::Native
}

/// Test-only seam: a fixed [`RuntimeEnvironment`] answer, for exercising WSL2-specific
/// behavior in code that consumes [`EnvironmentObserver`] without depending on the host this
/// test suite happens to run on.
#[derive(Debug, Clone, Copy)]
pub struct SyntheticEnvironmentObserver(pub RuntimeEnvironment);

impl EnvironmentObserver for SyntheticEnvironmentObserver {
    fn detect(&self) -> RuntimeEnvironment {
        self.0
    }
}

/// Where a path's underlying storage sits, relative to a WSL2 guest's own root filesystem.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FilesystemContext {
    /// A native Linux filesystem (the guest's own root, an overlay, tmpfs, ...).
    Linux,
    /// A Windows drive mounted into the WSL2 guest via `drvfs` (conventionally `/mnt/c`) -
    /// carries real performance/permission/atomicity differences from `Linux` above, not
    /// merely a different path prefix (`PLATFORM_MODEL.md`'s "WSL" section).
    WindowsMounted,
    /// A mount this classifier positively observed but does not recognize as either of the
    /// above (e.g. a `9p`/network filesystem) - disclosed with its real `fstype` rather than
    /// silently folded into `Linux` by default.
    Other { fstype: String },
}

/// What filesystem-context classification for one path can tell us. Mirrors this crate's
/// other observers' `Unsupported` convention: a platform/path this classifier cannot resolve
/// is a distinct, typed fact, never a guessed default.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FilesystemContextObservation {
    Classified(FilesystemContext),
    Unsupported { reason: String },
}

/// A source of filesystem-context facts. Mirrors this crate's other observer seams.
pub trait FilesystemContextObserver: Send + Sync {
    fn classify(&self, path: &Path) -> FilesystemContextObservation;
}

/// The real, `/proc/mounts`-backed observer.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemFilesystemContextObserver;

impl FilesystemContextObserver for SystemFilesystemContextObserver {
    fn classify(&self, path: &Path) -> FilesystemContextObservation {
        // Checked here, ahead of the platform-specific implementation below, so a relative
        // path is refused uniformly on every platform rather than only where the real
        // implementation happens to check it itself.
        if !path.is_absolute() {
            return FilesystemContextObservation::Unsupported {
                reason: "filesystem-context classification requires an absolute path".to_string(),
            };
        }
        classify_system_filesystem_context(path)
    }
}

/// Known native-Linux filesystem types this classifier recognizes as
/// [`FilesystemContext::Linux`] outright, rather than the more conservative
/// [`FilesystemContext::Other`]. Deliberately not exhaustive of every Linux filesystem that
/// exists - a type absent from this list is disclosed via `Other { fstype }`, never silently
/// assumed native, so an unrecognized entry is visible rather than mis-classified.
#[cfg(any(test, target_os = "linux"))]
const KNOWN_NATIVE_LINUX_FSTYPES: &[&str] = &[
    "ext4", "ext3", "ext2", "xfs", "btrfs", "overlay", "tmpfs", "proc", "sysfs", "devtmpfs",
    "squashfs", "cgroup2", "devpts", "mqueue",
];

#[cfg(any(test, target_os = "linux"))]
fn classify_fstype(fstype: &str) -> FilesystemContext {
    if fstype == "drvfs" {
        FilesystemContext::WindowsMounted
    } else if KNOWN_NATIVE_LINUX_FSTYPES.contains(&fstype) {
        FilesystemContext::Linux
    } else {
        FilesystemContext::Other {
            fstype: fstype.to_string(),
        }
    }
}

/// `/proc/mounts` (and `/proc/self/mountinfo`, `/etc/mtab`) escape space, tab, newline, and a
/// literal backslash in the device/mountpoint fields as octal `\NNN` sequences - the Linux
/// kernel's own `mangle()` convention (`fs/seq_file.c`), needed because those fields are
/// whitespace-separated: an unescaped space in a mountpoint name would be indistinguishable
/// from a field separator. E20-S02 round-1 independent verifier review found
/// `longest_matching_mount_fstype` compared the still-*escaped* field directly against a real
/// (unescaped) `Path` - a mountpoint containing any of those four characters (a real, common
/// case: `/mnt/My Drive` for a Windows drive with a space in its label) then silently failed
/// to match, and a shorter, less-specific, *wrong* mount won instead - the exact
/// misclassification SI-018/AC2 exist to prevent, not a cosmetic parsing gap. Every escape the
/// kernel actually produces decodes to a single ASCII byte (32/9/10/92), always valid as a
/// `char` on its own, so this never needs to reason about UTF-8 continuation bytes.
#[cfg(any(test, target_os = "linux"))]
fn unescape_proc_mounts_field(field: &str) -> String {
    let chars: Vec<char> = field.chars().collect();
    let mut out = String::with_capacity(field.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 3 < chars.len() {
            let octal: String = chars[i + 1..i + 4].iter().collect();
            if octal.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
                if let Ok(value) = u8::from_str_radix(&octal, 8) {
                    out.push(value as char);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Parses `/proc/mounts`-formatted content (whitespace-separated `device mountpoint fstype
/// options freq passno`, one mount per line) and returns the `fstype` of the mount whose
/// mountpoint is the longest matching prefix of `path` - the same "most specific mount wins"
/// resolution the kernel itself uses (a mount at `/mnt/c/Users` shadows one at `/mnt/c`, which
/// shadows the root `/`). Pure and platform-independent, so it is exhaustively unit-testable
/// with fabricated WSL2 mount tables on any host. A malformed line (fewer than three
/// whitespace-separated fields) is skipped rather than aborting the whole parse.
#[cfg(any(test, target_os = "linux"))]
fn longest_matching_mount_fstype<'a>(mounts: &'a str, path: &Path) -> Option<&'a str> {
    let path_str = path.to_str()?;
    let mut best: Option<(String, &str)> = None; // (unescaped mountpoint, fstype)
    for line in mounts.lines() {
        let mut fields = line.split_whitespace();
        let (Some(_device), Some(raw_mountpoint), Some(fstype)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let mountpoint = unescape_proc_mounts_field(raw_mountpoint);
        let matches = mountpoint == "/"
            || path_str == mountpoint
            || path_str.starts_with(&format!("{mountpoint}/"));
        if !matches {
            continue;
        }
        // `>=`, not `>`: /proc/mounts can list more than one mount at the identical
        // mountpoint (a real WSL2 guest's own root typically shows both a `rootfs` pseudo
        // entry and the real `overlay` mount at `/`) - the *later* line is the one currently
        // in effect (mount stacking shadows chronologically, most recent on top), matching
        // how the kernel itself resolves an overmounted point.
        if best
            .as_ref()
            .is_none_or(|(best_mountpoint, _)| mountpoint.len() >= best_mountpoint.len())
        {
            best = Some((mountpoint, fstype));
        }
    }
    best.map(|(_, fstype)| fstype)
}

#[cfg(target_os = "linux")]
fn classify_system_filesystem_context(path: &Path) -> FilesystemContextObservation {
    match std::fs::read_to_string("/proc/mounts") {
        Ok(mounts) => match longest_matching_mount_fstype(&mounts, path) {
            Some(fstype) => FilesystemContextObservation::Classified(classify_fstype(fstype)),
            None => FilesystemContextObservation::Unsupported {
                reason: "no matching entry in /proc/mounts for this path".to_string(),
            },
        },
        Err(e) => FilesystemContextObservation::Unsupported {
            reason: format!("could not read /proc/mounts: {e}"),
        },
    }
}

#[cfg(not(target_os = "linux"))]
fn classify_system_filesystem_context(_path: &Path) -> FilesystemContextObservation {
    FilesystemContextObservation::Unsupported {
        reason: "filesystem-context classification is only implemented on Linux (including a \
                 WSL2 guest); this platform has no /proc/mounts"
            .to_string(),
    }
}

/// Test-only seam: synthesize filesystem-context facts without touching the real filesystem.
/// A path with no fact explicitly `set` observes as `Unsupported`, matching this crate's other
/// synthetic observers' convention of never inventing a fact the test never configured -
/// except [`SyntheticIdentityObserver`]'s own convention is `Absent` for that case, since
/// "the path does not exist" is a meaningful default there; no equivalent default exists here
/// (a filesystem-context question about an unconfigured path has no honest answer but
/// "unsupported/unknown").
#[derive(Debug, Default)]
pub struct SyntheticFilesystemContextObserver {
    facts: std::collections::BTreeMap<std::path::PathBuf, FilesystemContextObservation>,
}

impl SyntheticFilesystemContextObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(
        &mut self,
        path: impl Into<std::path::PathBuf>,
        observation: FilesystemContextObservation,
    ) -> &mut Self {
        self.facts.insert(path.into(), observation);
        self
    }
}

impl FilesystemContextObserver for SyntheticFilesystemContextObserver {
    fn classify(&self, path: &Path) -> FilesystemContextObservation {
        self.facts
            .get(path)
            .cloned()
            .unwrap_or_else(|| FilesystemContextObservation::Unsupported {
                reason: "no synthetic fact configured for this path".to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- RuntimeEnvironment / osrelease classification: fabricated content standing in for
    // the real `/proc/sys/kernel/osrelease` this executor has no WSL2 guest to read for real.

    #[test]
    fn wsl2_default_kernel_osrelease_is_classified_as_wsl2() {
        assert_eq!(
            classify_osrelease("5.15.153.1-microsoft-standard-WSL2\n"),
            RuntimeEnvironment::Wsl2
        );
    }

    #[test]
    fn wsl1_kernel_osrelease_is_classified_as_native_not_wsl2() {
        // E20-S02 round-1 independent verifier review: WSL1 has no real Linux kernel/guest
        // underneath it (a syscall-translation layer on the Windows NT kernel instead), so
        // folding it into `Wsl2` on the strength of a shared "microsoft" substring was a real
        // misclassification, not a harmless generalization - this must be `Native`.
        assert_eq!(
            classify_osrelease("4.4.0-19041-Microsoft\n"),
            RuntimeEnvironment::Native
        );
    }

    #[test]
    fn a_case_variant_wsl2_marker_is_still_classified_as_wsl2() {
        // The match must not be case-sensitive - a differently-cased real kernel string must
        // not misclassify as Native.
        assert_eq!(
            classify_osrelease("5.15.153.1-MICROSOFT-STANDARD-wsl2\n"),
            RuntimeEnvironment::Wsl2
        );
    }

    #[test]
    fn native_linux_kernel_osrelease_is_classified_as_native() {
        assert_eq!(
            classify_osrelease("5.15.0-91-generic\n"),
            RuntimeEnvironment::Native
        );
    }

    #[test]
    fn empty_or_garbage_osrelease_is_classified_as_native_never_a_guess() {
        assert_eq!(classify_osrelease(""), RuntimeEnvironment::Native);
        assert_eq!(
            classify_osrelease("not-a-real-kernel-string"),
            RuntimeEnvironment::Native
        );
    }

    #[test]
    fn synthetic_environment_observer_reports_exactly_what_was_configured() {
        let wsl2 = SyntheticEnvironmentObserver(RuntimeEnvironment::Wsl2);
        assert_eq!(wsl2.detect(), RuntimeEnvironment::Wsl2);
        let native = SyntheticEnvironmentObserver(RuntimeEnvironment::Native);
        assert_eq!(native.detect(), RuntimeEnvironment::Native);
    }

    #[test]
    fn system_environment_observer_never_panics_on_this_host() {
        // AC1 ("WSL detection is explicit"): the real observer must always return an answer,
        // never fail/panic, regardless of what platform this test suite happens to run on.
        let observer = SystemEnvironmentObserver;
        let _ = observer.detect();
    }

    // --- Filesystem-context classification: a fabricated WSL2-shaped `/proc/mounts`,
    // standing in for the real file this executor has no WSL2 guest to read for real.

    const FABRICATED_WSL2_MOUNTS: &str = "\
rootfs / rootfs rw 0 0
none / overlay rw,relatime 0 0
none /tmp tmpfs rw,relatime 0 0
C:\\ /mnt/c drvfs rw,relatime,uid=1000,gid=1000,case=off 0 0
D:\\ /mnt/d drvfs rw,relatime,uid=1000,gid=1000,case=off 0 0
none /mnt/wsl tmpfs rw,relatime 0 0
\\\\server\\share /mnt/network 9p rw,dirsync,aname=drvfs;path=\\\\server\\share 0 0
";

    #[test]
    fn a_path_under_the_native_root_is_classified_linux() {
        assert_eq!(
            longest_matching_mount_fstype(FABRICATED_WSL2_MOUNTS, Path::new("/home/user/project")),
            Some("overlay")
        );
    }

    #[test]
    fn a_path_under_a_windows_drive_mount_is_classified_windows_mounted() {
        let fstype = longest_matching_mount_fstype(
            FABRICATED_WSL2_MOUNTS,
            Path::new("/mnt/c/Users/someone/AppData/Roaming/Claude"),
        )
        .expect("must match /mnt/c");
        assert_eq!(classify_fstype(fstype), FilesystemContext::WindowsMounted);
    }

    #[test]
    fn the_most_specific_mount_wins_over_a_shorter_matching_prefix() {
        // `/mnt/c` (drvfs) and the root `/` (overlay) both lexically match a path under
        // `/mnt/c` - the longest/most specific mountpoint must win, matching real kernel mount
        // resolution, not "first line in the table wins" or "shortest prefix wins".
        assert_eq!(
            longest_matching_mount_fstype(FABRICATED_WSL2_MOUNTS, Path::new("/mnt/c/Users")),
            Some("drvfs")
        );
    }

    #[test]
    fn an_unrecognized_fstype_is_disclosed_as_other_not_silently_absorbed() {
        let fstype = longest_matching_mount_fstype(
            FABRICATED_WSL2_MOUNTS,
            Path::new("/mnt/network/some/file"),
        )
        .expect("must match /mnt/network");
        assert_eq!(
            classify_fstype(fstype),
            FilesystemContext::Other {
                fstype: "9p".to_string()
            }
        );
    }

    #[test]
    fn unescape_proc_mounts_field_decodes_the_kernels_octal_escapes() {
        // The four characters the kernel's own `mangle()` actually escapes (space, tab,
        // newline, backslash), plus a plain string with nothing to decode.
        assert_eq!(unescape_proc_mounts_field(r"My\040Drive"), "My Drive");
        assert_eq!(unescape_proc_mounts_field(r"a\011b"), "a\tb");
        assert_eq!(unescape_proc_mounts_field(r"a\012b"), "a\nb");
        assert_eq!(unescape_proc_mounts_field(r"C:\134"), "C:\\");
        assert_eq!(unescape_proc_mounts_field("plain"), "plain");
    }

    #[test]
    fn a_mountpoint_containing_an_escaped_space_still_matches_its_real_path() {
        // E20-S02 round-1 independent verifier review's exact reproduction: a Windows drive
        // mounted at a name containing a space (`/mnt/My Drive`, escaped as `/mnt/My\040Drive`
        // in the raw file) must still resolve to `drvfs`, not silently fall through to a
        // shorter, wrong match (the root `overlay`) because the comparison never decoded it.
        let mounts = "none / overlay rw 0 0\nC:\\\\ /mnt/My\\040Drive drvfs rw 0 0\n";
        assert_eq!(
            longest_matching_mount_fstype(mounts, Path::new("/mnt/My Drive/file")),
            Some("drvfs")
        );
    }

    #[test]
    fn a_path_with_no_matching_mount_entry_is_none() {
        assert_eq!(
            longest_matching_mount_fstype("", Path::new("/anything")),
            None
        );
    }

    #[test]
    fn a_malformed_line_is_skipped_not_fatal_to_the_whole_parse() {
        let mounts = "this line has too few fields\n/ / ext4 rw 0 0\n";
        assert_eq!(
            longest_matching_mount_fstype(mounts, Path::new("/anywhere")),
            Some("ext4")
        );
    }

    #[test]
    fn classify_fstype_maps_known_native_types_to_linux() {
        for fstype in ["ext4", "btrfs", "overlay", "tmpfs"] {
            assert_eq!(classify_fstype(fstype), FilesystemContext::Linux);
        }
    }

    #[test]
    fn classify_fstype_maps_drvfs_to_windows_mounted() {
        assert_eq!(classify_fstype("drvfs"), FilesystemContext::WindowsMounted);
    }

    #[test]
    fn synthetic_filesystem_context_observer_reports_unsupported_for_unset_paths() {
        let observer = SyntheticFilesystemContextObserver::new();
        assert_eq!(
            observer.classify(Path::new("/never/configured")),
            FilesystemContextObservation::Unsupported {
                reason: "no synthetic fact configured for this path".to_string()
            }
        );
    }

    #[test]
    fn synthetic_filesystem_context_observer_reports_exactly_what_was_set() {
        let mut observer = SyntheticFilesystemContextObserver::new();
        observer.set(
            "/mnt/c/Users/someone",
            FilesystemContextObservation::Classified(FilesystemContext::WindowsMounted),
        );
        assert_eq!(
            observer.classify(Path::new("/mnt/c/Users/someone")),
            FilesystemContextObservation::Classified(FilesystemContext::WindowsMounted)
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn system_filesystem_context_observer_reports_unsupported_off_linux() {
        let observer = SystemFilesystemContextObserver;
        match observer.classify(Path::new("/")) {
            FilesystemContextObservation::Unsupported { .. } => {}
            other => panic!("expected Unsupported off Linux, got {other:?}"),
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn system_filesystem_context_observer_classifies_a_real_path_on_linux() {
        // Every real Linux host - WSL2 guest or otherwise - has at least a root mount.
        let observer = SystemFilesystemContextObserver;
        match observer.classify(Path::new("/")) {
            FilesystemContextObservation::Classified(_) => {}
            other => panic!("expected a real classification for '/' on Linux, got {other:?}"),
        }
    }

    #[test]
    fn a_relative_path_is_unsupported_not_silently_resolved_against_cwd() {
        let observer = SystemFilesystemContextObserver;
        assert_eq!(
            observer.classify(Path::new("relative/path")),
            FilesystemContextObservation::Unsupported {
                reason: "filesystem-context classification requires an absolute path".to_string()
            }
        );
    }
}

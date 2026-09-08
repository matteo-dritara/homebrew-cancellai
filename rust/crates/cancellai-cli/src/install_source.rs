//! Installation-source awareness (E17-S04): detect which package channel this binary was
//! installed through, from its own resolved executable path, and route upgrade guidance
//! through that same channel - never a different one (the story's AC: `update --check` never
//! silently switches package channels).
//!
//! No package manager currently registers itself with this binary (E17-S02's build pipeline
//! produces plain archives, not packages yet - `docs/RELEASING.md`'s "Beta side-by-side"
//! section), so detection is a path heuristic, not a query to a real package database. An
//! unmatched path reports [`InstallSource::Unknown`] honestly rather than guessing toward
//! whichever pattern looks closest (C-02: unknown state is never escalated toward a specific
//! claim it cannot support).

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallSource {
    Homebrew,
    WindowsPackage,
    LinuxPackage,
    DirectDownload,
    Unknown,
}

impl InstallSource {
    /// Stable, machine-readable label - not `Debug`, which is not a contract.
    pub fn label(self) -> &'static str {
        match self {
            InstallSource::Homebrew => "homebrew",
            InstallSource::WindowsPackage => "windows_package",
            InstallSource::LinuxPackage => "linux_package",
            InstallSource::DirectDownload => "direct_download",
            InstallSource::Unknown => "unknown",
        }
    }

    /// Upgrade guidance for this source specifically. `update --check` always prints the
    /// guidance for the source it just detected, never a different one - this function has no
    /// input other than `self`, so there is no path through it that could name another
    /// channel's mechanism instead.
    pub fn upgrade_guidance(self) -> &'static str {
        match self {
            InstallSource::Homebrew => {
                "Installed via Homebrew: run `brew upgrade cancellai-cli` (or `brew upgrade \
                 cancellai` for the Python v1 formula) - do not download a raw archive over a \
                 Homebrew-managed install."
            }
            InstallSource::WindowsPackage => {
                "Installed via a Windows package: reinstall the latest package from the same \
                 source you originally installed from."
            }
            InstallSource::LinuxPackage => {
                "Installed via a Linux system package: update through the same package manager \
                 that installed this binary."
            }
            InstallSource::DirectDownload => {
                "Installed via direct download: download the latest release archive for your \
                 platform and replace this binary. See docs/RELEASING.md and the release \
                 manifest (project/schemas/release_manifest.schema.json) for how canonical \
                 artifacts are named and checksummed."
            }
            InstallSource::Unknown => {
                "Installation source could not be determined from this binary's own path; no \
                 upgrade channel is assumed. See docs/RELEASING.md for how to obtain the latest \
                 release."
            }
        }
    }
}

/// Detect install source from the running binary's own resolved path.
///
/// Homebrew (including Linuxbrew) is checked first and independent of host OS, because a
/// `Cellar` path is the one pattern this codebase can assert with real confidence regardless
/// of platform. Every other pattern is platform-specific: an unmatched path on a recognized
/// platform is `DirectDownload` (a real, if unpackaged, install), never silently promoted to a
/// package-manager claim it cannot support.
pub fn detect_from_path(exe_path: &Path) -> InstallSource {
    let path_str = exe_path.to_string_lossy();
    if path_str.contains("/Cellar/") || path_str.contains("/homebrew/") {
        return InstallSource::Homebrew;
    }
    if cfg!(windows) {
        let lower = path_str.to_lowercase();
        if lower.contains("program files") || lower.contains(r"\appdata\local\programs\") {
            return InstallSource::WindowsPackage;
        }
        return InstallSource::DirectDownload;
    }
    if cfg!(target_os = "linux") {
        if path_str.starts_with("/usr/bin/") || path_str.starts_with("/usr/lib/") {
            return InstallSource::LinuxPackage;
        }
        return InstallSource::DirectDownload;
    }
    if cfg!(target_os = "macos") {
        return InstallSource::DirectDownload;
    }
    InstallSource::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homebrew_cellar_path_is_detected_independent_of_host_platform() {
        assert_eq!(
            detect_from_path(Path::new(
                "/opt/homebrew/Cellar/cancellai-cli/1.0.0/bin/cancellai-cli"
            )),
            InstallSource::Homebrew
        );
        assert_eq!(
            detect_from_path(Path::new(
                "/home/linuxbrew/.linuxbrew/Cellar/cancellai-cli/1.0.0/bin/cancellai-cli"
            )),
            InstallSource::Homebrew
        );
    }

    #[test]
    fn every_source_has_its_own_non_empty_guidance() {
        for source in [
            InstallSource::Homebrew,
            InstallSource::WindowsPackage,
            InstallSource::LinuxPackage,
            InstallSource::DirectDownload,
            InstallSource::Unknown,
        ] {
            assert!(!source.upgrade_guidance().is_empty(), "{source:?}");
            assert!(!source.label().is_empty(), "{source:?}");
        }
    }

    #[test]
    fn each_sources_guidance_names_no_other_sources_own_mechanism() {
        // AC: update --check never silently switches package channels - a source's own
        // guidance text must not itself point at a *different* source's mechanism.
        let mechanism_keyword = |s: InstallSource| -> Option<&'static str> {
            match s {
                InstallSource::Homebrew => Some("brew"),
                InstallSource::WindowsPackage => Some("Windows package"),
                InstallSource::LinuxPackage => Some("Linux system package"),
                InstallSource::DirectDownload => Some("direct download"),
                InstallSource::Unknown => None,
            }
        };
        let all = [
            InstallSource::Homebrew,
            InstallSource::WindowsPackage,
            InstallSource::LinuxPackage,
            InstallSource::DirectDownload,
            InstallSource::Unknown,
        ];
        for source in all {
            let guidance = source.upgrade_guidance();
            for other in all {
                if other == source {
                    continue;
                }
                if let Some(keyword) = mechanism_keyword(other) {
                    assert!(
                        !guidance.contains(keyword),
                        "{source:?}'s guidance unexpectedly names {other:?}'s mechanism ({keyword:?}): {guidance:?}"
                    );
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_system_bin_path_is_a_linux_package() {
        assert_eq!(
            detect_from_path(Path::new("/usr/bin/cancellai-cli")),
            InstallSource::LinuxPackage
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_arbitrary_extracted_path_is_direct_download() {
        assert_eq!(
            detect_from_path(Path::new(
                "/home/user/downloads/cancellai-cli-1.0.0/cancellai-cli"
            )),
            InstallSource::DirectDownload
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_non_homebrew_path_is_direct_download() {
        assert_eq!(
            detect_from_path(Path::new("/Users/dev/Downloads/cancellai-cli")),
            InstallSource::DirectDownload
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_program_files_path_is_a_windows_package() {
        assert_eq!(
            detect_from_path(Path::new(r"C:\Program Files\cancellai\cancellai-cli.exe")),
            InstallSource::WindowsPackage
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_arbitrary_path_is_direct_download() {
        assert_eq!(
            detect_from_path(Path::new(r"C:\Users\dev\Downloads\cancellai-cli.exe")),
            InstallSource::DirectDownload
        );
    }
}

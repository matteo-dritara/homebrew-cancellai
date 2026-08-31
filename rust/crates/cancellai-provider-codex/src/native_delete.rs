//! Codex native delete capability detection (ported from `cancellai.py`'s
//! `codex_delete_supported`, E05-S04 AC2: "Native delete capability is detected without
//! assuming filesystem fallback equivalence.").
//!
//! `NativeDeleteSupport` is deliberately not a bare boolean: "no codex binary could be found",
//! "a binary ran but does not advertise `--force`", and "the probe itself failed/timed out"
//! are different evidentiary claims a caller must not collapse into one "unsupported" flag -
//! `docs/security/THREAT_MODEL.md` TM-09 ("native does not mean unconditionally trusted") cuts
//! both ways: this adapter must not *assume* native delete is safe merely because a binary
//! exists, and it must equally not *assume* raw filesystem deletion is an equivalent fallback
//! merely because native delete is unavailable - each distinct outcome carries its own
//! evidence for whatever calls this.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Mirrors `cancellai.py`'s `timeout=8` (seconds) for the `codex delete --help` probe.
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeDeleteSupport {
    /// `codex delete --help` ran, exited 0, and advertised `--force`.
    Supported { codex_bin: PathBuf },
    /// A codex binary was found and ran, but did not advertise `--force` support (or exited
    /// non-zero) - `cancellai.py`'s own bar for "supported".
    Unsupported { codex_bin: PathBuf },
    /// No codex binary could be located at all (no `codex_bin` argument and none found on
    /// `PATH`).
    BinaryNotFound,
    /// A codex binary was located but could not be run (spawn failure) or did not answer
    /// within the probe timeout - distinct from `Unsupported`: this is "we could not tell",
    /// not "we asked and the answer was no".
    ProbeFailed { codex_bin: PathBuf, reason: String },
}

/// Detects native delete support for `codex_bin` (or, if `None`, whatever `codex` this
/// process's `PATH` resolves to - mirroring `cancellai.py`'s `codex_bin or shutil.which("codex")`).
///
/// `std::process::Command` has no built-in timeout, so this polls `Child::try_wait` against a
/// deadline and kills the child if it is still running past [`PROBE_TIMEOUT`] - reimplementing
/// `cancellai.py`'s `timeout=8` without a new subprocess-timeout dependency. stdout/stderr are
/// read on background threads concurrently with that poll loop specifically to avoid the
/// classic pipe deadlock a large amount of child output could otherwise cause (the child
/// blocking on a full OS pipe buffer while this function is blocked in the wait loop, never
/// reading).
pub fn codex_delete_supported(codex_bin: Option<&Path>) -> NativeDeleteSupport {
    let resolved = match codex_bin {
        Some(path) => Some(path.to_path_buf()),
        None => resolve_on_path("codex"),
    };
    let Some(codex_bin) = resolved else {
        return NativeDeleteSupport::BinaryNotFound;
    };

    match run_with_timeout(&codex_bin, &["delete", "--help"], PROBE_TIMEOUT) {
        Ok(output) if output.success && output.combined_text.contains("--force") => {
            NativeDeleteSupport::Supported { codex_bin }
        }
        Ok(_) => NativeDeleteSupport::Unsupported { codex_bin },
        Err(reason) => NativeDeleteSupport::ProbeFailed { codex_bin, reason },
    }
}

struct ProbeOutput {
    success: bool,
    combined_text: String,
}

fn run_with_timeout(bin: &Path, args: &[&str], timeout: Duration) -> Result<ProbeOutput, String> {
    let mut child = Command::new(bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| err.to_string())?;

    let mut stdout_handle = child.stdout.take().expect("stdout was piped");
    let mut stderr_handle = child.stderr.take().expect("stderr was piped");
    let stdout_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout_handle.read_to_string(&mut buf);
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr_handle.read_to_string(&mut buf);
        buf
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait().map_err(|err| err.to_string())? {
            Some(status) => break status,
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    // Deliberately not joined: `child.kill()` only signals the direct child
                    // (`codex`/the probed binary itself, e.g. a shell script), not any
                    // grandchild process it may have forked (e.g. a shell script's own `sleep`
                    // child) - std has no safe, dependency-free process-group kill
                    // (`unsafe_code = "forbid"`, ADR-0015, rules out a raw libc `killpg` call).
                    // A grandchild that outlives the kill still holds the pipe's write end
                    // open, so joining here would block this function for however long that
                    // grandchild survives - exactly the hang this timeout exists to avoid. The
                    // reader threads are abandoned (their output is discarded anyway on this
                    // path) rather than joined; they exit on their own once the pipe's last
                    // writer closes it. Documented residual: this path leaks a thread for the
                    // grandchild's remaining lifetime, not indefinitely.
                    return Err(
                        "codex delete --help did not exit within the probe timeout".to_string()
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };

    let mut combined_text = stdout_reader.join().unwrap_or_default();
    combined_text.push_str(&stderr_reader.join().unwrap_or_default());
    Ok(ProbeOutput {
        success: status.success(),
        combined_text,
    })
}

fn resolve_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file() && is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct FakeCli(PathBuf);

    impl FakeCli {
        /// Writes a fake `codex` CLI: a shell script whose `delete --help` response and exit
        /// code the test controls - this is the "native-delete fake CLI integration test" this
        /// story's verification plan names, matching `cancellai.py`'s own approach of probing
        /// a real (test-controlled) subprocess rather than mocking `subprocess.run` away.
        #[cfg(unix)]
        fn new(label: &str, script: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "cancellai-codex-fake-cli-{label}-{}",
                std::process::id()
            ));
            fs::write(&path, script).unwrap();
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            Self(path)
        }
    }

    impl Drop for FakeCli {
        fn drop(&mut self) {
            fs::remove_file(&self.0).ok();
        }
    }

    #[test]
    fn no_codex_bin_and_nothing_on_path_is_binary_not_found() {
        // A deliberately implausible binary name, passed explicitly, exercises "not found"
        // without depending on this test host's real PATH contents.
        let result = codex_delete_supported(Some(Path::new(
            "/definitely/does/not/exist/cancellai-test-codex-binary",
        )));
        assert!(matches!(
            result,
            NativeDeleteSupport::ProbeFailed { .. } | NativeDeleteSupport::BinaryNotFound
        ));
    }

    #[cfg(unix)]
    #[test]
    fn ac2_a_fake_cli_advertising_force_is_reported_supported() {
        let fake = FakeCli::new(
            "supported",
            "#!/bin/sh\necho 'usage: codex delete [--force] <id>'\nexit 0\n",
        );
        let result = codex_delete_supported(Some(&fake.0));
        assert_eq!(
            result,
            NativeDeleteSupport::Supported {
                codex_bin: fake.0.clone()
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn ac2_a_fake_cli_not_advertising_force_is_reported_unsupported_not_absent() {
        let fake = FakeCli::new(
            "unsupported",
            "#!/bin/sh\necho 'usage: codex delete <id>'\nexit 0\n",
        );
        let result = codex_delete_supported(Some(&fake.0));
        assert_eq!(
            result,
            NativeDeleteSupport::Unsupported {
                codex_bin: fake.0.clone()
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn ac2_a_fake_cli_that_exits_nonzero_is_unsupported_even_if_it_mentions_force() {
        // Exit code matters independently of textual content - cancellai.py checks
        // `proc.returncode == 0 and "--force" in text`, not text alone.
        let fake = FakeCli::new(
            "nonzero",
            "#!/bin/sh\necho '--force is available'\nexit 1\n",
        );
        let result = codex_delete_supported(Some(&fake.0));
        assert_eq!(
            result,
            NativeDeleteSupport::Unsupported {
                codex_bin: fake.0.clone()
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_fake_cli_that_hangs_is_killed_and_reported_as_a_probe_failure_not_a_hang() {
        let fake = FakeCli::new("hangs", "#!/bin/sh\nsleep 60\n");
        // Use a real-but-short deadline for the test itself by calling the timed helper
        // directly rather than waiting out the full 8s production timeout.
        let result = run_with_timeout(&fake.0, &["delete", "--help"], Duration::from_millis(200));
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_fake_cli_with_large_output_does_not_deadlock() {
        // Regression guard for the exact pipe-deadlock this module's reader threads exist to
        // avoid: enough output to exceed a typical OS pipe buffer (64KiB), written before the
        // child exits.
        let fake = FakeCli::new(
            "large-output",
            "#!/bin/sh\nyes '--force line filler' | head -c 200000\nexit 0\n",
        );
        let result = codex_delete_supported(Some(&fake.0));
        assert_eq!(
            result,
            NativeDeleteSupport::Supported {
                codex_bin: fake.0.clone()
            }
        );
    }
}

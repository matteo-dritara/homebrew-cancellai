//! Provider process-liveness as its own OS capability seam (E06-S01, ported from
//! `cancellai.py`'s `active_processes`).
//!
//! Mirrors this crate's other seams: production code takes `&dyn ProcessObserver` and uses
//! [`SystemProcessObserver`]; tests use [`SyntheticProcessObserver`]. This is the one place a
//! retention/clean decision can learn whether a provider process might currently be writing to
//! the very artifact a plan is about to delete - mtime alone cannot rule that out, and treating
//! "no evidence either way" as "not running" would be exactly the SI-008/SI-009 mistake this
//! seam exists to prevent (`cancellai.py`'s own comment: "Unknown activity is not absence of
//! activity").

use std::process::Command;
use std::time::Duration;

/// The result of one liveness probe for a set of provider process names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessObservation {
    /// Whether the probe itself succeeded well enough to trust a negative result. `false`
    /// means "unknown," never "not running" - a caller must treat every named process as
    /// possibly running when this is `false` (fail closed, matching `cancellai.py`'s
    /// `ProcessObservation.complete`).
    pub complete: bool,
    /// Which of the probed names were actually observed running. Only meaningful when
    /// `complete` is `true`; a caller that ignores `complete` and reads this directly could
    /// mistake "the probe never ran" for "nothing is running."
    pub running_names: Vec<String>,
}

impl ProcessObservation {
    /// Whether `name` was observed running - fails closed to `true` (possibly running) when
    /// the underlying probe was not `complete`, so a caller cannot get "not running" out of an
    /// incomplete observation by forgetting to check `complete` separately.
    pub fn is_running(&self, name: &str) -> bool {
        !self.complete || self.running_names.iter().any(|n| n == name)
    }
}

/// A source of provider process-liveness facts.
pub trait ProcessObserver: Send + Sync {
    /// Probe whether any process whose name matches one of `names` is currently running.
    fn observe(&self, names: &[&str]) -> ProcessObservation;
}

/// The real, OS-backed observer: shells out to `ps -axo pid=,comm=` with a bounded timeout,
/// matching `cancellai.py::active_processes` exactly (same arguments, same 5s timeout, same
/// fail-closed-on-any-error contract). Best-effort exact-name matching: false negatives
/// remain possible even on success (a differently-named binary, a container/namespace `ps`
/// cannot see into), so this is never the sole safety control - it is one named constraint
/// among several, the same way every other `AuthorityConstraint` is (`cancellai-safety::
/// authority`).
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemProcessObserver;

impl ProcessObserver for SystemProcessObserver {
    fn observe(&self, names: &[&str]) -> ProcessObservation {
        let unknown = || ProcessObservation {
            complete: false,
            running_names: Vec::new(),
        };
        let ps_bin = which_ps().unwrap_or_else(|| "/bin/ps".to_string());
        let Ok(child) = Command::new(&ps_bin)
            .args(["-axo", "pid=,comm="])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
        else {
            return unknown();
        };
        let Some(output) = run_with_timeout(child, Duration::from_secs(5)) else {
            return unknown();
        };
        if !output.status.success() {
            return unknown();
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut running_names = Vec::new();
        for line in text.lines() {
            let comm = line.trim().rsplit(' ').next().unwrap_or("").trim();
            let base = comm.rsplit('/').next().unwrap_or(comm);
            if names.contains(&base) && !running_names.iter().any(|n: &String| n == base) {
                running_names.push(base.to_string());
            }
        }
        ProcessObservation {
            complete: true,
            running_names,
        }
    }
}

fn which_ps() -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("ps");
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

/// Runs `child` to completion, or kills it and returns `None` if it does not exit within
/// `timeout` - a hand-rolled bound since this workspace has no async runtime/process-timeout
/// dependency to reach for instead, mirroring `cancellai.py`'s `subprocess.run(..., timeout=5)`.
fn run_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Option<std::process::Output> {
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    }
}

/// Test-only seam: synthesize a liveness observation without spawning a real process.
#[derive(Debug, Clone)]
pub struct SyntheticProcessObserver {
    observation: ProcessObservation,
}

impl SyntheticProcessObserver {
    pub fn complete(running_names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            observation: ProcessObservation {
                complete: true,
                running_names: running_names.into_iter().map(Into::into).collect(),
            },
        }
    }

    pub fn incomplete() -> Self {
        Self {
            observation: ProcessObservation {
                complete: false,
                running_names: Vec::new(),
            },
        }
    }
}

impl ProcessObserver for SyntheticProcessObserver {
    fn observe(&self, _names: &[&str]) -> ProcessObservation {
        self.observation.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_incomplete_observation_reports_every_name_as_possibly_running() {
        let observation = ProcessObservation {
            complete: false,
            running_names: Vec::new(),
        };
        assert!(observation.is_running("codex"));
        assert!(observation.is_running("claude"));
    }

    #[test]
    fn a_complete_observation_only_reports_names_actually_seen() {
        let observation = ProcessObservation {
            complete: true,
            running_names: vec!["codex".to_string()],
        };
        assert!(observation.is_running("codex"));
        assert!(!observation.is_running("claude"));
    }

    #[test]
    fn the_real_observer_returns_a_well_formed_result_on_this_platform() {
        // Not asserting a specific process list (non-deterministic across machines/CI) - only
        // that the probe itself does not panic and produces a coherent, typed result, the same
        // sanity-bound spirit as `SystemClock`'s own test.
        let observation = SystemProcessObserver.observe(&["definitely-not-a-real-process-name"]);
        if observation.complete {
            assert!(
                !observation
                    .running_names
                    .contains(&"definitely-not-a-real-process-name".to_string())
            );
        }
    }
}

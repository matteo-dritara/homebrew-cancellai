//! Claude session discovery (ported from `cancellai.py`'s `discover_claude_sessions`,
//! E05-S03).
//!
//! ## Scan completeness (E21-S03)
//!
//! E06-S02 gave this walk a completeness signal for exactly one branch: a session's *companion
//! payload* directory that could not be fully listed ([`SessionDiscoveryResult::
//! degraded_companions`]). The 2026-09-03 target-engine review
//! (`docs/audits/2026-09-03-CODE_REVIEW.md`, `CR-TE-01`) found the branch next to it still
//! open, and disclosed nowhere: an unreadable **project** directory under `projects/` was
//! dropped by a bare `else { continue }`, so the scope reported complete while a whole
//! project's sessions had never been seen. `cancellai.py` records that same failure through
//! `scan.record(project_dir, exc)` and withholds the entire tool.
//!
//! Discovery now carries [`cancellai_inventory::ScopeCompleteness`] (ADR-0018) covering every
//! way this walk can fail to observe part of the tree, mapped to the reference's own
//! `observe`/`iterdir` error channel:
//!
//! - a missing or symlinked `projects/` is `Unavailable` and **not** a reason - a structurally
//!   empty install is a known state, not missing evidence (SI-009);
//! - a `projects/` that exists and cannot be listed **is** a reason;
//! - a project directory that cannot be listed is a reason - the branch `CR-TE-01` found;
//! - a session file whose metadata cannot be read is a reason;
//! - a companion payload directory that could not be fully walked is a reason, and stays in
//!   `degraded_companions` as well, because that field answers a different question: *which
//!   artifact's* own evidence is degraded, for per-artifact confidence.
//!
//! Claude's session relationships are flat: each project directory groups zero or more
//! top-level session transcripts. There is no parent/child rollout graph the way Codex has
//! (`cancellai-provider-codex`'s own E05-S04 concern - not ported here, and not invented as a
//! payload type this crate does not need, matching E05-S01's deferred-payload precedent).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use cancellai_inventory::{CompletenessReason, ReasonLog, ScopeObservation};
use cancellai_provider_api::extract_uuid;

/// One discovered session transcript, with its companion payload directory's size/mtime
/// folded in when a companion exists (`cancellai.py`'s `discover_claude_sessions`: "size +=
/// directory_size(companion, scan)").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeSession {
    pub project: String,
    pub session_id: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified: Option<SystemTime>,
    pub companion_payload: Option<PathBuf>,
}

/// Whether `claude_home/projects` itself could be observed at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDiscoveryScope {
    /// `projects/` was present, not a symlink, and listable - `sessions` reflects everything
    /// this walk could read (see `degraded_companions` on [`SessionDiscoveryResult`] for
    /// companions that could not be *fully* read without the whole session going missing).
    Observed,
    /// `projects/` is missing or is a symlink - `cancellai.py`'s own early return, and a
    /// structurally empty install rather than missing evidence.
    Unavailable,
    /// `projects/` exists but could not be observed at all (permission, I/O). E21 round-1
    /// independent review found this collapsed into `Unavailable`, and `resolve_claude`'s
    /// early return for that variant then converted a genuinely `Unknown` observation into a
    /// clean empty scan - a real `clean --yes` exited `0` where the reference exits `4`.
    /// Distinguishing the two at the type level is what stops that from being reachable
    /// (SI-009: missing evidence is never absence of data).
    Unobservable,
}

/// The result of one `discover_claude_sessions` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDiscoveryResult {
    pub scope: SessionDiscoveryScope,
    pub sessions: Vec<ClaudeSession>,
    /// How completely this walk was able to observe the scope, with the truthful number of
    /// unobserved paths (E21-S03, ADR-0018). Authoritative for withholding:
    /// `degraded_companions` below is per-artifact attribution, not the scope verdict.
    pub observation: ScopeObservation,
    /// Companion payload directories whose own listing failed partway through (a locked
    /// subdirectory, a permission error) - the session itself is still reported (its own
    /// `.jsonl` file was readable), but its `size_bytes`/`modified` may be an undercount
    /// (SI-008/SI-009: a partial observation is reported as partial, never silently folded
    /// into a clean-looking total). `cancellai.py` records the same fact into its `Scan`
    /// object instead of returning it inline; this crate has no `Scan` type of its own to
    /// route through, so it is a direct field here.
    pub degraded_companions: Vec<PathBuf>,
}

/// Classifies one filesystem error into `cancellai-inventory`'s reason vocabulary, so a
/// permission denial, a mid-walk disappearance and an unclassifiable I/O failure stay
/// distinguishable (SI-010).
fn reason_for(path: &Path, error: &io::Error) -> CompletenessReason {
    match error.kind() {
        io::ErrorKind::PermissionDenied => CompletenessReason::PermissionDenied {
            path: path.to_path_buf(),
        },
        io::ErrorKind::NotFound => CompletenessReason::Disappeared {
            path: path.to_path_buf(),
        },
        _ => CompletenessReason::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        },
    }
}

pub fn discover_claude_sessions(claude_home: &Path) -> SessionDiscoveryResult {
    let projects = claude_home.join("projects");

    /// A `projects/` that is genuinely absent or symlinked: a structurally empty install, and
    /// the one shape that legitimately yields a `Complete` observation with no sessions.
    fn structurally_empty() -> SessionDiscoveryResult {
        SessionDiscoveryResult {
            scope: SessionDiscoveryScope::Unavailable,
            sessions: Vec::new(),
            observation: ScopeObservation::complete(),
            degraded_companions: Vec::new(),
        }
    }

    /// A `projects/` that exists and could not be observed. Kept distinct from the above at
    /// the type level: `resolve_claude` previously collapsed both into an empty `Complete`
    /// resolution, which is exactly how an unreadable root escaped withholding (E21 round-1
    /// independent review).
    fn unobservable(path: &Path, error: &io::Error) -> SessionDiscoveryResult {
        let mut log = ReasonLog::new();
        log.record_root_unavailable(CompletenessReason::ScopeRootUnavailable {
            path: path.to_path_buf(),
            detail: error.to_string(),
        });
        SessionDiscoveryResult {
            scope: SessionDiscoveryScope::Unobservable,
            sessions: Vec::new(),
            observation: log.into_observation(),
            degraded_companions: Vec::new(),
        }
    }

    let projects_meta = match fs::symlink_metadata(&projects) {
        Ok(meta) => meta,
        // Mirrors `cancellai.py::observe`: absent is a known-empty state and records nothing;
        // any other failure is missing evidence and must reduce authority.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return structurally_empty(),
        Err(error) => return unobservable(&projects, &error),
    };
    if projects_meta.file_type().is_symlink() || !projects_meta.is_dir() {
        return structurally_empty();
    }
    let project_entries = match fs::read_dir(&projects) {
        Ok(entries) => entries,
        Err(error) => return unobservable(&projects, &error),
    };

    let mut sessions = Vec::new();
    let mut degraded_companions = Vec::new();
    let mut log = ReasonLog::new();

    for project_entry in project_entries {
        let project_entry = match project_entry {
            Ok(entry) => entry,
            Err(error) => {
                log.record(reason_for(&projects, &error));
                continue;
            }
        };
        let project_path = project_entry.path();
        let project_file_type = match project_entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                log.record(reason_for(&project_path, &error));
                continue;
            }
        };
        if !project_file_type.is_dir() || project_file_type.is_symlink() {
            continue;
        }
        let project_name = project_entry.file_name().to_string_lossy().into_owned();
        let children = match fs::read_dir(&project_path) {
            Ok(children) => children,
            // `CR-TE-01`'s undisclosed Claude branch. The reference records exactly here
            // (`scan.record(project_dir, exc)`) and withholds the whole tool; this used to be
            // a bare `continue`, so an entire project's sessions could go unseen while the
            // scope still reported complete.
            Err(error) => {
                log.record(reason_for(&project_path, &error));
                continue;
            }
        };

        for child in children {
            let child = match child {
                Ok(child) => child,
                Err(error) => {
                    log.record(reason_for(&project_path, &error));
                    continue;
                }
            };
            let child_type = match child.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    log.record(reason_for(&child.path(), &error));
                    continue;
                }
            };
            if !child_type.is_file() || child_type.is_symlink() {
                continue;
            }
            let child_path = child.path();
            if child_path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(stem) = child_path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(session_id) = extract_uuid(stem) else {
                continue;
            };
            // The reference's `except OSError: scan.record(p, exc)` on `p.stat()`: a session
            // whose own metadata cannot be read is evidence this scan could not obtain, not a
            // session that is absent.
            let metadata = match child_path.symlink_metadata() {
                Ok(metadata) => metadata,
                Err(error) => {
                    log.record(reason_for(&child_path, &error));
                    continue;
                }
            };

            let mut size = metadata.len();
            // `.ok()` here silently turned "this platform/filesystem could not report an mtime"
            // into "no mtime", which downstream reads as an unknown activity state without ever
            // saying why (E21 round-1 review, SI-010). The failure is now recorded.
            let mut modified = match metadata.modified() {
                Ok(value) => Some(value),
                Err(error) => {
                    log.record(CompletenessReason::Io {
                        path: child_path.clone(),
                        message: format!("modification time unavailable: {error}"),
                    });
                    None
                }
            };
            let companion = project_path.join(stem);
            let mut companion_payload = None;

            // An `if let Ok(...)` here discarded a companion directory that exists but cannot be
            // stat'ed - the session's size and mtime would then silently exclude it while the
            // scope still read complete. Absent is fine and records nothing; anything else is
            // missing evidence.
            match fs::symlink_metadata(&companion) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => log.record(reason_for(&companion, &error)),
                Ok(companion_meta) => {
                    if companion_meta.is_dir() && !companion_meta.file_type().is_symlink() {
                        let walk = walk_companion_payload(&companion);
                        size += walk.size_bytes;
                        modified = match (modified, walk.latest_modified) {
                            (Some(a), Some(b)) => Some(if a > b { a } else { b }),
                            (Some(a), None) => Some(a),
                            (None, b) => b,
                        };
                        if !walk.reasons.is_empty() {
                            // Two channels, two questions: `degraded_companions` says *which
                            // artifact's* evidence is degraded (per-artifact confidence); the
                            // reason log says the *scope* was not fully observed (withholding).
                            // E06-S02 only ever had the first, which is why the scope verdict
                            // had to be reconstructed from it downstream.
                            degraded_companions.push(companion.clone());
                            // Each nested failure keeps its own path and cause. Collapsing them
                            // into one generic "could not be fully listed" was an SI-010 finding
                            // in E21 round-1 review: the operator could not tell which path to
                            // fix.
                            for reason in walk.reasons {
                                log.record(reason);
                            }
                        }
                        companion_payload = Some(companion);
                    }
                }
            }

            sessions.push(ClaudeSession {
                project: project_name.clone(),
                session_id,
                path: child_path,
                size_bytes: size,
                modified,
                companion_payload,
            });
        }
    }

    SessionDiscoveryResult {
        scope: SessionDiscoveryScope::Observed,
        sessions,
        observation: log.into_observation(),
        degraded_companions,
    }
}

/// What one companion payload walk observed. `reasons` replaces the previous `fully_read: bool`:
/// a boolean told the caller *that* something failed and nothing about *what*, so the caller
/// emitted one generic reason for the whole directory and every nested path/cause was lost
/// (SI-010, E21 round-1 independent review).
struct CompanionWalk {
    size_bytes: u64,
    latest_modified: Option<SystemTime>,
    reasons: Vec<CompletenessReason>,
}

/// Recursively sums size and finds the latest modification time under `root`, never following
/// symlinks, recording every path it could not observe rather than reducing them to a flag
/// (`cancellai.py`'s `directory_size`/`latest_mtime` record each failure into its own `Scan`).
/// A partial total is still returned rather than discarded - the caller reports it as partial.
fn walk_companion_payload(root: &Path) -> CompanionWalk {
    let mut size_bytes = 0u64;
    let mut latest: Option<SystemTime> = None;
    let mut reasons = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                reasons.push(reason_for(&dir, &error));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    reasons.push(reason_for(&dir, &error));
                    continue;
                }
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    reasons.push(reason_for(&path, &error));
                    continue;
                }
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(error) => {
                    reasons.push(reason_for(&path, &error));
                    continue;
                }
            };
            size_bytes += metadata.len();
            match metadata.modified() {
                Ok(modified) => {
                    latest = Some(match latest {
                        Some(current) if current > modified => current,
                        _ => modified,
                    });
                }
                Err(error) => reasons.push(CompletenessReason::Io {
                    path,
                    message: format!("modification time unavailable: {error}"),
                }),
            }
        }
    }

    CompanionWalk {
        size_bytes,
        latest_modified: latest,
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_inventory::ScopeCompleteness;
    use std::path::PathBuf;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-claude-session-test-{label}-{}",
                std::process::id()
            ));
            fs::remove_dir_all(&dir).ok();
            fs::create_dir_all(&dir).expect("create temp root");
            Self(dir)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn a_missing_projects_directory_is_unavailable_not_empty() {
        let tree = TempTree::new("missing-projects");
        let result = discover_claude_sessions(&tree.0);
        assert_eq!(result.scope, SessionDiscoveryScope::Unavailable);
        assert!(result.sessions.is_empty());
    }

    #[test]
    fn a_single_well_formed_session_is_discovered() {
        let tree = TempTree::new("single-session");
        let project = tree.0.join("projects/synthetic-project-a");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("11111111-1111-4111-8111-111111111111.jsonl"),
            "{\"type\": \"user\"}\n",
        )
        .unwrap();

        let result = discover_claude_sessions(&tree.0);
        assert_eq!(result.scope, SessionDiscoveryScope::Observed);
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.sessions[0].project, "synthetic-project-a");
        assert_eq!(
            result.sessions[0].session_id,
            "11111111-1111-4111-8111-111111111111"
        );
        assert!(result.degraded_companions.is_empty());
    }

    #[test]
    fn a_companion_payload_directory_contributes_to_size() {
        let tree = TempTree::new("companion");
        let project = tree.0.join("projects/synthetic-project-a");
        let session_id = "11111111-1111-4111-8111-111111111111";
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join(format!("{session_id}.jsonl")), "12345").unwrap();
        fs::create_dir_all(project.join(session_id).join("tool-results")).unwrap();
        fs::write(
            project.join(session_id).join("tool-results/large.txt"),
            "0123456789",
        )
        .unwrap();

        let result = discover_claude_sessions(&tree.0);
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.sessions[0].size_bytes, 5 + 10);
        assert!(result.sessions[0].companion_payload.is_some());
    }

    #[cfg(unix)]
    #[test]
    fn a_locked_companion_still_reports_the_session_but_marks_it_degraded() {
        use std::os::unix::fs::PermissionsExt;

        let tree = TempTree::new("locked-companion");
        let project = tree.0.join("projects/synthetic-project-c");
        let session_id = "55555555-5555-4555-8555-555555555553";
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join(format!("{session_id}.jsonl")), "{}").unwrap();
        let companion = project.join(session_id);
        fs::create_dir_all(&companion).unwrap();
        fs::write(companion.join("tool-results.txt"), "x").unwrap();
        fs::set_permissions(&companion, fs::Permissions::from_mode(0o000)).unwrap();

        let result = discover_claude_sessions(&tree.0);

        fs::set_permissions(&companion, fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(
            result.sessions.len(),
            1,
            "the session itself must still be reported even though its companion is locked"
        );
        assert_eq!(result.degraded_companions, vec![companion]);
    }

    #[test]
    fn a_project_subdirectory_that_is_a_symlink_is_never_walked() {
        let tree = TempTree::new("symlinked-project");
        let outside = tree.0.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(
            outside.join("11111111-1111-4111-8111-111111111111.jsonl"),
            "{}",
        )
        .unwrap();
        fs::create_dir_all(tree.0.join("projects")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, tree.0.join("projects/linked")).unwrap();

        let result = discover_claude_sessions(&tree.0);
        assert!(result.sessions.is_empty());
    }

    #[test]
    fn a_non_uuid_named_jsonl_file_is_ignored() {
        let tree = TempTree::new("non-uuid");
        let project = tree.0.join("projects/synthetic-project-a");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("not-a-session-id.jsonl"), "{}").unwrap();

        let result = discover_claude_sessions(&tree.0);
        assert!(result.sessions.is_empty());
    }

    // ----------------------------------------------------------------------------------
    // E21-S03: scan completeness. `CR-TE-01` reproduced the engine deleting artifacts the
    // frozen reference withholds, because these branches were bare `continue`s.
    // ----------------------------------------------------------------------------------

    /// chmod(0o000) denies a non-root reader only. Running the suite as root would make every
    /// "unreadable" case silently readable and the assertions below would pass for the wrong
    /// reason - a fixture that cannot fail is worse than no fixture. Skip loudly instead.
    #[cfg(unix)]
    fn can_deny_reads(path: &Path) -> bool {
        fs::read_dir(path).is_err()
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_project_directory_makes_the_scope_partial() {
        use std::os::unix::fs::PermissionsExt;

        let tree = TempTree::new("unreadable-project");
        let readable = tree.0.join("projects/synthetic-project-a");
        fs::create_dir_all(&readable).unwrap();
        fs::write(
            readable.join("11111111-1111-4111-8111-111111111111.jsonl"),
            "{}",
        )
        .unwrap();
        let locked = tree.0.join("projects/synthetic-project-b");
        fs::create_dir_all(&locked).unwrap();
        fs::write(
            locked.join("22222222-2222-4222-8222-222222222222.jsonl"),
            "{}",
        )
        .unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        if !can_deny_reads(&locked) {
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
            eprintln!("skipped: this process can read a 0o000 directory (running as root?)");
            return;
        }

        let result = discover_claude_sessions(&tree.0);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(result.scope, SessionDiscoveryScope::Observed);
        assert_eq!(
            result.sessions.len(),
            1,
            "the readable project's session is still reported"
        );
        assert!(
            result.degraded_companions.is_empty(),
            "no companion payload was involved: this is the project-directory branch, which is \
             exactly why deriving the scope verdict from degraded_companions was insufficient"
        );
        match result.observation.completeness() {
            ScopeCompleteness::Partial { reasons } => {
                assert_eq!(reasons.len(), 1);
                assert!(
                    matches!(&reasons[0], CompletenessReason::PermissionDenied { path } if *path == locked),
                    "expected a permission reason naming the locked project, got {:?}",
                    reasons[0]
                );
            }
            other => {
                panic!("an unreadable project directory must make the scope Partial, got {other:?}")
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_projects_root_is_unknown_not_a_clean_empty_scope() {
        use std::os::unix::fs::PermissionsExt;

        let tree = TempTree::new("unreadable-projects-root");
        let projects = tree.0.join("projects");
        fs::create_dir_all(projects.join("synthetic-project-a")).unwrap();
        fs::set_permissions(&projects, fs::Permissions::from_mode(0o000)).unwrap();
        if !can_deny_reads(&projects) {
            fs::set_permissions(&projects, fs::Permissions::from_mode(0o755)).unwrap();
            eprintln!("skipped: this process can read a 0o000 directory (running as root?)");
            return;
        }

        let result = discover_claude_sessions(&tree.0);
        fs::set_permissions(&projects, fs::Permissions::from_mode(0o755)).unwrap();

        // The distinction SI-009 turns on: a projects/ that is *missing* is a structurally
        // empty install (Complete); one that exists and cannot be read is missing evidence.
        assert_eq!(
            result.scope,
            SessionDiscoveryScope::Unobservable,
            "an unreadable root must be distinguishable from a structurally absent one, or the \
             policy layer cannot tell them apart (E21 round-1 independent review)"
        );
        assert!(
            matches!(
                result.observation.completeness(),
                cancellai_inventory::ScopeCompleteness::Unknown { .. }
            ),
            "an unreadable scope root is Unknown, not Complete: got {:?}",
            result.observation
        );
        assert_eq!(result.observation.unobserved_count(), 1);
    }

    #[test]
    fn a_missing_projects_directory_is_complete_not_partial() {
        // The counterpart of the test above, and the reason this is not simply "any failure is
        // a reason": `cancellai.py::observe` records every OSError *except* FileNotFoundError.
        // A provider that was never installed must not withhold cleanup for the other one.
        let tree = TempTree::new("absent-projects");
        let result = discover_claude_sessions(&tree.0);
        assert_eq!(result.scope, SessionDiscoveryScope::Unavailable);
        assert_eq!(
            result.observation,
            ScopeObservation::complete(),
            "a provider that is simply not installed is a known-empty state, not missing evidence"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_degraded_companion_is_reported_on_both_channels() {
        use std::os::unix::fs::PermissionsExt;

        // `degraded_companions` answers "which artifact's evidence is degraded" and
        // `completeness` answers "was the scope fully observed". Both must fire here, or the
        // per-artifact confidence and the scope verdict would disagree about the same fact.
        let tree = TempTree::new("degraded-both-channels");
        let project = tree.0.join("projects/synthetic-project-a");
        fs::create_dir_all(&project).unwrap();
        let session_id = "11111111-1111-4111-8111-111111111111";
        fs::write(project.join(format!("{session_id}.jsonl")), "{}").unwrap();
        let companion = project.join(session_id);
        fs::create_dir_all(companion.join("tool-results")).unwrap();
        fs::set_permissions(&companion, fs::Permissions::from_mode(0o000)).unwrap();
        if !can_deny_reads(&companion) {
            fs::set_permissions(&companion, fs::Permissions::from_mode(0o755)).unwrap();
            eprintln!("skipped: this process can read a 0o000 directory (running as root?)");
            return;
        }

        let result = discover_claude_sessions(&tree.0);
        fs::set_permissions(&companion, fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(result.degraded_companions, vec![companion]);
        assert!(!result.observation.is_complete());
    }
}

//! Claude session discovery (ported from `cancellai.py`'s `discover_claude_sessions`,
//! E05-S03).
//!
//! Claude's session relationships are flat: each project directory groups zero or more
//! top-level session transcripts. There is no parent/child rollout graph the way Codex has
//! (`cancellai-provider-codex`'s own E05-S04 concern - not ported here, and not invented as a
//! payload type this crate does not need, matching E05-S01's deferred-payload precedent).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
    /// `projects/` is missing or is a symlink - `cancellai.py`'s own early return. Distinct
    /// from "observed and empty" (SI-009: missing evidence is not interpreted as absence of
    /// active/protected data).
    Unavailable,
}

/// The result of one `discover_claude_sessions` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDiscoveryResult {
    pub scope: SessionDiscoveryScope,
    pub sessions: Vec<ClaudeSession>,
    /// Companion payload directories whose own listing failed partway through (a locked
    /// subdirectory, a permission error) - the session itself is still reported (its own
    /// `.jsonl` file was readable), but its `size_bytes`/`modified` may be an undercount
    /// (SI-008/SI-009: a partial observation is reported as partial, never silently folded
    /// into a clean-looking total). `cancellai.py` records the same fact into its `Scan`
    /// object instead of returning it inline; this crate has no `Scan` type of its own to
    /// route through, so it is a direct field here.
    pub degraded_companions: Vec<PathBuf>,
}

pub fn discover_claude_sessions(claude_home: &Path) -> SessionDiscoveryResult {
    let projects = claude_home.join("projects");
    let unavailable = || SessionDiscoveryResult {
        scope: SessionDiscoveryScope::Unavailable,
        sessions: Vec::new(),
        degraded_companions: Vec::new(),
    };

    let Ok(projects_meta) = fs::symlink_metadata(&projects) else {
        return unavailable();
    };
    if projects_meta.file_type().is_symlink() || !projects_meta.is_dir() {
        return unavailable();
    }
    let Ok(project_entries) = fs::read_dir(&projects) else {
        return unavailable();
    };

    let mut sessions = Vec::new();
    let mut degraded_companions = Vec::new();

    for project_entry in project_entries.flatten() {
        let Ok(project_file_type) = project_entry.file_type() else {
            continue;
        };
        if !project_file_type.is_dir() || project_file_type.is_symlink() {
            continue;
        }
        let project_path = project_entry.path();
        let project_name = project_entry.file_name().to_string_lossy().into_owned();
        let Ok(children) = fs::read_dir(&project_path) else {
            continue;
        };

        for child in children.flatten() {
            let Ok(child_type) = child.file_type() else {
                continue;
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
            let Ok(metadata) = child_path.symlink_metadata() else {
                continue;
            };

            let mut size = metadata.len();
            let mut modified = metadata.modified().ok();
            let companion = project_path.join(stem);
            let mut companion_payload = None;

            if let Ok(companion_meta) = fs::symlink_metadata(&companion) {
                if companion_meta.is_dir() && !companion_meta.file_type().is_symlink() {
                    let (extra_size, extra_modified, fully_read) =
                        directory_size_and_latest_mtime(&companion);
                    size += extra_size;
                    modified = match (modified, extra_modified) {
                        (Some(a), Some(b)) => Some(if a > b { a } else { b }),
                        (Some(a), None) => Some(a),
                        (None, b) => b,
                    };
                    if !fully_read {
                        degraded_companions.push(companion.clone());
                    }
                    companion_payload = Some(companion);
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
        degraded_companions,
    }
}

/// Recursively sums size and finds the latest modification time under `root`, never following
/// symlinks. Returns `fully_read = false` if any directory anywhere in the walk (including
/// `root` itself) could not be listed - the partial total/mtime already accumulated is still
/// returned rather than discarded (`cancellai.py`'s `directory_size`/`latest_mtime`: a listing
/// failure is recorded as a scan error, not treated as "empty directory").
fn directory_size_and_latest_mtime(root: &Path) -> (u64, Option<SystemTime>, bool) {
    let mut total = 0u64;
    let mut latest: Option<SystemTime> = None;
    let mut fully_read = true;
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            fully_read = false;
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(entry.path());
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            total += metadata.len();
            if let Ok(modified) = metadata.modified() {
                latest = Some(match latest {
                    Some(current) if current > modified => current,
                    _ => modified,
                });
            }
        }
    }

    (total, latest, fully_read)
}

#[cfg(test)]
mod tests {
    use super::*;
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
}

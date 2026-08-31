//! Codex rollout discovery (ported from `cancellai.py`'s `discover_codex_sessions`/
//! `read_codex_parent_session_id`, E05-S04).

use std::fs;
use std::path::{Path, PathBuf};

use cancellai_provider_api::extract_uuid;

/// Mirrors `cancellai.py`'s `("sessions", "session")`/`("archived_sessions", "archived-session")`
/// pair - which top-level directory a rollout was found under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolloutCategory {
    Session,
    ArchivedSession,
}

impl RolloutCategory {
    pub fn label(self) -> &'static str {
        match self {
            RolloutCategory::Session => "session",
            RolloutCategory::ArchivedSession => "archived-session",
        }
    }
}

/// One discovered Codex rollout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexSession {
    pub category: RolloutCategory,
    pub path: PathBuf,
    pub session_id: String,
    pub parent_session_id: Option<String>,
    pub size_bytes: u64,
}

const MAX_PARENT_SCAN_RECORDS: usize = 10;
const MAX_PARENT_SCAN_BYTES: usize = 512 * 1024;

/// Reads `parent_thread_id` from a rollout's `session_meta` record without scanning the whole
/// file - bounded to the first 10 lines / 512KiB, matching `cancellai.py`'s
/// `read_codex_parent_session_id`. Unknown/legacy formats, or a rollout with no `session_meta`
/// record in that budget, return `None` - the same as "no parent", by design: this is a cheap
/// discovery-time probe, not a full-file lineage proof.
pub fn read_codex_parent_session_id(path: &Path) -> Option<String> {
    let Ok(bytes) = fs::read(path) else {
        return None;
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut consumed = 0usize;

    for line in text.lines().take(MAX_PARENT_SCAN_RECORDS) {
        consumed += line.len() + 1;
        if consumed > MAX_PARENT_SCAN_BYTES {
            break;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("type").and_then(|v| v.as_str()) != Some("session_meta") {
            continue;
        }
        let payload = value.get("payload")?;
        if !payload.is_object() {
            return None;
        }
        let meta = payload
            .get("meta")
            .filter(|candidate| candidate.is_object())
            .unwrap_or(payload);
        return match meta.get("parent_thread_id") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(text)) => extract_uuid(text),
            Some(other) => extract_uuid(&other.to_string()),
        };
    }
    None
}

/// Walks `codex_home/sessions` and `codex_home/archived_sessions` (recursively, never
/// following symlinks, matching `cancellai.py`'s `iter_files`) for `rollout-*.jsonl` files.
/// A missing or symlinked top-level directory is skipped, not an error - `cancellai.py`'s own
/// early-continue behavior for each of the two roots independently.
pub fn discover_codex_sessions(codex_home: &Path) -> Vec<CodexSession> {
    let mut sessions = Vec::new();
    for (rel, category) in [
        ("sessions", RolloutCategory::Session),
        ("archived_sessions", RolloutCategory::ArchivedSession),
    ] {
        let root = codex_home.join(rel);
        let Ok(root_meta) = fs::symlink_metadata(&root) else {
            continue;
        };
        if root_meta.file_type().is_symlink() {
            continue;
        }
        walk_rollouts(&root, category, &mut sessions);
    }
    sessions
}

fn walk_rollouts(root: &Path, category: RolloutCategory, out: &mut Vec<CodexSession>) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(own_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();

            // Classify dir-vs-file by following symlinks, matching `os.walk`'s own default
            // (`cancellai.py`'s `iter_files`): a symlink to a directory is excluded from
            // *descent* below (mirroring `dirs[:] = [d for d in dirs if not is_symlink(d)]`),
            // but a symlink to a *file* is still processed as a file, unfiltered - exactly
            // like the Python reference, which never filters symlinked files out of `files`.
            let Ok(followed_metadata) = fs::metadata(&path) else {
                continue;
            };
            if followed_metadata.is_dir() {
                if !own_type.is_symlink() {
                    stack.push(path);
                }
                continue;
            }
            if !followed_metadata.is_file() {
                continue;
            }

            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.ends_with(".jsonl") || !name.starts_with("rollout-") {
                continue;
            }
            let Some(session_id) = extract_uuid(&name) else {
                continue;
            };
            // Unfollowed size (`entry.metadata()`, lstat-equivalent) - a symlink's own size,
            // never the target's, matching `cancellai.py`'s `p.lstat()` (SI-018: a symlink's
            // target is never accounted as this entry's own size).
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            out.push(CodexSession {
                category,
                session_id,
                parent_session_id: read_codex_parent_session_id(&path),
                size_bytes: metadata.len(),
                path,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-codex-session-test-{label}-{}",
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

    fn write_rollout(root: &Path, rel: &str, session_id: &str, parent: Option<&str>) -> PathBuf {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let meta = serde_json::json!({
            "type": "session_meta",
            "payload": {"meta": {"id": session_id, "parent_thread_id": parent}}
        });
        fs::write(&path, format!("{}\n", meta)).unwrap();
        path
    }

    #[test]
    fn a_missing_sessions_directory_yields_no_sessions() {
        let tree = TempTree::new("missing");
        assert!(discover_codex_sessions(&tree.0).is_empty());
    }

    #[test]
    fn a_nested_rollout_is_discovered_with_its_parent() {
        let tree = TempTree::new("nested");
        write_rollout(
            &tree.0,
            "sessions/2026/05/01/rollout-33333333-3333-4333-8333-333333333334.jsonl",
            "33333333-3333-4333-8333-333333333334",
            Some("33333333-3333-4333-8333-333333333333"),
        );

        let sessions = discover_codex_sessions(&tree.0);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].category, RolloutCategory::Session);
        assert_eq!(
            sessions[0].session_id,
            "33333333-3333-4333-8333-333333333334"
        );
        assert_eq!(
            sessions[0].parent_session_id.as_deref(),
            Some("33333333-3333-4333-8333-333333333333")
        );
    }

    #[test]
    fn a_root_rollout_with_no_parent_reports_none() {
        let tree = TempTree::new("no-parent");
        write_rollout(
            &tree.0,
            "sessions/rollout-22222222-2222-4222-8222-222222222222.jsonl",
            "22222222-2222-4222-8222-222222222222",
            None,
        );

        let sessions = discover_codex_sessions(&tree.0);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].parent_session_id, None);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_directory_is_never_descended_into_but_a_symlinked_file_still_is_a_rollout() {
        // cancellai.py's iter_files only ever filters *directory* symlinks out of descent
        // (`dirs[:] = [d for d in dirs if not is_symlink]`); a symlink whose target is a file
        // is still yielded as a file, unfiltered. This is the exact parity behavior AC1's
        // "Root/subagent trees are preserved as graph relationships" depends on not silently
        // dropping a real, UUID-named rollout merely because it happens to be a symlink.
        let tree = TempTree::new("symlinked-file-vs-dir");
        let outside_rollout = write_rollout(
            &tree.0,
            "outside/rollout-66666666-6666-4666-8666-666666666666.jsonl",
            "66666666-6666-4666-8666-666666666666",
            None,
        );
        let outside_dir = tree.0.join("outside-dir-with-a-rollout-inside");
        write_rollout(
            &outside_dir,
            "rollout-77777777-7777-4777-8777-777777777777.jsonl",
            "77777777-7777-4777-8777-777777777777",
            None,
        );
        fs::create_dir_all(tree.0.join("sessions")).unwrap();
        std::os::unix::fs::symlink(
            &outside_rollout,
            tree.0
                .join("sessions/rollout-66666666-6666-4666-8666-666666666666.jsonl"),
        )
        .unwrap();
        std::os::unix::fs::symlink(&outside_dir, tree.0.join("sessions/linked-dir")).unwrap();

        let sessions = discover_codex_sessions(&tree.0);
        let ids: Vec<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
        assert!(
            ids.contains(&"66666666-6666-4666-8666-666666666666"),
            "a symlinked rollout *file* must still be discovered"
        );
        assert!(
            !ids.contains(&"77777777-7777-4777-8777-777777777777"),
            "a rollout inside a symlinked *directory* must never be discovered"
        );
    }

    #[test]
    fn archived_sessions_are_reported_with_their_own_category() {
        let tree = TempTree::new("archived");
        write_rollout(
            &tree.0,
            "archived_sessions/rollout-44444444-4444-4444-8444-444444444444.jsonl",
            "44444444-4444-4444-8444-444444444444",
            None,
        );

        let sessions = discover_codex_sessions(&tree.0);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].category, RolloutCategory::ArchivedSession);
    }

    #[test]
    fn a_non_rollout_prefixed_jsonl_file_is_ignored() {
        let tree = TempTree::new("non-rollout");
        let path = tree.0.join("sessions/history.jsonl");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{}\n").unwrap();

        assert!(discover_codex_sessions(&tree.0).is_empty());
    }

    #[test]
    fn a_symlinked_sessions_directory_is_never_followed() {
        let tree = TempTree::new("symlinked-root");
        let outside = tree.0.join("outside-sessions");
        write_rollout(
            &outside,
            "rollout-55555555-5555-4555-8555-555555555555.jsonl",
            "55555555-5555-4555-8555-555555555555",
            None,
        );
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, tree.0.join("sessions")).unwrap();

        assert!(discover_codex_sessions(&tree.0).is_empty());
    }

    #[test]
    fn read_codex_parent_session_id_handles_a_missing_session_meta_record() {
        let tree = TempTree::new("no-session-meta");
        let path = tree.0.join("rollout.jsonl");
        fs::write(&path, "{\"type\": \"turn\"}\n").unwrap();
        assert_eq!(read_codex_parent_session_id(&path), None);
    }
}

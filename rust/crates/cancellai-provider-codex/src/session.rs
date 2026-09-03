//! Codex rollout discovery (ported from `cancellai.py`'s `discover_codex_sessions`/
//! `read_codex_parent_session_id`, E05-S04).
//!
//! ## Scan completeness (E21-S03)
//!
//! Until E21-S03 this walk answered only "which rollouts did I find", and every failure to
//! observe part of the tree was discarded by a bare `else { continue }`. The 2026-09-03
//! target-engine review (`docs/audits/2026-09-03-CODE_REVIEW.md`, `CR-TE-01`) reproduced what
//! that costs: with one `sessions/` subdirectory unreadable, `cancellai.py` withholds the whole
//! tool and exits `4`, while this engine reported the scope complete and deleted an eligible
//! rollout. Absence of evidence had become absence of data - exactly what SI-008/SI-009 and
//! constitutional C-02 forbid.
//!
//! Discovery now returns a [`RolloutDiscoveryResult`] carrying
//! [`cancellai_inventory::ScopeCompleteness`] alongside the sessions. The type is the one E04
//! already built and proved (ADR-0018): the adapters keep their layout-specific traversal,
//! because Codex's date-partitioned rollout tree and Claude's flat project layout are genuinely
//! different problems, but neither can express "I did not see all of it" as silence any more.
//!
//! The mapping mirrors `cancellai.py`'s own `observe`/`iter_files` error channel, which is the
//! behaviour the differential gate pins:
//!
//! - a missing `sessions/`/`archived_sessions/` root is **not** a reason (`observe` returns
//!   `None` for `FileNotFoundError` without recording): a provider that never created the
//!   directory is a known-empty state, not missing evidence (SI-009 distinguishes the two);
//! - a root that exists but cannot be `lstat`ed **is** a reason (`observe`'s `OSError` branch);
//! - a symlinked root is skipped silently, as `iter_files` does, and is not a reason;
//! - any directory that cannot be listed mid-walk is a reason (`os.walk`'s `onerror` hook);
//! - an entry whose own metadata cannot be read is a reason (`p.lstat()`'s `except OSError`).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cancellai_inventory::{CompletenessReason, ReasonLog, ScopeObservation};
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
///
/// E21-S06: this used to say all of the above and then call `fs::read(path)`, pulling the whole
/// transcript into memory and decoding it in full *before* applying the bound it documents. The
/// 2026-09-03 review (`CR-TE-04`) measured the cost on a single 287 MB rollout: 303 MB peak RSS
/// against the Python reference's 27.8 MB, and the Rust engine slower than the reference for the
/// only time in that review. The cost scaled with the largest transcript on disk, and agentic
/// session transcripts grow without bound.
///
/// The parsing is now in [`read_parent_from`], which takes a [`BufRead`] and never looks beyond
/// its budget. That is what makes the bound testable rather than asserted: `session.rs`'s own
/// `a_reader_is_never_consumed_beyond_the_budget` drives it with a byte-counting reader over a
/// synthetic 64 MiB input and asserts on bytes actually consumed - a direct proof, not a memory
/// proxy.
pub fn read_codex_parent_session_id(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    read_parent_from(io::BufReader::new(file))
}

/// The bounded parse, over any reader. Byte accounting is deliberately identical to the previous
/// whole-file implementation (`line.len() + 1` on the newline-stripped, lossily-decoded line), so
/// this story changes where the bytes come from and nothing about which record is selected - the
/// differential gate treats any change there as a divergence.
pub(crate) fn read_parent_from(mut reader: impl io::BufRead) -> Option<String> {
    // Two counters, deliberately. `read_total` is the number of bytes actually pulled out of the
    // reader and is what [`MAX_PARENT_SCAN_BYTES`] bounds - E21 round-1 independent review found
    // the previous `remaining + 1` budget allowed 524,289 bytes against a documented 512 KiB
    // maximum. `consumed` is the reference-compatible accounting (`line.len() + 1` on the
    // newline-stripped line) that decides where parsing stops, kept separate because a CRLF line
    // costs one more real byte than the reference counts for it.
    let mut read_total = 0usize;
    let mut consumed = 0usize;
    let mut raw = Vec::new();

    for _ in 0..MAX_PARENT_SCAN_RECORDS {
        raw.clear();
        let remaining = MAX_PARENT_SCAN_BYTES.saturating_sub(read_total);
        if remaining == 0 {
            break;
        }
        // `read_until` rather than `read_line`: a rollout is not guaranteed to be valid UTF-8,
        // and the previous implementation decoded lossily rather than failing. Bounded by
        // `take` so a single pathological line without a newline cannot pull the file in
        // through the back door - the exact failure mode this story exists to remove.
        let mut bounded = io::Read::take(&mut reader, remaining as u64);
        let read = match io::BufRead::read_until(&mut bounded, b'\n', &mut raw) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        read_total += read;
        // The budget cut this record short: a truncated record is not a record. Stopping without
        // parsing keeps selection identical to the whole-file version, which never saw a partial
        // line either - it simply stopped counting once `consumed` passed the bound.
        if read == remaining && raw.last() != Some(&b'\n') {
            break;
        }
        let decoded = String::from_utf8_lossy(&raw);
        // Matches `str::lines()`, which the whole-file version used: strip the trailing newline
        // and then a trailing carriage return.
        let line = decoded
            .strip_suffix('\n')
            .unwrap_or(&decoded)
            .strip_suffix('\r')
            .unwrap_or_else(|| decoded.strip_suffix('\n').unwrap_or(&decoded));
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

/// Everything one `discover_codex_sessions` call observed: the rollouts it found, and how
/// completely it was able to look. The two are deliberately one value - a caller cannot take
/// the sessions and leave the completeness behind (ADR-0018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutDiscoveryResult {
    pub sessions: Vec<CodexSession>,
    /// How completely this walk observed the scope, with the truthful number of unobserved
    /// paths - retention of individual reasons is bounded, the count is not (E21 round-1
    /// independent review: an unbounded reason vector is its own operability failure, C-11).
    pub observation: ScopeObservation,
}

/// Classifies one filesystem error into the reason vocabulary `cancellai-inventory` already
/// defines, so a permission denial, a mid-walk disappearance and an unclassifiable I/O failure
/// stay distinguishable (SI-010: scan errors are visible, never summarized into an opaque
/// count).
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

/// Walks `codex_home/sessions` and `codex_home/archived_sessions` (recursively, never
/// following symlinks, matching `cancellai.py`'s `iter_files`) for `rollout-*.jsonl` files.
/// A missing or symlinked top-level directory is skipped, not an error - `cancellai.py`'s own
/// early-continue behavior for each of the two roots independently. A root that exists but
/// cannot be observed, and any directory or entry that cannot be read mid-walk, is recorded
/// as a completeness reason rather than skipped silently (E21-S03; see module docs).
pub fn discover_codex_sessions(codex_home: &Path) -> RolloutDiscoveryResult {
    let mut sessions = Vec::new();
    let mut log = ReasonLog::new();
    for (rel, category) in [
        ("sessions", RolloutCategory::Session),
        ("archived_sessions", RolloutCategory::ArchivedSession),
    ] {
        let root = codex_home.join(rel);
        let root_meta = match fs::symlink_metadata(&root) {
            Ok(meta) => meta,
            // `cancellai.py::observe` returns `None` for `FileNotFoundError` *without*
            // recording it, and records every other `OSError`. A provider directory that was
            // never created is a known-empty state; one that exists and cannot be looked at is
            // missing evidence.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                log.record(reason_for(&root, &error));
                continue;
            }
        };
        if root_meta.file_type().is_symlink() {
            continue;
        }
        walk_rollouts(&root, category, &mut sessions, &mut log);
    }
    RolloutDiscoveryResult {
        sessions,
        observation: log.into_observation(),
    }
}

fn walk_rollouts(
    root: &Path,
    category: RolloutCategory,
    out: &mut Vec<CodexSession>,
    log: &mut ReasonLog,
) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            // The defect `CR-TE-01` reproduced was exactly this branch being `continue` with
            // nothing recorded: `os.walk`'s `onerror` hook fires here in the reference, and
            // `Scan.record` is what turns it into a withheld tool.
            Err(error) => {
                log.record(reason_for(&dir, &error));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    log.record(reason_for(&dir, &error));
                    continue;
                }
            };
            let path = entry.path();
            let own_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    log.record(reason_for(&path, &error));
                    continue;
                }
            };

            // Classify dir-vs-file by following symlinks, matching `os.walk`'s own default
            // (`cancellai.py`'s `iter_files`): a symlink to a directory is excluded from
            // *descent* below (mirroring `dirs[:] = [d for d in dirs if not is_symlink(d)]`),
            // but a symlink to a *file* is still processed as a file, unfiltered - exactly
            // like the Python reference, which never filters symlinked files out of `files`.
            let followed_metadata = match fs::metadata(&path) {
                Ok(metadata) => metadata,
                // A dangling symlink resolves to nothing and is simply not a file to account -
                // the reference's `os.walk` reaches the same outcome by listing it under
                // `files` and having the later `p.lstat()`/name filters reject it. Any other
                // failure means this entry's own nature could not be established, which is
                // missing evidence about the tree, not an entry to skip.
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => {
                    log.record(reason_for(&path, &error));
                    continue;
                }
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
            // target is never accounted as this entry's own size). A failure here is the
            // reference's `except OSError: scan.record(p, exc)` branch: a rollout whose size
            // cannot be read is a rollout this scan cannot account for.
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(error) => {
                    log.record(reason_for(&path, &error));
                    continue;
                }
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
    // Only the `#[cfg(unix)]` partial-scope assertions name this type; every other test
    // asserts through `ScopeObservation`. Scoped rather than blanket-allowed so a future
    // unconditional use is still checked.
    #[cfg(unix)]
    use cancellai_inventory::ScopeCompleteness;

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
        assert!(discover_codex_sessions(&tree.0).sessions.is_empty());
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

        let sessions = discover_codex_sessions(&tree.0).sessions;
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

        let sessions = discover_codex_sessions(&tree.0).sessions;
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

        let sessions = discover_codex_sessions(&tree.0).sessions;
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

        let sessions = discover_codex_sessions(&tree.0).sessions;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].category, RolloutCategory::ArchivedSession);
    }

    #[test]
    fn a_non_rollout_prefixed_jsonl_file_is_ignored() {
        let tree = TempTree::new("non-rollout");
        let path = tree.0.join("sessions/history.jsonl");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{}\n").unwrap();

        assert!(discover_codex_sessions(&tree.0).sessions.is_empty());
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

        assert!(discover_codex_sessions(&tree.0).sessions.is_empty());
    }

    #[test]
    fn read_codex_parent_session_id_handles_a_missing_session_meta_record() {
        let tree = TempTree::new("no-session-meta");
        let path = tree.0.join("rollout.jsonl");
        fs::write(&path, "{\"type\": \"turn\"}\n").unwrap();
        assert_eq!(read_codex_parent_session_id(&path), None);
    }

    // ----------------------------------------------------------------------------------
    // E21-S03: scan completeness. Before this story `discover_codex_sessions` returned a bare
    // Vec with no way to say "I could not see all of it", so an unreadable directory under
    // sessions/ was indistinguishable from an empty one - and `CR-TE-01` reproduced the engine
    // deleting an eligible rollout that the frozen reference withholds.
    // ----------------------------------------------------------------------------------

    /// chmod(0o000) denies a non-root reader only. Running as root would make the "unreadable"
    /// cases readable and the assertions would pass for the wrong reason; skip loudly instead.
    #[cfg(unix)]
    fn can_deny_reads(path: &Path) -> bool {
        fs::read_dir(path).is_err()
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_session_directory_makes_the_scope_partial() {
        use std::os::unix::fs::PermissionsExt;

        let tree = TempTree::new("unreadable-session-dir");
        write_rollout(
            &tree.0,
            "sessions/2026/05/01/rollout-88888888-8888-4888-8888-888888888881.jsonl",
            "88888888-8888-4888-8888-888888888881",
            None,
        );
        write_rollout(
            &tree.0,
            "sessions/2026/05/02/rollout-88888888-8888-4888-8888-888888888882.jsonl",
            "88888888-8888-4888-8888-888888888882",
            None,
        );
        let locked = tree.0.join("sessions/2026/05/02");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        if !can_deny_reads(&locked) {
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
            eprintln!("skipped: this process can read a 0o000 directory (running as root?)");
            return;
        }

        let result = discover_codex_sessions(&tree.0);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(
            result.sessions.len(),
            1,
            "the readable rollout is still discovered"
        );
        match result.observation.completeness() {
            ScopeCompleteness::Partial { reasons } => {
                assert_eq!(reasons.len(), 1);
                assert!(
                    matches!(&reasons[0], CompletenessReason::PermissionDenied { path } if *path == locked),
                    "expected a permission reason naming the locked directory, got {:?}",
                    reasons[0]
                );
            }
            other => {
                panic!("an unreadable session directory must make the scope Partial, got {other:?}")
            }
        }
    }

    #[test]
    fn a_fully_readable_tree_is_complete() {
        // The counterexample that keeps the assertion above meaningful: if every scope were
        // Partial, withholding would be unconditional and the engine would never clean anything.
        let tree = TempTree::new("fully-readable");
        write_rollout(
            &tree.0,
            "sessions/2026/05/01/rollout-88888888-8888-4888-8888-888888888881.jsonl",
            "88888888-8888-4888-8888-888888888881",
            None,
        );
        let result = discover_codex_sessions(&tree.0);
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.observation, ScopeObservation::complete());
    }

    #[test]
    fn a_missing_sessions_root_is_complete_not_partial() {
        // `cancellai.py::observe` records every OSError *except* FileNotFoundError: a provider
        // that never created sessions/ is a known-empty state (SI-009), and treating it as
        // missing evidence would withhold cleanup on every machine without Codex installed.
        let tree = TempTree::new("absent-sessions-root");
        let result = discover_codex_sessions(&tree.0);
        assert!(result.sessions.is_empty());
        assert_eq!(result.observation, ScopeObservation::complete());
    }

    #[cfg(unix)]
    #[test]
    fn an_archived_sessions_failure_is_recorded_too() {
        use std::os::unix::fs::PermissionsExt;

        // Both roots feed one scope verdict. A defect that only checked sessions/ would leave
        // archived_sessions/ as a silent hole with exactly the same consequence.
        let tree = TempTree::new("unreadable-archived");
        write_rollout(
            &tree.0,
            "sessions/2026/05/01/rollout-88888888-8888-4888-8888-888888888881.jsonl",
            "88888888-8888-4888-8888-888888888881",
            None,
        );
        let archived = tree.0.join("archived_sessions/2026");
        fs::create_dir_all(&archived).unwrap();
        fs::set_permissions(&archived, fs::Permissions::from_mode(0o000)).unwrap();
        if !can_deny_reads(&archived) {
            fs::set_permissions(&archived, fs::Permissions::from_mode(0o755)).unwrap();
            eprintln!("skipped: this process can read a 0o000 directory (running as root?)");
            return;
        }

        let result = discover_codex_sessions(&tree.0);
        fs::set_permissions(&archived, fs::Permissions::from_mode(0o755)).unwrap();

        assert!(
            !result.observation.is_complete(),
            "an unreadable archived_sessions/ subtree is missing evidence just like sessions/"
        );
    }

    // ----------------------------------------------------------------------------------
    // E21-S06: the documented bound is now enforced by the reader, not by the caller's luck.
    // ----------------------------------------------------------------------------------

    /// A `BufRead` that reports how many bytes were actually pulled out of it. This is what
    /// makes `CR-TE-04` a testable claim rather than a memory-profiler proxy: the assertion is
    /// on bytes consumed, which is the quantity the defect was about.
    struct CountingReader<R> {
        inner: R,
        consumed: std::rc::Rc<std::cell::Cell<usize>>,
    }

    impl<R: io::Read> io::Read for CountingReader<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = self.inner.read(buf)?;
            self.consumed.set(self.consumed.get() + n);
            Ok(n)
        }
    }

    impl<R: io::BufRead> io::BufRead for CountingReader<R> {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            self.inner.fill_buf()
        }

        fn consume(&mut self, amt: usize) {
            self.consumed.set(self.consumed.get() + amt);
            self.inner.consume(amt);
        }
    }

    #[test]
    fn a_reader_is_never_consumed_beyond_the_budget() {
        // 64 MiB of junk after the metadata record. The previous implementation read every byte
        // of the file and then decoded all of it before applying the bound it documented.
        let mut input = String::from(
            r#"{"type":"session_meta","payload":{"meta":{"parent_thread_id":"33333333-3333-4333-8333-333333333333"}}}"#,
        );
        input.push('\n');
        let filler_line = format!("{}\n", "x".repeat(1023));
        for _ in 0..65_536 {
            input.push_str(&filler_line);
        }
        assert!(
            input.len() > 64 * 1024 * 1024,
            "the input must dwarf the budget"
        );

        let consumed = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let reader = CountingReader {
            inner: io::BufReader::new(io::Cursor::new(input.as_bytes())),
            consumed: std::rc::Rc::clone(&consumed),
        };

        assert_eq!(
            read_parent_from(reader).as_deref(),
            Some("33333333-3333-4333-8333-333333333333")
        );
        // The parent is on line 1, so the bound that actually binds here is the record count,
        // not the byte budget - and either way the reader must never be drained.
        assert!(
            consumed.get() <= MAX_PARENT_SCAN_BYTES,
            "consumed {} bytes from a {}-byte input; the documented bound is {}",
            consumed.get(),
            input.len(),
            MAX_PARENT_SCAN_BYTES
        );
    }

    #[test]
    fn a_single_enormous_line_cannot_pull_the_file_in_through_the_back_door() {
        // The failure mode a naive `read_line` loop would still have: one pathological record
        // with no newline in it. `Read::take` bounds each read, so the budget holds regardless
        // of how the bytes are laid out.
        let mut input = String::from("{");
        input.push_str(&"y".repeat(8 * 1024 * 1024));
        let consumed = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let reader = CountingReader {
            inner: io::BufReader::new(io::Cursor::new(input.as_bytes())),
            consumed: std::rc::Rc::clone(&consumed),
        };

        assert_eq!(read_parent_from(reader), None);
        assert!(
            consumed.get() <= MAX_PARENT_SCAN_BYTES,
            "consumed {} bytes from a single {}-byte line; the documented maximum is {} and \
             admits no off-by-one (E21 round-1 independent review)",
            consumed.get(),
            input.len(),
            MAX_PARENT_SCAN_BYTES
        );
    }

    #[test]
    fn record_selection_is_unchanged_by_streaming() {
        // The behavioural half: which record wins, CRLF handling, non-UTF-8 tolerance, and the
        // 10-record cutoff must all be exactly what the whole-file version produced, or the
        // differential gate would (rightly) call this a divergence rather than a fix.
        let with_crlf = "{\"type\":\"other\"}\r\n{\"type\":\"session_meta\",\"payload\":{\"meta\":{\"parent_thread_id\":\"44444444-4444-4444-8444-444444444444\"}}}\r\n";
        assert_eq!(
            read_parent_from(io::BufReader::new(io::Cursor::new(with_crlf.as_bytes()))).as_deref(),
            Some("44444444-4444-4444-8444-444444444444")
        );

        // Beyond the 10-record budget: found by neither implementation.
        let mut late = String::new();
        for _ in 0..12 {
            late.push_str("{\"type\":\"noise\"}\n");
        }
        late.push_str(r#"{"type":"session_meta","payload":{"meta":{"parent_thread_id":"55555555-5555-4555-8555-555555555555"}}}"#);
        assert_eq!(
            read_parent_from(io::BufReader::new(io::Cursor::new(late.as_bytes()))),
            None
        );

        // Invalid UTF-8 is tolerated, not fatal - the previous version decoded lossily.
        let mut bytes = b"{\"type\":\"noise\",\"b\":\"".to_vec();
        bytes.extend_from_slice(&[0xff, 0xfe]);
        bytes.extend_from_slice(b"\"}\n");
        bytes.extend_from_slice(
            br#"{"type":"session_meta","payload":{"meta":{"parent_thread_id":"66666666-6666-4666-8666-666666666666"}}}"#,
        );
        assert_eq!(
            read_parent_from(io::BufReader::new(io::Cursor::new(bytes))).as_deref(),
            Some("66666666-6666-4666-8666-666666666666")
        );
    }
}

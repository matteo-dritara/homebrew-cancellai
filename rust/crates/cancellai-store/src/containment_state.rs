//! Persistence for signed incident containment (E06-S07, ADR-0039): the ledger's history and the
//! owner's own trusted publishers, both under cancellAI's local-state root.
//!
//! **This module never interprets a notice.** It stores the raw text of each installed bundle
//! and each local lift as opaque strings; `cancellai-safety::ContainmentLedger::replay` parses and
//! re-verifies them every time the ledger is needed. Storing verified *evidence* instead would
//! give the kernel a second way to construct `IncidentEvidence` - from a file - which SI-022
//! forbids.
//!
//! **Append-only, one line per event, one `write` per line, opened with `O_APPEND`.** There is no
//! temporary file, no rename and no lock file: a lock file would have to be removed, which is a
//! mutation outside the one safety boundary (SI-019), and a read-modify-rename cycle can lose a
//! concurrent writer's event. Two concurrent appends each land whole. What this does not do is
//! serialize the *decision*: two installs from one publisher racing each other can land out of
//! sequence order, and a crash mid-write can leave a torn last line. Both make the history fail
//! to replay, which the caller treats as an unknown ledger - fail closed, never a silently
//! weaker one.
//!
//! **What the history guarantees, and what it does not (owner decision, 2026-09-23).** The
//! history resists everything that arrives from outside - a notice, a replayed or expired bundle,
//! a forged signature - because every event is re-verified. It does not resist the owner's own
//! account: a process running as the same user can delete the file, truncate it to an earlier
//! valid prefix, or append a lift, and each of those lifts containment. ADR-0039 already treats
//! deleting this state as a local act; editing it is the same act. A same-user attacker can also
//! delete the provider files directly, so the history adds no capability that attacker lacks.

use std::io::{Read, Write};
use std::path::Path;

use crate::LocalStateRoot;

const LOG_FILENAME: &str = "containment_log.jsonl";
const TRUST_FILENAME: &str = "trusted_publishers.json";
/// The largest history this module will read. A larger file is unreadable, not truncated.
pub const MAX_LOG_BYTES: u64 = 16 * 1024 * 1024;
/// The largest owner trust file this module will read.
pub const MAX_TRUST_BYTES: u64 = 64 * 1024;

/// One persisted history entry, as opaque text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredEvent {
    Install(String),
    Lift(String),
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
enum Line {
    Install(String),
    Lift(String),
}

/// What reading a persisted file found. `Missing` and `Unreadable` are deliberately distinct:
/// a file that was never written is a known-empty state; one that exists and cannot be read is
/// missing evidence (SI-009).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Load<T> {
    Missing,
    Found(T),
    Unreadable(String),
}

/// One owner-trusted publisher, as written in the trust file: an id and a hex public key. The
/// kernel validates both.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustEntry {
    pub publisher_id: String,
    pub public_key: String,
}

fn read_bounded(path: &Path, cap: u64) -> Load<String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Load::Missing,
        Err(error) => return Load::Unreadable(format!("{}: {error}", path.display())),
    };
    let mut bytes = Vec::new();
    if let Err(error) = file.take(cap + 1).read_to_end(&mut bytes) {
        return Load::Unreadable(format!("{}: {error}", path.display()));
    }
    if bytes.len() as u64 > cap {
        return Load::Unreadable(format!("{} is larger than {cap} bytes", path.display()));
    }
    match String::from_utf8(bytes) {
        Ok(text) => Load::Found(text),
        Err(_) => Load::Unreadable(format!("{} is not valid UTF-8", path.display())),
    }
}

/// The containment history, in order. Any line that is not exactly one well-formed event -
/// including a torn last line - makes the whole history unreadable.
pub fn load_log(root: &LocalStateRoot) -> Load<Vec<StoredEvent>> {
    let path = match root.path_for(LOG_FILENAME) {
        Ok(path) => path,
        Err(error) => return Load::Unreadable(error.to_string()),
    };
    let text = match read_bounded(&path, MAX_LOG_BYTES) {
        Load::Found(text) => text,
        Load::Missing => return Load::Missing,
        Load::Unreadable(reason) => return Load::Unreadable(reason),
    };
    if !text.is_empty() && !text.ends_with('\n') {
        return Load::Unreadable(format!(
            "{}: the last event is incomplete (a write was interrupted)",
            path.display()
        ));
    }
    let mut events = Vec::new();
    for (number, line) in text.lines().enumerate() {
        match serde_json::from_str::<Line>(line) {
            Ok(Line::Install(bundle)) => events.push(StoredEvent::Install(bundle)),
            Ok(Line::Lift(incident)) => events.push(StoredEvent::Lift(incident)),
            Err(error) => {
                return Load::Unreadable(format!(
                    "{} line {}: {error}",
                    path.display(),
                    number + 1
                ));
            }
        }
    }
    Load::Found(events)
}

/// Appends one event as a single line with a single `write`, creating the file (and the state
/// root, which the caller established) if needed. On an error the caller must reload: a partial
/// write is detected by [`load_log`] as an incomplete last event.
pub fn append_event(root: &LocalStateRoot, event: &StoredEvent) -> Result<(), String> {
    let path = root.path_for(LOG_FILENAME).map_err(|e| e.to_string())?;
    let line = match event {
        StoredEvent::Install(text) => Line::Install(text.clone()),
        StoredEvent::Lift(incident) => Line::Lift(incident.clone()),
    };
    let mut bytes = serde_json::to_vec(&line).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    let mut options = std::fs::OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The owner's trusted publishers. A missing file means none.
pub fn load_trust(root: &LocalStateRoot) -> Load<Vec<TrustEntry>> {
    let path = match root.path_for(TRUST_FILENAME) {
        Ok(path) => path,
        Err(error) => return Load::Unreadable(error.to_string()),
    };
    match read_bounded(&path, MAX_TRUST_BYTES) {
        Load::Found(text) => match serde_json::from_str::<Vec<TrustEntry>>(&text) {
            Ok(entries) => Load::Found(entries),
            Err(error) => Load::Unreadable(format!("{}: {error}", path.display())),
        },
        Load::Missing => Load::Missing,
        Load::Unreadable(reason) => Load::Unreadable(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(label: &str) -> (Self, LocalStateRoot) {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-containment-state-{label}-{}",
                std::process::id()
            ));
            let root = LocalStateRoot::resolve(&dir).unwrap();
            (Self(dir), root)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn a_missing_log_is_missing_not_empty_found() {
        let (_guard, root) = TempRoot::new("missing");
        assert_eq!(load_log(&root), Load::Missing);
        assert_eq!(load_trust(&root), Load::Missing);
    }

    #[test]
    fn appended_events_round_trip_in_order() {
        let (_guard, root) = TempRoot::new("round-trip");
        let events = vec![
            StoredEvent::Install("{\"a\":\"line\\nbreak\"}".to_string()),
            StoredEvent::Lift("INC-1".to_string()),
        ];
        for event in &events {
            append_event(&root, event).unwrap();
        }
        assert_eq!(load_log(&root), Load::Found(events));
    }

    #[test]
    fn a_torn_last_line_or_a_malformed_line_makes_the_log_unreadable() {
        let (_guard, root) = TempRoot::new("torn");
        append_event(&root, &StoredEvent::Lift("INC-1".to_string())).unwrap();
        let path = root.path_for(LOG_FILENAME).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(b"{\"lift\":\"INC-").unwrap();
        assert!(matches!(load_log(&root), Load::Unreadable(_)));
        file.write_all(b"2\"}\n{\"grant\":\"everything\"}\n")
            .unwrap();
        assert!(matches!(load_log(&root), Load::Unreadable(_)));
    }

    #[test]
    fn an_oversized_log_is_unreadable_not_truncated() {
        let (_guard, root) = TempRoot::new("oversized");
        let path = root.path_for(LOG_FILENAME).unwrap();
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_LOG_BYTES + 1).unwrap();
        assert!(matches!(load_log(&root), Load::Unreadable(_)));
    }

    #[test]
    fn a_malformed_trust_file_is_unreadable() {
        let (_guard, root) = TempRoot::new("trust");
        let path = root.path_for(TRUST_FILENAME).unwrap();
        std::fs::write(&path, "[{\"publisher_id\":\"acme\"}]").unwrap();
        assert!(matches!(load_trust(&root), Load::Unreadable(_)));
        std::fs::write(&path, "[{\"publisher_id\":\"acme\",\"public_key\":\"00\"}]").unwrap();
        assert!(matches!(load_trust(&root), Load::Found(entries) if entries.len() == 1));
    }
}

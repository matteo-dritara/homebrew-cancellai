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
//! temporary file and no rename: a read-modify-rename cycle can lose a concurrent writer's event.
//! Appends are serialized by an exclusive SQLite transaction on `containment_lock.sqlite3` beside
//! the history - a file that is created once and never removed or written, so taking the lock is
//! not a mutation outside the one safety boundary (SI-019), and the operating system releases it
//! when its holder exits or crashes. Under that lock [`append_event`] re-reads the chain head and
//! writes only when it is still the head the caller decided on: of concurrent appends decided on
//! one history exactly one writes, and every other leaves the file byte for byte as it was (E06
//! review round 9). A crash mid-write can still leave a torn last line, which makes the history
//! fail to replay; the caller treats that as an unknown ledger - fail closed, never a silently
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

use sha2::{Digest, Sha256};

use crate::LocalStateRoot;

const LOG_FILENAME: &str = "containment_log.jsonl";
const LOCK_FILENAME: &str = "containment_lock.sqlite3";
/// How long an append waits for a concurrent one before giving up without writing.
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(30);
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

/// One persisted line: exactly one of `install`/`lift`, the digest of every line accepted before
/// it (`prev`), and a nonce naming this append. A line whose `prev` is not the digest of the lines
/// accepted before it lost a race with a concurrent append and is skipped, never applied (E06
/// review round 8: concurrent refreshes wrote duplicate sequences and broke replay). The appender
/// re-reads and learns from its nonce whether it won.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Line {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    install: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lift: Option<String>,
    prev: String,
    nonce: String,
}

/// The accepted history: the events, and the nonce of the append that wrote each.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct History {
    pub events: Vec<StoredEvent>,
    pub nonces: Vec<String>,
    /// The chain digest after the last accepted line: what an append decided on this history
    /// must name as its `prev`.
    pub head: String,
}

/// The chain head of an empty (or missing) history.
pub fn empty_head() -> String {
    hex(&Sha256::new().finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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

/// The containment history, in order - only the lines that continue the digest chain. Any line
/// that is not one well-formed event, including a torn last line, makes the history unreadable.
pub fn load_history(root: &LocalStateRoot) -> Load<History> {
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
    let mut chain = Sha256::new();
    let mut history = History::default();
    for (number, line) in text.lines().enumerate() {
        let parsed: Line = match serde_json::from_str(line) {
            Ok(parsed) => parsed,
            Err(error) => {
                return Load::Unreadable(format!(
                    "{} line {}: {error}",
                    path.display(),
                    number + 1
                ));
            }
        };
        let event = match (parsed.install, parsed.lift) {
            (Some(bundle), None) => StoredEvent::Install(bundle),
            (None, Some(incident)) => StoredEvent::Lift(incident),
            _ => {
                return Load::Unreadable(format!(
                    "{} line {}: exactly one of install/lift is required",
                    path.display(),
                    number + 1
                ));
            }
        };
        if parsed.prev != hex(&chain.clone().finalize()) {
            // Lost a race with a concurrent append: not part of the history.
            continue;
        }
        chain.update(line.as_bytes());
        chain.update(b"\n");
        history.events.push(event);
        history.nonces.push(parsed.nonce);
    }
    history.head = hex(&chain.finalize());
    Load::Found(history)
}

/// The accepted events alone. See [`load_history`].
pub fn load_log(root: &LocalStateRoot) -> Load<Vec<StoredEvent>> {
    match load_history(root) {
        Load::Found(history) => Load::Found(history.events),
        Load::Missing => Load::Missing,
        Load::Unreadable(reason) => Load::Unreadable(reason),
    }
}

fn fresh_nonce() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut digest = Sha256::new();
    digest.update(std::process::id().to_le_bytes());
    digest.update(COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    digest.update(nanos.to_le_bytes());
    hex(&digest.finalize())
        .get(..32)
        .unwrap_or_default()
        .to_string()
}

/// Appends `event` as a continuation of the history whose chain head is `head` - the history the
/// caller read and decided on - as one line with one `write`, creating the file if needed.
///
/// Compare-and-append: under the exclusive append lock the history is re-read, and if anything
/// was accepted after the caller's read the event is not written at all and `Ok(None)` says so;
/// a decision made on a stale history is never applied and never leaves a line behind (E06 review
/// rounds 8-9). `Ok(Some(nonce))` names the line written. On an error nothing was decided; the
/// caller must reload, because a partial write is detected by [`load_log`] as an incomplete last
/// event.
pub fn append_event(
    root: &LocalStateRoot,
    event: &StoredEvent,
    head: &str,
) -> Result<Option<String>, String> {
    let lock_path = root.path_for(LOCK_FILENAME).map_err(|e| e.to_string())?;
    let lock = rusqlite::Connection::open(&lock_path)
        .and_then(|conn| conn.busy_timeout(LOCK_WAIT).map(|()| conn))
        .and_then(|conn| conn.execute_batch("BEGIN EXCLUSIVE").map(|()| conn))
        .map_err(|e| {
            format!(
                "{}: could not take the append lock: {e}",
                lock_path.display()
            )
        })?;
    let current = match load_history(root) {
        Load::Found(history) => history.head,
        Load::Missing => empty_head(),
        Load::Unreadable(reason) => return Err(reason),
    };
    if current != head {
        return Ok(None);
    }
    let written = write_line(root, event, head);
    // Nothing was written through the connection; ending the transaction only releases the lock,
    // which dropping the connection would also do.
    drop(lock.execute_batch("COMMIT"));
    written.map(Some)
}

fn write_line(root: &LocalStateRoot, event: &StoredEvent, head: &str) -> Result<String, String> {
    let path = root.path_for(LOG_FILENAME).map_err(|e| e.to_string())?;
    let nonce = fresh_nonce();
    let (install, lift) = match event {
        StoredEvent::Install(text) => (Some(text.clone()), None),
        StoredEvent::Lift(incident) => (None, Some(incident.clone())),
    };
    let line = Line {
        install,
        lift,
        prev: head.to_string(),
        nonce: nonce.clone(),
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
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(nonce)
}

const FEED_OVERRIDE_FILENAME: &str = "containment_feed_url";

/// The owner's override of the containment feed URL (E33-S01), as raw text; the CLI validates it.
/// A missing file means the compiled project feed.
pub fn load_feed_override(root: &LocalStateRoot) -> Load<String> {
    let path = match root.path_for(FEED_OVERRIDE_FILENAME) {
        Ok(path) => path,
        Err(error) => return Load::Unreadable(error.to_string()),
    };
    read_bounded(&path, 4096)
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

    fn current_head(root: &LocalStateRoot) -> String {
        match load_history(root) {
            Load::Found(history) => history.head,
            _ => empty_head(),
        }
    }

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
            append_event(&root, event, &current_head(&root))
                .unwrap()
                .unwrap();
        }
        assert_eq!(load_log(&root), Load::Found(events));
    }

    #[test]
    fn a_torn_last_line_or_a_malformed_line_makes_the_log_unreadable() {
        let (_guard, root) = TempRoot::new("torn");
        append_event(
            &root,
            &StoredEvent::Lift("INC-1".to_string()),
            &current_head(&root),
        )
        .unwrap();
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
    fn a_line_that_lost_a_concurrent_append_is_skipped_and_its_nonce_is_not_accepted() {
        let (_guard, root) = TempRoot::new("stale-append");
        let won = append_event(
            &root,
            &StoredEvent::Lift("INC-1".to_string()),
            &current_head(&root),
        )
        .unwrap()
        .unwrap();
        // A second writer that read the history before the first append landed: its `prev` is
        // the digest of the empty history.
        let path = root.path_for(LOG_FILENAME).unwrap();
        let stale = Line {
            install: None,
            lift: Some("INC-2".to_string()),
            prev: hex(&Sha256::new().finalize()),
            nonce: "lost".to_string(),
        };
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(format!("{}\n", serde_json::to_string(&stale).unwrap()).as_bytes())
            .unwrap();
        let Load::Found(history) = load_history(&root) else {
            panic!("history must load");
        };
        assert_eq!(history.events, vec![StoredEvent::Lift("INC-1".to_string())]);
        assert_eq!(history.nonces, vec![won]);
        // A later append continues the accepted chain, not the skipped line.
        let next = append_event(
            &root,
            &StoredEvent::Lift("INC-3".to_string()),
            &current_head(&root),
        )
        .unwrap()
        .unwrap();
        let Load::Found(history) = load_history(&root) else {
            panic!("history must load");
        };
        assert_eq!(history.events.len(), 2);
        assert_eq!(history.nonces.last(), Some(&next));
    }

    // E06 review round 9: a stale append wrote its line before learning it had lost, changing
    // the bytes of a history it then reported as current.
    #[test]
    fn a_stale_append_writes_nothing() {
        let (_guard, root) = TempRoot::new("stale-writes-nothing");
        let event = StoredEvent::Install("{\"notice\":1}".to_string());
        let decided_on = empty_head();
        assert!(append_event(&root, &event, &decided_on).unwrap().is_some());
        let path = root.path_for(LOG_FILENAME).unwrap();
        let before = std::fs::read(&path).unwrap();
        assert_eq!(append_event(&root, &event, &decided_on).unwrap(), None);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn concurrent_appends_decided_on_one_history_write_exactly_one_line() {
        let (_guard, root) = TempRoot::new("concurrent-appends");
        let decided_on = empty_head();
        let outcomes: Vec<Option<String>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..16)
                .map(|n| {
                    let root = &root;
                    let decided_on = &decided_on;
                    scope.spawn(move || {
                        let event = StoredEvent::Lift(format!("INC-{n}"));
                        append_event(root, &event, decided_on).unwrap()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert_eq!(outcomes.iter().filter(|o| o.is_some()).count(), 1);
        let text = std::fs::read_to_string(root.path_for(LOG_FILENAME).unwrap()).unwrap();
        assert_eq!(text.lines().count(), 1);
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

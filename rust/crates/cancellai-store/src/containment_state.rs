//! Persistence for signed incident containment (E06-S07, ADR-0039): the ledger's history and the
//! owner's own trusted publishers, both under cancellAI's local-state root.
//!
//! **This module never interprets a notice.** It stores the raw text of each installed bundle
//! and each local lift as opaque strings; `cancellai-safety::ContainmentLedger::replay` parses and
//! re-verifies them every time the ledger is needed. Storing verified *evidence* instead would
//! give the kernel a second way to construct `IncidentEvidence` - from a file - which SI-022
//! forbids.
//!
//! **One SQLite table, decided and appended in one transaction (ADR-0040, E33-S03).** Each event is
//! one row of raw text. [`transact`] opens a `BEGIN IMMEDIATE` transaction, reads every row, hands
//! them to the caller - which replays and verifies them through `cancellai-safety` and decides -
//! and inserts at most the one row the caller decided on before committing. A decision that refuses,
//! finds the notice already current, or runs after a concurrent change inserts nothing, and an
//! interrupted transaction contributes no row: there is no losing line and no torn line. Rounds 8
//! and 9 of the E06 review found both in the JSONL history this replaces; containment had not
//! shipped, so nothing migrates. SQLite is persistence only: it stores opaque text and decides
//! nothing.
//!
//! **What the history guarantees, and what it does not (owner decision, 2026-09-23).** The
//! history resists everything that arrives from outside - a notice, a replayed or expired bundle,
//! a forged signature - because every event is re-verified. It does not resist the owner's own
//! account: a process running as the same user can delete the file, truncate it to an earlier
//! valid prefix, or append a lift, and each of those lifts containment. ADR-0039 already treats
//! deleting this state as a local act; editing it is the same act. A same-user attacker can also
//! delete the provider files directly, so the history adds no capability that attacker lacks.

use std::io::Read;
use std::path::Path;

use rusqlite::{Connection, OpenFlags, TransactionBehavior};

use crate::LocalStateRoot;

const LEDGER_FILENAME: &str = "containment_ledger.sqlite3";
/// How long a transaction waits for a concurrent one before giving up without writing.
const BUSY_WAIT: std::time::Duration = std::time::Duration::from_secs(30);
const TRUST_FILENAME: &str = "trusted_publishers.json";
/// The largest ledger this module will read. A larger file is unreadable, not truncated.
pub const MAX_LEDGER_BYTES: u64 = 16 * 1024 * 1024;
const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS events (\
    seq INTEGER PRIMARY KEY AUTOINCREMENT, \
    kind TEXT NOT NULL CHECK (kind IN ('install', 'lift')), \
    payload TEXT NOT NULL)";
/// The largest owner trust file this module will read.
pub const MAX_TRUST_BYTES: u64 = 64 * 1024;

/// One persisted history entry, as opaque text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredEvent {
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

/// What a [`transact`] caller decided, having read the history: append one event, or keep the
/// history exactly as it is. Either carries the caller's own result.
pub enum Decision<T> {
    Append(StoredEvent, T),
    Keep(T),
}

fn too_large(path: &Path) -> Option<String> {
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_LEDGER_BYTES => Some(format!(
            "{} is larger than {MAX_LEDGER_BYTES} bytes",
            path.display()
        )),
        _ => None,
    }
}

fn read_rows(conn: &Connection, path: &Path) -> Result<Vec<StoredEvent>, String> {
    let describe = |e: rusqlite::Error| format!("{}: {e}", path.display());
    let mut statement = conn
        .prepare("SELECT kind, payload FROM events ORDER BY seq")
        .map_err(describe)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(describe)?;
    let mut events = Vec::new();
    for row in rows {
        let (kind, payload) = row.map_err(describe)?;
        events.push(match kind.as_str() {
            "install" => StoredEvent::Install(payload),
            "lift" => StoredEvent::Lift(payload),
            other => {
                return Err(format!(
                    "{}: an event of unknown kind {other:?}",
                    path.display()
                ));
            }
        });
    }
    Ok(events)
}

/// The containment history, in order, read without writing anything. A database that exists and
/// cannot be opened, read or parsed - a missing table included - is unreadable, never empty.
pub fn load_events(root: &LocalStateRoot) -> Load<Vec<StoredEvent>> {
    let path = match root.path_for(LEDGER_FILENAME) {
        Ok(path) => path,
        Err(error) => return Load::Unreadable(error.to_string()),
    };
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Load::Missing,
        Err(error) => return Load::Unreadable(format!("{}: {error}", path.display())),
        Ok(_) => {}
    }
    if let Some(reason) = too_large(&path) {
        return Load::Unreadable(reason);
    }
    let opened = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .and_then(|conn| conn.busy_timeout(BUSY_WAIT).map(|()| conn));
    match opened {
        Ok(conn) => match read_rows(&conn, &path) {
            Ok(events) => Load::Found(events),
            Err(reason) => Load::Unreadable(reason),
        },
        Err(error) => Load::Unreadable(format!("{}: {error}", path.display())),
    }
}

/// Reads the history, lets `decide` judge it, and appends at most the one event it decided on -
/// all inside one `BEGIN IMMEDIATE` transaction, so no other process can append between the read
/// and the write. [`Decision::Keep`] inserts nothing. `before_commit` runs after the insert and
/// before the commit; the CLI's crash harness kills the process there. An `Err` means nothing was
/// decided or nothing was committed.
pub fn transact<T>(
    root: &LocalStateRoot,
    decide: impl FnOnce(&[StoredEvent]) -> Decision<T>,
    before_commit: impl FnOnce(),
) -> Result<T, String> {
    let path = root.path_for(LEDGER_FILENAME).map_err(|e| e.to_string())?;
    if let Some(reason) = too_large(&path) {
        return Err(reason);
    }
    create_private(&path)?;
    let describe = |e: rusqlite::Error| format!("{}: {e}", path.display());
    let mut conn = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(describe)?;
    conn.busy_timeout(BUSY_WAIT).map_err(describe)?;
    // Committed on its own, so every ledger this module ever created has its table, whatever a
    // decision later does.
    conn.execute(CREATE_TABLE, []).map_err(describe)?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(describe)?;
    let events = read_rows(&transaction, &path)?;
    match decide(&events) {
        Decision::Keep(result) => Ok(result), // dropping the transaction rolls it back
        Decision::Append(event, result) => {
            let (kind, payload) = match event {
                StoredEvent::Install(text) => ("install", text),
                StoredEvent::Lift(incident) => ("lift", incident),
            };
            transaction
                .execute(
                    "INSERT INTO events (kind, payload) VALUES (?1, ?2)",
                    [kind, payload.as_str()],
                )
                .map_err(describe)?;
            before_commit();
            transaction.commit().map_err(describe)?;
            Ok(result)
        }
    }
}

/// Creates the ledger file owner-only if it does not exist yet; SQLite would create it with the
/// process umask.
fn create_private(path: &Path) -> Result<(), String> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map(drop)
        .map_err(|e| format!("{}: {e}", path.display()))
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

    fn append(root: &LocalStateRoot, event: StoredEvent) {
        transact(root, |_| Decision::Append(event, ()), || {}).unwrap();
    }

    #[test]
    fn a_missing_ledger_is_missing_not_empty_found() {
        let (_guard, root) = TempRoot::new("missing");
        assert_eq!(load_events(&root), Load::Missing);
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
            append(&root, event.clone());
        }
        assert_eq!(load_events(&root), Load::Found(events));
    }

    #[test]
    fn the_decision_sees_the_history_it_appends_to() {
        let (_guard, root) = TempRoot::new("decision-sees");
        append(&root, StoredEvent::Lift("INC-1".to_string()));
        let seen = transact(&root, |events| Decision::Keep(events.to_vec()), || {}).unwrap();
        assert_eq!(seen, vec![StoredEvent::Lift("INC-1".to_string())]);
    }

    // E33-S03 AC2: a decision that keeps the history inserts nothing, and a first decision that
    // keeps it still leaves a readable, empty ledger behind.
    #[test]
    fn a_kept_decision_inserts_nothing() {
        let (_guard, root) = TempRoot::new("keep");
        transact(&root, |_| Decision::Keep(()), || {}).unwrap();
        assert_eq!(load_events(&root), Load::Found(Vec::new()));
        append(&root, StoredEvent::Lift("INC-1".to_string()));
        transact(&root, |_| Decision::Keep(()), || {}).unwrap();
        assert_eq!(
            load_events(&root),
            Load::Found(vec![StoredEvent::Lift("INC-1".to_string())])
        );
    }

    // E06 review rounds 8-9: concurrent appends decided on one history. Each thread decides to
    // append only if the history is still empty; exactly one may.
    #[test]
    fn concurrent_decisions_on_one_history_append_exactly_one_event() {
        let (_guard, root) = TempRoot::new("concurrent");
        transact(&root, |_| Decision::Keep(()), || {}).unwrap();
        let appended: Vec<bool> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..16)
                .map(|n| {
                    let root = &root;
                    scope.spawn(move || {
                        transact(
                            root,
                            |events| {
                                if events.is_empty() {
                                    Decision::Append(StoredEvent::Lift(format!("INC-{n}")), true)
                                } else {
                                    Decision::Keep(false)
                                }
                            },
                            || {},
                        )
                        .unwrap()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert_eq!(appended.iter().filter(|a| **a).count(), 1);
        let Load::Found(events) = load_events(&root) else {
            panic!("ledger must load");
        };
        assert_eq!(events.len(), 1);
    }

    // E33-S03 AC3: an event whose transaction never commits leaves no trace. A panic inside
    // `before_commit` unwinds through the uncommitted transaction.
    #[test]
    fn an_uncommitted_event_leaves_no_trace() {
        let (_guard, root) = TempRoot::new("uncommitted");
        append(&root, StoredEvent::Lift("INC-1".to_string()));
        let interrupted = std::panic::catch_unwind(|| {
            transact(
                &root,
                |_| Decision::Append(StoredEvent::Lift("INC-2".to_string()), ()),
                || panic!("interrupted before commit"),
            )
        });
        assert!(interrupted.is_err());
        assert_eq!(
            load_events(&root),
            Load::Found(vec![StoredEvent::Lift("INC-1".to_string())])
        );
    }

    #[test]
    fn a_corrupt_or_foreign_database_is_unreadable_and_refuses_writes() {
        let (_guard, root) = TempRoot::new("corrupt");
        let path = root.path_for(LEDGER_FILENAME).unwrap();
        std::fs::write(
            &path,
            b"not a database at all, padded past the header size......",
        )
        .unwrap();
        assert!(matches!(load_events(&root), Load::Unreadable(_)));
        let before = std::fs::read(&path).unwrap();
        assert!(
            transact(
                &root,
                |_| Decision::Append(StoredEvent::Lift("INC-1".to_string()), ()),
                || {}
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn an_event_of_unknown_kind_makes_the_ledger_unreadable() {
        let (_guard, root) = TempRoot::new("unknown-kind");
        append(&root, StoredEvent::Lift("INC-1".to_string()));
        let path = root.path_for(LEDGER_FILENAME).unwrap();
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE loose (kind TEXT, payload TEXT); DROP TABLE events; \
             ALTER TABLE loose RENAME TO events; \
             INSERT INTO events VALUES ('grant', 'everything');",
        )
        .unwrap();
        drop(conn);
        assert!(matches!(load_events(&root), Load::Unreadable(_)));
    }

    #[test]
    fn an_oversized_ledger_is_unreadable_not_truncated() {
        let (_guard, root) = TempRoot::new("oversized");
        let path = root.path_for(LEDGER_FILENAME).unwrap();
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_LEDGER_BYTES + 1).unwrap();
        assert!(matches!(load_events(&root), Load::Unreadable(_)));
        assert!(transact(&root, |_| Decision::Keep(()), || {}).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn the_ledger_is_created_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let (_guard, root) = TempRoot::new("mode");
        append(&root, StoredEvent::Lift("INC-1".to_string()));
        let path = root.path_for(LEDGER_FILENAME).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
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

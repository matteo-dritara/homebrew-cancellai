//! The operational event ledger (E13-S02, `docs/architecture/PERSISTENCE_MODEL.md`'s "Layer 2:
//! Operational Event Ledger"). Records that a significant classification/policy/mutation/
//! lifecycle event happened, never the artifact content it happened to. Lives in this crate
//! rather than a new one for the same reason ADR-0019 already names the store and the ledger
//! together as one story pair ("E13 a SQLite current-state store and event ledger"): both are
//! bundled-`rusqlite` (outer ring), both hold cancellAI's own disposable state, and a caller
//! that opens one typically opens the other.
//!
//! ## Append-only, enforced by the database itself (AC "immutable after commit")
//!
//! `ledger_events` has no public update/delete API - `append` is the only way a row is ever
//! written, and the schema also carries `BEFORE UPDATE`/`BEFORE DELETE` triggers that `RAISE
//! (ABORT, ...)` unconditionally for update, and for delete unless a one-row gate
//! (`ledger_control.compaction_in_progress`) is set. Only [`EventLedger::compact_range`] ever
//! sets that gate, inside the same transaction as the delete it performs and the compaction
//! summary it writes - so the *only* way an event row is ever removed is through an explicit,
//! non-silent compaction that replaces the range with a signed/hashed summary row, never a
//! future method that forgets to check anything. This is enforced at the SQLite layer, not
//! merely by the absence of a Rust method - `tests::raw_update_against_a_committed_event_is_
//! rejected_by_the_database_itself` and its delete counterpart attempt exactly that bypass
//! directly against the connection and assert the trigger refuses it.
//!
//! `ledger_compactions` (the summary rows) is immutable the same way, unconditionally - a
//! summary itself is never revised once written.
//!
//! ## Mutation events carry plan/evidence references (AC "every mutation references plan ID
//! and evidence ID(s)")
//!
//! [`EventKind::is_mutation`] names the six kinds `docs/architecture/PERSISTENCE_MODEL.md`'s
//! event-kind list and this story's own contract treat as representing an action rather than a
//! pure observation: `PLAN_CREATED`, `ACTION_BLOCKED`, `QUARANTINED`, `RESTORED`, `ARCHIVED`,
//! `PURGED`. [`EventLedger::append`] refuses (fail-closed, no partial write) any event of one
//! of those kinds whose [`NewEvent::mutation`] is absent, carries an empty `plan_id`, or an
//! empty `evidence_ids` list.
//!
//! ## Contentless by default (`docs/architecture/PERSISTENCE_MODEL.md`: "Event payloads are
//! contentless by default")
//!
//! [`EventMetadata`] is a closed, allowlisted set of fields (`artifact_id`, `provider_id`,
//! `category`, `policy_id`, `reason_code`) - there is no free-form map or blob field a caller
//! could use to smuggle a path, transcript, or artifact content into the ledger.
//! `tests::ledger_events_schema_has_only_the_allowlisted_columns` pins the actual table shape
//! against that closed set, so a future change that widens it is a visible, reviewable diff
//! rather than a silent schema drift. This crate cannot stop a caller from putting something it
//! should not into e.g. `reason_code` (a `String`, not a content-typed field) - the same
//! residual `cancellai_model::Action`/`Precondition` already accept as inert data; see this
//! story's evidence packet for the residual-risk record.
//!
//! ## Compaction preserves audit/aggregate semantics (AC "compaction into signed/hashed
//! summary records")
//!
//! [`EventLedger::compact_range`] requires the requested `[from, to]` range to be exactly and
//! contiguously present (row count must equal `to - from + 1`) - this is what stops a range
//! that overlaps an earlier compaction, or reaches past the newest appended event, from
//! silently summarizing fewer events than the caller asked for
//! (`tests::compact_range_rejects_a_range_with_a_gap_from_a_prior_compaction`). The resulting
//! [`CompactionSummary`] carries the exact event count, a per-kind breakdown
//! (`kind_counts` - so "how many `QUARANTINED` events happened in this window" stays
//! answerable after the raw rows are gone), and a SHA-256 digest over a canonical, ordered
//! encoding of every summarized event (tamper-evident: a summary cannot be quietly
//! re-attributed to a different set of events without changing the digest).

use cancellai_model::{ArtifactId, EvidenceId};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Why an [`EventLedger`] operation failed - the underlying SQLite/JSON error, or this
/// module's own contract violation (a mutation-class event missing its plan/evidence
/// reference, or a compaction range that is not exactly and contiguously present).
#[derive(Debug)]
pub struct LedgerError(String);

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LedgerError {}

impl From<rusqlite::Error> for LedgerError {
    fn from(e: rusqlite::Error) -> Self {
        Self(e.to_string())
    }
}

impl From<serde_json::Error> for LedgerError {
    fn from(e: serde_json::Error) -> Self {
        Self(format!(
            "stored evidence_ids column did not round-trip as JSON: {e}"
        ))
    }
}

/// The ledger's own migration history - a separate `PRAGMA user_version` namespace from
/// [`crate::CurrentStateStore`]'s, because each opens its own `Connection` to its own file
/// (`docs/architecture/PERSISTENCE_MODEL.md`'s Layer 1/Layer 2 carry separate self-budgets, so
/// a caller is expected to give them separate paths). See this module's own doc for why the
/// triggers here are load-bearing, not incidental.
const MIGRATIONS: &[&str] = &["
    CREATE TABLE ledger_events (
        event_id INTEGER PRIMARY KEY AUTOINCREMENT,
        kind TEXT NOT NULL,
        recorded_at INTEGER NOT NULL,
        artifact_id TEXT,
        provider_id TEXT,
        category TEXT,
        policy_id TEXT,
        reason_code TEXT,
        plan_id TEXT,
        evidence_ids TEXT NOT NULL DEFAULT '[]'
    ) STRICT;

    CREATE TABLE ledger_control (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        compaction_in_progress INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    INSERT INTO ledger_control (id, compaction_in_progress) VALUES (1, 0);

    CREATE TABLE ledger_compactions (
        compaction_id INTEGER PRIMARY KEY AUTOINCREMENT,
        from_event_id INTEGER NOT NULL,
        to_event_id INTEGER NOT NULL,
        event_count INTEGER NOT NULL,
        kind_counts TEXT NOT NULL,
        digest_hex TEXT NOT NULL,
        created_at INTEGER NOT NULL
    ) STRICT;

    CREATE TABLE cancellai_ledger_identity (marker TEXT PRIMARY KEY) STRICT;
    INSERT INTO cancellai_ledger_identity (marker) VALUES ('cancellai-event-ledger-v1');

    CREATE TRIGGER ledger_events_forbid_update
    BEFORE UPDATE ON ledger_events
    BEGIN
        SELECT RAISE(ABORT, 'ledger events are immutable: update is never permitted');
    END;

    CREATE TRIGGER ledger_events_forbid_delete
    BEFORE DELETE ON ledger_events
    WHEN (SELECT compaction_in_progress FROM ledger_control WHERE id = 1) = 0
    BEGIN
        SELECT RAISE(ABORT, 'ledger events can only be removed by an explicit compaction');
    END;

    CREATE TRIGGER ledger_compactions_forbid_update
    BEFORE UPDATE ON ledger_compactions
    BEGIN
        SELECT RAISE(ABORT, 'compaction summaries are immutable');
    END;

    CREATE TRIGGER ledger_compactions_forbid_delete
    BEFORE DELETE ON ledger_compactions
    BEGIN
        SELECT RAISE(ABORT, 'compaction summaries are immutable');
    END;
"];

/// Applies every migration in `migrations` the database has not already applied - a scoped
/// copy of `crate::apply_migrations`'s own logic (same `PRAGMA user_version`/one-transaction-
/// per-migration pattern) rather than a shared function, so this module's [`LedgerError`] does
/// not have to become [`crate::StoreError`]'s concern or vice versa - the same
/// keep-error-types-local precedent `cancellai_platform::mutation`'s own local `encode_hex`
/// copy of `cancellai_safety::knowledge_bundle`'s already uses in this workspace.
fn apply_migrations(conn: &Connection, migrations: &[&str]) -> Result<(), LedgerError> {
    let current_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let current_version = usize::try_from(current_version).unwrap_or(0);
    for (index, migration) in migrations.iter().enumerate().skip(current_version) {
        let next_version = index + 1;
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(migration)?;
        tx.execute_batch(&format!("PRAGMA user_version = {next_version}"))?;
        tx.commit()?;
    }
    Ok(())
}

/// The value [`MIGRATIONS`]' first migration inserts into `cancellai_ledger_identity` - the
/// event ledger's own copy of `crate::verify_store_identity`'s marker check (SI-026, round 2
/// independent verifier review's finding against `CurrentStateStore` reproduced identically
/// here: a hand-crafted file with `ledger_events`/`ledger_control`/`ledger_compactions` in
/// migration 0's exact shape and a matching `user_version` opened and reset with no ownership
/// check at all - self-review, round 2 pre-independent-round-3). Each layer keeps its own
/// migration history and `Connection` (this module's own doc, above), so this check is a
/// separate per-module marker/constant rather than a shared one - the same independence
/// precedent this crate already applies to schema, connection and error type.
const LEDGER_IDENTITY_MARKER: &str = "cancellai-event-ledger-v1";

/// Refuses to treat `conn` as a valid event ledger unless it carries this module's own identity
/// marker - see [`LEDGER_IDENTITY_MARKER`]'s own doc and `crate::verify_store_identity`'s
/// identical reasoning for `CurrentStateStore`.
fn verify_ledger_identity(conn: &Connection) -> Result<(), LedgerError> {
    let marker: Result<String, rusqlite::Error> =
        conn.query_row("SELECT marker FROM cancellai_ledger_identity", [], |row| {
            row.get(0)
        });
    match marker {
        Ok(value) if value == LEDGER_IDENTITY_MARKER => Ok(()),
        _ => Err(LedgerError(
            "this database does not carry cancellai-store's own ledger identity marker - \
             refusing to open it as an event ledger rather than risk treating provider-owned or \
             unrelated content as this crate's own"
                .into(),
        )),
    }
}

/// Refuses (`Err`) a file whose `PRAGMA user_version` is already past the first migration but
/// which does not carry [`LEDGER_IDENTITY_MARKER`] - checked *before* [`apply_migrations`] runs
/// anything further, so a file that turns out not to be this crate's own is never mutated on the
/// way to being refused (self-review's own further finding: checking identity only *after*
/// migrations already ran had let a non-owned intermediate-version file receive a real schema
/// mutation before its open was refused). A `user_version` of `0` (a brand new or genuinely empty
/// file) has not run migration 0 yet, so it cannot carry the marker yet either - that is the
/// ordinary fresh-database path, not a rejection.
fn verify_ledger_identity_before_migrating(conn: &Connection) -> Result<(), LedgerError> {
    let current_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if current_version > 0 {
        verify_ledger_identity(conn)?;
    }
    Ok(())
}

/// Hex-encodes `bytes` in lowercase, one `%02x` pair per byte - the same local-copy convention
/// `cancellai_platform::mutation::encode_hex` documents for itself.
fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// One of the event kinds `docs/architecture/PERSISTENCE_MODEL.md`'s Layer 2 names. `Discovered`
/// through `AnomalyDetected` are pure observations; [`EventKind::is_mutation`] names the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Discovered,
    Classified,
    LifecycleChanged,
    PolicyChanged,
    AnomalyDetected,
    PlanCreated,
    ActionBlocked,
    Quarantined,
    Restored,
    Archived,
    Purged,
}

impl EventKind {
    /// The exact string this kind is stored/read under - an explicit, exhaustive mapping
    /// (matching `crate::activity_state_key`'s own precedent) so a future variant this module
    /// does not yet handle fails to compile rather than silently falling back to something.
    fn key(self) -> &'static str {
        match self {
            EventKind::Discovered => "DISCOVERED",
            EventKind::Classified => "CLASSIFIED",
            EventKind::LifecycleChanged => "LIFECYCLE_CHANGED",
            EventKind::PolicyChanged => "POLICY_CHANGED",
            EventKind::AnomalyDetected => "ANOMALY_DETECTED",
            EventKind::PlanCreated => "PLAN_CREATED",
            EventKind::ActionBlocked => "ACTION_BLOCKED",
            EventKind::Quarantined => "QUARANTINED",
            EventKind::Restored => "RESTORED",
            EventKind::Archived => "ARCHIVED",
            EventKind::Purged => "PURGED",
        }
    }

    fn from_key(key: &str) -> Result<Self, LedgerError> {
        match key {
            "DISCOVERED" => Ok(EventKind::Discovered),
            "CLASSIFIED" => Ok(EventKind::Classified),
            "LIFECYCLE_CHANGED" => Ok(EventKind::LifecycleChanged),
            "POLICY_CHANGED" => Ok(EventKind::PolicyChanged),
            "ANOMALY_DETECTED" => Ok(EventKind::AnomalyDetected),
            "PLAN_CREATED" => Ok(EventKind::PlanCreated),
            "ACTION_BLOCKED" => Ok(EventKind::ActionBlocked),
            "QUARANTINED" => Ok(EventKind::Quarantined),
            "RESTORED" => Ok(EventKind::Restored),
            "ARCHIVED" => Ok(EventKind::Archived),
            "PURGED" => Ok(EventKind::Purged),
            other => Err(LedgerError(format!("unknown stored event kind: {other}"))),
        }
    }

    /// The kinds `docs/architecture/PERSISTENCE_MODEL.md`'s "Mutation events reference the
    /// plan ID, evidence IDs..." sentence applies to - events that represent an action taken
    /// or blocked, not a pure observation. [`EventLedger::append`] requires a
    /// [`MutationReference`] for exactly these.
    pub fn is_mutation(self) -> bool {
        matches!(
            self,
            EventKind::PlanCreated
                | EventKind::ActionBlocked
                | EventKind::Quarantined
                | EventKind::Restored
                | EventKind::Archived
                | EventKind::Purged
        )
    }
}

/// The closed, allowlisted metadata an event may carry - this module's own doc,
/// "Contentless by default". Every field is optional: a caller supplies only what it knows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventMetadata {
    pub artifact_id: Option<ArtifactId>,
    pub provider_id: Option<String>,
    pub category: Option<String>,
    pub policy_id: Option<String>,
    pub reason_code: Option<String>,
}

/// What a mutation-class event references - required for every [`EventKind::is_mutation`]
/// kind, per this module's own doc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationReference {
    pub plan_id: String,
    pub evidence_ids: Vec<EvidenceId>,
}

/// One event to append. `recorded_at` is seconds since the Unix epoch, supplied by the
/// caller rather than read from `std::time::SystemTime` here - the same "production code
/// never calls `SystemTime::now()` directly" seam `cancellai_platform::clock::Clock`
/// documents for itself; a future orchestrator wires a real clock, this crate stays
/// deterministic and independently testable without one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent {
    pub kind: EventKind,
    pub recorded_at: u64,
    pub metadata: EventMetadata,
    pub mutation: Option<MutationReference>,
}

/// An opaque, monotonically increasing append-order reference - the database's own `rowid`
/// under `AUTOINCREMENT`, which SQLite guarantees never reuses a value once assigned, even
/// after the row it named is later removed by [`EventLedger::compact_range`] (AC "append
/// order" - a compacted event's id is retired, never handed to a new, unrelated event).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(pub i64);

/// One event as read back from the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEvent {
    pub event_id: EventId,
    pub kind: EventKind,
    pub recorded_at: u64,
    pub metadata: EventMetadata,
    pub mutation: Option<MutationReference>,
}

/// The signed/hashed record [`EventLedger::compact_range`] writes in place of the raw events
/// it removes - this module's own doc, "Compaction preserves audit/aggregate semantics".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionSummary {
    pub compaction_id: i64,
    pub from_event_id: EventId,
    pub to_event_id: EventId,
    pub event_count: u64,
    /// `(event kind key, count)`, ordered by kind key - the per-kind breakdown that keeps an
    /// aggregate such as "how many `QUARANTINED` events" answerable once the raw rows summarized
    /// here are gone.
    pub kind_counts: Vec<(String, u64)>,
    /// Lowercase hex SHA-256 over a canonical, ordered encoding of every summarized event.
    pub digest_hex: String,
    pub created_at: u64,
}

/// The append-only operational event ledger (E13-S02, this module's own doc). Holds one open
/// connection for its lifetime, matching `crate::CurrentStateStore`'s own single-owner design.
pub struct EventLedger {
    conn: Connection,
}

impl EventLedger {
    /// Opens (creating if absent) the ledger database at `path`, applying any migration this
    /// database has not already seen. Use a path distinct from
    /// [`crate::CurrentStateStore::open`]'s - see this module's own doc on `MIGRATIONS`.
    pub fn open(path: &Path) -> Result<Self, LedgerError> {
        let conn = Connection::open(path)?;
        verify_ledger_identity_before_migrating(&conn)?;
        apply_migrations(&conn, MIGRATIONS)?;
        verify_ledger_identity(&conn)?;
        Ok(Self { conn })
    }

    /// An in-memory ledger for tests and short-lived callers that never need a file on disk.
    pub fn open_in_memory() -> Result<Self, LedgerError> {
        let conn = Connection::open_in_memory()?;
        apply_migrations(&conn, MIGRATIONS)?;
        verify_ledger_identity(&conn)?;
        Ok(Self { conn })
    }

    /// Appends one event, returning the [`EventId`] SQLite assigned it. Fails closed - no row
    /// is written at all - when `event.kind.is_mutation()` and `event.mutation` is absent, or
    /// carries an empty `plan_id`/`evidence_ids` (this module's own doc, "Mutation events carry
    /// plan/evidence references").
    pub fn append(&mut self, event: NewEvent) -> Result<EventId, LedgerError> {
        if event.kind.is_mutation() {
            match &event.mutation {
                Some(m) if !m.plan_id.trim().is_empty() && !m.evidence_ids.is_empty() => {}
                _ => {
                    return Err(LedgerError(format!(
                        "{} is a mutation-class event and requires a non-empty plan_id and \
                         at least one evidence_id",
                        event.kind.key()
                    )));
                }
            }
        }

        let evidence_ids_json = match &event.mutation {
            Some(m) => serde_json::to_string(&m.evidence_ids)?,
            None => "[]".to_string(),
        };
        let plan_id = event.mutation.as_ref().map(|m| m.plan_id.as_str());
        let recorded_at = i64::try_from(event.recorded_at).map_err(|_| {
            LedgerError("recorded_at does not fit in a signed 64-bit column".into())
        })?;

        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO ledger_events \
                (kind, recorded_at, artifact_id, provider_id, category, policy_id, \
                 reason_code, plan_id, evidence_ids) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                event.kind.key(),
                recorded_at,
                event.metadata.artifact_id.as_ref().map(|a| a.0.as_str()),
                event.metadata.provider_id,
                event.metadata.category,
                event.metadata.policy_id,
                event.metadata.reason_code,
                plan_id,
                evidence_ids_json,
            ],
        )?;
        let event_id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(EventId(event_id))
    }

    /// Every event currently in the ledger, ordered by append order (AC "append order") - the
    /// same order [`EventLedger::append`] assigned them, independent of each event's own
    /// caller-supplied `recorded_at`.
    pub fn read_all(&self) -> Result<Vec<LedgerEvent>, LedgerError> {
        let mut stmt = self.conn.prepare(
            "SELECT event_id, kind, recorded_at, artifact_id, provider_id, category, \
                    policy_id, reason_code, plan_id, evidence_ids \
             FROM ledger_events ORDER BY event_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?;

        let mut result = Vec::new();
        for row in rows {
            let (
                event_id,
                kind,
                recorded_at,
                artifact_id,
                provider_id,
                category,
                policy_id,
                reason_code,
                plan_id,
                evidence_ids_json,
            ) = row?;
            let evidence_ids: Vec<EvidenceId> = serde_json::from_str(&evidence_ids_json)?;
            let mutation = plan_id.map(|plan_id| MutationReference {
                plan_id,
                evidence_ids,
            });
            result.push(LedgerEvent {
                event_id: EventId(event_id),
                kind: EventKind::from_key(&kind)?,
                recorded_at: u64::try_from(recorded_at).unwrap_or(0),
                metadata: EventMetadata {
                    artifact_id: artifact_id.map(ArtifactId::new),
                    provider_id,
                    category,
                    policy_id,
                    reason_code,
                },
                mutation,
            });
        }
        Ok(result)
    }

    /// Replaces every event in the inclusive `[from, to]` range with one signed/hashed
    /// [`CompactionSummary`], in a single transaction (this module's own doc, "Compaction
    /// preserves audit/aggregate semantics"). Refuses - leaving the ledger completely
    /// unchanged - when `from > to`, or when the range is not exactly and contiguously present
    /// (fewer than `to - from + 1` matching rows: already compacted, not yet appended, or
    /// otherwise incomplete), so a caller can never silently lose events that were never
    /// summarized anywhere.
    pub fn compact_range(
        &mut self,
        from: EventId,
        to: EventId,
        created_at: u64,
    ) -> Result<CompactionSummary, LedgerError> {
        if from.0 > to.0 {
            return Err(LedgerError(format!(
                "invalid compaction range: from ({}) is after to ({})",
                from.0, to.0
            )));
        }
        // `to.0 - from.0` alone can overflow `i64` at the extremes (e.g. `from =
        // i64::MIN`, `to = i64::MAX`) - checked arithmetic turns that into a rejection
        // rather than a panic (round 1 independent verifier review).
        let expected_count =
            to.0.checked_sub(from.0)
                .and_then(|span| span.checked_add(1))
                .ok_or_else(|| {
                    LedgerError(format!(
                        "compaction range [{}, {}] does not fit in a representable event count",
                        from.0, to.0
                    ))
                })?;
        let created_at_i64 = i64::try_from(created_at)
            .map_err(|_| LedgerError("created_at does not fit in a signed 64-bit column".into()))?;

        let tx = self.conn.transaction()?;

        let actual_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM ledger_events WHERE event_id BETWEEN ?1 AND ?2",
            params![from.0, to.0],
            |row| row.get(0),
        )?;
        if actual_count != expected_count {
            return Err(LedgerError(format!(
                "compaction range [{}, {}] is not exactly and contiguously present in the \
                 ledger (expected {expected_count} events, found {actual_count} - already \
                 compacted, not yet appended, or otherwise incomplete)",
                from.0, to.0
            )));
        }

        let mut hasher = Sha256::new();
        let mut kind_counts: std::collections::BTreeMap<String, u64> =
            std::collections::BTreeMap::new();
        {
            let mut stmt = tx.prepare(
                "SELECT event_id, kind, recorded_at, artifact_id, \
                        provider_id, category, policy_id, \
                        reason_code, plan_id, evidence_ids \
                 FROM ledger_events WHERE event_id BETWEEN ?1 AND ?2 ORDER BY event_id",
            )?;
            let rows = stmt.query_map(params![from.0, to.0], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                ))
            })?;
            for row in rows {
                let (
                    event_id,
                    kind,
                    recorded_at,
                    artifact_id,
                    provider_id,
                    category,
                    policy_id,
                    reason_code,
                    plan_id,
                    evidence_ids,
                ) = row?;
                *kind_counts.entry(kind.clone()).or_insert(0) += 1;
                // Length-prefixed, not delimiter-joined: a delimiter byte (even a rare one
                // like `\u{1}`) can appear inside a caller-supplied field, and two different
                // events can then concatenate to the same byte string across a field
                // boundary - round 1 independent verifier review reproduced exactly that
                // collision between `provider_id`/`category`. Prefixing every field with its
                // own length makes the encoding injective: the boundary is a byte count, not
                // a byte value a field's own content could also contain.
                hasher.update(event_id.to_be_bytes());
                hasher.update(recorded_at.to_be_bytes());
                hasher.update((kind.len() as u64).to_be_bytes());
                hasher.update(kind.as_bytes());
                // `artifact_id`..`plan_id` are nullable columns: absence (`NULL`) and an
                // empty string are semantically distinct `EventMetadata` values and round 2
                // independent verifier review reproduced them hashing identically once a
                // SQL-side `IFNULL(x,'')` erased the distinction before it ever reached this
                // loop. A one-byte presence tag ahead of the length prefix keeps `None`
                // (tag 0, no further bytes) and `Some("")` (tag 1, then length 0) apart.
                for field in [
                    &artifact_id,
                    &provider_id,
                    &category,
                    &policy_id,
                    &reason_code,
                    &plan_id,
                ] {
                    match field {
                        None => hasher.update([0u8]),
                        Some(value) => {
                            hasher.update([1u8]);
                            hasher.update((value.len() as u64).to_be_bytes());
                            hasher.update(value.as_bytes());
                        }
                    }
                }
                hasher.update((evidence_ids.len() as u64).to_be_bytes());
                hasher.update(evidence_ids.as_bytes());
            }
        }
        let digest_hex = encode_hex(&hasher.finalize());
        let kind_counts: Vec<(String, u64)> = kind_counts.into_iter().collect();
        let kind_counts_json = serde_json::to_string(&kind_counts)?;

        // The one and only window in which `ledger_events_forbid_delete` permits a delete.
        tx.execute(
            "UPDATE ledger_control SET compaction_in_progress = 1 WHERE id = 1",
            [],
        )?;
        tx.execute(
            "DELETE FROM ledger_events WHERE event_id BETWEEN ?1 AND ?2",
            params![from.0, to.0],
        )?;
        tx.execute(
            "UPDATE ledger_control SET compaction_in_progress = 0 WHERE id = 1",
            [],
        )?;

        tx.execute(
            "INSERT INTO ledger_compactions \
                (from_event_id, to_event_id, event_count, kind_counts, digest_hex, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                from.0,
                to.0,
                expected_count,
                kind_counts_json,
                digest_hex,
                created_at_i64,
            ],
        )?;
        let compaction_id = tx.last_insert_rowid();
        tx.commit()?;

        Ok(CompactionSummary {
            compaction_id,
            from_event_id: from,
            to_event_id: to,
            event_count: u64::try_from(expected_count).unwrap_or(0),
            kind_counts,
            digest_hex,
            created_at,
        })
    }

    /// Every compaction summary this ledger holds, ordered by `compaction_id` (creation order).
    pub fn compactions(&self) -> Result<Vec<CompactionSummary>, LedgerError> {
        let mut stmt = self.conn.prepare(
            "SELECT compaction_id, from_event_id, to_event_id, event_count, kind_counts, \
                    digest_hex, created_at \
             FROM ledger_compactions ORDER BY compaction_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (
                compaction_id,
                from_event_id,
                to_event_id,
                event_count,
                kind_counts_json,
                digest_hex,
                created_at,
            ) = row?;
            let kind_counts: Vec<(String, u64)> = serde_json::from_str(&kind_counts_json)?;
            result.push(CompactionSummary {
                compaction_id,
                from_event_id: EventId(from_event_id),
                to_event_id: EventId(to_event_id),
                event_count: u64::try_from(event_count).unwrap_or(0),
                kind_counts,
                digest_hex,
                created_at: u64::try_from(created_at).unwrap_or(0),
            });
        }
        Ok(result)
    }

    /// The oldest and newest [`EventId`] currently present in the ledger, and the total row
    /// count - `None` when the ledger holds no events. One query (`MIN`/`MAX`/`COUNT` together)
    /// rather than three separate reads, so a concurrent append or compaction between two
    /// queries cannot produce a bound that never actually held together at any single instant.
    /// `docs/architecture/PERSISTENCE_MODEL.md`'s "Self-budget" (SI-026);
    /// [`EventLedger::compact_oldest_to_fit`] is this method's own caller.
    pub fn event_id_bounds(&self) -> Result<Option<(EventId, EventId, u64)>, LedgerError> {
        let (min, max, count): (Option<i64>, Option<i64>, i64) = self.conn.query_row(
            "SELECT MIN(event_id), MAX(event_id), COUNT(*) FROM ledger_events",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        Ok(match (min, max) {
            (Some(min), Some(max)) => Some((
                EventId(min),
                EventId(max),
                u64::try_from(count).unwrap_or(0),
            )),
            _ => None,
        })
    }

    /// Compacts just enough of the oldest events to bring the ledger's row count down to at most
    /// `keep_at_most`, or does nothing (`Ok(None)`) if it is already at or under that count -
    /// `docs/architecture/PERSISTENCE_MODEL.md`'s "Self-budget" (SI-026): the "invoke the
    /// compaction that already exists before growth continues" side of self-budget enforcement,
    /// [`crate::budget::enforce_ledger_budget`]'s own implementation. Always targets the oldest
    /// contiguous prefix - the exact `[from, to]` [`EventLedger::compact_range`] this method
    /// itself computes from the ledger's current bounds, rather than asking a caller to name
    /// event ids directly. Fails - leaving the ledger completely unchanged, matching
    /// `compact_range`'s own atomicity - if the oldest `count - keep_at_most` events are not
    /// exactly and contiguously present (e.g. an earlier compaction of a non-prefix range left a
    /// gap in front of what remains): the same fail-closed behavior `compact_range` already gives
    /// any caller, never a silent partial compaction that leaves the ledger in a state worse than
    /// the budget overrun it was trying to fix.
    pub fn compact_oldest_to_fit(
        &mut self,
        keep_at_most: u64,
        now: u64,
    ) -> Result<Option<CompactionSummary>, LedgerError> {
        let Some((min_id, _max_id, count)) = self.event_id_bounds()? else {
            return Ok(None);
        };
        if count <= keep_at_most {
            return Ok(None);
        }
        let excess = count - keep_at_most;
        let excess_i64 = i64::try_from(excess).map_err(|_| {
            LedgerError(format!(
                "excess event count {excess} does not fit in a signed 64-bit range"
            ))
        })?;
        let to = EventId(min_id.0 + excess_i64 - 1);
        self.compact_range(min_id, to, now).map(Some)
    }

    /// Empties the ledger completely - every event and every compaction summary - and restores
    /// its schema to freshly migrated, all in one transaction (this crate's own
    /// "`reset --local-state`" primitive, `docs/architecture/PERSISTENCE_MODEL.md`'s
    /// "Self-budget", SI-026). Unlike [`EventLedger::compact_range`], which can never remove a
    /// `ledger_compactions` row (that table's own `BEFORE DELETE` trigger refuses
    /// unconditionally - "compaction summaries are immutable"), a reset is a deliberate,
    /// whole-store wipe a caller asked for by name, not a compaction - so it uses
    /// `DROP TABLE`/re-migrate (DDL, which the `BEFORE DELETE` triggers do not intercept, since
    /// they fire only on `DELETE` statements against a table that still exists) rather than
    /// `DELETE FROM` against the tables those triggers guard. The whole operation - drop,
    /// recreate, reseed `ledger_control` - runs in one transaction: a failure at any point (a
    /// concurrent writer holding the file lock, for instance) leaves the ledger exactly as it
    /// was, never half-dropped. `PRAGMA user_version` ends at the same value it started at (every
    /// migration in [`MIGRATIONS`] re-runs against the freshly empty schema), so a caller can
    /// keep using this same open connection immediately afterward - no re-open, no partial
    /// migration state to reconcile. Takes no path and no caller-supplied target: the only thing
    /// this method can ever act on is the connection it already owns (AC "`reset --local-state`
    /// cannot target provider roots" - this crate never touches a provider path at all, this
    /// module's own doc).
    pub fn reset(&mut self) -> Result<(), LedgerError> {
        let tx = self.conn.transaction()?;
        tx.execute_batch(
            "DROP TABLE IF EXISTS ledger_events;
             DROP TABLE IF EXISTS ledger_control;
             DROP TABLE IF EXISTS ledger_compactions;
             DROP TABLE IF EXISTS cancellai_ledger_identity;",
        )?;
        for migration in MIGRATIONS {
            tx.execute_batch(migration)?;
        }
        tx.execute_batch(&format!("PRAGMA user_version = {}", MIGRATIONS.len()))?;
        tx.commit()?;
        Ok(())
    }

    /// Test-only, crate-visible raw access to the underlying connection - used to attempt a
    /// bypass of the immutability guarantee directly against the database, independent of
    /// whatever public methods this type happens to expose today. Never part of the public
    /// API (this module's own doc, "Append-only, enforced by the database itself").
    #[cfg(test)]
    fn raw_conn(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discovered(recorded_at: u64) -> NewEvent {
        NewEvent {
            kind: EventKind::Discovered,
            recorded_at,
            metadata: EventMetadata {
                artifact_id: Some(ArtifactId::new("artifact-0001")),
                provider_id: Some("codex".to_string()),
                ..Default::default()
            },
            mutation: None,
        }
    }

    fn quarantined(recorded_at: u64, plan_id: &str) -> NewEvent {
        NewEvent {
            kind: EventKind::Quarantined,
            recorded_at,
            metadata: EventMetadata {
                artifact_id: Some(ArtifactId::new("artifact-0001")),
                ..Default::default()
            },
            mutation: Some(MutationReference {
                plan_id: plan_id.to_string(),
                evidence_ids: vec![EvidenceId::new("evidence-0001")],
            }),
        }
    }

    #[test]
    fn append_then_read_all_round_trips_every_field() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let id = ledger
            .append(quarantined(1_000, "plan-0001"))
            .expect("append");

        let events = ledger.read_all().expect("read_all");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, id);
        assert_eq!(events[0].kind, EventKind::Quarantined);
        assert_eq!(events[0].recorded_at, 1_000);
        assert_eq!(
            events[0].metadata.artifact_id,
            Some(ArtifactId::new("artifact-0001"))
        );
        assert_eq!(
            events[0].mutation,
            Some(MutationReference {
                plan_id: "plan-0001".to_string(),
                evidence_ids: vec![EvidenceId::new("evidence-0001")],
            })
        );
    }

    #[test]
    fn read_all_returns_events_in_append_order_not_in_recorded_at_order() {
        // Falsifier: append order must win even when a caller-supplied `recorded_at` is out
        // of order (a skewed or malicious clock must not be able to reorder the audit trail).
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let first = ledger.append(discovered(5_000)).expect("append first");
        let second = ledger.append(discovered(1_000)).expect("append second");
        let third = ledger.append(discovered(9_000)).expect("append third");

        let events = ledger.read_all().expect("read_all");
        assert_eq!(
            events.iter().map(|e| e.event_id).collect::<Vec<_>>(),
            vec![first, second, third],
            "read_all must return events in the order they were appended"
        );
    }

    #[test]
    fn append_rejects_a_mutation_event_with_no_mutation_reference_at_all() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let event = NewEvent {
            kind: EventKind::Quarantined,
            recorded_at: 1,
            metadata: EventMetadata::default(),
            mutation: None,
        };
        assert!(ledger.append(event).is_err());
        assert!(
            ledger.read_all().expect("read_all").is_empty(),
            "a rejected append must not leave a partial row behind"
        );
    }

    #[test]
    fn append_rejects_a_mutation_event_with_an_empty_plan_id() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let event = NewEvent {
            kind: EventKind::Archived,
            recorded_at: 1,
            metadata: EventMetadata::default(),
            mutation: Some(MutationReference {
                plan_id: "   ".to_string(),
                evidence_ids: vec![EvidenceId::new("evidence-0001")],
            }),
        };
        assert!(ledger.append(event).is_err());
    }

    #[test]
    fn append_rejects_a_mutation_event_with_no_evidence_ids() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let event = NewEvent {
            kind: EventKind::Purged,
            recorded_at: 1,
            metadata: EventMetadata::default(),
            mutation: Some(MutationReference {
                plan_id: "plan-0001".to_string(),
                evidence_ids: Vec::new(),
            }),
        };
        assert!(ledger.append(event).is_err());
    }

    #[test]
    fn append_accepts_a_pure_observation_event_with_no_mutation_reference() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        assert!(ledger.append(discovered(1)).is_ok());
    }

    #[test]
    fn raw_update_against_a_committed_event_is_rejected_by_the_database_itself() {
        // Immutability must hold even if a future change added a buggy public update method
        // that issued raw SQL - the database itself refuses it, not merely the absence of a
        // Rust method today.
        let mut ledger = EventLedger::open_in_memory().expect("open");
        ledger.append(discovered(1)).expect("append");

        let result = ledger.raw_conn().execute(
            "UPDATE ledger_events SET recorded_at = 999 WHERE event_id = 1",
            [],
        );
        assert!(
            result.is_err(),
            "a direct UPDATE against ledger_events must be rejected by the trigger"
        );
        let events = ledger.read_all().expect("read_all");
        assert_eq!(events[0].recorded_at, 1, "the row must be unchanged");
    }

    #[test]
    fn raw_delete_against_a_committed_event_is_rejected_outside_compaction() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        ledger.append(discovered(1)).expect("append");

        let result = ledger
            .raw_conn()
            .execute("DELETE FROM ledger_events WHERE event_id = 1", []);
        assert!(
            result.is_err(),
            "a direct DELETE against ledger_events must be rejected outside compact_range"
        );
        assert_eq!(ledger.read_all().expect("read_all").len(), 1);
    }

    #[test]
    fn raw_update_against_a_compaction_summary_is_rejected_by_the_database_itself() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        ledger.append(discovered(1)).expect("append");
        ledger
            .compact_range(EventId(1), EventId(1), 100)
            .expect("compact");

        let result = ledger.raw_conn().execute(
            "UPDATE ledger_compactions SET digest_hex = 'forged' WHERE compaction_id = 1",
            [],
        );
        assert!(result.is_err());
    }

    #[test]
    fn compact_range_replaces_the_events_with_one_summary_and_they_are_gone_from_read_all() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let first = ledger.append(discovered(1)).expect("append");
        let second = ledger.append(quarantined(2, "plan-0001")).expect("append");

        let summary = ledger
            .compact_range(first, second, 500)
            .expect("compact_range");
        assert_eq!(summary.event_count, 2);
        assert_eq!(summary.from_event_id, first);
        assert_eq!(summary.to_event_id, second);
        assert_eq!(
            summary.kind_counts,
            vec![
                ("DISCOVERED".to_string(), 1),
                ("QUARANTINED".to_string(), 1),
            ]
        );
        assert_eq!(summary.digest_hex.len(), 64, "SHA-256 hex is 64 characters");
        assert!(summary.digest_hex.bytes().all(|b| b.is_ascii_hexdigit()));

        assert!(
            ledger.read_all().expect("read_all").is_empty(),
            "compacted events must no longer appear in read_all"
        );
        assert_eq!(ledger.compactions().expect("compactions").len(), 1);
    }

    #[test]
    fn compact_range_is_atomic_leaving_events_and_compactions_untouched_on_failure() {
        // A failure partway (here: the range is not contiguously present) must not leave a
        // partial compaction behind - the same transactional-atomicity property
        // `crate::tests::apply_migrations_leaves_user_version_unchanged_when_a_later_migration_
        // fails` proves for schema migrations, exercised here for a mid-operation SQL failure.
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let first = ledger.append(discovered(1)).expect("append");
        // event_id 2 does not exist: the requested range [first, first+1] has a gap.
        let missing = EventId(first.0 + 1);

        let result = ledger.compact_range(first, missing, 100);
        assert!(result.is_err());
        assert_eq!(
            ledger.read_all().expect("read_all").len(),
            1,
            "a failed compaction must not delete any event"
        );
        assert!(
            ledger.compactions().expect("compactions").is_empty(),
            "a failed compaction must not write a summary"
        );
    }

    #[test]
    fn compact_range_rejects_a_range_with_a_gap_from_a_prior_compaction() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let a = ledger.append(discovered(1)).expect("append a");
        let b = ledger.append(discovered(2)).expect("append b");
        let c = ledger.append(discovered(3)).expect("append c");
        let d = ledger.append(discovered(4)).expect("append d");

        ledger.compact_range(a, b, 100).expect("first compaction");

        // [a, d] now has a gap where a/b used to be - must not silently compact only c/d
        // under a summary that claims to cover a..d.
        let result = ledger.compact_range(a, d, 200);
        assert!(result.is_err());
        assert_eq!(
            ledger.compactions().expect("compactions").len(),
            1,
            "the rejected attempt must not add a second, wrong summary"
        );

        // The still-present events are untouched and the correct narrower range still works.
        let events = ledger.read_all().expect("read_all");
        assert_eq!(
            events.iter().map(|e| e.event_id).collect::<Vec<_>>(),
            vec![c, d]
        );
        ledger
            .compact_range(c, d, 300)
            .expect("the exact remaining range must still compact");
    }

    #[test]
    fn compact_range_rejects_an_inverted_range() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let a = ledger.append(discovered(1)).expect("append");
        assert!(ledger.compact_range(EventId(a.0 + 1), a, 1).is_err());
    }

    #[test]
    fn compact_range_rejects_an_unrepresentable_span_instead_of_panicking() {
        // Round 1 independent verifier review: `to.0 - from.0` alone overflows `i64` at
        // these extremes. This must return `LedgerError`, never panic.
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let result = ledger.compact_range(EventId(i64::MIN), EventId(i64::MAX), 1);
        assert!(result.is_err());
    }

    #[test]
    fn compact_range_digest_does_not_collide_across_a_field_boundary() {
        // Round 1 independent verifier review: the prior delimiter-joined encoding hashed
        // `provider_id="a\u{1}b", category="c"` identically to `provider_id="a",
        // category="b\u{1}c"`, because the delimiter it relied on could itself appear inside
        // a caller-supplied field. The length-prefixed encoding must tell these apart.
        let mut left = EventLedger::open_in_memory().expect("open");
        let a = left
            .append(NewEvent {
                kind: EventKind::Discovered,
                recorded_at: 1,
                metadata: EventMetadata {
                    artifact_id: None,
                    provider_id: Some("a\u{1}b".to_string()),
                    category: Some("c".to_string()),
                    policy_id: None,
                    reason_code: None,
                },
                mutation: None,
            })
            .expect("append");
        let left_summary = left.compact_range(a, a, 100).expect("compact");

        let mut right = EventLedger::open_in_memory().expect("open");
        let b = right
            .append(NewEvent {
                kind: EventKind::Discovered,
                recorded_at: 1,
                metadata: EventMetadata {
                    artifact_id: None,
                    provider_id: Some("a".to_string()),
                    category: Some("b\u{1}c".to_string()),
                    policy_id: None,
                    reason_code: None,
                },
                mutation: None,
            })
            .expect("append");
        let right_summary = right.compact_range(b, b, 100).expect("compact");

        assert_ne!(
            left_summary.digest_hex, right_summary.digest_hex,
            "two events whose fields differ only in which side of a delimiter byte carries \
             it must not hash identically"
        );
    }

    #[test]
    fn compact_range_digest_distinguishes_absent_metadata_from_empty_metadata() {
        // Round 2 independent verifier review: the query wrapped every nullable metadata
        // column in `IFNULL(x,'')`, so `provider_id: None` and `provider_id: Some("")` both
        // reached the hasher as `""` and produced the same digest for two semantically
        // different committed event records. A presence tag ahead of the length prefix must
        // keep `None` and `Some("")` apart.
        let mut absent = EventLedger::open_in_memory().expect("open");
        let a = absent
            .append(NewEvent {
                kind: EventKind::Discovered,
                recorded_at: 1,
                metadata: EventMetadata {
                    artifact_id: None,
                    provider_id: None,
                    category: None,
                    policy_id: None,
                    reason_code: None,
                },
                mutation: None,
            })
            .expect("append");
        let absent_summary = absent.compact_range(a, a, 100).expect("compact");

        let mut empty = EventLedger::open_in_memory().expect("open");
        let b = empty
            .append(NewEvent {
                kind: EventKind::Discovered,
                recorded_at: 1,
                metadata: EventMetadata {
                    artifact_id: None,
                    provider_id: Some(String::new()),
                    category: None,
                    policy_id: None,
                    reason_code: None,
                },
                mutation: None,
            })
            .expect("append");
        let empty_summary = empty.compact_range(b, b, 100).expect("compact");

        assert_ne!(
            absent_summary.digest_hex, empty_summary.digest_hex,
            "a None metadata field and a Some(\"\") metadata field are different committed \
             event records and must not hash identically"
        );
    }

    #[test]
    fn append_after_reopen_preserves_prior_events_and_never_reuses_an_event_id() {
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-ledger-test-reopen-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let db_path = dir.join("ledger.sqlite3");

        let first_id = {
            let mut ledger = EventLedger::open(&db_path).expect("first open");
            let a = ledger.append(discovered(1)).expect("append a");
            ledger.append(discovered(2)).expect("append b");
            a
        };
        {
            let mut ledger = EventLedger::open(&db_path).expect("reopen after close");
            let events = ledger.read_all().expect("read_all after reopen");
            assert_eq!(events.len(), 2, "events must survive a close/reopen");
            let third = ledger.append(discovered(3)).expect("append after reopen");
            assert!(
                third.0 > first_id.0 + 1,
                "a new event id must never collide with or precede ids assigned before reopen"
            );
        }

        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    fn compaction_survives_a_reopen_and_stays_the_only_way_events_disappeared() {
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-ledger-test-compact-reopen-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let db_path = dir.join("ledger.sqlite3");

        let (from, to) = {
            let mut ledger = EventLedger::open(&db_path).expect("open");
            let a = ledger.append(discovered(1)).expect("append a");
            let b = ledger.append(discovered(2)).expect("append b");
            ledger.compact_range(a, b, 42).expect("compact");
            (a, b)
        };
        {
            let ledger = EventLedger::open(&db_path).expect("reopen");
            assert!(ledger.read_all().expect("read_all").is_empty());
            let compactions = ledger.compactions().expect("compactions");
            assert_eq!(compactions.len(), 1);
            assert_eq!(compactions[0].from_event_id, from);
            assert_eq!(compactions[0].to_event_id, to);
        }

        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    fn ledger_events_schema_has_only_the_allowlisted_columns() {
        // Pins the actual table shape against the closed metadata set this module's doc
        // promises ("Contentless by default") - a future change that adds e.g. a `content` or
        // `raw_path` column fails this test rather than drifting in silently.
        let ledger = EventLedger::open_in_memory().expect("open");
        let mut stmt = ledger
            .raw_conn()
            .prepare("PRAGMA table_info(ledger_events)")
            .expect("prepare");
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("collect");
        assert_eq!(
            columns,
            vec![
                "event_id",
                "kind",
                "recorded_at",
                "artifact_id",
                "provider_id",
                "category",
                "policy_id",
                "reason_code",
                "plan_id",
                "evidence_ids",
            ]
        );
    }

    #[test]
    fn malformed_stored_evidence_ids_is_reported_as_an_error_not_a_panic() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        ledger.append(quarantined(1, "plan-0001")).expect("append");
        ledger
            .raw_conn()
            .execute(
                "UPDATE ledger_control SET compaction_in_progress = 1 WHERE id = 1",
                [],
            )
            .expect("open the gate directly for this corruption test");
        // Corrupt via delete+reinsert since ledger_events forbids UPDATE unconditionally.
        ledger
            .raw_conn()
            .execute("DELETE FROM ledger_events WHERE event_id = 1", [])
            .expect("remove the row so it can be reinserted corrupted");
        ledger
            .raw_conn()
            .execute(
                "INSERT INTO ledger_events \
                    (event_id, kind, recorded_at, plan_id, evidence_ids) \
                 VALUES (1, 'QUARANTINED', 1, 'plan-0001', 'not valid json')",
                [],
            )
            .expect("reinsert with corrupted evidence_ids");
        ledger
            .raw_conn()
            .execute(
                "UPDATE ledger_control SET compaction_in_progress = 0 WHERE id = 1",
                [],
            )
            .expect("close the gate again");

        assert!(
            ledger.read_all().is_err(),
            "a corrupted evidence_ids column must surface as an error from read_all, not a panic"
        );
    }

    #[test]
    fn event_id_bounds_is_none_for_an_empty_ledger() {
        let ledger = EventLedger::open_in_memory().expect("open");
        assert_eq!(ledger.event_id_bounds().expect("event_id_bounds"), None);
    }

    #[test]
    fn event_id_bounds_reports_min_max_and_count() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let a = ledger.append(discovered(1)).expect("append a");
        ledger.append(discovered(2)).expect("append b");
        let c = ledger.append(discovered(3)).expect("append c");
        assert_eq!(
            ledger.event_id_bounds().expect("event_id_bounds"),
            Some((a, c, 3))
        );
    }

    #[test]
    fn compact_oldest_to_fit_does_not_compact_exactly_at_the_limit() {
        // Falsifier: budget exactly at the limit must not trigger compaction.
        let mut ledger = EventLedger::open_in_memory().expect("open");
        for i in 0..5 {
            ledger.append(discovered(i)).expect("append");
        }
        let result = ledger
            .compact_oldest_to_fit(5, 1_000)
            .expect("compact_oldest_to_fit");
        assert_eq!(result, None, "exactly at the limit must not compact");
        assert_eq!(ledger.read_all().expect("read_all").len(), 5);
        assert!(ledger.compactions().expect("compactions").is_empty());
    }

    #[test]
    fn compact_oldest_to_fit_compacts_exactly_the_excess_one_event_over_the_limit() {
        // Falsifier: one event over the limit must trigger compaction of exactly the excess,
        // never more, never a silent no-op that lets growth continue unchecked.
        let mut ledger = EventLedger::open_in_memory().expect("open");
        for i in 0..6 {
            ledger.append(discovered(i)).expect("append");
        }
        let summary = ledger
            .compact_oldest_to_fit(5, 1_000)
            .expect("compact_oldest_to_fit")
            .expect("one event over the limit must compact");
        assert_eq!(summary.event_count, 1, "must compact only the excess");
        assert_eq!(
            ledger
                .event_id_bounds()
                .expect("event_id_bounds")
                .unwrap()
                .2,
            5,
            "the ledger must end at exactly the configured limit, not below or above it"
        );
    }

    #[test]
    fn compact_oldest_to_fit_fails_closed_when_the_computed_prefix_has_a_gap() {
        // Falsifier: a compaction that fails partway (here: the computed range is not
        // contiguous, because an earlier non-prefix compaction already removed the middle of
        // it) must not leave the ledger worse off than the original budget overrun - nothing is
        // deleted, no summary is written.
        let mut ledger = EventLedger::open_in_memory().expect("open");
        ledger.append(discovered(0)).expect("append a");
        let b = ledger.append(discovered(1)).expect("append b");
        let c = ledger.append(discovered(2)).expect("append c");
        ledger.append(discovered(3)).expect("append d");
        ledger.append(discovered(4)).expect("append e");
        // Compact a middle range directly, leaving a gap in what would otherwise be the oldest
        // contiguous prefix.
        ledger
            .compact_range(b, c, 500)
            .expect("seed a non-prefix compaction to create a gap");

        let before = ledger.read_all().expect("read_all before");
        let result = ledger.compact_oldest_to_fit(1, 1_000);
        assert!(
            result.is_err(),
            "a non-contiguous computed prefix must be refused, not silently compacted partially"
        );
        assert_eq!(
            ledger.read_all().expect("read_all after"),
            before,
            "a refused compact_oldest_to_fit must leave every remaining event untouched"
        );
        assert_eq!(
            ledger.compactions().expect("compactions").len(),
            1,
            "only the seeded compaction must be recorded, never a second, wrong one"
        );
    }

    #[test]
    fn reset_empties_every_table_including_compaction_summaries() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let a = ledger.append(discovered(1)).expect("append a");
        ledger.append(discovered(2)).expect("append b");
        ledger.compact_range(a, a, 100).expect("compact one event");
        assert_eq!(ledger.read_all().expect("read_all").len(), 1);
        assert_eq!(ledger.compactions().expect("compactions").len(), 1);

        ledger.reset().expect("reset");

        assert!(
            ledger.read_all().expect("read_all after reset").is_empty(),
            "reset must remove every remaining event"
        );
        assert!(
            ledger
                .compactions()
                .expect("compactions after reset")
                .is_empty(),
            "reset must also remove compaction summaries, even though compact_range itself \
             can never touch ledger_compactions (its BEFORE DELETE trigger is unconditional) - \
             reset uses DDL, not DELETE, precisely to reach this table"
        );
    }

    #[test]
    fn reset_leaves_user_version_at_the_same_fully_migrated_value_and_the_ledger_reusable() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let version_before: i64 = ledger
            .raw_conn()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read user_version before reset");
        ledger.append(discovered(1)).expect("append before reset");

        ledger.reset().expect("reset");

        let version_after: i64 = ledger
            .raw_conn()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read user_version after reset");
        assert_eq!(
            version_before, version_after,
            "reset must leave user_version at the same fully-migrated value, not stuck at 0 \
             or requiring a fresh open() to become usable again"
        );

        // The store must be immediately reusable on the same open connection, with no re-open.
        let id = ledger.append(discovered(2)).expect("append after reset");
        assert_eq!(ledger.read_all().expect("read_all").len(), 1);
        assert_eq!(id.0, 1, "a fresh schema must not carry over old event ids");
    }

    #[test]
    fn reset_never_touches_a_provider_path() {
        // AC2/SI-026: reset takes no path at all - it can only ever act on the connection it
        // already owns. This proves that structurally by placing a real "provider artifact" file
        // next to the ledger's own database file and asserting reset leaves it completely alone.
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-ledger-test-reset-provider-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let provider_artifact_path = dir.join("provider-artifact.jsonl");
        std::fs::write(
            &provider_artifact_path,
            b"a real provider session transcript",
        )
        .expect("create the provider artifact");
        let db_path = dir.join("ledger.sqlite3");

        {
            let mut ledger = EventLedger::open(&db_path).expect("open");
            ledger.append(discovered(1)).expect("append");
            ledger.reset().expect("reset");
        }

        assert!(
            provider_artifact_path.exists(),
            "reset must never delete a provider artifact"
        );
        assert_eq!(
            std::fs::read_to_string(&provider_artifact_path).expect("read provider artifact"),
            "a real provider session transcript",
            "reset must never touch a provider artifact's content"
        );

        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    fn open_refuses_a_provider_owned_file_that_only_mimics_this_ledgers_schema() {
        // Self-review, round 2 pre-independent-round-3: the exact reproduction round 2 used
        // against `CurrentStateStore` applies identically here - a hand-crafted file matching
        // this module's own migrated schema and `user_version` was previously accepted by
        // `open` with no ownership check at all, and `.reset()` destroyed its row.
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-ledger-test-open-mimic-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let provider_db_path = dir.join("provider-owned.sqlite3");
        {
            let conn = Connection::open(&provider_db_path).expect("open raw sqlite file");
            conn.execute_batch(
                "CREATE TABLE ledger_events (
                    event_id INTEGER PRIMARY KEY AUTOINCREMENT,
                    kind TEXT NOT NULL,
                    recorded_at INTEGER NOT NULL,
                    artifact_id TEXT,
                    provider_id TEXT,
                    category TEXT,
                    policy_id TEXT,
                    reason_code TEXT,
                    plan_id TEXT,
                    evidence_ids TEXT NOT NULL DEFAULT '[]'
                ) STRICT;
                INSERT INTO ledger_events (kind, recorded_at, evidence_ids)
                    VALUES ('DISCOVERED', 1, '[]');
                PRAGMA user_version = 1;",
            )
            .expect("craft a provider-owned lookalike database");
        }

        let opened = EventLedger::open(&provider_db_path);
        assert!(
            opened.is_err(),
            "open must refuse a file that mimics this module's schema/user_version but was \
             never created by this module's own migrations"
        );

        let verify = Connection::open(&provider_db_path).expect("reopen raw sqlite file");
        let row_count: i64 = verify
            .query_row("SELECT COUNT(*) FROM ledger_events", [], |row| row.get(0))
            .expect("count provider rows");
        assert_eq!(
            row_count, 1,
            "a refused open must leave the provider-owned file completely untouched"
        );

        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    fn open_refuses_before_mutating_a_file_at_an_intermediate_user_version_with_no_marker() {
        // Self-review's own further finding: checking identity only after `apply_migrations`
        // ran let a non-owned file whose `user_version` sat between migrations receive a real
        // schema mutation before its `open` was refused. This module has only one migration
        // today, so the only "intermediate" version to falsify against is `user_version = 1`
        // with a schema that does not actually match migration 0's real output (missing the
        // identity marker table) - `open` must refuse it without running anything further.
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-ledger-test-open-no-mutate-before-refuse-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let db_path = dir.join("mimic.sqlite3");
        {
            let conn = Connection::open(&db_path).expect("open raw sqlite file");
            conn.execute_batch(
                "CREATE TABLE ledger_events (
                    event_id INTEGER PRIMARY KEY AUTOINCREMENT,
                    kind TEXT NOT NULL,
                    recorded_at INTEGER NOT NULL,
                    artifact_id TEXT,
                    provider_id TEXT,
                    category TEXT,
                    policy_id TEXT,
                    reason_code TEXT,
                    plan_id TEXT,
                    evidence_ids TEXT NOT NULL DEFAULT '[]'
                ) STRICT;
                PRAGMA user_version = 1;",
            )
            .expect("craft a file with no identity marker at user_version 1");
        }

        assert!(
            EventLedger::open(&db_path).is_err(),
            "open must refuse this file"
        );

        let verify = Connection::open(&db_path).expect("reopen raw sqlite file");
        let migration_ran: i64 = verify
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type = 'table' AND name IN \
                 ('cancellai_ledger_identity', 'ledger_control', 'ledger_compactions')",
                [],
                |row| row.get(0),
            )
            .expect("count tables migration 0 would have created");
        assert_eq!(
            migration_ran, 0,
            "a refused open must never run migration 0 (identity marker or any other table it \
             creates) against a file this module did not create"
        );

        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    fn reset_under_a_held_write_lock_fails_closed_without_corrupting_existing_data() {
        // Concurrency falsifier: a reset that cannot acquire the write lock (another connection
        // is mid-transaction) must fail cleanly - never panic, never partially apply.
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-ledger-test-reset-concurrent-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let db_path = dir.join("ledger.sqlite3");

        let mut ledger = EventLedger::open(&db_path).expect("open");
        ledger.append(discovered(1)).expect("append");

        // A second connection to the same file holds an exclusive write lock open.
        let blocker = Connection::open(&db_path).expect("open blocking connection");
        blocker
            .execute_batch("BEGIN IMMEDIATE;")
            .expect("hold a write lock via a second connection");

        let result = ledger.reset();
        assert!(
            result.is_err(),
            "reset must fail, not panic or block forever, while a competing writer holds the lock"
        );

        blocker
            .execute_batch("COMMIT;")
            .expect("release the blocking connection's lock");

        assert_eq!(
            ledger
                .read_all()
                .expect("read_all after failed reset")
                .len(),
            1,
            "a failed reset must leave the ledger's prior content completely unchanged"
        );

        // Windows refuses to delete a file with an open handle; both connections must close
        // before cleanup, matching every other test in this module.
        drop(ledger);
        drop(blocker);

        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }
}

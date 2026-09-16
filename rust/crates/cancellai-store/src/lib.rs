//! The current-state SQLite store (E13-S01, `docs/architecture/PERSISTENCE_MODEL.md`'s
//! "Layer 1: Current State"). Never the source of truth for provider state (C-10): disposable
//! and rebuildable, and never destructive truth for mutation preconditions (SI-024) - nothing
//! in this crate makes, or is consulted for, a mutation decision; it only stores and returns
//! [`cancellai_model::AgentArtifact`] rows a caller already produced by scanning.
//!
//! [`ledger`] (E13-S02) adds Layer 2, the append-only operational event ledger - a distinct
//! module with its own `EventLedger`/`Connection`/schema, described in its own module doc.
//!
//! [`rollup`] (E13-S03) adds Layer 3, analytical rollups and retention - a third, independent
//! module with its own `AnalyticalMemory`/`Connection`/schema, described in its own module doc.
//!
//! ## Why a bundled `rusqlite` (ADR-0019)
//!
//! `cancellai-store` is in ADR-0019's outer ring, which named `rusqlite` with a bundled SQLite
//! for exactly this story at planning time, subject to reconfirming the ring's criteria at
//! adoption: MIT-licensed (already in `rust/deny.toml`'s allow-list), it does not reach into
//! authority/identity/mutation decisions (this crate makes none), and this story is what it
//! replaces (nothing - the first real current-state persistence layer). `bundled` compiles
//! SQLite's own C source directly rather than linking whatever system SQLite version a host
//! happens to have, so behavior does not vary across macOS/Linux/Windows or a host's own
//! package state.
//!
//! ## Reconstructible by construction (C-10, AC1/AC2)
//!
//! This crate never touches a provider path - its only filesystem interaction is the SQLite
//! file at the path a caller supplies for cancellAI's own state directory. Deleting that file
//! is exactly `reset --local-state`'s own contract (`docs/architecture/PERSISTENCE_MODEL.md`):
//! it cannot delete a provider artifact, because no code path here ever names one.
//! [`CurrentStateStore::rebuild`] is the reconstruction primitive: given the same set of
//! `AgentArtifact` rows a scan produces, it replaces the entire table's contents in one
//! transaction, so the store's content after a rebuild depends only on what was just scanned,
//! never on what the store happened to contain before.
//!
//! ## Schema and migrations (AC3)
//!
//! Schema versioning uses SQLite's own `PRAGMA user_version` rather than a bookkeeping table -
//! one less thing to keep consistent with the database's actual shape. Each migration in
//! [`MIGRATIONS`] runs inside its own transaction that also advances `user_version`; SQLite's
//! DDL is itself transactional, so a migration that fails partway - a syntax error, a
//! constraint violation - rolls back everything it had already done, including any earlier
//! statement in the same migration string, and `user_version` is left exactly where it was.
//! [`tests::apply_migrations`] tests this directly against a synthetic, deliberately-broken
//! migration set, independent of the real, fixed [`MIGRATIONS`] this crate ships.
//!
//! Each row's full content is stored as one JSON column (`data`), round-tripped through
//! `AgentArtifact`'s own `Serialize`/`Deserialize` - the same wire format
//! `docs/architecture/JSON_CONTRACTS.md` already defines, not a second, parallel column-per-field
//! mapping this crate would otherwise have to keep in sync with every future `AgentArtifact`
//! change. `provider_id` and `activity_state` are pulled out as their own indexed columns for
//! the "fast queries" `docs/architecture/PERSISTENCE_MODEL.md`'s Layer 1 names as this store's
//! purpose - the two axes an actual caller (inventory summaries, lifecycle sweeps) needs to
//! filter by without deserializing every row. Widening this to more indexed columns, or a
//! fully normalized schema, is a follow-up story's job once a real caller needs a query this
//! shape cannot answer efficiently; nothing here forecloses that.

use cancellai_model::{AgentArtifact, ArtifactId};
use rusqlite::Connection;
use std::path::Path;

/// The append-only operational event ledger (E13-S02, "Layer 2: Operational Event Ledger").
/// A separate module and a separate `EventLedger`/`Connection` from this file's own
/// `CurrentStateStore` - see [`ledger`]'s own module doc for why they stay independent
/// despite sharing this crate and its `rusqlite` dependency.
pub mod ledger;

/// Analytical rollups and retention (E13-S03) - Layer 3 of `docs/architecture/PERSISTENCE_MODEL.md`.
/// A third module and a third independent `AnalyticalMemory`/`Connection` from this file's own
/// `CurrentStateStore` and [`ledger`]'s `EventLedger` - see [`rollup`]'s own module doc for why
/// each layer keeps its own schema, file and retention shape despite sharing this crate.
pub mod rollup;

/// Self-budget enforcement and local-state reset (E13-S04) - `docs/architecture/
/// PERSISTENCE_MODEL.md`'s "Self-budget", SI-026. A fourth module that adds no schema/file/
/// connection of its own: it is a thin policy layer over the compaction/reset primitives
/// [`CurrentStateStore`], [`ledger::EventLedger`] and [`rollup::AnalyticalMemory`] already
/// expose, deciding *when* to compact/reset, never *how* - see [`budget`]'s own module doc.
pub mod budget;

/// Why a [`CurrentStateStore`] operation failed. Always the underlying SQLite error or a
/// stored row's own content failing to round-trip as JSON - this crate does not otherwise
/// interpret or classify failures.
#[derive(Debug)]
pub struct StoreError(String);

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self(e.to_string())
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        Self(format!(
            "stored row content did not round-trip as JSON: {e}"
        ))
    }
}

/// The ordered schema history. Index `n` (zero-based) is the migration that takes the database
/// from `user_version = n` to `user_version = n + 1` - see this module's own doc, "Schema and
/// migrations", for why a failure partway through any one of these leaves `user_version`
/// exactly where it started rather than at a half-applied state.
const MIGRATIONS: &[&str] = &["
    CREATE TABLE agent_artifacts (
        artifact_id TEXT PRIMARY KEY,
        provider_id TEXT NOT NULL,
        activity_state TEXT NOT NULL,
        data TEXT NOT NULL
    ) STRICT;
    CREATE INDEX idx_agent_artifacts_provider_id ON agent_artifacts(provider_id);
    CREATE INDEX idx_agent_artifacts_activity_state ON agent_artifacts(activity_state);
"];

/// Applies every migration in `migrations` the database has not already applied, reading and
/// advancing `PRAGMA user_version` one migration at a time. Free-standing (not a method) so
/// this crate's own tests can exercise it directly against a synthetic migration set, never
/// only indirectly through [`CurrentStateStore::open`]'s fixed, real one.
fn apply_migrations(conn: &Connection, migrations: &[&str]) -> Result<(), StoreError> {
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

/// The exact string [`ActivityState`](cancellai_model::ActivityState) is indexed under - an
/// explicit, exhaustive mapping (a future variant this crate does not yet handle fails to
/// compile) rather than reusing the JSON wire encoding, so this purely-internal index column's
/// shape stays independent of the wire format's own evolution.
fn activity_state_key(state: &cancellai_model::ActivityState) -> &'static str {
    use cancellai_model::ActivityState;
    match state {
        ActivityState::Active => "active",
        ActivityState::Idle => "idle",
        ActivityState::Stale => "stale",
        ActivityState::Orphaned => "orphaned",
        ActivityState::Unknown => "unknown",
    }
}

/// The current-state SQLite store: a reconstructible cache/index over the
/// [`AgentArtifact`]s a scan already produced (this module's own doc). Holds one open
/// connection for its lifetime, matching `rusqlite::Connection`'s own single-owner design.
pub struct CurrentStateStore {
    conn: Connection,
}

impl CurrentStateStore {
    /// Opens (creating if absent) the current-state database at `path`, applying any migration
    /// this database has not already seen.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        apply_migrations(&conn, MIGRATIONS)?;
        Ok(Self { conn })
    }

    /// An in-memory store for tests and short-lived callers that never need a file on disk.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        apply_migrations(&conn, MIGRATIONS)?;
        Ok(Self { conn })
    }

    /// Replaces every row this store holds with exactly `artifacts`, in one transaction - the
    /// reconstruction primitive this module's own doc describes ("Reconstructible by
    /// construction"). The store's content after this call depends only on `artifacts`, never
    /// on whatever the store held before it (including nothing, on a freshly created database).
    pub fn rebuild(&mut self, artifacts: &[AgentArtifact]) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM agent_artifacts", [])?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO agent_artifacts (artifact_id, provider_id, activity_state, data) \
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for artifact in artifacts {
                let data = serde_json::to_string(artifact)?;
                insert.execute(rusqlite::params![
                    artifact.artifact_id.0,
                    artifact.provider_id,
                    activity_state_key(&artifact.activity_state),
                    data,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Every artifact this store currently holds, ordered by [`ArtifactId`] for a deterministic
    /// read - callers that need a different order sort what they get back.
    pub fn all(&self) -> Result<Vec<AgentArtifact>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data FROM agent_artifacts ORDER BY artifact_id")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(serde_json::from_str(&row?)?);
        }
        Ok(result)
    }

    /// One artifact by its [`ArtifactId`], or `None` if this store holds no row for it.
    pub fn get(&self, id: &ArtifactId) -> Result<Option<AgentArtifact>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data FROM agent_artifacts WHERE artifact_id = ?1")?;
        let mut rows = stmt.query(rusqlite::params![id.0])?;
        match rows.next()? {
            Some(row) => Ok(Some(serde_json::from_str(&row.get::<_, String>(0)?)?)),
            None => Ok(None),
        }
    }

    /// The number of artifacts this store currently holds - `docs/architecture/
    /// PERSISTENCE_MODEL.md`'s "Self-budget" (SI-026), an observation [`crate::budget`] uses.
    /// Layer 1 has no compaction primitive of its own to pair this with (this module's own doc,
    /// "Reconstructible by construction": its content is entirely determined by the last
    /// `rebuild` a caller performed, so this crate autonomously discarding rows here would
    /// silently diverge from the last scan rather than compact anything) - `docs/architecture/
    /// PERSISTENCE_MODEL.md`'s own "Safety-critical current facts may force analytical sampling
    /// to degrade rather than exceed the budget" names Layer 3 sampling, not Layer 1 itself, as
    /// what yields under Layer 1 budget pressure.
    pub fn row_count(&self) -> Result<u64, StoreError> {
        let count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM agent_artifacts", [], |row| row.get(0))?;
        Ok(u64::try_from(count).unwrap_or(0))
    }

    /// Empties this store completely - this crate's own "`reset --local-state`" primitive for
    /// Layer 1 (`docs/architecture/PERSISTENCE_MODEL.md`'s "Self-budget", SI-026). Exactly
    /// [`CurrentStateStore::rebuild`] given an empty slice: the same one transaction, `DELETE
    /// FROM agent_artifacts`, this store's content depending only on what it is given (nothing).
    /// Takes no path and no caller-supplied target - the only thing this method can ever act on
    /// is the connection it already owns, matching this module's own doc, "Reconstructible by
    /// construction."
    pub fn reset(&mut self) -> Result<(), StoreError> {
        self.rebuild(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_model::{
        ActivitySignal, ActivityState, ArtifactRelationship, AttributionSource, EvidenceId,
        IntegrityState, KnowledgeConfidence, ProjectAttribution, ProjectRef, ProtectionState,
        RelationshipKind, ResidencyState, Reversibility, RiskClass,
    };

    fn artifact(id: &str, provider_id: &str, activity_state: ActivityState) -> AgentArtifact {
        AgentArtifact {
            artifact_id: ArtifactId::new(id),
            identity_token: format!("{provider_id}:{id}"),
            provider_id: provider_id.to_string(),
            artifact_type: "session".to_string(),
            risk_class: RiskClass::R3Resumable,
            reversibility: Reversibility::Irreversible,
            knowledge_confidence: KnowledgeConfidence::Verified,
            activity_state,
            residency_state: ResidencyState::Hot,
            protection_state: ProtectionState::Normal,
            integrity_state: IntegrityState::Healthy,
            authority_ceiling: cancellai_model::AuthorityLevel::Quarantine,
            evidence_ids: vec![EvidenceId::new("evidence-0001")],
            relationships: Vec::new(),
            project_attribution: None,
            activity_signal: None,
        }
    }

    /// The fully-populated corner every optional/collection field can carry, so a round-trip
    /// test cannot pass merely because the sparse fields were never exercised.
    fn fully_populated_artifact() -> AgentArtifact {
        let mut a = artifact("artifact-full", "codex", ActivityState::Orphaned);
        a.relationships.push(ArtifactRelationship {
            kind: RelationshipKind::ChildOf,
            related_artifact_id: ArtifactId::new("artifact-root"),
        });
        a.project_attribution = Some(ProjectAttribution {
            project_ref: ProjectRef::new("-Users-example-project"),
            source: AttributionSource::ExplicitProviderMetadata,
            confidence: KnowledgeConfidence::Verified,
        });
        a.activity_signal = Some(ActivitySignal {
            evidence_ids: vec![EvidenceId::new("evidence-0002")],
            explanation: "parent session id was not discovered in this scan".to_string(),
        });
        a
    }

    #[test]
    fn rebuild_then_all_round_trips_every_field_exactly() {
        let mut store = CurrentStateStore::open_in_memory().expect("open");
        let artifacts = vec![
            artifact("artifact-0001", "codex", ActivityState::Active),
            fully_populated_artifact(),
        ];
        store.rebuild(&artifacts).expect("rebuild");

        let mut round_tripped = store.all().expect("all");
        round_tripped.sort_by(|a, b| a.artifact_id.cmp(&b.artifact_id));
        let mut expected = artifacts;
        expected.sort_by(|a, b| a.artifact_id.cmp(&b.artifact_id));
        assert_eq!(round_tripped, expected);
    }

    #[test]
    fn rebuild_replaces_the_entire_previous_content_rather_than_accumulating() {
        let mut store = CurrentStateStore::open_in_memory().expect("open");
        store
            .rebuild(&[artifact("artifact-old", "codex", ActivityState::Active)])
            .expect("first rebuild");
        store
            .rebuild(&[artifact("artifact-new", "claude", ActivityState::Idle)])
            .expect("second rebuild");

        let all = store.all().expect("all");
        assert_eq!(
            all.iter()
                .map(|a| a.artifact_id.0.as_str())
                .collect::<Vec<_>>(),
            vec!["artifact-new"],
            "a rebuild must replace the table's content, not add to it"
        );
    }

    #[test]
    fn rebuild_to_empty_leaves_no_rows_behind() {
        let mut store = CurrentStateStore::open_in_memory().expect("open");
        store
            .rebuild(&[artifact("artifact-0001", "codex", ActivityState::Active)])
            .expect("first rebuild");
        store.rebuild(&[]).expect("rebuild to empty");
        assert!(store.all().expect("all").is_empty());
    }

    #[test]
    fn get_finds_exactly_the_matching_artifact_and_nothing_for_an_absent_id() {
        let mut store = CurrentStateStore::open_in_memory().expect("open");
        store
            .rebuild(&[artifact("artifact-0001", "codex", ActivityState::Active)])
            .expect("rebuild");

        assert_eq!(
            store.get(&ArtifactId::new("artifact-0001")).expect("get"),
            Some(artifact("artifact-0001", "codex", ActivityState::Active))
        );
        assert_eq!(
            store
                .get(&ArtifactId::new("artifact-missing"))
                .expect("get"),
            None
        );
    }

    #[test]
    fn deleting_the_database_file_never_touches_a_provider_artifact() {
        // AC1, C-10: the falsification this test is built to catch is a future change that
        // makes this crate touch a real provider path - it must not exist for this test to
        // keep passing, not merely "happen to" as a side effect of today's implementation.
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-test-provider-artifact-{}-{}",
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
        let db_path = dir.join("current-state.sqlite3");

        {
            let mut store = CurrentStateStore::open(&db_path).expect("open");
            store
                .rebuild(&[artifact("artifact-0001", "codex", ActivityState::Active)])
                .expect("rebuild");
        }
        assert!(db_path.exists(), "sanity: the database file was created");

        std::fs::remove_file(&db_path).expect("delete the current-state database");

        assert!(
            provider_artifact_path.exists(),
            "deleting the current-state database must never delete a provider artifact"
        );
        assert_eq!(
            std::fs::read_to_string(&provider_artifact_path).expect("read provider artifact"),
            "a real provider session transcript",
            "the provider artifact's content must be completely untouched"
        );

        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    fn apply_migrations_leaves_user_version_unchanged_when_a_later_migration_fails() {
        // AC3: a migration's own transaction covers every statement in it, so a failure
        // partway through rolls back everything that migration had already done, and
        // `user_version` never advances past the last migration that fully succeeded.
        let conn = Connection::open_in_memory().expect("open");
        let migrations = [
            "CREATE TABLE first_migration_table (id INTEGER PRIMARY KEY);",
            "CREATE TABLE second_migration_table (id INTEGER PRIMARY KEY); \
             THIS IS NOT VALID SQL;",
        ];

        let result = apply_migrations(&conn, &migrations);
        assert!(
            result.is_err(),
            "an invalid migration must be reported, not silently swallowed"
        );

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read user_version");
        assert_eq!(
            version, 1,
            "user_version must stay at the last migration that fully committed"
        );

        let table_exists = |name: &str| {
            conn.query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                rusqlite::params![name],
                |_| Ok(()),
            )
            .is_ok()
        };
        assert!(
            table_exists("first_migration_table"),
            "the migration that fully committed must not be rolled back by a later failure"
        );
        assert!(
            !table_exists("second_migration_table"),
            "a migration that fails partway must roll back everything it had already done, \
             not leave a half-applied schema behind"
        );
    }

    #[test]
    fn apply_migrations_is_a_no_op_when_every_migration_was_already_applied() {
        let conn = Connection::open_in_memory().expect("open");
        let migrations = ["CREATE TABLE t (id INTEGER PRIMARY KEY);"];
        apply_migrations(&conn, &migrations).expect("first apply");
        // Re-applying the same fixed list must not try to re-run a migration whose table
        // already exists (which would itself fail, `CREATE TABLE` not being idempotent) -
        // this is exactly what makes `CurrentStateStore::open` safe to call again on an
        // already-migrated database.
        apply_migrations(&conn, &migrations).expect("second apply must be a no-op");
    }

    #[test]
    fn open_creates_the_real_schema_and_is_idempotent_across_repeated_opens() {
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-test-reopen-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let db_path = dir.join("current-state.sqlite3");

        {
            let mut store = CurrentStateStore::open(&db_path).expect("first open");
            store
                .rebuild(&[artifact("artifact-0001", "codex", ActivityState::Active)])
                .expect("rebuild");
        }
        {
            let store = CurrentStateStore::open(&db_path).expect("second open must not fail");
            assert_eq!(
                store.all().expect("all").len(),
                1,
                "reopening an already-migrated database must preserve its content"
            );
        }

        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    fn malformed_stored_content_is_reported_as_an_error_not_a_panic() {
        // Fail-closed axis: a row's `data` column could in principle be corrupted (a hand
        // edit, a partial write from an unrelated tool) - reading it back must never panic,
        // and must never silently substitute a default value for content that does not
        // actually round-trip.
        let mut store = CurrentStateStore::open_in_memory().expect("open");
        store
            .rebuild(&[artifact("artifact-0001", "codex", ActivityState::Active)])
            .expect("rebuild");
        store
            .conn
            .execute(
                "UPDATE agent_artifacts SET data = ?1 WHERE artifact_id = ?2",
                rusqlite::params!["not valid json", "artifact-0001"],
            )
            .expect("corrupt the stored row directly");

        assert!(
            store.all().is_err(),
            "a corrupted row must surface as an error from all()"
        );
        assert!(
            store.get(&ArtifactId::new("artifact-0001")).is_err(),
            "a corrupted row must surface as an error from get()"
        );
    }

    #[test]
    fn row_count_reports_the_current_row_count() {
        let mut store = CurrentStateStore::open_in_memory().expect("open");
        assert_eq!(store.row_count().expect("row_count"), 0);
        store
            .rebuild(&[
                artifact("artifact-0001", "codex", ActivityState::Active),
                artifact("artifact-0002", "claude", ActivityState::Idle),
            ])
            .expect("rebuild");
        assert_eq!(store.row_count().expect("row_count"), 2);
    }

    #[test]
    fn reset_empties_the_store_and_is_equivalent_to_rebuild_with_no_artifacts() {
        let mut store = CurrentStateStore::open_in_memory().expect("open");
        store
            .rebuild(&[artifact("artifact-0001", "codex", ActivityState::Active)])
            .expect("rebuild");
        assert_eq!(store.row_count().expect("row_count"), 1);

        store.reset().expect("reset");

        assert_eq!(store.row_count().expect("row_count"), 0);
        assert!(store.all().expect("all").is_empty());
    }

    #[test]
    fn reset_never_touches_a_provider_path() {
        // AC2/SI-026: reset takes no path at all - the same falsifier
        // `deleting_the_database_file_never_touches_a_provider_artifact` already proves for a
        // raw filesystem delete, exercised here against `reset` itself.
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-test-reset-provider-{}-{}",
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
        let db_path = dir.join("current-state.sqlite3");

        {
            let mut store = CurrentStateStore::open(&db_path).expect("open");
            store
                .rebuild(&[artifact("artifact-0001", "codex", ActivityState::Active)])
                .expect("rebuild");
            store.reset().expect("reset");
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
}

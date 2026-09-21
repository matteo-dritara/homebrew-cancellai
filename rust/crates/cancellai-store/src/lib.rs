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
//!
//! ## Incremental reuse (E13-S05, SI-024: "persistent cache is never destructive truth")
//!
//! [`CacheInvalidationKey`] is a small, caller-supplied bundle of primitive invalidation
//! evidence (identity, mtime, provider fingerprint, knowledge version, completeness) that a
//! caller may attach to an already-`rebuild`-written row via [`CurrentStateStore::set_invalidation_key`],
//! and later compare a *fresh* observation of the same axes against via
//! [`CurrentStateStore::cache_read_hint`]. This crate deliberately does not depend on
//! `cancellai-inventory` for this - `FileFacts`/`ScopeCompleteness` are that crate's own
//! scan-shaped vocabulary, and Layer 1 is documented as a generic reconstructible cache/index,
//! not specific to one scanner's output shape (this module's own doc, above). `CacheCompleteness`
//! is a small, local, three-value echo of the same complete/partial/unknown concept a caller
//! (`cancellai-inventory::completeness::ScopeCompleteness` today) already classifies scans by -
//! a caller maps its own richer type down to this one, not the reverse.
//!
//! **`cache_read_hint` returns [`CacheReadHint`], never a `bool` and never anything named or
//! shaped like an authorization.** `ReuseForReading` means exactly what its name says: this
//! cached row is worth reading as a fast, non-authoritative shortcut (a UI list, a preliminary
//! plan) - never a substitute for the fresh, execution-time observation
//! `cancellai-safety::mutation_executor` performs immediately before any mutation. Nothing in
//! this module calls, references, or re-exports the safety crate's own mutation-execution
//! capability (the two symbols `scripts/check_mutation_boundary.py`, SI-019, scans every crate's
//! production code for - deliberately not spelled out literally in this doc comment, which is
//! itself production code that script scans) - `cancellai-store`'s own `Cargo.toml` does not even
//! depend on `cancellai-safety`, so no such call could compile here even by accident
//! (`tests::cargo_toml_declares_no_dependency_on_cancellai_safety` pins this directly). Two rows
//! whose `set_invalidation_key` was never called - including every row written by a plain
//! `rebuild` alone - compare as `Revalidate` by construction: the persisted
//! `cache_identity_token` column stays `NULL` until a caller explicitly sets it, and `NULL`
//! there always means "no invalidation key on record," never "matches whatever the caller now
//! asks about." A crash between `rebuild` and every intended `set_invalidation_key` call
//! therefore fails safe - the affected rows are simply never reusable, not silently treated as
//! fresh.
//!
//! A row is `ReuseForReading` only when **every** axis matches exactly: `identity_token` equal,
//! `modified` equal (an `Option<u64>` compared for exact equality - a backward-moved mtime is a
//! *change*, like a forward one, never treated as "no change"; clock skew is never safe by
//! assumption, matching this repository's own "ambiguity never escalates privilege"), the same
//! `provider_fingerprint` and `knowledge_version`, and **both** the persisted and the fresh
//! `completeness` equal to `CacheCompleteness::Complete`. A row persisted under `Partial`/
//! `Unknown` evidence is never `ReuseForReading`, even against an identical fresh `Partial`/
//! `Unknown` observation - it is never treated as more reliable than it was when written, which
//! this construction makes the only reliable value `Complete` can ever compare equal to.
//! `cancellai-store` provides only this primitive; wiring a real caller (deciding whether to
//! re-invoke `cancellai-inventory` for a given artifact) is a later story's orchestration, not
//! this one's (matching E13-S01 through E13-S04's own "primitive delivered, no orchestrator yet"
//! precedent).
//!
//! [`CurrentStateStore::set_invalidation_key`] itself refuses (`Err`) a key whose
//! `identity_token` does not match the persisted row's own `AgentArtifact::identity_token` -
//! `ArtifactId` is a stable key across scans, but a row's content can legitimately belong to a
//! different `identity_token` after a `rebuild`, and a key is only ever a claim about the content
//! actually stored, never about whatever `ArtifactId` it happens to be filed under.

use cancellai_model::{AgentArtifact, ArtifactId};
use rusqlite::{Connection, OptionalExtension};
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

/// Purge tombstones (E12-S04) - `docs/architecture/PERSISTENCE_MODEL.md`'s "Tombstones",
/// SI-020. Adds no schema of its own: a typed, narrower front door onto [`ledger::EventLedger`]'s
/// existing `Purged` event kind - see [`tombstone`]'s own module doc.
pub mod tombstone;

/// cancellAI's own local-state root capability (E13-S06) - the one thing every layer's
/// production `open()` derives its database path from, closing E13-S04's round-3 marker-mimicry
/// finding. See [`local_state_root`]'s own module doc.
mod local_state_root;
pub use local_state_root::{LocalStateRoot, LocalStateRootError};

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

impl From<LocalStateRootError> for StoreError {
    fn from(e: LocalStateRootError) -> Self {
        Self(e.to_string())
    }
}

/// The ordered schema history. Index `n` (zero-based) is the migration that takes the database
/// from `user_version = n` to `user_version = n + 1` - see this module's own doc, "Schema and
/// migrations", for why a failure partway through any one of these leaves `user_version`
/// exactly where it started rather than at a half-applied state.
const MIGRATIONS: &[&str] = &[
    "
    CREATE TABLE agent_artifacts (
        artifact_id TEXT PRIMARY KEY,
        provider_id TEXT NOT NULL,
        activity_state TEXT NOT NULL,
        data TEXT NOT NULL
    ) STRICT;
    CREATE INDEX idx_agent_artifacts_provider_id ON agent_artifacts(provider_id);
    CREATE INDEX idx_agent_artifacts_activity_state ON agent_artifacts(activity_state);
    CREATE TABLE cancellai_store_identity (marker TEXT PRIMARY KEY) STRICT;
    INSERT INTO cancellai_store_identity (marker) VALUES ('cancellai-current-state-store-v1');
",
    // E13-S05: incremental-reuse invalidation key, stored alongside each row rather than in a
    // second table - a plain `DELETE FROM agent_artifacts` (every `rebuild`, including
    // `reset`'s empty one) already wipes these columns for every row, so a rebuilt row can
    // never carry a stale invalidation key left over from a previous scan by construction, not
    // by a second cleanup step this migration would otherwise have to keep in sync. Every
    // column is nullable with no default, so an `INSERT` that does not name them (`rebuild`'s
    // own, unchanged since E13-S01) leaves them `NULL` - "never had an invalidation key set" -
    // this module's own doc, "Incremental reuse", relies on exactly that default.
    "
    ALTER TABLE agent_artifacts ADD COLUMN cache_identity_token TEXT;
    ALTER TABLE agent_artifacts ADD COLUMN cache_modified INTEGER;
    ALTER TABLE agent_artifacts ADD COLUMN cache_provider_fingerprint TEXT;
    ALTER TABLE agent_artifacts ADD COLUMN cache_knowledge_version TEXT;
    ALTER TABLE agent_artifacts ADD COLUMN cache_completeness TEXT;
",
];

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

/// The value [`MIGRATIONS`]' first migration inserts into `cancellai_store_identity`.
const STORE_IDENTITY_MARKER: &str = "cancellai-current-state-store-v1";

/// The fixed filename [`CurrentStateStore::open`]'s production entry point joins onto a
/// [`LocalStateRoot`] - never a caller-suppliable path (E13-S06, this crate's own
/// [`local_state_root`] module doc).
const CURRENT_STATE_FILENAME: &str = "current_state.sqlite3";

/// Refuses to treat `conn` as a valid current-state store unless it carries this crate's own
/// identity marker (SI-026, "reset --local-state cannot target provider roots"). `PRAGMA
/// user_version` and even an `agent_artifacts`-shaped schema are not proof of ownership - round 2
/// independent verifier review opened a hand-crafted provider-owned SQLite file with a matching
/// `user_version` and table shape through [`CurrentStateStore::open`] and reset it, deleting rows
/// that were never this crate's to delete. A marker table only this crate's own migration ever
/// creates is not something a provider database happens to also carry, so its absence (a file
/// [`apply_migrations`] treated as already-migrated because a caller-supplied `user_version`
/// matched, but which never actually ran migration 0) fails the open closed rather than silently
/// granting a reset-capable handle over content this crate did not create.
///
/// The marker lives inside migration 0's own SQL rather than a new, later migration appended to
/// [`MIGRATIONS`]: [`apply_migrations`] runs every migration whose index is at or past a file's
/// current `user_version`, regardless of whether that file's earlier history is genuine, so a
/// later "add the marker" migration would apply itself just as readily to the same hand-crafted
/// `user_version = 2` mimicry file this check exists to refuse - it would not close the gap, it
/// would auto-grant the marker to it. Requiring migration 0 itself to have run for real is the
/// only version of this check a caller cannot satisfy by guessing a `user_version` number. This
/// crate has shipped in no release yet (E06-S04's canonical-engine-switch gate is still open), so
/// editing migration 0's own content carries no live-database compatibility obligation today;
/// once real on-disk stores exist, adding a marker retroactively would need its own migration
/// story, not a silent edit to history.
fn verify_store_identity(conn: &Connection) -> Result<(), StoreError> {
    let marker: Result<String, rusqlite::Error> =
        conn.query_row("SELECT marker FROM cancellai_store_identity", [], |row| {
            row.get(0)
        });
    match marker {
        Ok(value) if value == STORE_IDENTITY_MARKER => Ok(()),
        _ => Err(StoreError(
            "this database does not carry cancellai-store's own identity marker - refusing to \
             open it as a current-state store rather than risk treating provider-owned or \
             unrelated content as this crate's own"
                .into(),
        )),
    }
}

/// Refuses (`Err`) a file whose `PRAGMA user_version` is already past the first migration but
/// which does not carry [`STORE_IDENTITY_MARKER`] - checked *before* [`apply_migrations`] runs
/// anything further, so a file that turns out not to be this crate's own is never mutated on the
/// way to being refused. Self-review found that checking identity only *after* migrations ran
/// let a non-owned file at an intermediate `user_version` receive a real schema mutation (an
/// `ALTER TABLE`) before its `open` was refused - a genuinely fresh file (`user_version = 0`)
/// has not run migration 0 yet, so it cannot carry the marker yet either; that is the ordinary
/// creation path, not a rejection.
fn verify_store_identity_before_migrating(conn: &Connection) -> Result<(), StoreError> {
    let current_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if current_version > 0 {
        verify_store_identity(conn)?;
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

/// How complete the evidence behind a [`CacheInvalidationKey`] was, echoing (without depending
/// on) `cancellai-inventory::completeness::ScopeCompleteness`'s complete/partial/unknown
/// vocabulary at the primitive level this crate's own dependency ring admits (this module's own
/// doc, "Incremental reuse"). A caller collapses its own richer completeness type (with reasons)
/// down to one of these three values; this crate never inspects *why* evidence was partial, only
/// *whether* it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheCompleteness {
    Complete,
    Partial,
    Unknown,
}

/// The exact string [`CacheCompleteness`] is stored under - the same explicit, exhaustive
/// mapping convention [`activity_state_key`] already uses for the same reason: a future variant
/// this crate does not yet handle fails to compile rather than silently storing an empty string.
fn cache_completeness_key(completeness: &CacheCompleteness) -> &'static str {
    match completeness {
        CacheCompleteness::Complete => "complete",
        CacheCompleteness::Partial => "partial",
        CacheCompleteness::Unknown => "unknown",
    }
}

/// The reverse of [`cache_completeness_key`]. Returns `Err` for anything else, including a
/// corrupted or hand-edited value - the same fail-closed convention
/// `malformed_stored_content_is_reported_as_an_error_not_a_panic` already establishes for this
/// crate's other stored columns (never silently substitute a default completeness for content
/// that does not actually decode).
fn parse_cache_completeness(value: &str) -> Result<CacheCompleteness, StoreError> {
    match value {
        "complete" => Ok(CacheCompleteness::Complete),
        "partial" => Ok(CacheCompleteness::Partial),
        "unknown" => Ok(CacheCompleteness::Unknown),
        other => Err(StoreError(format!(
            "stored cache_completeness value {other:?} is not one of complete/partial/unknown"
        ))),
    }
}

/// A small bundle of primitive, caller-supplied invalidation evidence for one artifact's row -
/// the same shape whether it names what a row *was written with* (persisted, via
/// [`CurrentStateStore::set_invalidation_key`]) or what a caller *observes now* (fresh, passed to
/// [`CurrentStateStore::cache_read_hint`]). This module's own doc, "Incremental reuse", explains
/// why every field is a primitive rather than a `cancellai-inventory`/`cancellai-platform` type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheInvalidationKey {
    /// The wire-format stable identity this row was observed under (the same concept as
    /// [`AgentArtifact::identity_token`], supplied separately here because a caller may know a
    /// fresher one than whatever is already persisted in `data`).
    pub identity_token: String,
    /// A modification timestamp in whole seconds, platform-defined epoch - primitive rather
    /// than `cancellai_platform::Timestamp` so this crate's dependency ring stays unchanged.
    /// `None` if the underlying observation could not report one (never treated as "unchanged"
    /// against a persisted `Some`, or vice versa - a change in either direction, including from
    /// known to unknown, is exact-equality-false, per this module's own doc).
    pub modified: Option<u64>,
    /// An opaque, caller-defined fingerprint of the provider's observed layout/version (this
    /// module's own doc's "provider-layout-change" falsifier). `None` when the caller has no
    /// such fingerprint to offer.
    pub provider_fingerprint: Option<String>,
    /// An opaque, caller-defined version marker for whatever provider-knowledge bundle produced
    /// this row (e.g. `cancellai-safety::knowledge_bundle`'s own version). `None` when the
    /// caller has none to offer.
    pub knowledge_version: Option<String>,
    /// How complete the evidence behind this key was.
    pub completeness: CacheCompleteness,
}

/// Whether a cached row is worth reading as a fast, non-authoritative shortcut - never an
/// authorization, confirmation, or substitute for fresh execution-time observation (SI-024, this
/// module's own doc, "Incremental reuse"). The two variants are deliberately named around
/// *reading*, not around validity/authorization/safety, so a caller cannot mistake this for a
/// mutation-precondition check by the type's own name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheReadHint {
    /// Every invalidation axis this crate checks matched exactly, and both the persisted and
    /// the fresh completeness are `Complete`. Worth reading as a shortcut; still never a
    /// substitute for fresh observation before any mutation decision.
    ReuseForReading,
    /// No row exists for this id, no invalidation key was ever persisted for it, or at least one
    /// axis diverged (including either completeness being anything but `Complete`) - treat this
    /// exactly like a cache miss.
    Revalidate,
}

/// The current-state SQLite store: a reconstructible cache/index over the
/// [`AgentArtifact`]s a scan already produced (this module's own doc). Holds one open
/// connection for its lifetime, matching `rusqlite::Connection`'s own single-owner design.
pub struct CurrentStateStore {
    conn: Connection,
}

impl CurrentStateStore {
    /// Opens (creating if absent) cancellAI's own current-state database under `root`, at the
    /// fixed [`CURRENT_STATE_FILENAME`] this crate alone names - never a path `root`'s caller
    /// supplies directly (E13-S06: a provider-owned or mimicked file at any other location,
    /// even one carrying this crate's own identity marker, is unreachable from this entry point
    /// by construction, not because it is inspected and rejected - see [`local_state_root`]'s
    /// own module doc). Existing unit tests keep exercising the underlying open/migrate/verify
    /// path directly via [`Self::open_at_path`].
    pub fn open(root: &LocalStateRoot) -> Result<Self, StoreError> {
        Self::open_at_path(&root.path_for(CURRENT_STATE_FILENAME)?)
    }

    /// Opens (creating if absent) the current-state database at the exact `path` given, applying
    /// any migration this database has not already seen. Refuses (`Err`, no handle returned) a
    /// file whose `user_version`/schema make [`apply_migrations`] treat it as already-migrated
    /// but which does not carry this crate's own identity marker - see
    /// [`verify_store_identity`]. `pub(crate)` rather than a public API: production code reaches
    /// this only through [`Self::open`]'s [`LocalStateRoot`]-bound path; this crate's own tests
    /// call it directly with an explicit path, unchanged from before E13-S06.
    pub(crate) fn open_at_path(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        verify_store_identity_before_migrating(&conn)?;
        apply_migrations(&conn, MIGRATIONS)?;
        verify_store_identity(&conn)?;
        Ok(Self { conn })
    }

    /// An in-memory store for tests and short-lived callers that never need a file on disk.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        apply_migrations(&conn, MIGRATIONS)?;
        verify_store_identity(&conn)?;
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

    /// Attaches (replacing any previous one) a [`CacheInvalidationKey`] to an already-persisted
    /// row - this module's own doc, "Incremental reuse". `Err` if no row exists for `id`: a
    /// caller only ever has a fresh key to attach right after a `rebuild`/`get` confirmed the
    /// row is there, so an id with no row is a caller mistake this method surfaces rather than
    /// silently accepting and doing nothing.
    ///
    /// `Err` too when `key.identity_token` does not match the persisted row's own
    /// [`AgentArtifact::identity_token`] (round 2 independent verifier review's required repair):
    /// `ArtifactId` is a stable key across scans, but `identity_token` is what a scan actually
    /// observed the artifact to be *this time* - the same row's `ArtifactId` can legitimately be
    /// attached to different content across a rebuild. Binding a key stamped with one identity to
    /// a row whose persisted content is a different identity would later let
    /// [`CurrentStateStore::cache_read_hint`] report `ReuseForReading` for facts that were never
    /// observed under the identity the caller is asking about - the exact cross-identity cache
    /// reuse this check exists to make impossible to attach in the first place, not just
    /// difficult to trigger.
    pub fn set_invalidation_key(
        &mut self,
        id: &ArtifactId,
        key: &CacheInvalidationKey,
    ) -> Result<(), StoreError> {
        let modified = key
            .modified
            .map(|m| {
                i64::try_from(m).map_err(|_| {
                    StoreError(format!("cache_modified value {m} does not fit in i64"))
                })
            })
            .transpose()?;

        let tx = self.conn.transaction()?;
        let data: Option<String> = tx
            .query_row(
                "SELECT data FROM agent_artifacts WHERE artifact_id = ?1",
                rusqlite::params![id.0],
                |row| row.get(0),
            )
            .optional()?;
        let Some(data) = data else {
            return Err(StoreError(format!(
                "no row for artifact_id {:?} to attach an invalidation key to",
                id.0
            )));
        };
        let artifact: AgentArtifact = serde_json::from_str(&data)?;
        if artifact.identity_token != key.identity_token {
            return Err(StoreError(format!(
                "invalidation key identity_token {:?} does not match the persisted artifact's \
                 own identity_token {:?} for artifact_id {:?} - refusing to bind a cache key to \
                 a different identity than the row it would be attached to",
                key.identity_token, artifact.identity_token, id.0
            )));
        }

        let changed = tx.execute(
            "UPDATE agent_artifacts SET \
                cache_identity_token = ?1, \
                cache_modified = ?2, \
                cache_provider_fingerprint = ?3, \
                cache_knowledge_version = ?4, \
                cache_completeness = ?5 \
             WHERE artifact_id = ?6",
            rusqlite::params![
                key.identity_token,
                modified,
                key.provider_fingerprint,
                key.knowledge_version,
                cache_completeness_key(&key.completeness),
                id.0,
            ],
        )?;
        if changed == 0 {
            return Err(StoreError(format!(
                "no row for artifact_id {:?} to attach an invalidation key to",
                id.0
            )));
        }
        tx.commit()?;
        Ok(())
    }

    /// Compares `fresh` against whatever [`CacheInvalidationKey`] this row was last attached
    /// with, and returns a read-only hint - never an authorization (this module's own doc,
    /// "Incremental reuse", and [`CacheReadHint`]'s own doc). `Revalidate` for an absent row, a
    /// row with no invalidation key ever attached, or any axis that diverges - matching or
    /// exceeding completeness is the one and only path to `ReuseForReading`.
    pub fn cache_read_hint(
        &self,
        id: &ArtifactId,
        fresh: &CacheInvalidationKey,
    ) -> Result<CacheReadHint, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT cache_identity_token, cache_modified, cache_provider_fingerprint, \
                    cache_knowledge_version, cache_completeness \
             FROM agent_artifacts WHERE artifact_id = ?1",
        )?;
        let mut rows = stmt.query(rusqlite::params![id.0])?;
        let Some(row) = rows.next()? else {
            return Ok(CacheReadHint::Revalidate);
        };

        let identity_token: Option<String> = row.get(0)?;
        let Some(identity_token) = identity_token else {
            // NULL means "no invalidation key was ever attached to this row" - this module's
            // own doc's fail-safe default, never confused with "matches whatever is fresh."
            return Ok(CacheReadHint::Revalidate);
        };
        let modified_raw: Option<i64> = row.get(1)?;
        let modified = match modified_raw {
            Some(m) => Some(u64::try_from(m).map_err(|_| {
                StoreError(format!(
                    "stored cache_modified value {m} does not fit in u64"
                ))
            })?),
            None => None,
        };
        let provider_fingerprint: Option<String> = row.get(2)?;
        let knowledge_version: Option<String> = row.get(3)?;
        let completeness_raw: String = row.get(4)?;
        let persisted_completeness = parse_cache_completeness(&completeness_raw)?;

        let persisted = CacheInvalidationKey {
            identity_token,
            modified,
            provider_fingerprint,
            knowledge_version,
            completeness: persisted_completeness,
        };

        // Round 1 independent verifier review: plain `==` on an `Option<T>` axis treats
        // `None == None` as a match, but "neither side could observe this axis" is
        // uncertainty, not a confirmed absence of change - this module's own doc on
        // `modified` already says so ("never treated as unchanged ... a change ... to
        // unknown, is exact-equality-false"), which the code did not actually implement. An
        // axis only counts as unchanged when BOTH sides positively know it and it is equal;
        // `None` on either side (an unavailable provider fingerprint, mtime, or
        // knowledge-version) forces `Revalidate`.
        let modified_confirmed_unchanged =
            matches!((persisted.modified, fresh.modified), (Some(p), Some(f)) if p == f);
        let provider_fingerprint_confirmed_unchanged = matches!(
            (&persisted.provider_fingerprint, &fresh.provider_fingerprint),
            (Some(p), Some(f)) if p == f
        );
        let knowledge_version_confirmed_unchanged = matches!(
            (&persisted.knowledge_version, &fresh.knowledge_version),
            (Some(p), Some(f)) if p == f
        );

        let matches = persisted.identity_token == fresh.identity_token
            && modified_confirmed_unchanged
            && provider_fingerprint_confirmed_unchanged
            && knowledge_version_confirmed_unchanged
            && persisted.completeness == CacheCompleteness::Complete
            && fresh.completeness == CacheCompleteness::Complete;

        Ok(if matches {
            CacheReadHint::ReuseForReading
        } else {
            CacheReadHint::Revalidate
        })
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
            let mut store = CurrentStateStore::open_at_path(&db_path).expect("open");
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
            let mut store = CurrentStateStore::open_at_path(&db_path).expect("first open");
            store
                .rebuild(&[artifact("artifact-0001", "codex", ActivityState::Active)])
                .expect("rebuild");
        }
        {
            let store =
                CurrentStateStore::open_at_path(&db_path).expect("second open must not fail");
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
            let mut store = CurrentStateStore::open_at_path(&db_path).expect("open");
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

    #[test]
    fn open_refuses_a_provider_owned_file_that_only_mimics_this_crates_schema() {
        // Round 2 independent verifier review: a hand-crafted SQLite file with an
        // `agent_artifacts` table/row and `PRAGMA user_version` already at the fully-migrated
        // value made `apply_migrations` treat it as an already-migrated store of this crate's
        // own, so `open` handed back a reset-capable handle over content this crate never
        // created. `open` must refuse such a file instead (SI-026), because it never actually
        // ran this crate's own migration 0 and therefore never carries the identity marker that
        // migration inserts.
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-test-open-mimic-{}-{}",
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
                "CREATE TABLE agent_artifacts (
                    artifact_id TEXT PRIMARY KEY,
                    provider_id TEXT NOT NULL,
                    activity_state TEXT NOT NULL,
                    data TEXT NOT NULL
                ) STRICT;
                INSERT INTO agent_artifacts (artifact_id, provider_id, activity_state, data)
                    VALUES ('provider-row', 'some-provider', 'active', '{}');
                PRAGMA user_version = 2;",
            )
            .expect("craft a provider-owned lookalike database");
        }

        let opened = CurrentStateStore::open_at_path(&provider_db_path);
        assert!(
            opened.is_err(),
            "open must refuse a file that mimics this crate's schema/user_version but was \
             never created by this crate's own migrations"
        );

        let verify = Connection::open(&provider_db_path).expect("reopen raw sqlite file");
        let row_count: i64 = verify
            .query_row("SELECT COUNT(*) FROM agent_artifacts", [], |row| row.get(0))
            .expect("count provider rows");
        assert_eq!(
            row_count, 1,
            "a refused open must leave the provider-owned file completely untouched"
        );

        // Windows refuses to delete a file with an open handle; `verify` must close before
        // cleanup, matching this module's own established precedent (96f645e).
        drop(verify);
        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    fn open_via_local_state_root_never_reaches_a_marker_bearing_mimic_elsewhere_on_disk() {
        // E13-VERIFIER-REVIEW-ROUND3.md's actual reproduction: a provider-owned file carrying
        // this crate's exact schema, `user_version`, and a byte-for-byte copy of
        // `STORE_IDENTITY_MARKER` - stronger than the round-2 mimic above, which lacked the
        // marker. `CurrentStateStore::open`'s production, `LocalStateRoot`-bound entry point
        // must never touch this file at all, regardless of its content, because it never
        // constructs a handle anywhere but `<resolved root>/current_state.sqlite3`.
        let base = std::env::temp_dir().join(format!(
            "cancellai-store-test-root-boundary-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state_dir = base.join("state-root");
        let attacker_dir = base.join("attacker");
        std::fs::create_dir_all(&state_dir).expect("create state root dir");
        std::fs::create_dir_all(&attacker_dir).expect("create attacker dir");

        // A full marker-bearing mimic, placed both outside the state root and, under the exact
        // reserved filename, inside a sibling directory - not the resolved root itself.
        let mimic_path = attacker_dir.join(CURRENT_STATE_FILENAME);
        {
            let conn = Connection::open(&mimic_path).expect("open raw sqlite file");
            conn.execute_batch(&format!(
                "CREATE TABLE agent_artifacts (
                    artifact_id TEXT PRIMARY KEY,
                    provider_id TEXT NOT NULL,
                    activity_state TEXT NOT NULL,
                    data TEXT NOT NULL
                ) STRICT;
                CREATE TABLE cancellai_store_identity (marker TEXT PRIMARY KEY) STRICT;
                INSERT INTO cancellai_store_identity (marker) VALUES ('{STORE_IDENTITY_MARKER}');
                INSERT INTO agent_artifacts (artifact_id, provider_id, activity_state, data)
                    VALUES ('provider-row', 'some-provider', 'active', '{{}}');
                PRAGMA user_version = 2;"
            ))
            .expect("craft a full marker-bearing mimic");
        }

        let root = LocalStateRoot::resolve(&state_dir).expect("resolve local-state root");
        let mut store =
            CurrentStateStore::open(&root).expect("open must succeed against the real root");
        assert_eq!(
            store.row_count().expect("row count"),
            0,
            "open via the resolved root must start from a fresh store, never the mimic's content"
        );
        store.reset().expect("reset the real store");

        let verify = Connection::open(&mimic_path).expect("reopen the mimic file");
        let row_count: i64 = verify
            .query_row("SELECT COUNT(*) FROM agent_artifacts", [], |row| row.get(0))
            .expect("count mimic rows");
        assert_eq!(
            row_count, 1,
            "the marker-bearing mimic must be completely untouched - open() never named its path"
        );

        // Windows refuses to delete a directory containing a file with an open handle; `store`
        // itself (not just `verify`) holds one on the real database this test opened, so it must
        // close first too, matching this module's own established precedent (96f645e).
        drop(store);
        drop(verify);
        std::fs::remove_dir_all(&base).expect("clean up test dir");
    }

    #[test]
    #[cfg(unix)]
    fn open_via_local_state_root_refuses_a_fixed_filename_symlink_planted_at_the_leaf() {
        // Round 4 independent review of E13-S06's own reproduction: after a genuine, legitimate
        // resolve() of a real root, a symlink at the fixed leaf filename redirected production
        // open()/reset() to a provider-owned file elsewhere and erased its one row. open() must
        // now refuse outright rather than follow the symlink.
        let base = std::env::temp_dir().join(format!(
            "cancellai-store-test-symlink-boundary-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state_dir = base.join("state-root");
        let attacker_dir = base.join("attacker");
        std::fs::create_dir_all(&state_dir).expect("create state root dir");
        std::fs::create_dir_all(&attacker_dir).expect("create attacker dir");

        let provider_owned_path = attacker_dir.join("provider-owned.sqlite3");
        {
            let conn = Connection::open(&provider_owned_path).expect("open raw sqlite file");
            conn.execute_batch(
                "CREATE TABLE agent_artifacts (
                    artifact_id TEXT PRIMARY KEY,
                    provider_id TEXT NOT NULL,
                    activity_state TEXT NOT NULL,
                    data TEXT NOT NULL
                ) STRICT;
                INSERT INTO agent_artifacts (artifact_id, provider_id, activity_state, data)
                    VALUES ('provider-row', 'some-provider', 'active', '{}');",
            )
            .expect("craft a provider-owned store-shaped file");
        }

        let root = LocalStateRoot::resolve(&state_dir).expect("resolve local-state root");
        std::os::unix::fs::symlink(&provider_owned_path, state_dir.join(CURRENT_STATE_FILENAME))
            .expect("plant symlink at the fixed leaf");

        assert!(
            CurrentStateStore::open(&root).is_err(),
            "open() must refuse a symlinked leaf, not follow it into a provider-owned file"
        );

        let verify = Connection::open(&provider_owned_path).expect("reopen the provider file");
        let row_count: i64 = verify
            .query_row("SELECT COUNT(*) FROM agent_artifacts", [], |row| row.get(0))
            .expect("count provider rows");
        assert_eq!(
            row_count, 1,
            "the provider-owned file must be completely untouched"
        );

        drop(verify);
        std::fs::remove_dir_all(&base).expect("clean up test dir");
    }

    #[test]
    fn open_refuses_before_mutating_a_file_at_an_intermediate_user_version_with_no_marker() {
        // Self-review's own further finding: checking identity only after `apply_migrations`
        // ran let a non-owned file whose `user_version` sat between migrations receive a real
        // schema mutation (migration 1's `ALTER TABLE ... ADD COLUMN`) before its `open` was
        // refused. A file at `user_version = 1` with `agent_artifacts` in migration 0's shape
        // but no identity-marker table must be refused without migration 1 ever touching it.
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-test-open-no-mutate-before-refuse-{}-{}",
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
                "CREATE TABLE agent_artifacts (
                    artifact_id TEXT PRIMARY KEY,
                    provider_id TEXT NOT NULL,
                    activity_state TEXT NOT NULL,
                    data TEXT NOT NULL
                ) STRICT;
                PRAGMA user_version = 1;",
            )
            .expect("craft a file with no identity marker at user_version 1");
        }

        assert!(
            CurrentStateStore::open_at_path(&db_path).is_err(),
            "open must refuse this file"
        );

        let verify = Connection::open(&db_path).expect("reopen raw sqlite file");
        let has_cache_column: i64 = verify
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('agent_artifacts') \
                 WHERE name = 'cache_identity_token'",
                [],
                |row| row.get(0),
            )
            .expect("inspect columns");
        assert_eq!(
            has_cache_column, 0,
            "a refused open must never run migration 1's ALTER TABLE against a file this crate \
             did not create"
        );

        // Windows refuses to delete a file with an open handle; `verify` must close before
        // cleanup, matching this module's own established precedent (96f645e).
        drop(verify);
        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    // ------------------------------------------------------------------------------------
    // E13-S05: incremental reuse / SI-024 falsification plan.
    // ------------------------------------------------------------------------------------

    /// A fully-matching, `Complete` invalidation key - the one shape that can ever compare
    /// `ReuseForReading` against an identically-written persisted row.
    fn full_key(identity_token: &str, modified: Option<u64>) -> CacheInvalidationKey {
        CacheInvalidationKey {
            identity_token: identity_token.to_string(),
            modified,
            provider_fingerprint: Some("codex-cli-1.2.3".to_string()),
            knowledge_version: Some("kb-2026-05-01".to_string()),
            completeness: CacheCompleteness::Complete,
        }
    }

    fn store_with_one_row(identity_token: &str) -> (CurrentStateStore, ArtifactId) {
        let mut store = CurrentStateStore::open_in_memory().expect("open");
        let id = ArtifactId::new("artifact-0001");
        let mut a = artifact("artifact-0001", "codex", ActivityState::Active);
        a.identity_token = identity_token.to_string();
        store.rebuild(&[a]).expect("rebuild");
        (store, id)
    }

    #[test]
    fn cache_read_hint_is_revalidate_when_no_row_exists_for_the_id() {
        let store = CurrentStateStore::open_in_memory().expect("open");
        let hint = store
            .cache_read_hint(
                &ArtifactId::new("artifact-missing"),
                &full_key("codex:sessions/x", Some(1_000)),
            )
            .expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::Revalidate);
    }

    #[test]
    fn cache_read_hint_is_revalidate_when_no_invalidation_key_was_ever_set() {
        // A row written by a plain `rebuild` alone (this module's own doc's fail-safe default -
        // `cache_identity_token` stays NULL until a caller explicitly attaches a key).
        let (store, id) = store_with_one_row("codex:sessions/x");
        let hint = store
            .cache_read_hint(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::Revalidate);
    }

    #[test]
    fn set_invalidation_key_fails_for_an_id_with_no_row() {
        let mut store = CurrentStateStore::open_in_memory().expect("open");
        let result = store.set_invalidation_key(
            &ArtifactId::new("artifact-missing"),
            &full_key("codex:sessions/x", Some(1_000)),
        );
        assert!(
            result.is_err(),
            "attaching an invalidation key to a non-existent row must fail, not silently no-op"
        );
    }

    #[test]
    fn set_invalidation_key_rejects_a_key_whose_identity_does_not_match_the_persisted_row() {
        // Round 2 independent verifier review's exact reproduction: a row persisted for
        // `identity-A` must not accept an invalidation key stamped `identity-B` - doing so would
        // later let `cache_read_hint` report `ReuseForReading` against fresh `identity-B` facts
        // for a row whose actual content was never observed under that identity.
        let (mut store, id) = store_with_one_row("identity-A");
        let result = store.set_invalidation_key(&id, &full_key("identity-B", Some(1_000)));
        assert!(
            result.is_err(),
            "a key stamped with a different identity than the persisted row must be rejected"
        );

        let hint = store
            .cache_read_hint(&id, &full_key("identity-B", Some(1_000)))
            .expect("cache_read_hint");
        assert_eq!(
            hint,
            CacheReadHint::Revalidate,
            "a rejected set_invalidation_key must leave the row with no usable invalidation key"
        );
    }

    #[test]
    fn cache_read_hint_is_reuse_for_reading_when_every_axis_matches_and_both_sides_are_complete() {
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("set_invalidation_key");

        let hint = store
            .cache_read_hint(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::ReuseForReading);
    }

    #[test]
    fn cache_read_hint_is_revalidate_when_neither_side_has_a_modified_timestamp() {
        // Round 1 independent verifier review: `None == None` was previously treated as
        // "unchanged," but AC2 names "mtime/metadata ... uncertainty" as something that must
        // invalidate the cache scope, and SI-024 requires the same for any axis neither side
        // can positively confirm. Two platforms that both cannot report mtime have not
        // thereby confirmed the file is unchanged - they have confirmed nothing about it.
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", None))
            .expect("set_invalidation_key");

        let hint = store
            .cache_read_hint(&id, &full_key("codex:sessions/x", None))
            .expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::Revalidate);
    }

    #[test]
    fn falsifier_unavailable_provider_fingerprint_is_never_treated_as_unchanged() {
        // Round 1 independent verifier review's exact reproduction: identity/mtime/knowledge
        // all equal, but `provider_fingerprint: None` on the fresh side. An unavailable
        // fingerprint cannot confirm the provider's layout/version is unchanged, so this must
        // force `Revalidate`, not `ReuseForReading`.
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("set_invalidation_key");

        let fresh = CacheInvalidationKey {
            identity_token: "codex:sessions/x".to_string(),
            modified: Some(1_000),
            provider_fingerprint: None,
            knowledge_version: Some("kb-2026-05-01".to_string()),
            completeness: CacheCompleteness::Complete,
        };
        let hint = store.cache_read_hint(&id, &fresh).expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::Revalidate);
    }

    #[test]
    fn falsifier_unavailable_knowledge_version_is_never_treated_as_unchanged() {
        // Same axis-uncertainty requirement as the provider-fingerprint falsifier above, for
        // `knowledge_version`.
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("set_invalidation_key");

        let fresh = CacheInvalidationKey {
            identity_token: "codex:sessions/x".to_string(),
            modified: Some(1_000),
            provider_fingerprint: Some("codex-cli-1.2.3".to_string()),
            knowledge_version: None,
            completeness: CacheCompleteness::Complete,
        };
        let hint = store.cache_read_hint(&id, &fresh).expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::Revalidate);
    }

    #[test]
    fn falsifier_stale_cache_same_identity_changed_mtime_is_never_reusable() {
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("set_invalidation_key");

        let hint = store
            .cache_read_hint(&id, &full_key("codex:sessions/x", Some(1_001)))
            .expect("cache_read_hint");
        assert_eq!(
            hint,
            CacheReadHint::Revalidate,
            "an mtime change under an unchanged identity must still force revalidation"
        );
    }

    #[test]
    fn falsifier_clock_skew_mtime_moved_backward_is_never_treated_as_unchanged() {
        // Constitutional "ambiguity never escalates privilege": a clock moving backward is a
        // change like any other, never silently accepted as "no change."
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("set_invalidation_key");

        let hint = store
            .cache_read_hint(&id, &full_key("codex:sessions/x", Some(999)))
            .expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::Revalidate);
    }

    #[test]
    fn cache_read_hint_matches_on_byte_for_byte_identical_mtime() {
        // The positive edge alongside the two negative clock/mtime falsifiers above: an
        // unchanged mtime, compared exactly, must not itself force an unnecessary revalidation.
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(1_234_567)))
            .expect("set_invalidation_key");

        let hint = store
            .cache_read_hint(&id, &full_key("codex:sessions/x", Some(1_234_567)))
            .expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::ReuseForReading);
    }

    #[test]
    fn falsifier_identity_changed_is_never_reusable() {
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("set_invalidation_key");

        let hint = store
            .cache_read_hint(&id, &full_key("codex:sessions/y", Some(1_000)))
            .expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::Revalidate);
    }

    #[test]
    fn falsifier_provider_layout_change_same_identity_and_mtime_different_fingerprint_is_never_reusable()
     {
        // Simulates a provider adapter whose on-disk format/version changed under an otherwise
        // unchanged path and mtime - the fingerprint axis must catch what identity/mtime cannot.
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("set_invalidation_key");

        let mut fresh = full_key("codex:sessions/x", Some(1_000));
        fresh.provider_fingerprint = Some("codex-cli-2.0.0".to_string());

        let hint = store.cache_read_hint(&id, &fresh).expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::Revalidate);
    }

    #[test]
    fn falsifier_knowledge_version_changed_is_never_reusable() {
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("set_invalidation_key");

        let mut fresh = full_key("codex:sessions/x", Some(1_000));
        fresh.knowledge_version = Some("kb-2026-06-01".to_string());

        let hint = store.cache_read_hint(&id, &fresh).expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::Revalidate);
    }

    #[test]
    fn falsifier_partial_scan_complete_row_against_a_degraded_fresh_observation_is_never_reusable()
    {
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("set_invalidation_key");

        for degraded in [CacheCompleteness::Partial, CacheCompleteness::Unknown] {
            let mut fresh = full_key("codex:sessions/x", Some(1_000));
            fresh.completeness = degraded;
            let hint = store.cache_read_hint(&id, &fresh).expect("cache_read_hint");
            assert_eq!(
                hint,
                CacheReadHint::Revalidate,
                "a Complete row must never be treated as reusable against a degraded {degraded:?} \
                 fresh observation"
            );
        }
    }

    #[test]
    fn falsifier_partial_scan_a_row_persisted_as_partial_is_never_reusable_even_against_an_identical_partial_observation()
     {
        // Explicit, tested behavior (not an implicit default): a row written under Partial/
        // Unknown evidence must never be treated as more reliable than it was when written - it
        // is never `ReuseForReading`, even against a byte-for-byte identical fresh observation
        // that is itself still Partial/Unknown.
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        let mut persisted = full_key("codex:sessions/x", Some(1_000));
        persisted.completeness = CacheCompleteness::Partial;
        store
            .set_invalidation_key(&id, &persisted)
            .expect("set_invalidation_key");

        let mut fresh = full_key("codex:sessions/x", Some(1_000));
        fresh.completeness = CacheCompleteness::Partial;

        let hint = store.cache_read_hint(&id, &fresh).expect("cache_read_hint");
        assert_eq!(
            hint,
            CacheReadHint::Revalidate,
            "a row persisted under Partial evidence must never be ReuseForReading, even against \
             an identical fresh Partial observation"
        );
    }

    #[test]
    fn set_invalidation_key_replaces_a_previously_attached_key_rather_than_merging_it() {
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("first set_invalidation_key");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(2_000)))
            .expect("second set_invalidation_key");

        // The stale first mtime must not still satisfy a fresh comparison - the second call
        // must have fully replaced the first, not left a stray old value behind.
        let hint = store
            .cache_read_hint(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::Revalidate);

        let hint = store
            .cache_read_hint(&id, &full_key("codex:sessions/x", Some(2_000)))
            .expect("cache_read_hint");
        assert_eq!(hint, CacheReadHint::ReuseForReading);
    }

    #[test]
    fn rebuild_clears_every_previously_attached_invalidation_key() {
        // A `rebuild` (the reconstruction primitive) must never leave a stale invalidation key
        // attached to a row that a later scan reinserts under the same artifact_id - the same
        // "content depends only on what was just scanned" guarantee `rebuild` already gives the
        // rest of the row, extended to these columns without any extra cleanup step.
        let (mut store, id) = store_with_one_row("codex:sessions/x");
        store
            .set_invalidation_key(&id, &full_key("codex:sessions/x", Some(1_000)))
            .expect("set_invalidation_key");
        assert_eq!(
            store
                .cache_read_hint(&id, &full_key("codex:sessions/x", Some(1_000)))
                .expect("cache_read_hint"),
            CacheReadHint::ReuseForReading
        );

        store
            .rebuild(&[artifact("artifact-0001", "codex", ActivityState::Active)])
            .expect("rebuild");

        let hint = store
            .cache_read_hint(
                &ArtifactId::new("artifact-0001"),
                &full_key("codex:sessions/x", Some(1_000)),
            )
            .expect("cache_read_hint");
        assert_eq!(
            hint,
            CacheReadHint::Revalidate,
            "a rebuilt row must never inherit a stale invalidation key from before the rebuild"
        );
    }

    #[test]
    fn cache_completeness_round_trips_through_parse_and_key() {
        for completeness in [
            CacheCompleteness::Complete,
            CacheCompleteness::Partial,
            CacheCompleteness::Unknown,
        ] {
            let key = cache_completeness_key(&completeness);
            assert_eq!(parse_cache_completeness(key).expect("parse"), completeness);
        }
    }

    #[test]
    fn parse_cache_completeness_reports_an_unrecognized_value_as_an_error_not_a_panic() {
        // Fail-closed axis, matching `malformed_stored_content_is_reported_as_an_error_not_a_panic`:
        // a hand-edited or corrupted `cache_completeness` column must never be silently coerced
        // into a default value.
        assert!(parse_cache_completeness("not-a-real-value").is_err());
    }

    #[test]
    fn cargo_toml_declares_no_dependency_on_cancellai_safety() {
        // SI-024: nothing in this crate can reach the safety crate's mutation-execution
        // capability, because this crate's own manifest does not depend on `cancellai-safety`
        // at all - a compile-time impossibility, not merely a convention this test could
        // accidentally stop enforcing. `scripts/check_mutation_boundary.py check` (SI-019) is
        // the authoritative, workspace-wide static scan for the raw capability itself; this pins
        // the narrower, crate-local precondition that makes referencing it impossible here in
        // the first place.
        let manifest = include_str!("../Cargo.toml");
        // A dependency table key, not a `#`-comment mention (this very manifest already has one,
        // in `sha2`'s own rationale comment, explaining a different crate's precedent - checking
        // for a line that actually *starts* with the crate name is what distinguishes "declared
        // as a dependency" from "named in prose").
        let declares_dependency = manifest
            .lines()
            .any(|line| line.trim_start().starts_with("cancellai-safety"));
        assert!(
            !declares_dependency,
            "cancellai-store must never depend on cancellai-safety - a cache-read-hint \
             primitive must never gain a path to the mutation executor"
        );
    }
}

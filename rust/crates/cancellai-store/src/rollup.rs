//! Analytical rollups and retention (E13-S03, `docs/architecture/PERSISTENCE_MODEL.md`'s "Layer
//! 3: Analytical Memory"). Guardian-facing time-series memory: fine-grained numeric measurements
//! ("samples") age out of a recent window into hourly rollups, hourly rollups age out of a medium
//! window into daily rollups, and daily rollups age out of a long window into one bounded
//! per-metric/scope long-term aggregate - "time-series rollups rather than permanent raw
//! samples," never a fourth, unbounded tier.
//!
//! ## Why this is a distinct source of writes from [`crate::ledger`] (Layer 2), not a consumer of it
//!
//! Layer 2's `EventLedger` records that a discrete, significant thing happened (a classification,
//! a mutation) - its own module doc calls it an audit trail, and its `EventKind` set is closed
//! and semantic. Layer 3 is a different kind of data: repeated numeric observations of the same
//! quantity over time (a footprint size, an artifact count), whose individual readings are not
//! independently significant and whose value is in the trend, not the instant. Deriving hourly/
//! daily statistics from `EventKind::Discovered`/`Classified` counts would conflate "an event
//! happened" with "a measurement was taken," and would make the ledger's own bounded-retention
//! story (`EventLedger::compact_range`) responsible for Layer 3's very different, continuously-
//! sampled retention shape. This module therefore owns its own ingestion (`record_sample`) rather
//! than reading `ledger_events` - the two layers stay independently retained, matching
//! `PERSISTENCE_MODEL.md` treating them as two separate layers with two separate budgets under
//! "Self-budget." A future orchestrator may well call both `EventLedger::append` and
//! `AnalyticalMemory::record_sample` for the same real-world observation; that is a caller-side
//! decision, not a reason to make one layer depend on the other's schema.
//!
//! ## Deterministic time, no `SystemTime::now()` in this module's own logic
//!
//! Every time-dependent method here (`record_sample`, `compact`) takes `now`/`recorded_at` as an
//! explicit `u64` (seconds since the Unix epoch) parameter, the same seam
//! [`crate::ledger`]'s own `NewEvent::recorded_at`/`EventLedger::compact_range`'s `created_at`
//! already use, and for the same reason stated there: production code never calls
//! `std::time::SystemTime::now()` inside this crate's business logic, so a test can move `now`
//! forward across window boundaries deterministically, without sleeping and without a mock
//! framework. A future orchestrator supplies the real reading (e.g. from
//! `cancellai_platform::clock::Clock`, which this outer-ring crate does not depend on merely to
//! read a clock it can just as well receive as a parameter - see ADR-0019: an outer-ring
//! dependency is for what a story cannot express with `std` alone, and a `u64` parameter already
//! expresses this).
//!
//! ## Retention is policy, not a hard-coded constant (AC "raw samples expire according to policy")
//!
//! [`RetentionPolicy`] carries the three window lengths `PERSISTENCE_MODEL.md`'s Layer 3 names
//! (recent/medium/long) as plain, caller-supplied durations - [`RetentionPolicy::DEFAULT`] is one
//! reasonable choice, not the only legal one. [`RetentionPolicy::new`] is the only public
//! constructor and rejects a policy whose windows are not non-decreasing (recent <= medium <=
//! long), so a caller cannot construct a policy this module would otherwise silently
//! misinterpret.
//!
//! ## Contentless by construction (AC "aggregates ... without retaining sensitive content")
//!
//! [`MetricKind`] is a closed, exhaustive enum (matching [`crate::ledger::EventKind`]'s own
//! precedent: a future variant this module does not yet handle fails to compile rather than
//! silently accepting anything a caller spells) - a caller cannot smuggle a path, transcript, or
//! other artifact content through the metric name, because the type does not admit one. The only
//! other caller-supplied strings are [`SampleScope`]'s `provider_id`/`category` fields, the same
//! closed, allowlisted shape [`crate::ledger::EventMetadata`] already uses for the same reason;
//! [`NewSample::value`] is a plain `f64` - numeric by construction. `tests::rollup_tables_have_
//! only_the_allowlisted_columns` pins the actual schema of all four tables against this closed
//! set, so a future change that widens it is a visible, reviewable diff.
//!
//! ## Compaction: one explicit primitive, cascading, atomic, and idempotent
//!
//! [`AnalyticalMemory::compact`] is the only way a sample or rollup ever leaves its tier - there
//! is no implicit background expiry. One call runs all three promotions (raw -> hourly -> daily
//! -> long-term) in a single transaction, in that order, so a `now` that has advanced past more
//! than one window boundary since the last compaction cascades a sample all the way to whatever
//! tier its age now warrants in one step, rather than requiring one call per window crossed - a
//! sample newly promoted to an hourly bucket that is itself already older than the medium window
//! is promoted again to daily within the same call, and the same for daily -> long-term. Each
//! promotion groups its eligible source rows by `(metric, provider_id, category, bucket)`,
//! merges each group into any existing destination row for that same key (or creates one), and
//! only then deletes the source rows - so a second `compact` call over an unchanged database
//! finds nothing newly eligible and is a no-op, and a promotion that already happened is never
//! double-counted by a subsequent call. Grouping keys on the sample's own `recorded_at`
//! (`raw_samples`) or the bucket it already carries (`hourly_rollups`/`daily_rollups`), never on
//! insertion order, so a backdated or clock-skewed sample lands in the historically correct
//! bucket rather than corrupting whichever bucket happens to be "current."

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::collections::BTreeMap;
use std::path::Path;

const SECONDS_PER_HOUR: u64 = 3_600;
const SECONDS_PER_DAY: u64 = 24 * SECONDS_PER_HOUR;

/// Why an [`AnalyticalMemory`] operation failed - the underlying SQLite error, a stored row that
/// does not decode, or this module's own contract violation (an out-of-range timestamp, or an
/// invalid [`RetentionPolicy`]).
#[derive(Debug)]
pub struct RollupError(String);

impl std::fmt::Display for RollupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for RollupError {}

impl From<rusqlite::Error> for RollupError {
    fn from(e: rusqlite::Error) -> Self {
        Self(e.to_string())
    }
}

/// This module's own migration history - a separate `PRAGMA user_version` namespace from
/// [`crate::CurrentStateStore`]'s and [`crate::ledger::EventLedger`]'s, because each opens its
/// own `Connection` to its own file, matching both of their own documented precedent for why a
/// caller is expected to give each layer a separate path (separate self-budgets, per
/// `docs/architecture/PERSISTENCE_MODEL.md`'s "Self-budget").
const MIGRATIONS: &[&str] = &["
    CREATE TABLE raw_samples (
        sample_id INTEGER PRIMARY KEY AUTOINCREMENT,
        metric TEXT NOT NULL,
        recorded_at INTEGER NOT NULL,
        provider_id TEXT,
        category TEXT,
        value REAL NOT NULL
    ) STRICT;
    CREATE INDEX idx_raw_samples_recorded_at ON raw_samples(recorded_at);

    CREATE TABLE hourly_rollups (
        rollup_id INTEGER PRIMARY KEY AUTOINCREMENT,
        metric TEXT NOT NULL,
        provider_id TEXT,
        category TEXT,
        bucket_start INTEGER NOT NULL,
        sample_count INTEGER NOT NULL,
        sum_value REAL NOT NULL,
        min_value REAL NOT NULL,
        max_value REAL NOT NULL
    ) STRICT;
    CREATE INDEX idx_hourly_rollups_bucket_start ON hourly_rollups(bucket_start);

    CREATE TABLE daily_rollups (
        rollup_id INTEGER PRIMARY KEY AUTOINCREMENT,
        metric TEXT NOT NULL,
        provider_id TEXT,
        category TEXT,
        bucket_start INTEGER NOT NULL,
        sample_count INTEGER NOT NULL,
        sum_value REAL NOT NULL,
        min_value REAL NOT NULL,
        max_value REAL NOT NULL
    ) STRICT;
    CREATE INDEX idx_daily_rollups_bucket_start ON daily_rollups(bucket_start);

    CREATE TABLE long_term_aggregates (
        aggregate_id INTEGER PRIMARY KEY AUTOINCREMENT,
        metric TEXT NOT NULL,
        provider_id TEXT,
        category TEXT,
        sample_count INTEGER NOT NULL,
        sum_value REAL NOT NULL,
        min_value REAL NOT NULL,
        max_value REAL NOT NULL,
        first_bucket_start INTEGER NOT NULL,
        last_bucket_start INTEGER NOT NULL
    ) STRICT;
"];

/// A scoped copy of `crate::apply_migrations`'s/`crate::ledger::apply_migrations`'s own logic -
/// each module keeps its own copy rather than sharing one, the precedent
/// `crate::ledger`'s own doc already states for its `LedgerError` (this module's `RollupError`
/// stays this module's own concern).
fn apply_migrations(conn: &Connection, migrations: &[&str]) -> Result<(), RollupError> {
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

fn to_i64(value: u64, what: &str) -> Result<i64, RollupError> {
    i64::try_from(value)
        .map_err(|_| RollupError(format!("{what} does not fit in a signed 64-bit column")))
}

fn hour_bucket_start(recorded_at: u64) -> u64 {
    (recorded_at / SECONDS_PER_HOUR) * SECONDS_PER_HOUR
}

fn day_bucket_start(recorded_at: u64) -> u64 {
    (recorded_at / SECONDS_PER_DAY) * SECONDS_PER_DAY
}

/// The closed set of numeric measurements this module accepts - deliberately exhaustive (this
/// module's own doc, "Contentless by construction"). Adding a metric is a visible, reviewable
/// diff to this enum and its `key`/`from_key` mapping, never a caller passing an arbitrary
/// string. Named for the detection signals `docs/architecture/GUARDIAN_MODEL.md`'s own
/// "Detection" list already names (free-disk capacity, provider/project budgets, session-count
/// explosion, orphan-state growth) without committing to Guardian's full future taxonomy, which
/// is that document's own scope, not this story's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetricKind {
    /// A count of artifacts observed (e.g. per provider/category) at the sample's timestamp.
    ArtifactCount,
    /// A provider or project footprint, in bytes, at the sample's timestamp.
    ProviderFootprintBytes,
    /// A reclaimable-space observation, in bytes, at the sample's timestamp.
    ReclaimableBytes,
    /// A count of artifacts in an orphaned/unattributed state at the sample's timestamp.
    OrphanCount,
}

impl MetricKind {
    /// The exact string this kind is stored/read under - matching `crate::ledger::EventKind::
    /// key`'s own explicit, exhaustive precedent so a future variant fails to compile here
    /// rather than falling back to something.
    fn key(self) -> &'static str {
        match self {
            MetricKind::ArtifactCount => "ARTIFACT_COUNT",
            MetricKind::ProviderFootprintBytes => "PROVIDER_FOOTPRINT_BYTES",
            MetricKind::ReclaimableBytes => "RECLAIMABLE_BYTES",
            MetricKind::OrphanCount => "ORPHAN_COUNT",
        }
    }

    fn from_key(key: &str) -> Result<Self, RollupError> {
        match key {
            "ARTIFACT_COUNT" => Ok(MetricKind::ArtifactCount),
            "PROVIDER_FOOTPRINT_BYTES" => Ok(MetricKind::ProviderFootprintBytes),
            "RECLAIMABLE_BYTES" => Ok(MetricKind::ReclaimableBytes),
            "ORPHAN_COUNT" => Ok(MetricKind::OrphanCount),
            other => Err(RollupError(format!("unknown stored metric kind: {other}"))),
        }
    }
}

/// The closed, allowlisted dimensions a sample may be scoped to - the same shape
/// [`crate::ledger::EventMetadata`] already uses for the same "no free-form/content field"
/// reason. Every field is optional: a caller supplies only what it knows; both absent means a
/// host-wide (unscoped) measurement.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SampleScope {
    pub provider_id: Option<String>,
    pub category: Option<String>,
}

/// One fine-grained measurement to record.
#[derive(Debug, Clone, PartialEq)]
pub struct NewSample {
    pub metric: MetricKind,
    pub recorded_at: u64,
    pub scope: SampleScope,
    pub value: f64,
}

/// A raw sample as read back.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub sample_id: i64,
    pub metric: MetricKind,
    pub recorded_at: u64,
    pub scope: SampleScope,
    pub value: f64,
}

/// A bounded statistic over a bucket of samples (an hour, for [`AnalyticalMemory::hourly_
/// rollups`], or a day, for [`AnalyticalMemory::daily_rollups`]) - never the individual readings
/// that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct Rollup {
    pub rollup_id: i64,
    pub metric: MetricKind,
    pub scope: SampleScope,
    pub bucket_start: u64,
    pub sample_count: u64,
    pub sum_value: f64,
    pub min_value: f64,
    pub max_value: f64,
}

/// The bounded statistic beyond the long window - one row per `(metric, scope)`, with no further
/// time bucketing (`PERSISTENCE_MODEL.md`: "beyond long window: bounded statistics/tombstone
/// aggregates").
#[derive(Debug, Clone, PartialEq)]
pub struct LongTermAggregate {
    pub aggregate_id: i64,
    pub metric: MetricKind,
    pub scope: SampleScope,
    pub sample_count: u64,
    pub sum_value: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub first_bucket_start: u64,
    pub last_bucket_start: u64,
}

/// The three window lengths `docs/architecture/PERSISTENCE_MODEL.md`'s Layer 3 names, as
/// explicit, caller-supplied durations in seconds - "exact periods and budgets are product
/// policy, not hard-coded architecture constants" (that document's own words). [`Self::DEFAULT`]
/// is one reasonable choice, not the only legal one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    recent_window_secs: u64,
    medium_window_secs: u64,
    long_window_secs: u64,
}

impl RetentionPolicy {
    /// One day of raw samples, one week of hourly rollups, ninety days of daily rollups before
    /// collapsing to a single long-term aggregate per `(metric, scope)`.
    pub const DEFAULT: RetentionPolicy = RetentionPolicy {
        recent_window_secs: SECONDS_PER_DAY,
        medium_window_secs: 7 * SECONDS_PER_DAY,
        long_window_secs: 90 * SECONDS_PER_DAY,
    };

    /// The only public constructor - rejects windows that are not non-decreasing
    /// (`recent <= medium <= long`) or a zero recent window, either of which this module would
    /// otherwise have to special-case silently in every promotion.
    pub fn new(
        recent_window_secs: u64,
        medium_window_secs: u64,
        long_window_secs: u64,
    ) -> Result<Self, RollupError> {
        if recent_window_secs == 0 {
            return Err(RollupError(
                "recent_window_secs must be greater than zero".into(),
            ));
        }
        if medium_window_secs < recent_window_secs {
            return Err(RollupError(
                "medium_window_secs must be at least recent_window_secs".into(),
            ));
        }
        if long_window_secs < medium_window_secs {
            return Err(RollupError(
                "long_window_secs must be at least medium_window_secs".into(),
            ));
        }
        Ok(Self {
            recent_window_secs,
            medium_window_secs,
            long_window_secs,
        })
    }

    pub fn recent_window_secs(&self) -> u64 {
        self.recent_window_secs
    }

    pub fn medium_window_secs(&self) -> u64 {
        self.medium_window_secs
    }

    pub fn long_window_secs(&self) -> u64 {
        self.long_window_secs
    }
}

/// How many source rows [`AnalyticalMemory::compact`] promoted out of each tier - observability
/// only, not itself a safety property (unlike `crate::ledger::CompactionSummary`, nothing here
/// needs a tamper-evident digest: a rollup statistic carries no audit obligation the way a
/// mutation event does).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompactionReport {
    pub raw_samples_promoted: u64,
    pub hourly_rollups_promoted: u64,
    pub daily_rollups_promoted: u64,
}

/// Running count/sum/min/max, combinable either from individual readings or from other
/// already-aggregated statistics - the same associative combination either way, which is what
/// makes cascading multiple promotions in one `compact` call (this module's own doc) correct:
/// summing sums and taking the min-of-mins/max-of-maxes over a re-grouping never loses precision
/// relative to aggregating the original readings directly.
#[derive(Debug, Clone, Copy)]
struct Accumulator {
    count: u64,
    sum: f64,
    min: f64,
    max: f64,
}

impl Accumulator {
    fn from_value(value: f64) -> Self {
        Self {
            count: 1,
            sum: value,
            min: value,
            max: value,
        }
    }

    fn observe_value(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }

    fn merge_aggregate(&mut self, count: u64, sum: f64, min: f64, max: f64) {
        self.count += count;
        self.sum += sum;
        self.min = self.min.min(min);
        self.max = self.max.max(max);
    }
}

/// `(metric key, provider_id, category, bucket_start)` - the grouping/upsert key for both
/// `hourly_rollups` and `daily_rollups` rows.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BucketKey {
    metric_key: String,
    provider_id: Option<String>,
    category: Option<String>,
    bucket_start: i64,
}

/// Finds and merges into, or inserts, the `table` row (`hourly_rollups` or `daily_rollups`,
/// same column shape) matching `key`. `table` is always one of this module's own two fixed
/// literals, never caller-supplied, so building its SQL with `format!` carries no injection
/// surface.
fn upsert_bucket(
    tx: &Transaction,
    table: &str,
    key: &BucketKey,
    acc: Accumulator,
) -> Result<(), RollupError> {
    let existing: Option<(i64, i64, f64, f64, f64)> = tx
        .query_row(
            &format!(
                "SELECT rollup_id, sample_count, sum_value, min_value, max_value FROM {table} \
                 WHERE metric = ?1 AND provider_id IS ?2 AND category IS ?3 AND bucket_start = ?4"
            ),
            params![
                key.metric_key,
                key.provider_id,
                key.category,
                key.bucket_start
            ],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;

    match existing {
        Some((rollup_id, count, sum, min, max)) => {
            let existing_count = u64::try_from(count).unwrap_or(0);
            let mut merged = Accumulator {
                count: existing_count,
                sum,
                min,
                max,
            };
            merged.merge_aggregate(acc.count, acc.sum, acc.min, acc.max);
            tx.execute(
                &format!(
                    "UPDATE {table} SET sample_count = ?1, sum_value = ?2, min_value = ?3, \
                     max_value = ?4 WHERE rollup_id = ?5"
                ),
                params![
                    to_i64(merged.count, "sample_count")?,
                    merged.sum,
                    merged.min,
                    merged.max,
                    rollup_id,
                ],
            )?;
        }
        None => {
            tx.execute(
                &format!(
                    "INSERT INTO {table} \
                        (metric, provider_id, category, bucket_start, sample_count, sum_value, \
                         min_value, max_value) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
                ),
                params![
                    key.metric_key,
                    key.provider_id,
                    key.category,
                    key.bucket_start,
                    to_i64(acc.count, "sample_count")?,
                    acc.sum,
                    acc.min,
                    acc.max,
                ],
            )?;
        }
    }
    Ok(())
}

/// Promotes every `raw_samples` row with `recorded_at <= now - policy.recent_window_secs`
/// (inclusive: a sample exactly at the boundary has left the recent window) into `hourly_
/// rollups`, grouped by `(metric, provider_id, category, hour_bucket(recorded_at))`. Returns the
/// number of raw samples promoted.
fn promote_raw_to_hourly(
    tx: &Transaction,
    policy: &RetentionPolicy,
    now: u64,
) -> Result<u64, RollupError> {
    let cutoff = now.saturating_sub(policy.recent_window_secs);
    let cutoff_i64 = to_i64(cutoff, "recent-window cutoff")?;

    let mut groups: BTreeMap<BucketKey, Accumulator> = BTreeMap::new();
    {
        let mut stmt = tx.prepare(
            "SELECT metric, provider_id, category, recorded_at, value FROM raw_samples \
             WHERE recorded_at <= ?1",
        )?;
        let rows = stmt.query_map(params![cutoff_i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, f64>(4)?,
            ))
        })?;
        for row in rows {
            let (metric_key, provider_id, category, recorded_at, value) = row?;
            let bucket_start =
                i64::try_from(hour_bucket_start(u64::try_from(recorded_at).unwrap_or(0)))
                    .unwrap_or(recorded_at);
            let key = BucketKey {
                metric_key,
                provider_id,
                category,
                bucket_start,
            };
            groups
                .entry(key)
                .and_modify(|acc| acc.observe_value(value))
                .or_insert_with(|| Accumulator::from_value(value));
        }
    }

    for (key, acc) in &groups {
        upsert_bucket(tx, "hourly_rollups", key, *acc)?;
    }

    let deleted = tx.execute(
        "DELETE FROM raw_samples WHERE recorded_at <= ?1",
        params![cutoff_i64],
    )?;
    Ok(u64::try_from(deleted).unwrap_or(0))
}

/// Promotes every `hourly_rollups` row with `bucket_start <= now - policy.medium_window_secs`
/// into `daily_rollups`, grouped by `(metric, provider_id, category, day_bucket(bucket_start))`.
/// Returns the number of hourly rollups promoted.
fn promote_hourly_to_daily(
    tx: &Transaction,
    policy: &RetentionPolicy,
    now: u64,
) -> Result<u64, RollupError> {
    let cutoff = now.saturating_sub(policy.medium_window_secs);
    let cutoff_i64 = to_i64(cutoff, "medium-window cutoff")?;

    let mut groups: BTreeMap<BucketKey, Accumulator> = BTreeMap::new();
    {
        let mut stmt = tx.prepare(
            "SELECT metric, provider_id, category, bucket_start, sample_count, sum_value, \
                    min_value, max_value \
             FROM hourly_rollups WHERE bucket_start <= ?1",
        )?;
        let rows = stmt.query_map(params![cutoff_i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, f64>(7)?,
            ))
        })?;
        for row in rows {
            let (metric_key, provider_id, category, bucket_start, count, sum, min, max) = row?;
            let day_start =
                i64::try_from(day_bucket_start(u64::try_from(bucket_start).unwrap_or(0)))
                    .unwrap_or(bucket_start);
            let key = BucketKey {
                metric_key,
                provider_id,
                category,
                bucket_start: day_start,
            };
            let count = u64::try_from(count).unwrap_or(0);
            groups
                .entry(key)
                .and_modify(|acc| acc.merge_aggregate(count, sum, min, max))
                .or_insert(Accumulator {
                    count,
                    sum,
                    min,
                    max,
                });
        }
    }

    for (key, acc) in &groups {
        upsert_bucket(tx, "daily_rollups", key, *acc)?;
    }

    let deleted = tx.execute(
        "DELETE FROM hourly_rollups WHERE bucket_start <= ?1",
        params![cutoff_i64],
    )?;
    Ok(u64::try_from(deleted).unwrap_or(0))
}

/// `(metric key, provider_id, category)` - the grouping/upsert key for `long_term_aggregates`,
/// which carries no further time bucket.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ScopeKey {
    metric_key: String,
    provider_id: Option<String>,
    category: Option<String>,
}

/// An [`Accumulator`] plus the earliest/latest bucket it was built from - `long_term_aggregates`'
/// own extra pair of columns beyond the shared count/sum/min/max shape.
#[derive(Debug, Clone, Copy)]
struct LongTermAccumulator {
    acc: Accumulator,
    first_bucket_start: i64,
    last_bucket_start: i64,
}

/// Promotes every `daily_rollups` row with `bucket_start <= now - policy.long_window_secs` into
/// `long_term_aggregates`, grouped by `(metric, provider_id, category)` only - no time bucket
/// survives beyond the long window (`PERSISTENCE_MODEL.md`: "beyond long window: bounded
/// statistics/tombstone aggregates"). Returns the number of daily rollups promoted.
fn promote_daily_to_long_term(
    tx: &Transaction,
    policy: &RetentionPolicy,
    now: u64,
) -> Result<u64, RollupError> {
    let cutoff = now.saturating_sub(policy.long_window_secs);
    let cutoff_i64 = to_i64(cutoff, "long-window cutoff")?;

    let mut groups: BTreeMap<ScopeKey, LongTermAccumulator> = BTreeMap::new();
    {
        let mut stmt = tx.prepare(
            "SELECT metric, provider_id, category, bucket_start, sample_count, sum_value, \
                    min_value, max_value \
             FROM daily_rollups WHERE bucket_start <= ?1",
        )?;
        let rows = stmt.query_map(params![cutoff_i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, f64>(7)?,
            ))
        })?;
        for row in rows {
            let (metric_key, provider_id, category, bucket_start, count, sum, min, max) = row?;
            let count = u64::try_from(count).unwrap_or(0);
            let key = ScopeKey {
                metric_key,
                provider_id,
                category,
            };
            groups
                .entry(key)
                .and_modify(|entry| {
                    entry.acc.merge_aggregate(count, sum, min, max);
                    entry.first_bucket_start = entry.first_bucket_start.min(bucket_start);
                    entry.last_bucket_start = entry.last_bucket_start.max(bucket_start);
                })
                .or_insert(LongTermAccumulator {
                    acc: Accumulator {
                        count,
                        sum,
                        min,
                        max,
                    },
                    first_bucket_start: bucket_start,
                    last_bucket_start: bucket_start,
                });
        }
    }

    for (key, entry) in &groups {
        let existing: Option<(i64, i64, f64, f64, f64, i64, i64)> = tx
            .query_row(
                "SELECT aggregate_id, sample_count, sum_value, min_value, max_value, \
                        first_bucket_start, last_bucket_start \
                 FROM long_term_aggregates \
                 WHERE metric = ?1 AND provider_id IS ?2 AND category IS ?3",
                params![key.metric_key, key.provider_id, key.category],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()?;

        match existing {
            Some((aggregate_id, count, sum, min, max, first, last)) => {
                let mut merged = Accumulator {
                    count: u64::try_from(count).unwrap_or(0),
                    sum,
                    min,
                    max,
                };
                merged.merge_aggregate(
                    entry.acc.count,
                    entry.acc.sum,
                    entry.acc.min,
                    entry.acc.max,
                );
                let first = first.min(entry.first_bucket_start);
                let last = last.max(entry.last_bucket_start);
                tx.execute(
                    "UPDATE long_term_aggregates SET sample_count = ?1, sum_value = ?2, \
                        min_value = ?3, max_value = ?4, first_bucket_start = ?5, \
                        last_bucket_start = ?6 \
                     WHERE aggregate_id = ?7",
                    params![
                        to_i64(merged.count, "sample_count")?,
                        merged.sum,
                        merged.min,
                        merged.max,
                        first,
                        last,
                        aggregate_id,
                    ],
                )?;
            }
            None => {
                tx.execute(
                    "INSERT INTO long_term_aggregates \
                        (metric, provider_id, category, sample_count, sum_value, min_value, \
                         max_value, first_bucket_start, last_bucket_start) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        key.metric_key,
                        key.provider_id,
                        key.category,
                        to_i64(entry.acc.count, "sample_count")?,
                        entry.acc.sum,
                        entry.acc.min,
                        entry.acc.max,
                        entry.first_bucket_start,
                        entry.last_bucket_start,
                    ],
                )?;
            }
        }
    }

    let deleted = tx.execute(
        "DELETE FROM daily_rollups WHERE bucket_start <= ?1",
        params![cutoff_i64],
    )?;
    Ok(u64::try_from(deleted).unwrap_or(0))
}

/// Layer 3's analytical memory: fine-grained sample ingestion plus the hourly/daily/long-term
/// rollup cascade (this module's own doc). Holds one open connection for its lifetime, matching
/// [`crate::CurrentStateStore`]'s and [`crate::ledger::EventLedger`]'s own single-owner design.
pub struct AnalyticalMemory {
    conn: Connection,
}

impl AnalyticalMemory {
    /// Opens (creating if absent) the analytical-memory database at `path`, applying any
    /// migration this database has not already seen. Use a path distinct from
    /// [`crate::CurrentStateStore::open`]'s and [`crate::ledger::EventLedger::open`]'s.
    pub fn open(path: &Path) -> Result<Self, RollupError> {
        let conn = Connection::open(path)?;
        apply_migrations(&conn, MIGRATIONS)?;
        Ok(Self { conn })
    }

    /// An in-memory store for tests and short-lived callers that never need a file on disk.
    pub fn open_in_memory() -> Result<Self, RollupError> {
        let conn = Connection::open_in_memory()?;
        apply_migrations(&conn, MIGRATIONS)?;
        Ok(Self { conn })
    }

    /// Records one fine-grained measurement. Never expires anything itself - expiry only
    /// happens through an explicit [`AnalyticalMemory::compact`] call (this module's own doc).
    pub fn record_sample(&mut self, sample: NewSample) -> Result<(), RollupError> {
        let recorded_at = to_i64(sample.recorded_at, "recorded_at")?;
        self.conn.execute(
            "INSERT INTO raw_samples (metric, recorded_at, provider_id, category, value) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                sample.metric.key(),
                recorded_at,
                sample.scope.provider_id,
                sample.scope.category,
                sample.value,
            ],
        )?;
        Ok(())
    }

    /// Runs the full raw -> hourly -> daily -> long-term promotion cascade against `now`, in one
    /// transaction (this module's own doc, "Compaction"). A failure at any point rolls back the
    /// entire call - no tier is left partially promoted.
    pub fn compact(
        &mut self,
        policy: &RetentionPolicy,
        now: u64,
    ) -> Result<CompactionReport, RollupError> {
        let tx = self.conn.transaction()?;
        let raw_samples_promoted = promote_raw_to_hourly(&tx, policy, now)?;
        let hourly_rollups_promoted = promote_hourly_to_daily(&tx, policy, now)?;
        let daily_rollups_promoted = promote_daily_to_long_term(&tx, policy, now)?;
        tx.commit()?;
        Ok(CompactionReport {
            raw_samples_promoted,
            hourly_rollups_promoted,
            daily_rollups_promoted,
        })
    }

    /// Every raw sample this store currently holds, ordered by `sample_id`.
    pub fn raw_samples(&self) -> Result<Vec<Sample>, RollupError> {
        let mut stmt = self.conn.prepare(
            "SELECT sample_id, metric, recorded_at, provider_id, category, value \
             FROM raw_samples ORDER BY sample_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, f64>(5)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (sample_id, metric, recorded_at, provider_id, category, value) = row?;
            result.push(Sample {
                sample_id,
                metric: MetricKind::from_key(&metric)?,
                recorded_at: u64::try_from(recorded_at).unwrap_or(0),
                scope: SampleScope {
                    provider_id,
                    category,
                },
                value,
            });
        }
        Ok(result)
    }

    fn read_rollups(&self, table: &str) -> Result<Vec<Rollup>, RollupError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT rollup_id, metric, provider_id, category, bucket_start, sample_count, \
                    sum_value, min_value, max_value \
             FROM {table} ORDER BY rollup_id"
        ))?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, f64>(7)?,
                row.get::<_, f64>(8)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (
                rollup_id,
                metric,
                provider_id,
                category,
                bucket_start,
                sample_count,
                sum_value,
                min_value,
                max_value,
            ) = row?;
            result.push(Rollup {
                rollup_id,
                metric: MetricKind::from_key(&metric)?,
                scope: SampleScope {
                    provider_id,
                    category,
                },
                bucket_start: u64::try_from(bucket_start).unwrap_or(0),
                sample_count: u64::try_from(sample_count).unwrap_or(0),
                sum_value,
                min_value,
                max_value,
            });
        }
        Ok(result)
    }

    /// Every hourly rollup this store currently holds, ordered by `rollup_id`.
    pub fn hourly_rollups(&self) -> Result<Vec<Rollup>, RollupError> {
        self.read_rollups("hourly_rollups")
    }

    /// Every daily rollup this store currently holds, ordered by `rollup_id`.
    pub fn daily_rollups(&self) -> Result<Vec<Rollup>, RollupError> {
        self.read_rollups("daily_rollups")
    }

    /// Every long-term aggregate this store currently holds, ordered by `aggregate_id`.
    pub fn long_term_aggregates(&self) -> Result<Vec<LongTermAggregate>, RollupError> {
        let mut stmt = self.conn.prepare(
            "SELECT aggregate_id, metric, provider_id, category, sample_count, sum_value, \
                    min_value, max_value, first_bucket_start, last_bucket_start \
             FROM long_term_aggregates ORDER BY aggregate_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, f64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (
                aggregate_id,
                metric,
                provider_id,
                category,
                sample_count,
                sum_value,
                min_value,
                max_value,
                first_bucket_start,
                last_bucket_start,
            ) = row?;
            result.push(LongTermAggregate {
                aggregate_id,
                metric: MetricKind::from_key(&metric)?,
                scope: SampleScope {
                    provider_id,
                    category,
                },
                sample_count: u64::try_from(sample_count).unwrap_or(0),
                sum_value,
                min_value,
                max_value,
                first_bucket_start: u64::try_from(first_bucket_start).unwrap_or(0),
                last_bucket_start: u64::try_from(last_bucket_start).unwrap_or(0),
            });
        }
        Ok(result)
    }

    /// Test-only, crate-visible raw access to the underlying connection - used to inspect the
    /// actual schema directly, independent of whatever read methods this type happens to expose
    /// today. Never part of the public API, matching `crate::ledger::EventLedger::raw_conn`'s
    /// own precedent.
    #[cfg(test)]
    fn raw_conn(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(metric: MetricKind, recorded_at: u64, value: f64) -> NewSample {
        NewSample {
            metric,
            recorded_at,
            scope: SampleScope {
                provider_id: Some("codex".to_string()),
                category: None,
            },
            value,
        }
    }

    #[test]
    fn record_sample_then_raw_samples_round_trips_every_field() {
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        memory
            .record_sample(sample(MetricKind::ArtifactCount, 1_000, 42.0))
            .expect("record");

        let samples = memory.raw_samples().expect("raw_samples");
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].metric, MetricKind::ArtifactCount);
        assert_eq!(samples[0].recorded_at, 1_000);
        assert_eq!(samples[0].value, 42.0);
        assert_eq!(samples[0].scope.provider_id.as_deref(), Some("codex"));
        assert_eq!(samples[0].scope.category, None);
    }

    #[test]
    fn compact_with_no_samples_is_a_no_op() {
        // Falsifier: a rollup that should aggregate zero samples (an empty window) must not
        // fabricate a row or error.
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        let report = memory
            .compact(&RetentionPolicy::DEFAULT, 1_000_000)
            .expect("compact");
        assert_eq!(report, CompactionReport::default());
        assert!(memory.raw_samples().expect("raw_samples").is_empty());
        assert!(memory.hourly_rollups().expect("hourly_rollups").is_empty());
        assert!(memory.daily_rollups().expect("daily_rollups").is_empty());
        assert!(
            memory
                .long_term_aggregates()
                .expect("long_term_aggregates")
                .is_empty()
        );
    }

    #[test]
    fn a_sample_still_inside_the_recent_window_is_not_promoted() {
        let policy = RetentionPolicy::new(100, 200, 300).expect("policy");
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        // now=1000, recorded at 901: age is 99, one second short of the 100s recent window.
        memory
            .record_sample(sample(MetricKind::ArtifactCount, 901, 5.0))
            .expect("record");

        memory.compact(&policy, 1_000).expect("compact");

        assert_eq!(memory.raw_samples().expect("raw_samples").len(), 1);
        assert!(memory.hourly_rollups().expect("hourly_rollups").is_empty());
    }

    #[test]
    fn a_sample_exactly_at_the_recent_medium_boundary_is_promoted() {
        // Falsifier: a sample exactly at the recent-window boundary (age == recent_window_secs)
        // must be treated as having left the recent window (inclusive), not retained one more
        // compaction cycle by an off-by-one. `now` is anchored away from the epoch and the
        // medium/long windows are wide relative to recent, so the promoted sample lands in
        // `hourly_rollups` and is observable there rather than cascading straight through it.
        let now = 864_000u64; // 10 days since the epoch
        let policy = RetentionPolicy::new(7_200, 259_200, 2_592_000).expect("policy");
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        // recorded at now - recent_window_secs exactly: age is exactly the recent window's own
        // length.
        memory
            .record_sample(sample(MetricKind::ArtifactCount, now - 7_200, 5.0))
            .expect("record");

        memory.compact(&policy, now).expect("compact");

        assert!(
            memory.raw_samples().expect("raw_samples").is_empty(),
            "a sample exactly at the boundary must be promoted out of the recent window"
        );
        let hourly = memory.hourly_rollups().expect("hourly_rollups");
        assert_eq!(hourly.len(), 1);
        assert_eq!(hourly[0].sample_count, 1);
        assert_eq!(hourly[0].sum_value, 5.0);
    }

    #[test]
    fn raw_samples_in_the_same_hour_aggregate_into_one_bounded_hourly_rollup() {
        let now = 864_000u64;
        let policy = RetentionPolicy::new(10, 259_200, 2_592_000).expect("policy");
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        // All three fall in the same hour bucket (860_400) and are all old enough to leave the
        // ten-second recent window, but not old enough to leave the three-day medium window.
        for (recorded_at, value) in [(860_400u64, 10.0), (861_000, 20.0), (863_990, 30.0)] {
            memory
                .record_sample(sample(MetricKind::ArtifactCount, recorded_at, value))
                .expect("record");
        }

        let report = memory.compact(&policy, now).expect("compact");
        assert_eq!(report.raw_samples_promoted, 3);

        let hourly = memory.hourly_rollups().expect("hourly_rollups");
        assert_eq!(
            hourly.len(),
            1,
            "all three samples fall in hour bucket 860_400"
        );
        assert_eq!(hourly[0].bucket_start, 860_400);
        assert_eq!(hourly[0].sample_count, 3);
        assert_eq!(hourly[0].sum_value, 60.0);
        assert_eq!(hourly[0].min_value, 10.0);
        assert_eq!(hourly[0].max_value, 30.0);
        assert!(
            memory.raw_samples().expect("raw_samples").is_empty(),
            "promoted raw samples must be removed - the aggregate is what survives, not the \
             individual readings"
        );
    }

    #[test]
    fn compacting_twice_over_the_same_data_is_idempotent() {
        // Falsifier: a rollup run twice over an unchanged database must not double the count/
        // sum, because the source rows it already promoted are gone and its own upsert only
        // acts on what is still eligible.
        let policy = RetentionPolicy::new(10, 20, 30).expect("policy");
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        memory
            .record_sample(sample(MetricKind::ArtifactCount, 0, 7.0))
            .expect("record");

        let first = memory.compact(&policy, 1_000).expect("first compact");
        let long_term_after_first = memory.long_term_aggregates().expect("long_term_aggregates");

        let second = memory.compact(&policy, 1_000).expect("second compact");
        let long_term_after_second = memory.long_term_aggregates().expect("long_term_aggregates");

        assert_eq!(first.raw_samples_promoted, 1);
        assert_eq!(
            second,
            CompactionReport::default(),
            "nothing left to promote"
        );
        assert_eq!(
            long_term_after_first.len(),
            1,
            "the tiny windows here cascade the sample all the way to long_term_aggregates"
        );
        assert_eq!(
            long_term_after_first[0].sample_count, 1,
            "must not be pre-doubled before the second compact even runs"
        );
        assert_eq!(
            long_term_after_first, long_term_after_second,
            "a repeated compact over unchanged data must not alter the aggregate, e.g. by \
             double-counting an already-promoted daily rollup"
        );
    }

    #[test]
    fn a_late_arriving_backdated_sample_lands_in_its_historical_bucket_not_the_current_one() {
        // Falsifier: clock skew (a sample recorded with an old timestamp, inserted after a more
        // recent one) must not corrupt aggregation order - it must group by its own recorded_at,
        // not by insertion order or by whatever bucket is "current" at the time it arrives.
        let now = 864_000u64;
        let policy = RetentionPolicy::new(10, 259_200, 2_592_000).expect("policy");
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");

        // Insert a "future-looking" sample (hour bucket 860_400, closer to `now`) first, then a
        // backdated one (hour bucket 856_800, an hour earlier) - reversed from chronological
        // order. Both are old enough to leave the ten-second recent window but not the three-day
        // medium window, so both remain observable as distinct hourly rollups.
        memory
            .record_sample(sample(MetricKind::ArtifactCount, 863_990, 100.0))
            .expect("record future-looking sample first");
        memory
            .record_sample(sample(MetricKind::ArtifactCount, 860_390, 1.0))
            .expect("record backdated sample second");

        memory.compact(&policy, now).expect("compact");

        let mut hourly = memory.hourly_rollups().expect("hourly_rollups");
        hourly.sort_by_key(|r| r.bucket_start);
        assert_eq!(
            hourly.len(),
            2,
            "each sample lands in its own distinct hour bucket"
        );
        assert_eq!(hourly[0].bucket_start, 856_800);
        assert_eq!(hourly[0].sum_value, 1.0);
        assert_eq!(hourly[1].bucket_start, 860_400);
        assert_eq!(hourly[1].sum_value, 100.0);
    }

    #[test]
    fn jumping_past_every_window_in_one_compact_call_still_reaches_long_term_without_skipping() {
        // Falsifier: advancing `now` past all three window boundaries in a single `compact` call
        // must cascade the sample all the way to `long_term_aggregates`, not strand it in an
        // intermediate tier because only one promotion ran.
        let policy = RetentionPolicy::new(10, 20, 30).expect("policy");
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        memory
            .record_sample(sample(MetricKind::ReclaimableBytes, 0, 512.0))
            .expect("record");

        let report = memory.compact(&policy, 1_000_000).expect("compact");

        assert_eq!(report.raw_samples_promoted, 1);
        assert_eq!(report.hourly_rollups_promoted, 1);
        assert_eq!(report.daily_rollups_promoted, 1);
        assert!(memory.raw_samples().expect("raw_samples").is_empty());
        assert!(memory.hourly_rollups().expect("hourly_rollups").is_empty());
        assert!(memory.daily_rollups().expect("daily_rollups").is_empty());

        let long_term = memory.long_term_aggregates().expect("long_term_aggregates");
        assert_eq!(long_term.len(), 1);
        assert_eq!(long_term[0].metric, MetricKind::ReclaimableBytes);
        assert_eq!(long_term[0].sample_count, 1);
        assert_eq!(long_term[0].sum_value, 512.0);
        assert_eq!(long_term[0].min_value, 512.0);
        assert_eq!(long_term[0].max_value, 512.0);
    }

    #[test]
    fn long_term_aggregates_keep_growing_bounded_as_more_daily_rollups_are_promoted_into_them() {
        // A second, later batch of data promoted into an existing long-term aggregate must
        // extend it (merge), not create a second competing row for the same (metric, scope).
        let policy = RetentionPolicy::new(10, 20, 30).expect("policy");
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        memory
            .record_sample(sample(MetricKind::ArtifactCount, 0, 3.0))
            .expect("record first");
        memory.compact(&policy, 1_000_000).expect("first compact");

        memory
            .record_sample(sample(MetricKind::ArtifactCount, 1_000_000, 9.0))
            .expect("record second");
        memory.compact(&policy, 2_000_000).expect("second compact");

        let long_term = memory.long_term_aggregates().expect("long_term_aggregates");
        assert_eq!(
            long_term.len(),
            1,
            "the same (metric, scope) must extend one row, not create a second"
        );
        assert_eq!(long_term[0].sample_count, 2);
        assert_eq!(long_term[0].sum_value, 12.0);
        assert_eq!(long_term[0].min_value, 3.0);
        assert_eq!(long_term[0].max_value, 9.0);
    }

    #[test]
    fn distinct_scopes_never_merge_into_each_other() {
        let policy = RetentionPolicy::new(10, 20, 30).expect("policy");
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        memory
            .record_sample(NewSample {
                metric: MetricKind::ArtifactCount,
                recorded_at: 0,
                scope: SampleScope {
                    provider_id: Some("codex".to_string()),
                    category: None,
                },
                value: 1.0,
            })
            .expect("record codex sample");
        memory
            .record_sample(NewSample {
                metric: MetricKind::ArtifactCount,
                recorded_at: 0,
                scope: SampleScope {
                    provider_id: Some("claude".to_string()),
                    category: None,
                },
                value: 2.0,
            })
            .expect("record claude sample");

        memory.compact(&policy, 1_000_000).expect("compact");

        let long_term = memory.long_term_aggregates().expect("long_term_aggregates");
        assert_eq!(
            long_term.len(),
            2,
            "distinct provider scopes stay distinct rows"
        );
    }

    #[test]
    fn an_unscoped_sample_with_no_provider_or_category_still_aggregates_correctly() {
        // Falsifier: SQL UNIQUE indexes treat every NULL as distinct from every other NULL,
        // which would silently create a new row per unscoped sample instead of aggregating them
        // - this crate does its own `IS`-based upsert lookup specifically to avoid that trap.
        let now = 864_000u64;
        let policy = RetentionPolicy::new(10, 259_200, 2_592_000).expect("policy");
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        for recorded_at in [860_400u64, 860_500, 860_600] {
            memory
                .record_sample(NewSample {
                    metric: MetricKind::ArtifactCount,
                    recorded_at,
                    scope: SampleScope::default(),
                    value: 1.0,
                })
                .expect("record unscoped sample");
        }

        memory.compact(&policy, now).expect("compact");

        let hourly = memory.hourly_rollups().expect("hourly_rollups");
        assert_eq!(
            hourly.len(),
            1,
            "unscoped samples in the same bucket must merge into one row"
        );
        assert_eq!(hourly[0].sample_count, 3);
    }

    #[test]
    fn retention_policy_rejects_windows_that_are_not_non_decreasing() {
        assert!(
            RetentionPolicy::new(0, 10, 10).is_err(),
            "zero recent window"
        );
        assert!(RetentionPolicy::new(10, 5, 10).is_err(), "medium < recent");
        assert!(RetentionPolicy::new(10, 10, 5).is_err(), "long < medium");
        assert!(
            RetentionPolicy::new(10, 10, 10).is_ok(),
            "equal windows are legal"
        );
    }

    #[test]
    fn rollup_tables_have_only_the_allowlisted_columns() {
        // Pins the actual table shape of all four tiers against the closed, contentless field
        // set this module's own doc promises - a future change that adds e.g. a `path` or
        // `note` column fails this test rather than drifting in silently.
        let memory = AnalyticalMemory::open_in_memory().expect("open");
        let columns_of = |table: &str| -> Vec<String> {
            let mut stmt = memory
                .raw_conn()
                .prepare(&format!("PRAGMA table_info({table})"))
                .expect("prepare");
            stmt.query_map([], |row| row.get::<_, String>(1))
                .expect("query")
                .collect::<Result<_, _>>()
                .expect("collect")
        };

        assert_eq!(
            columns_of("raw_samples"),
            vec![
                "sample_id",
                "metric",
                "recorded_at",
                "provider_id",
                "category",
                "value"
            ]
        );
        for table in ["hourly_rollups", "daily_rollups"] {
            assert_eq!(
                columns_of(table),
                vec![
                    "rollup_id",
                    "metric",
                    "provider_id",
                    "category",
                    "bucket_start",
                    "sample_count",
                    "sum_value",
                    "min_value",
                    "max_value",
                ],
                "table {table} must carry only the allowlisted columns"
            );
        }
        assert_eq!(
            columns_of("long_term_aggregates"),
            vec![
                "aggregate_id",
                "metric",
                "provider_id",
                "category",
                "sample_count",
                "sum_value",
                "min_value",
                "max_value",
                "first_bucket_start",
                "last_bucket_start",
            ]
        );
    }

    #[test]
    fn a_metric_name_cannot_carry_arbitrary_content_because_the_type_does_not_admit_one() {
        // Falsifier: "an aggregate must remain contentless even if the caller tries to pass
        // sensitive data" - MetricKind is a closed enum, so a caller attempting to smuggle e.g.
        // a real filesystem path as the metric cannot do so at all; from_key rejects anything
        // outside the four known keys rather than accepting it as opaque passthrough content.
        assert!(
            MetricKind::from_key("/Users/example/.claude/projects/secret-transcript.jsonl")
                .is_err()
        );
        assert!(MetricKind::from_key("ARTIFACT_COUNT").is_ok());
    }

    #[test]
    fn malformed_stored_metric_is_reported_as_an_error_not_a_panic() {
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        memory
            .record_sample(sample(MetricKind::ArtifactCount, 0, 1.0))
            .expect("record");
        memory
            .raw_conn()
            .execute(
                "UPDATE raw_samples SET metric = 'NOT_A_REAL_METRIC' WHERE sample_id = 1",
                [],
            )
            .expect("corrupt the stored row directly");

        assert!(
            memory.raw_samples().is_err(),
            "a corrupted metric column must surface as an error from raw_samples, not a panic"
        );
    }

    #[test]
    fn reopen_after_close_preserves_every_tier() {
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-rollup-test-reopen-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let db_path = dir.join("rollup.sqlite3");
        let policy = RetentionPolicy::new(10, 20, 30).expect("policy");

        {
            let mut memory = AnalyticalMemory::open(&db_path).expect("first open");
            memory
                .record_sample(sample(MetricKind::ArtifactCount, 0, 1.0))
                .expect("record");
            memory.compact(&policy, 1_000_000).expect("compact");
        }
        {
            let memory = AnalyticalMemory::open(&db_path).expect("reopen must not fail");
            let long_term = memory
                .long_term_aggregates()
                .expect("long_term_aggregates after reopen");
            assert_eq!(long_term.len(), 1, "reopening must preserve promoted data");
        }

        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }
}

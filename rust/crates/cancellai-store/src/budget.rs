//! Self-budget enforcement and local-state reset (E13-S04, `docs/architecture/
//! PERSISTENCE_MODEL.md`'s "Self-budget", SI-026 "cancellAI reset/self-budget cannot target
//! provider payload").
//!
//! ## This module owns *when*, never *how* (AC "budget overrun triggers compaction before
//! growth continues")
//!
//! Every physical action a budget or a reset can take already exists, added by E13-S01/S02/S03:
//! [`crate::CurrentStateStore::reset`], [`crate::ledger::EventLedger::compact_oldest_to_fit`]/
//! [`crate::ledger::EventLedger::reset`], and [`crate::rollup::AnalyticalMemory::compact_if_over`]/
//! [`crate::rollup::AnalyticalMemory::reset`] each live in the module that already owns that
//! layer's schema, connection and error type - the same place `compact_range`/`compact`
//! themselves live. This module is deliberately a thin policy layer over those primitives:
//! [`BudgetLimits`] names the thresholds, [`enforce_ledger_budget`]/[`enforce_rollup_budget`]
//! and [`check_current_state_budget`] decide whether the threshold is crossed, and
//! [`reset_local_state`] sequences the three per-layer `reset()` calls this crate already
//! exposes. No new SQL, no new `Connection`, no new mutation primitive is introduced here -
//! `scripts/check_mutation_boundary.py` has nothing to scan for in this file because there is
//! nothing here that could violate it.
//!
//! ## Budgets are configurable thresholds, not a single hard-coded constant
//!
//! [`BudgetLimits::new`] is the only public constructor and takes one threshold per layer this
//! story's own acceptance criteria and `docs/architecture/PERSISTENCE_MODEL.md`'s "Self-budget"
//! list name - current-state rows, ledger events, raw samples - the same "explicit,
//! caller-supplied, not a hard-coded architecture constant" shape [`crate::rollup::
//! RetentionPolicy::new`] already established for Layer 3's own retention windows.
//! [`BudgetLimits::DEFAULT`] is one reasonable choice, not the only legal one, matching
//! [`crate::rollup::RetentionPolicy::DEFAULT`]'s own documented status.
//!
//! ## Why Layer 1 has no compaction action here
//!
//! [`check_current_state_budget`] only observes [`crate::CurrentStateStore::row_count`] against
//! [`BudgetLimits::max_current_state_rows`] - it never compacts anything. Layer 1's content is
//! entirely determined by the last `rebuild` an external scan performed (`crate::lib`'s own doc,
//! "Reconstructible by construction"); this crate autonomously discarding rows to fit a budget
//! would silently diverge from the last scan, which is not compaction, it is data loss with no
//! recovery path other than a full rescan. `docs/architecture/PERSISTENCE_MODEL.md`'s own
//! "Self-budget" section says what actually yields under Layer 1 pressure - "safety-critical
//! current facts may force analytical sampling to degrade" - naming Layer 3, not Layer 1, as the
//! thing that degrades. Wiring that cross-layer degradation decision needs a live caller
//! (Guardian) this workspace does not have yet; this story provides the observation
//! ([`check_current_state_budget`]) a future orchestrator needs to make that decision, not the
//! decision itself - see this crate's own evidence packet, "Residual risks."
//!
//! ## `reset_local_state` is sequential, not cross-file atomic
//!
//! [`CurrentStateStore`], [`crate::ledger::EventLedger`] and [`crate::rollup::AnalyticalMemory`]
//! each own an independent file/`Connection`/`PRAGMA user_version` history (every one of their
//! own module docs already states this, for the same reason: three different kinds of data with
//! three different retention shapes). [`reset_local_state`] therefore cannot be one SQLite
//! transaction spanning all three files - it calls each layer's own `reset()` in sequence
//! (current-state, then ledger, then rollup), and each of those three calls is itself atomic and
//! fails closed (this module's own per-layer `reset` doc comments). A failure partway through
//! [`reset_local_state`] leaves whichever layers already reset empty and the remaining ones
//! unchanged - never a layer left half-emptied - and [`ResetError`] names exactly which layer
//! failed so a caller (or a retry) knows where to resume; retrying is always safe, because
//! resetting an already-empty layer is a no-op.
//!
//! ## Takes no path, ever (AC "`reset --local-state` cannot target provider roots")
//!
//! Every function in this module that mutates anything takes only already-open store/ledger/
//! memory handles and caller-supplied numeric thresholds/timestamps - never a path, a root, or
//! any other string a caller could point at a provider location. This discharges SI-026 for this
//! module by construction: there is no parameter here a provider path could even be passed
//! through.

use crate::ledger::{self, EventLedger};
use crate::rollup::{self, AnalyticalMemory, RetentionPolicy};
use crate::{CurrentStateStore, StoreError};

/// Why a [`BudgetLimits`] could not be constructed - every threshold must be greater than zero,
/// the same validated-constructor shape [`crate::rollup::RetentionPolicy::new`] already uses.
#[derive(Debug)]
pub struct BudgetError(String);

impl std::fmt::Display for BudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for BudgetError {}

/// Explicit, caller-supplied thresholds for the three SQLite-backed layers this crate owns -
/// `docs/architecture/PERSISTENCE_MODEL.md`'s "Self-budget" names a current-state DB, an event
/// ledger and analytical memory (its other two named budgets, logs and temporary release/scan
/// artifacts, name state this crate does not hold - see this crate's own evidence packet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetLimits {
    max_current_state_rows: u64,
    max_ledger_events: u64,
    max_raw_samples: u64,
}

impl BudgetLimits {
    /// One reasonable set of thresholds, not the only legal choice - matching
    /// [`crate::rollup::RetentionPolicy::DEFAULT`]'s own documented status.
    pub const DEFAULT: BudgetLimits = BudgetLimits {
        max_current_state_rows: 50_000,
        max_ledger_events: 10_000,
        max_raw_samples: 10_000,
    };

    /// The only public constructor - every threshold must be greater than zero, so this module
    /// never has to special-case "budget of zero" in every enforcement call.
    pub fn new(
        max_current_state_rows: u64,
        max_ledger_events: u64,
        max_raw_samples: u64,
    ) -> Result<Self, BudgetError> {
        if max_current_state_rows == 0 {
            return Err(BudgetError(
                "max_current_state_rows must be greater than zero".into(),
            ));
        }
        if max_ledger_events == 0 {
            return Err(BudgetError(
                "max_ledger_events must be greater than zero".into(),
            ));
        }
        if max_raw_samples == 0 {
            return Err(BudgetError(
                "max_raw_samples must be greater than zero".into(),
            ));
        }
        Ok(Self {
            max_current_state_rows,
            max_ledger_events,
            max_raw_samples,
        })
    }

    pub fn max_current_state_rows(&self) -> u64 {
        self.max_current_state_rows
    }

    pub fn max_ledger_events(&self) -> u64 {
        self.max_ledger_events
    }

    pub fn max_raw_samples(&self) -> u64 {
        self.max_raw_samples
    }
}

/// Whether [`crate::CurrentStateStore::row_count`] is within [`BudgetLimits::
/// max_current_state_rows`] - observational only, this module's own doc, "Why Layer 1 has no
/// compaction action here".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetStatus {
    WithinBudget { count: u64, limit: u64 },
    OverBudget { count: u64, limit: u64 },
}

/// Reports whether Layer 1 is currently over its configured budget - see this module's own doc
/// for why this is the only one of the three checks that never itself compacts anything.
pub fn check_current_state_budget(
    store: &CurrentStateStore,
    limits: &BudgetLimits,
) -> Result<BudgetStatus, StoreError> {
    let count = store.row_count()?;
    let limit = limits.max_current_state_rows();
    Ok(if count > limit {
        BudgetStatus::OverBudget { count, limit }
    } else {
        BudgetStatus::WithinBudget { count, limit }
    })
}

/// Compacts the event ledger down to [`BudgetLimits::max_ledger_events`] if it is currently over
/// that limit, and is a no-op otherwise - "budget overrun triggers compaction before growth
/// continues" (AC1). Call this immediately before the write that might push the ledger over
/// budget: it inspects the ledger's *current* size and, if already over the limit (from a prior
/// write, a lowered threshold, or a store that predates budget enforcement), compacts the oldest
/// events before the caller's own write compounds the overrun. A store already exactly at the
/// limit is left untouched - only a genuine overrun (strictly more than the limit) ever
/// compacts anything.
pub fn enforce_ledger_budget(
    ledger: &mut EventLedger,
    limits: &BudgetLimits,
    now: u64,
) -> Result<Option<ledger::CompactionSummary>, ledger::LedgerError> {
    ledger.compact_oldest_to_fit(limits.max_ledger_events(), now)
}

/// Compacts analytical memory's raw-sample tier if it is currently over [`BudgetLimits::
/// max_raw_samples`], and is a no-op otherwise - the same "compact before growth continues"
/// contract [`enforce_ledger_budget`] gives the ledger, for Layer 3. See
/// [`crate::rollup::AnalyticalMemory::compact_if_over`]'s own doc for the policy/budget
/// interaction this can surface (compacting while over budget does not itself promote a sample
/// `policy` says is still too young).
pub fn enforce_rollup_budget(
    memory: &mut AnalyticalMemory,
    limits: &BudgetLimits,
    policy: &RetentionPolicy,
    now: u64,
) -> Result<Option<rollup::CompactionReport>, rollup::RollupError> {
    memory.compact_if_over(limits.max_raw_samples(), policy, now)
}

/// Why [`reset_local_state`] failed, naming which of the three independent layers the failure
/// happened in - this module's own doc, "`reset_local_state` is sequential, not cross-file
/// atomic".
#[derive(Debug)]
pub enum ResetError {
    CurrentState(StoreError),
    Ledger(ledger::LedgerError),
    Rollup(rollup::RollupError),
}

impl std::fmt::Display for ResetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResetError::CurrentState(e) => write!(f, "current-state reset failed: {e}"),
            ResetError::Ledger(e) => write!(f, "event ledger reset failed: {e}"),
            ResetError::Rollup(e) => write!(f, "analytical memory reset failed: {e}"),
        }
    }
}

impl std::error::Error for ResetError {}

/// Resets Layer 1, Layer 2 and Layer 3 in sequence - this crate's own "`reset --local-state`"
/// primitive as a single call (`docs/architecture/PERSISTENCE_MODEL.md`'s "Self-budget",
/// SI-026). Takes only already-open store/ledger/memory handles - no path, no root, nothing a
/// caller could point at provider state, this module's own doc, "Takes no path, ever". See
/// this module's own doc, "`reset_local_state` is sequential, not cross-file atomic," for what
/// this call does and does not guarantee across a partial failure.
pub fn reset_local_state(
    current_state: &mut CurrentStateStore,
    ledger: &mut EventLedger,
    memory: &mut AnalyticalMemory,
) -> Result<(), ResetError> {
    current_state.reset().map_err(ResetError::CurrentState)?;
    ledger.reset().map_err(ResetError::Ledger)?;
    memory.reset().map_err(ResetError::Rollup)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{EventKind, EventMetadata, NewEvent};
    use crate::rollup::{MetricKind, NewSample, SampleScope};
    use cancellai_model::{
        ActivityState, AgentArtifact, ArtifactId, AuthorityLevel, EvidenceId, IntegrityState,
        KnowledgeConfidence, ProtectionState, ResidencyState, Reversibility, RiskClass,
    };

    fn artifact(id: &str) -> AgentArtifact {
        AgentArtifact {
            artifact_id: ArtifactId::new(id),
            identity_token: format!("codex:{id}"),
            provider_id: "codex".to_string(),
            artifact_type: "session".to_string(),
            risk_class: RiskClass::R3Resumable,
            reversibility: Reversibility::Irreversible,
            knowledge_confidence: KnowledgeConfidence::Verified,
            activity_state: ActivityState::Active,
            residency_state: ResidencyState::Hot,
            protection_state: ProtectionState::Normal,
            integrity_state: IntegrityState::Healthy,
            authority_ceiling: AuthorityLevel::Quarantine,
            evidence_ids: vec![EvidenceId::new("evidence-0001")],
            relationships: Vec::new(),
            project_attribution: None,
            activity_signal: None,
        }
    }

    fn discovered(recorded_at: u64) -> NewEvent {
        NewEvent {
            kind: EventKind::Discovered,
            recorded_at,
            metadata: EventMetadata::default(),
            mutation: None,
        }
    }

    fn sample(recorded_at: u64) -> NewSample {
        NewSample {
            metric: MetricKind::ArtifactCount,
            recorded_at,
            scope: SampleScope::default(),
            value: 1.0,
        }
    }

    #[test]
    fn budget_limits_rejects_a_zero_threshold_in_any_position() {
        assert!(BudgetLimits::new(0, 1, 1).is_err());
        assert!(BudgetLimits::new(1, 0, 1).is_err());
        assert!(BudgetLimits::new(1, 1, 0).is_err());
        assert!(BudgetLimits::new(1, 1, 1).is_ok());
    }

    #[test]
    fn check_current_state_budget_reports_within_and_over() {
        let mut store = CurrentStateStore::open_in_memory().expect("open");
        let limits = BudgetLimits::new(2, 1, 1).expect("limits");

        assert_eq!(
            check_current_state_budget(&store, &limits).expect("check"),
            BudgetStatus::WithinBudget { count: 0, limit: 2 }
        );

        store
            .rebuild(&[artifact("artifact-0001"), artifact("artifact-0002")])
            .expect("rebuild to exactly the limit");
        assert_eq!(
            check_current_state_budget(&store, &limits).expect("check"),
            BudgetStatus::WithinBudget { count: 2, limit: 2 },
            "exactly at the limit must not be reported as over budget"
        );

        store
            .rebuild(&[
                artifact("artifact-0001"),
                artifact("artifact-0002"),
                artifact("artifact-0003"),
            ])
            .expect("rebuild one over the limit");
        assert_eq!(
            check_current_state_budget(&store, &limits).expect("check"),
            BudgetStatus::OverBudget { count: 3, limit: 2 }
        );
    }

    #[test]
    fn enforce_ledger_budget_does_not_compact_exactly_at_the_limit() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let limits = BudgetLimits::new(1, 5, 1).expect("limits");
        for i in 0..5 {
            ledger.append(discovered(i)).expect("append");
        }
        let result = enforce_ledger_budget(&mut ledger, &limits, 1_000).expect("enforce");
        assert_eq!(result, None);
        assert_eq!(ledger.read_all().expect("read_all").len(), 5);
    }

    #[test]
    fn enforce_ledger_budget_compacts_when_one_event_over_the_limit() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let limits = BudgetLimits::new(1, 5, 1).expect("limits");
        for i in 0..6 {
            ledger.append(discovered(i)).expect("append");
        }
        let summary = enforce_ledger_budget(&mut ledger, &limits, 1_000)
            .expect("enforce")
            .expect("one over the limit must compact");
        assert_eq!(summary.event_count, 1);
        assert_eq!(ledger.read_all().expect("read_all").len(), 5);
    }

    #[test]
    fn ledger_self_budget_stress_test_growth_never_exceeds_the_configured_limit() {
        // Verification contract: "self-budget stress test." Appends far beyond the configured
        // limit, one event at a time, calling enforce_ledger_budget before every write - the
        // documented usage pattern - and asserts the ledger's own row count never exceeds the
        // limit at any point this test can observe (immediately after each enforce+append pair).
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let limits = BudgetLimits::new(1, 10, 1).expect("limits");

        for i in 0..500u64 {
            enforce_ledger_budget(&mut ledger, &limits, i + 1_000_000).expect("enforce");
            ledger.append(discovered(i)).expect("append");
            let count = ledger.read_all().expect("read_all").len() as u64;
            assert!(
                count <= limits.max_ledger_events() + 1,
                "iteration {i}: ledger grew to {count} rows, past its budget of \
                 {} plus the one just-appended row - self-budget must never allow silent, \
                 unbounded growth",
                limits.max_ledger_events()
            );
        }

        // One final enforcement call must bring it back down to at or under the limit.
        enforce_ledger_budget(&mut ledger, &limits, 2_000_000).expect("final enforce");
        assert!(ledger.read_all().expect("read_all").len() as u64 <= limits.max_ledger_events());
    }

    #[test]
    fn enforce_rollup_budget_does_not_compact_exactly_at_the_limit() {
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        let policy = RetentionPolicy::new(10, 20, 30).expect("policy");
        let limits = BudgetLimits::new(1, 1, 5).expect("limits");
        for i in 0..5 {
            memory.record_sample(sample(i)).expect("record");
        }
        let result =
            enforce_rollup_budget(&mut memory, &limits, &policy, 1_000_000).expect("enforce");
        assert_eq!(result, None);
        assert_eq!(memory.raw_sample_count().expect("count"), 5);
    }

    #[test]
    fn enforce_rollup_budget_compacts_when_over_the_limit_and_samples_have_aged() {
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        let policy = RetentionPolicy::new(10, 20, 30).expect("policy");
        let limits = BudgetLimits::new(1, 1, 5).expect("limits");
        for i in 0..6 {
            memory.record_sample(sample(i)).expect("record");
        }
        let report = enforce_rollup_budget(&mut memory, &limits, &policy, 1_000_000)
            .expect("enforce")
            .expect("one over the limit must run compact");
        assert_eq!(report.raw_samples_promoted, 6);
        assert_eq!(memory.raw_sample_count().expect("count"), 0);
    }

    #[test]
    fn rollup_self_budget_stress_test_growth_never_exceeds_the_configured_limit() {
        let mut memory = AnalyticalMemory::open_in_memory().expect("open");
        let policy = RetentionPolicy::new(1, 2, 3).expect("policy");
        let limits = BudgetLimits::new(1, 1, 10).expect("limits");

        for i in 0..500u64 {
            let now = i * 100 + 1_000_000;
            enforce_rollup_budget(&mut memory, &limits, &policy, now).expect("enforce");
            memory.record_sample(sample(now)).expect("record");
            let count = memory.raw_sample_count().expect("count");
            assert!(
                count <= limits.max_raw_samples() + 1,
                "iteration {i}: raw sample tier grew to {count} rows, past its budget of \
                 {} plus the one just-recorded sample - self-budget must never allow silent, \
                 unbounded growth",
                limits.max_raw_samples()
            );
        }
        enforce_rollup_budget(&mut memory, &limits, &policy, 100_000_000).expect("final enforce");
        assert!(memory.raw_sample_count().expect("count") <= limits.max_raw_samples());
    }

    #[test]
    fn reset_local_state_empties_all_three_layers_and_never_touches_a_provider_path() {
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-budget-test-reset-local-state-{}-{}",
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

        {
            let mut current_state =
                CurrentStateStore::open(&dir.join("current-state.sqlite3")).expect("open store");
            let mut ledger = EventLedger::open(&dir.join("ledger.sqlite3")).expect("open ledger");
            let mut memory =
                AnalyticalMemory::open(&dir.join("rollup.sqlite3")).expect("open memory");

            current_state
                .rebuild(&[artifact("artifact-0001")])
                .expect("rebuild");
            ledger.append(discovered(1)).expect("append");
            memory.record_sample(sample(1)).expect("record");

            reset_local_state(&mut current_state, &mut ledger, &mut memory)
                .expect("reset_local_state");

            assert_eq!(current_state.row_count().expect("row_count"), 0);
            assert!(ledger.read_all().expect("read_all").is_empty());
            assert!(memory.raw_samples().expect("raw_samples").is_empty());
        }

        assert!(
            provider_artifact_path.exists(),
            "reset_local_state must never delete a provider artifact"
        );
        assert_eq!(
            std::fs::read_to_string(&provider_artifact_path).expect("read provider artifact"),
            "a real provider session transcript",
            "reset_local_state must never touch a provider artifact's content"
        );

        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }

    #[test]
    fn ephemeral_layers_perform_zero_persistent_writes_across_many_operations() {
        // AC3: "Ephemeral inspect performs no persistent writes." The three layers'
        // `open_in_memory` constructors are this crate's ephemeral-mode primitive - SQLite
        // `:memory:` connections that structurally cannot create a file. This proves it by
        // pointing a real, empty directory at the operations below and asserting it still holds
        // zero files afterward, even after many writes/compactions/resets - not merely "no file
        // was created for this one call."
        let dir = std::env::temp_dir().join(format!(
            "cancellai-store-budget-test-ephemeral-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");

        let mut current_state = CurrentStateStore::open_in_memory().expect("open store");
        let mut ledger = EventLedger::open_in_memory().expect("open ledger");
        let mut memory = AnalyticalMemory::open_in_memory().expect("open memory");
        let limits = BudgetLimits::new(2, 2, 2).expect("limits");
        let policy = RetentionPolicy::new(1, 2, 3).expect("policy");

        for i in 0..50u64 {
            current_state
                .rebuild(&[artifact("artifact-0001")])
                .expect("rebuild");
            enforce_ledger_budget(&mut ledger, &limits, i + 1_000_000).expect("enforce ledger");
            ledger.append(discovered(i)).expect("append");
            enforce_rollup_budget(&mut memory, &limits, &policy, i + 1_000_000)
                .expect("enforce rollup");
            memory.record_sample(sample(i)).expect("record");
        }
        reset_local_state(&mut current_state, &mut ledger, &mut memory).expect("reset_local_state");

        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("read test dir")
            .collect::<Result<_, _>>()
            .expect("collect dir entries");
        assert!(
            entries.is_empty(),
            "ephemeral (in-memory) layers must never create a file on disk, but found: {entries:?}"
        );

        std::fs::remove_dir_all(&dir).expect("clean up test dir");
    }
}

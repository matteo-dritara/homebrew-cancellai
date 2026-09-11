//! Typed policy and the retention resolver (`docs/architecture/POLICY_MODEL.md`,
//! `docs/architecture/TARGET.md`'s CLASSIFY/RESOLVE/PLAN stages). Depends on `cancellai-safety`
//! because policy can only select within the authority ceiling safety computes; it can never
//! raise it (SI-025 - no `Action` this crate produces sets `authority` above what
//! `cancellai_safety::effective_authority` independently agrees to).
//!
//! E06-S01 adds the first real logic: [`trust::builtin_provider_trust`] and
//! [`retention::RetentionPolicy`]/[`retention::resolve_claude`]/[`retention::resolve_codex`]/
//! [`retention::build_actions`] - see `retention`'s own module docs for the classification
//! rationale, and `docs/adrs/0016-rust-artifact-risk-classification.md` for the
//! `RiskClass -> AuthorityCeiling` mapping decision this module implements. E08-S04 adds
//! [`views`], the first occupant of `docs/architecture/TARGET.md`'s "Engine / Query API" layer -
//! see that module's own doc for why it lives here rather than in a dedicated crate. E09-S02
//! adds [`atlas`], the second occupant, for `cancellai-tui`'s Atlas screen. E09-S03 adds
//! [`explain`], the third, for `cancellai-tui`'s artifact explain view. E09-S04 (Plan review)
//! adds no new module - it reads `explain`'s own output through `cancellai-tui`'s
//! `EngineData::plan_context`.

pub mod atlas;
pub mod explain;
pub mod retention;
mod trust;
pub mod views;

pub use atlas::{AtlasSummary, ProjectTotals, ProviderTotals, TopContributor, summarize};
pub use cancellai_model::{KnowledgeConfidence, Reversibility};
pub use explain::{AttributedProject, ExplainView, PolicyOutcome, explain, is_low_confidence};
pub use retention::{
    ClassifiedArtifact, ProviderPlanningView, ProviderResolution, RetentionPolicy, ToolScope,
    build_actions, resolve_claude, resolve_codex,
};
pub use trust::builtin_provider_trust;
pub use views::{
    MachineBucket, ProjectBucket, ProjectBucketKey, ProviderBucket, SessionBucket, by_artifact,
    by_machine, by_project, by_provider, by_session,
};

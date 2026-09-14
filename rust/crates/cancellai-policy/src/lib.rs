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
//! `EngineData::plan_context`. E11-S01 adds [`schema`], the declarative policy document this
//! crate's `retention`/`explain` modules do not yet read from - see that module's own doc for
//! the scope hierarchy and what parsing it deliberately does not compute. E11-S02 adds
//! [`resolver`], which turns a parsed `schema::PolicyDocument` into the single requested
//! authority the scope ladder resolves to, and folds it into `cancellai_safety::
//! effective_authority` - see that module's own doc for why it computes no ceiling itself.
//! E11-S03 adds [`explanation`], which reshapes `resolver::resolve_effective_authority`'s own
//! output into one ordered, deterministic graph a caller can render - not a new module named
//! `explain` a second time, since `explain` (E09-S03) already answers a different question (one
//! artifact's built-in retention outcome) from a different input (`ClassifiedArtifact`/`Action`).
//! E11-S04 adds [`pinning`] (a session pin resolves to `ProtectionState::Pinned`, feeding the
//! same pre-existing lifecycle constraint every other protection fact already does) and
//! [`budget`] (age/count retention and budget text parsing, plus budget-pressure selection
//! restricted by construction to what `retention::build_actions` already marked eligible).

pub mod atlas;
pub mod budget;
pub mod explain;
pub mod explanation;
pub mod pinning;
pub mod resolver;
pub mod retention;
pub mod schema;
mod trust;
pub mod views;

pub use atlas::{AtlasSummary, ProjectTotals, ProviderTotals, TopContributor, summarize};
pub use budget::{
    BudgetSelection, ValueParseError, parse_age_days, parse_budget_bytes, resolve_age_retention,
    resolve_budget_bytes, resolve_keep_latest, select_under_budget_pressure,
};
pub use cancellai_model::{AuthorityLevel, KnowledgeConfidence, Reversibility};
pub use explain::{AttributedProject, ExplainView, PolicyOutcome, explain, is_low_confidence};
pub use explanation::{ExplanationStep, PolicyExplanation, explain_policy};
pub use pinning::{is_pinned, resolve_protection};
pub use resolver::{
    PolicyContext, PolicyScopeSource, ResolvedRequest, resolve_effective_authority,
    resolve_requested_authority,
};
pub use retention::{
    ClassifiedArtifact, ProviderPlanningView, ProviderResolution, RetentionPolicy, ToolScope,
    build_actions, resolve_claude, resolve_codex,
};
pub use schema::{
    CURRENT_SCHEMA_VERSION, PinEntry, PolicyDocument, PolicyError, ScopePolicy, parse_policy,
};
pub use trust::builtin_provider_trust;
pub use views::{
    MachineBucket, ProjectBucket, ProjectBucketKey, ProviderBucket, SessionBucket, by_artifact,
    by_machine, by_project, by_provider, by_session,
};

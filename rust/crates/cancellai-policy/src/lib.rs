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
//! `RiskClass -> AuthorityCeiling` mapping decision this module implements.

pub mod retention;
mod trust;

pub use retention::{
    ClassifiedArtifact, ProviderPlanningView, ProviderResolution, RetentionPolicy, ToolScope,
    build_actions, resolve_claude, resolve_codex,
};
pub use trust::builtin_provider_trust;

use cancellai_model as _;
use cancellai_safety as _;

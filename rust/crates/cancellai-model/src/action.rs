//! `Action`: one candidate unit of work inside a plan document
//! (`docs/architecture/DOMAIN_MODEL.md` "Action", `docs/architecture/JSON_CONTRACTS.md` "Plan
//! document"). Inert data until a plan is approved and re-validated - nothing here mutates
//! anything. Distinct from `cancellai_safety::SealedPlan`: `SealedPlan` is the execution-time,
//! single-target, capability-bound object the safety kernel actually revalidates/executes
//! against (E03-S02); `Action` is the wire-format, potentially-multi-target envelope a plan
//! *document* carries for observation/explanation before any target is bound to a real
//! `ApprovedRoot`/`BoundedPath`. A caller building a real mutation seals one `SealedPlan` per
//! approved `Action` immediately before execution - the `Action` itself is never accepted as
//! authority on its own (C-06: evidence before action, and only the safety kernel's own
//! capabilities establish authority).

use crate::agent_artifact::ArtifactId;
use crate::evidence::EvidenceId;
use crate::vocabulary::{ActionClass, AuthorityLevel, Reversibility};

/// An opaque, engine-assigned action reference. Never a differential-comparison matching key
/// on its own - `docs/development/VERIFICATION_STRATEGY.md`'s comparator resolves
/// `(target_artifact_ids, action_class)` instead, precisely because two conformant engines are
/// never required to assign the same `action_id` to semantically identical actions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct ActionId(pub String);

impl ActionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// A single value a [`Precondition`] expects - deliberately a small closed set (text or
/// boolean) rather than an open-ended JSON value, so this crate stays free of a `serde_json`
/// dependency for something that, per `docs/architecture/JSON_CONTRACTS.md`'s two worked
/// examples (`root_identity_token` -> string, `process_not_running` -> bool), never needs
/// more expressiveness than this.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum PreconditionValue {
    Text(String),
    Bool(bool),
}

impl From<String> for PreconditionValue {
    fn from(value: String) -> Self {
        PreconditionValue::Text(value)
    }
}

impl From<&str> for PreconditionValue {
    fn from(value: &str) -> Self {
        PreconditionValue::Text(value.to_string())
    }
}

impl From<bool> for PreconditionValue {
    fn from(value: bool) -> Self {
        PreconditionValue::Bool(value)
    }
}

/// What must still be true immediately before an action executes, and what makes it
/// `STALE_PLAN` otherwise (SI-013, `docs/architecture/JSON_CONTRACTS.md` "Plan document").
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Precondition {
    pub kind: String,
    pub expected: PreconditionValue,
}

impl Precondition {
    pub fn new(kind: impl Into<String>, expected: impl Into<PreconditionValue>) -> Self {
        Self {
            kind: kind.into(),
            expected: expected.into(),
        }
    }
}

/// One candidate action inside a plan document. `execution_preconditions` may be empty only
/// when `action_class == ActionClass::Observe` (`docs/architecture/JSON_CONTRACTS.md`:
/// "observation mutates nothing, so there is nothing to revalidate") - this type does not
/// enforce that itself (it is inert data, per module docs); the plan builder that constructs
/// one is responsible for the invariant, and `scripts/check_schemas.py` checks it against the
/// golden documents.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Action {
    pub action_id: ActionId,
    pub target_artifact_ids: Vec<ArtifactId>,
    pub action_class: ActionClass,
    pub reason: String,
    pub authority: AuthorityLevel,
    pub reversibility: Reversibility,
    pub evidence_ids: Vec<EvidenceId>,
    pub execution_preconditions: Vec<Precondition>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precondition_value_serializes_untagged_matching_the_golden_plan_document() {
        let text = Precondition::new("root_identity_token", "root-codex-0001@fingerprint-abc");
        let flag = Precondition::new("process_not_running", true);
        assert_eq!(
            serde_json::to_value(&text).unwrap()["expected"],
            serde_json::json!("root-codex-0001@fingerprint-abc")
        );
        assert_eq!(
            serde_json::to_value(&flag).unwrap()["expected"],
            serde_json::json!(true)
        );
    }
}

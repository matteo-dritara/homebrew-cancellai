//! The dashboard's view model, built only from what the desktop API returned.
//!
//! Nothing here classifies, plans or decides. Every number is either copied from the engine's
//! envelope (`provider_summaries`, `scan_incomplete`, `withheld_by_root_authority`) or counted
//! from the plan document's own `actions` exactly the way the CLI's human `plan` summary counts
//! them, so the dashboard and the CLI cannot disagree about the same query (E19-S02).

use cancellai_desktop_api::{DocumentEnvelope, DocumentKind, Query};

/// One provider row: the CLI `status` line, plus the root facts the inventory document states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRow {
    pub provider_id: String,
    pub artifacts: u64,
    pub bytes: u64,
    pub scan_complete: bool,
    /// The document's `origin` for this provider's root (`default`, `override`, ...), or
    /// `unknown` when the document names none.
    pub root_origin: String,
    pub mutation_eligible: bool,
}

/// One proposed action, as the plan document states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRow {
    pub action_class: String,
    pub targets: Vec<String>,
    pub reason: String,
}

/// The plan section: the CLI `plan` summary's three counts, the withholding it reports, and the
/// actions themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanView {
    pub total_actions: usize,
    pub delete_candidates: usize,
    pub observations: usize,
    pub withheld_by_root_authority: Vec<String>,
    pub actions: Vec<ActionRow>,
}

/// Everything the dashboard renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardView {
    pub engine_version: String,
    pub query: Query,
    pub providers: Vec<ProviderRow>,
    pub scan_incomplete: bool,
    pub plan: PlanView,
}

/// Why two envelopes could not be turned into a view. A view is never built from a document it
/// could not read: a guessed zero would look like "nothing to clean".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewError {
    WrongKind(&'static str),
    QueryMismatch,
    Malformed(&'static str),
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongKind(which) => write!(f, "the {which} envelope is the wrong document kind"),
            Self::QueryMismatch => f.write_str("the inventory and plan answer different queries"),
            Self::Malformed(what) => write!(f, "the engine document has no readable {what}"),
        }
    }
}

fn root_facts(status: &DocumentEnvelope, provider_id: &str) -> (String, bool) {
    status
        .document
        .get("provider_roots")
        .and_then(serde_json::Value::as_array)
        .and_then(|roots| {
            roots
                .iter()
                .find(|root| root.get("provider_id").and_then(|v| v.as_str()) == Some(provider_id))
        })
        .map_or_else(
            || ("unknown".to_string(), false),
            |root| {
                (
                    root.get("origin")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    root.get("mutation_eligible")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                )
            },
        )
}

fn action_rows(plan: &DocumentEnvelope) -> Result<Vec<ActionRow>, ViewError> {
    let actions = plan
        .document
        .get("actions")
        .and_then(serde_json::Value::as_array)
        .ok_or(ViewError::Malformed("actions list"))?;
    actions
        .iter()
        .map(|action| {
            let action_class = action
                .get("action_class")
                .and_then(|v| v.as_str())
                .ok_or(ViewError::Malformed("action class"))?
                .to_string();
            let targets = action
                .get("target_artifact_ids")
                .and_then(serde_json::Value::as_array)
                .ok_or(ViewError::Malformed("action targets"))?
                .iter()
                .map(|t| t.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or(ViewError::Malformed("action target id"))?;
            let reason = action
                .get("reason")
                .and_then(|v| v.as_str())
                .ok_or(ViewError::Malformed("action reason"))?
                .to_string();
            Ok(ActionRow {
                action_class,
                targets,
                reason,
            })
        })
        .collect()
}

/// Builds the view from a `status` envelope and a `plan` envelope for the same query.
pub fn build(
    engine_version: &str,
    status: &DocumentEnvelope,
    plan: &DocumentEnvelope,
) -> Result<DashboardView, ViewError> {
    if status.kind != DocumentKind::Status {
        return Err(ViewError::WrongKind("inventory"));
    }
    if plan.kind != DocumentKind::Plan {
        return Err(ViewError::WrongKind("plan"));
    }
    if status.query != plan.query {
        return Err(ViewError::QueryMismatch);
    }
    let providers = status
        .provider_summaries
        .iter()
        .map(|summary| {
            let (root_origin, mutation_eligible) = root_facts(status, &summary.provider_id);
            ProviderRow {
                provider_id: summary.provider_id.clone(),
                artifacts: summary.artifacts,
                bytes: summary.bytes,
                scan_complete: summary.scan_complete,
                root_origin,
                mutation_eligible,
            }
        })
        .collect();
    let actions = action_rows(plan)?;
    // The CLI's own split (`print_plan_summary`): deletes, and everything else as observations.
    let delete_candidates = actions
        .iter()
        .filter(|a| a.action_class == "delete")
        .count();
    Ok(DashboardView {
        engine_version: engine_version.to_string(),
        query: status.query,
        providers,
        scan_incomplete: status.scan_incomplete || plan.scan_incomplete,
        plan: PlanView {
            total_actions: actions.len(),
            delete_candidates,
            observations: actions.len().saturating_sub(delete_candidates),
            withheld_by_root_authority: plan.withheld_by_root_authority.clone(),
            actions,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_desktop_api::ProviderSummary;

    fn envelope(kind: DocumentKind, document: serde_json::Value) -> DocumentEnvelope {
        DocumentEnvelope {
            kind,
            query: Query::default(),
            document,
            scan_incomplete: false,
            withheld_by_root_authority: Vec::new(),
            provider_summaries: vec![ProviderSummary {
                provider_id: "claude-code".to_string(),
                artifacts: 2,
                bytes: 10,
                scan_complete: true,
            }],
        }
    }

    fn status() -> DocumentEnvelope {
        envelope(
            DocumentKind::Status,
            serde_json::json!({"provider_roots": [
                {"provider_id": "claude-code", "origin": "default", "mutation_eligible": true}
            ]}),
        )
    }

    fn plan(actions: serde_json::Value) -> DocumentEnvelope {
        envelope(
            DocumentKind::Plan,
            serde_json::json!({ "actions": actions }),
        )
    }

    #[test]
    fn counts_follow_the_cli_plan_summary() {
        let view = build(
            "0.1.0",
            &status(),
            &plan(serde_json::json!([
                {"action_class": "delete", "target_artifact_ids": ["a"], "reason": "stale"},
                {"action_class": "observe", "target_artifact_ids": ["b"], "reason": "kept"},
                {"action_class": "delete", "target_artifact_ids": ["c"], "reason": "stale"}
            ])),
        )
        .expect("view");
        assert_eq!(view.plan.total_actions, 3);
        assert_eq!(view.plan.delete_candidates, 2);
        assert_eq!(view.plan.observations, 1);
        assert_eq!(view.providers[0].root_origin, "default");
        assert!(view.providers[0].mutation_eligible);
        assert_eq!(view.providers[0].bytes, 10);
    }

    #[test]
    fn a_provider_the_document_does_not_describe_is_not_eligible() {
        let mut bare = status();
        bare.document = serde_json::json!({});
        let view = build("0.1.0", &bare, &plan(serde_json::json!([]))).expect("view");
        assert_eq!(view.providers[0].root_origin, "unknown");
        assert!(!view.providers[0].mutation_eligible);
    }

    #[test]
    fn an_unreadable_plan_is_an_error_not_an_empty_plan() {
        for broken in [
            serde_json::json!({}),
            serde_json::json!({"actions": "none"}),
            serde_json::json!({"actions": [{"target_artifact_ids": [], "reason": "r"}]}),
            serde_json::json!({"actions": [{"action_class": "delete", "reason": "r"}]}),
            serde_json::json!({"actions": [{"action_class": "delete", "target_artifact_ids": [1], "reason": "r"}]}),
            serde_json::json!({"actions": [{"action_class": "delete", "target_artifact_ids": []}]}),
        ] {
            let mut bad = plan(serde_json::json!([]));
            bad.document = broken.clone();
            assert!(
                matches!(
                    build("0.1.0", &status(), &bad),
                    Err(ViewError::Malformed(_))
                ),
                "{broken}"
            );
        }
    }

    #[test]
    fn mismatched_envelopes_are_refused() {
        let empty = plan(serde_json::json!([]));
        assert_eq!(
            build("0.1.0", &empty, &empty),
            Err(ViewError::WrongKind("inventory"))
        );
        assert_eq!(
            build("0.1.0", &status(), &status()),
            Err(ViewError::WrongKind("plan"))
        );
        let mut other = empty.clone();
        other.query.days = 30;
        assert_eq!(
            build("0.1.0", &status(), &other),
            Err(ViewError::QueryMismatch)
        );
        for error in [
            ViewError::WrongKind("plan"),
            ViewError::QueryMismatch,
            ViewError::Malformed("x"),
        ] {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn incompleteness_and_withholding_are_carried_through() {
        let mut incomplete = status();
        incomplete.scan_incomplete = true;
        let mut withheld = plan(serde_json::json!([]));
        withheld.withheld_by_root_authority = vec!["claude-code".to_string()];
        let view = build("0.1.0", &incomplete, &withheld).expect("view");
        assert!(view.scan_incomplete);
        assert_eq!(view.plan.withheld_by_root_authority, vec!["claude-code"]);
    }
}

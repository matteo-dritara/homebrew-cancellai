//! Age/count retention resolution and budget-pressure selection (E11-S04,
//! `docs/architecture/POLICY_MODEL.md`'s `retention`/`total_budget`/`budget` scope fields).
//!
//! Three independent pieces, deliberately not fused into one function:
//!
//! - [`parse_age_days`]/[`resolve_age_retention`] and [`resolve_keep_latest`] turn a scope
//!   ladder's `retention`/`keep_latest` text and counts (`crate::schema::ScopePolicy`) into the
//!   same two knobs `crate::retention::RetentionPolicy` already exposes (`days`, `keep_latest`) -
//!   a future integration plugs the resolved values straight into that existing struct rather
//!   than this module inventing a second retention-window concept.
//! - [`parse_budget_bytes`] turns budget text (`"50GB"`) into a byte count, using the same
//!   binary-1024 `B`/`KB`/`MB`/`GB`/`TB` convention `cancellai.py::format_bytes` and
//!   `cancellai-tui::format::format_bytes` already use - the inverse of that function's own
//!   labels, not a second, differently-scaled unit system.
//! - [`select_under_budget_pressure`] is the piece this story's AC2 ("Budget pressure chooses
//!   only artifacts already eligible under safety/lifecycle rules") is actually about: it takes
//!   the *already-computed* `crate::retention::build_actions` output as its own filter, so
//!   nothing it returns can be a candidate that pipeline did not already deem eligible - see
//!   that function's own doc for why this is true by construction, not by a second eligibility
//!   check this module invents.

use std::collections::HashSet;

use cancellai_model::{Action, ActionClass, ArtifactId};

use crate::resolver::{PolicyContext, PolicyScopeSource};
use crate::retention::ClassifiedArtifact;
use crate::schema::{PolicyDocument, ScopePolicy};

/// Why a retention/budget text value could not be parsed. Kept separate from
/// `crate::schema::PolicyError` - that error is about document *structure* (E11-S01); this one
/// is about the *grammar* of one already-structurally-valid string, decided by this story.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueParseError {
    Malformed(String),
}

impl std::fmt::Display for ValueParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueParseError::Malformed(text) => write!(f, "could not parse {text:?}"),
        }
    }
}

impl std::error::Error for ValueParseError {}

/// Parses a retention age string (`docs/architecture/POLICY_MODEL.md`'s `retention: 30d`) into a
/// day count. Grammar: one or more ASCII digits followed by the literal suffix `d` - the only
/// unit that document's own example ever shows. A wider grammar (weeks, months) is a deliberate,
/// separate decision this story does not make.
pub fn parse_age_days(text: &str) -> Result<u32, ValueParseError> {
    let trimmed = text.trim();
    let digits = trimmed
        .strip_suffix('d')
        .ok_or_else(|| ValueParseError::Malformed(text.to_string()))?;
    if digits.is_empty() {
        return Err(ValueParseError::Malformed(text.to_string()));
    }
    digits
        .parse::<u32>()
        .map_err(|_| ValueParseError::Malformed(text.to_string()))
}

/// Binary-1024 units, longest suffix first so `"GB"` is matched before the trailing `"B"` every
/// unit's label shares with it. Mirrors `cancellai.py::format_bytes` /
/// `cancellai-tui::format::format_bytes`'s own `UNITS` table and divisor exactly.
const BYTE_UNITS: [(&str, u64); 5] = [
    ("TB", 1024u64.pow(4)),
    ("GB", 1024u64.pow(3)),
    ("MB", 1024u64.pow(2)),
    ("KB", 1024),
    ("B", 1),
];

/// Parses a budget string (`"50GB"`) into a byte count. See [`BYTE_UNITS`] for the convention.
/// Overflow (a value whose byte count cannot be represented in `u64`) is reported as malformed
/// rather than silently wrapping - a policy document cannot express a budget that quietly
/// becomes a much smaller one.
pub fn parse_budget_bytes(text: &str) -> Result<u64, ValueParseError> {
    let trimmed = text.trim();
    for (suffix, multiplier) in BYTE_UNITS {
        if let Some(digits) = trimmed.strip_suffix(suffix) {
            let digits = digits.trim();
            if digits.is_empty() {
                return Err(ValueParseError::Malformed(text.to_string()));
            }
            let value: u64 = digits
                .parse()
                .map_err(|_| ValueParseError::Malformed(text.to_string()))?;
            return value
                .checked_mul(multiplier)
                .ok_or_else(|| ValueParseError::Malformed(text.to_string()));
        }
    }
    Err(ValueParseError::Malformed(text.to_string()))
}

/// Walks the same scope ladder `crate::resolver::resolve_requested_authority` uses
/// (`ARTIFACT_TYPE -> PROJECT -> PROVIDER -> MACHINE -> GLOBAL`), returning the first scope for
/// which `extract` produces `Some`, and which rung it came from. Private: every public function
/// in this module that needs ladder resolution (age, count, budget) goes through this one
/// implementation rather than three independently hand-rolled walks that could drift apart.
fn walk_ladder<'a, T>(
    document: &'a PolicyDocument,
    context: &PolicyContext<'_>,
    extract: impl Fn(&'a ScopePolicy) -> Option<T>,
) -> Option<(T, PolicyScopeSource)> {
    let rungs: [(Option<&'a ScopePolicy>, PolicyScopeSource); 5] = [
        (
            context
                .artifact_type
                .and_then(|key| document.artifact_types.get(key)),
            PolicyScopeSource::ArtifactType,
        ),
        (
            context
                .project_ref
                .and_then(|key| document.projects.get(key)),
            PolicyScopeSource::Project,
        ),
        (
            context
                .provider_id
                .and_then(|key| document.providers.get(key)),
            PolicyScopeSource::Provider,
        ),
        (
            context.machine_id.and_then(|key| document.machine.get(key)),
            PolicyScopeSource::Machine,
        ),
        (document.global.as_ref(), PolicyScopeSource::Global),
    ];

    for (scope, source) in rungs {
        if let Some(value) = scope.and_then(&extract) {
            return Some((value, source));
        }
    }
    None
}

/// Resolves the most specific `retention` text set for `context` and parses it. `Ok(None)` means
/// no scope declared an age retention at all (not an error - the caller decides what "no policy"
/// means for its own command, same as `crate::resolver`'s own `NoMatch` case). `Err` means a
/// scope *did* declare one and it could not be parsed - reported rather than silently skipped to
/// the next less specific scope, so a malformed value is never silently reinterpreted as "no
/// constraint here" (`docs/architecture/POLICY_MODEL.md`'s "Policy migration": "the user receives
/// a migration plan rather than silent reinterpretation").
pub fn resolve_age_retention(
    document: &PolicyDocument,
    context: &PolicyContext<'_>,
) -> Result<Option<(u32, PolicyScopeSource)>, ValueParseError> {
    match walk_ladder(document, context, |scope| scope.retention.as_deref()) {
        Some((text, source)) => parse_age_days(text).map(|days| Some((days, source))),
        None => Ok(None),
    }
}

/// Resolves the most specific `keep_latest` count set for `context`. No parsing needed - the
/// schema already carries this as a typed `u32` (E11-S01/E11-S04's own doc on
/// `ScopePolicy::keep_latest`).
pub fn resolve_keep_latest(
    document: &PolicyDocument,
    context: &PolicyContext<'_>,
) -> Option<(u32, PolicyScopeSource)> {
    walk_ladder(document, context, |scope| scope.keep_latest)
}

/// Resolves the most specific `budget` text set for `context` and parses it. Same `Ok(None)` /
/// `Err` split as [`resolve_age_retention`], for the same reason.
pub fn resolve_budget_bytes(
    document: &PolicyDocument,
    context: &PolicyContext<'_>,
) -> Result<Option<(u64, PolicyScopeSource)>, ValueParseError> {
    match walk_ladder(document, context, |scope| scope.budget.as_deref()) {
        Some((text, source)) => parse_budget_bytes(text).map(|bytes| Some((bytes, source))),
        None => Ok(None),
    }
}

/// The result of one budget-pressure selection: which candidates were chosen, how many bytes
/// they would free, and whether that is enough to satisfy the pressure.
#[derive(Debug, Clone)]
pub struct BudgetSelection<'a> {
    pub selected: Vec<&'a ClassifiedArtifact>,
    pub freed_bytes: u64,
    pub satisfied: bool,
}

/// Selects a deterministic subset of `candidates` to relieve budget pressure, **restricted by
/// construction to artifacts `eligible_actions` already marks `Delete`/`Quarantine`/`Archive`**
/// (`crate::retention::build_actions`'s own output - already safety- and lifecycle-checked: an
/// artifact only reaches one of those action classes after passing scan-completeness, activity,
/// protection, and `reachable_authority` checks). This function performs no independent
/// eligibility check of its own; it can only ever narrow that set further, never widen it - the
/// literal mechanism behind AC2 ("Budget pressure chooses only artifacts already eligible under
/// safety/lifecycle rules").
///
/// Selection order is largest-first (frees the most pressure per artifact, and is what any
/// reasonable budget-pressure policy would want), tie-broken by `identity_token` for determinism
/// across runs with identical inputs. Stops as soon as enough bytes are freed to satisfy
/// `current_usage_bytes.saturating_sub(budget_limit_bytes)` - never selects more than needed.
pub fn select_under_budget_pressure<'a>(
    candidates: &'a [ClassifiedArtifact],
    eligible_actions: &[Action],
    current_usage_bytes: u64,
    budget_limit_bytes: u64,
) -> BudgetSelection<'a> {
    let bytes_over_budget = current_usage_bytes.saturating_sub(budget_limit_bytes);

    let eligible_ids: HashSet<&ArtifactId> = eligible_actions
        .iter()
        .filter(|action| {
            matches!(
                action.action_class,
                ActionClass::Delete | ActionClass::Quarantine | ActionClass::Archive
            )
        })
        .flat_map(|action| action.target_artifact_ids.iter())
        .collect();

    let mut pool: Vec<&ClassifiedArtifact> = candidates
        .iter()
        .filter(|classified| eligible_ids.contains(&classified.artifact.artifact_id))
        .collect();
    pool.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.artifact.identity_token.cmp(&b.artifact.identity_token))
    });

    let mut selected = Vec::new();
    let mut freed_bytes = 0u64;
    for candidate in pool {
        if freed_bytes >= bytes_over_budget {
            break;
        }
        freed_bytes = freed_bytes.saturating_add(candidate.size_bytes);
        selected.push(candidate);
    }

    BudgetSelection {
        selected,
        freed_bytes,
        satisfied: freed_bytes >= bytes_over_budget,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retention::ClassifiedArtifact;
    use cancellai_model::{
        ActivityState, AgentArtifact, ArtifactId, AuthorityLevel, EvidenceId, IntegrityState,
        KnowledgeConfidence, Precondition, Reversibility, RiskClass,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    // --- parse_age_days -------------------------------------------------------------------

    #[test]
    fn parses_a_simple_day_count() {
        assert_eq!(parse_age_days("30d"), Ok(30));
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(parse_age_days("  7d "), Ok(7));
    }

    #[test]
    fn rejects_a_missing_unit() {
        assert!(parse_age_days("30").is_err());
    }

    #[test]
    fn rejects_an_unknown_unit() {
        assert!(parse_age_days("30w").is_err());
    }

    #[test]
    fn rejects_a_bare_unit_with_no_digits() {
        assert!(parse_age_days("d").is_err());
    }

    #[test]
    fn rejects_a_negative_looking_value() {
        assert!(parse_age_days("-5d").is_err());
    }

    #[test]
    fn rejects_an_empty_string() {
        assert!(parse_age_days("").is_err());
    }

    #[test]
    fn zero_days_parses_as_a_real_zero_not_an_error() {
        assert_eq!(parse_age_days("0d"), Ok(0));
    }

    // --- parse_budget_bytes ----------------------------------------------------------------

    #[test]
    fn parses_gigabytes_using_the_binary_1024_convention() {
        assert_eq!(parse_budget_bytes("50GB"), Ok(50 * 1024u64.pow(3)));
    }

    #[test]
    fn parses_every_unit_with_the_longest_suffix_matched_first() {
        assert_eq!(parse_budget_bytes("10TB"), Ok(10 * 1024u64.pow(4)));
        assert_eq!(parse_budget_bytes("10GB"), Ok(10 * 1024u64.pow(3)));
        assert_eq!(parse_budget_bytes("10MB"), Ok(10 * 1024u64.pow(2)));
        assert_eq!(parse_budget_bytes("10KB"), Ok(10 * 1024));
        assert_eq!(parse_budget_bytes("10B"), Ok(10));
    }

    #[test]
    fn budget_rejects_an_unknown_unit() {
        assert!(parse_budget_bytes("50PB").is_err());
    }

    #[test]
    fn budget_rejects_a_missing_unit() {
        assert!(parse_budget_bytes("50").is_err());
    }

    #[test]
    fn rejects_overflow_rather_than_silently_wrapping() {
        assert!(parse_budget_bytes("99999999999999999999TB").is_err());
    }

    // --- resolve_age_retention / resolve_keep_latest / resolve_budget_bytes ----------------

    fn scope_with_retention(
        retention: &str,
        keep_latest: Option<u32>,
        budget: Option<&str>,
    ) -> ScopePolicy {
        ScopePolicy {
            authority: None,
            retention: Some(retention.to_string()),
            keep_latest,
            budget: budget.map(str::to_string),
        }
    }

    fn empty_document() -> PolicyDocument {
        PolicyDocument {
            schema_version: crate::schema::CURRENT_SCHEMA_VERSION,
            global: None,
            machine: BTreeMap::new(),
            providers: BTreeMap::new(),
            projects: BTreeMap::new(),
            artifact_types: BTreeMap::new(),
            pins: Vec::new(),
        }
    }

    #[test]
    fn resolve_age_retention_picks_the_most_specific_scope() {
        let mut document = empty_document();
        document.global = Some(scope_with_retention("90d", None, None));
        document.projects.insert(
            "cancellai".to_string(),
            scope_with_retention("30d", None, None),
        );
        let context = PolicyContext {
            project_ref: Some("cancellai"),
            ..PolicyContext::default()
        };
        let result = resolve_age_retention(&document, &context).unwrap();
        assert_eq!(result, Some((30, PolicyScopeSource::Project)));
    }

    #[test]
    fn resolve_age_retention_returns_none_when_no_scope_sets_it() {
        let document = empty_document();
        let context = PolicyContext::default();
        assert_eq!(resolve_age_retention(&document, &context), Ok(None));
    }

    #[test]
    fn resolve_age_retention_propagates_a_malformed_value_rather_than_deferring() {
        let mut document = empty_document();
        document.projects.insert(
            "cancellai".to_string(),
            scope_with_retention("thirty days", None, None),
        );
        document.global = Some(scope_with_retention("90d", None, None));
        let context = PolicyContext {
            project_ref: Some("cancellai"),
            ..PolicyContext::default()
        };
        let result = resolve_age_retention(&document, &context);
        assert!(
            result.is_err(),
            "a malformed value at the winning rung must be reported, not silently skipped to \
             the next less specific scope (which would incorrectly return Ok(Some((90, Global))))"
        );
    }

    #[test]
    fn resolve_keep_latest_picks_the_most_specific_scope() {
        let mut document = empty_document();
        document.global = Some(ScopePolicy {
            authority: None,
            retention: None,
            keep_latest: Some(10),
            budget: None,
        });
        document.artifact_types.insert(
            "session".to_string(),
            ScopePolicy {
                authority: None,
                retention: None,
                keep_latest: Some(3),
                budget: None,
            },
        );
        let context = PolicyContext {
            artifact_type: Some("session"),
            ..PolicyContext::default()
        };
        assert_eq!(
            resolve_keep_latest(&document, &context),
            Some((3, PolicyScopeSource::ArtifactType))
        );
    }

    #[test]
    fn resolve_budget_bytes_picks_the_most_specific_scope() {
        let mut document = empty_document();
        document.global = Some(scope_with_retention("90d", None, Some("50GB")));
        document.providers.insert(
            "codex".to_string(),
            scope_with_retention("90d", None, Some("20GB")),
        );
        let context = PolicyContext {
            provider_id: Some("codex"),
            ..PolicyContext::default()
        };
        let result = resolve_budget_bytes(&document, &context).unwrap();
        assert_eq!(
            result,
            Some((20 * 1024u64.pow(3), PolicyScopeSource::Provider))
        );
    }

    // --- select_under_budget_pressure -------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn classified(
        id: &str,
        size_bytes: u64,
        reachable_authority: AuthorityLevel,
    ) -> ClassifiedArtifact {
        ClassifiedArtifact {
            artifact: AgentArtifact {
                artifact_id: ArtifactId::new(id),
                identity_token: format!("codex-cli:{id}"),
                provider_id: "codex-cli".to_string(),
                artifact_type: "session".to_string(),
                risk_class: RiskClass::R3Resumable,
                reversibility: Reversibility::Irreversible,
                knowledge_confidence: KnowledgeConfidence::Verified,
                activity_state: ActivityState::Stale,
                residency_state: cancellai_model::ResidencyState::Hot,
                protection_state: cancellai_model::ProtectionState::Normal,
                integrity_state: IntegrityState::Healthy,
                authority_ceiling: AuthorityLevel::Govern,
                evidence_ids: vec![EvidenceId::new(format!("evidence-{id}"))],
                relationships: Vec::new(),
                project_attribution: None,
                activity_signal: None,
            },
            path: PathBuf::from(format!("/synthetic/{id}")),
            size_bytes,
            reachable_authority,
            binding_constraints: Vec::new(),
        }
    }

    fn delete_action(target: &ClassifiedArtifact) -> Action {
        Action {
            action_id: cancellai_model::ActionId::new(format!(
                "delete-{}",
                target.artifact.artifact_id.0
            )),
            target_artifact_ids: vec![target.artifact.artifact_id.clone()],
            action_class: ActionClass::Delete,
            reason: "past retention cutoff".to_string(),
            authority: AuthorityLevel::Govern,
            reversibility: Reversibility::Irreversible,
            evidence_ids: target.artifact.evidence_ids.clone(),
            execution_preconditions: vec![Precondition::new(
                "identity_token",
                target.artifact.identity_token.clone(),
            )],
        }
    }

    fn observe_action(target: &ClassifiedArtifact) -> Action {
        Action {
            action_id: cancellai_model::ActionId::new(format!(
                "observe-{}",
                target.artifact.artifact_id.0
            )),
            target_artifact_ids: vec![target.artifact.artifact_id.clone()],
            action_class: ActionClass::Observe,
            reason: "blocked".to_string(),
            authority: AuthorityLevel::Observe,
            reversibility: Reversibility::Rebuildable,
            evidence_ids: target.artifact.evidence_ids.clone(),
            execution_preconditions: Vec::new(),
        }
    }

    #[test]
    fn selects_the_largest_eligible_candidates_first() {
        let small = classified("small", 10, AuthorityLevel::Govern);
        let large = classified("large", 100, AuthorityLevel::Govern);
        let medium = classified("medium", 50, AuthorityLevel::Govern);
        let candidates = vec![small.clone(), large.clone(), medium.clone()];
        let actions = vec![
            delete_action(&small),
            delete_action(&large),
            delete_action(&medium),
        ];

        let selection = select_under_budget_pressure(&candidates, &actions, 200, 100);
        // 100 bytes over budget: large (100) alone satisfies it.
        assert_eq!(selection.selected.len(), 1);
        assert_eq!(selection.selected[0].artifact.artifact_id.0, "large");
        assert_eq!(selection.freed_bytes, 100);
        assert!(selection.satisfied);
    }

    #[test]
    fn selects_multiple_candidates_when_one_is_not_enough() {
        let a = classified("a", 60, AuthorityLevel::Govern);
        let b = classified("b", 60, AuthorityLevel::Govern);
        let c = classified("c", 60, AuthorityLevel::Govern);
        let candidates = vec![a.clone(), b.clone(), c.clone()];
        let actions = vec![delete_action(&a), delete_action(&b), delete_action(&c)];

        let selection = select_under_budget_pressure(&candidates, &actions, 300, 200);
        // 100 bytes over budget, each candidate frees 60 -> needs two.
        assert_eq!(selection.selected.len(), 2);
        assert_eq!(selection.freed_bytes, 120);
        assert!(selection.satisfied);
    }

    #[test]
    fn no_pressure_selects_nothing() {
        let a = classified("a", 1_000, AuthorityLevel::Govern);
        let candidates = vec![a.clone()];
        let actions = vec![delete_action(&a)];
        let selection = select_under_budget_pressure(&candidates, &actions, 50, 100);
        assert!(selection.selected.is_empty());
        assert_eq!(selection.freed_bytes, 0);
        assert!(selection.satisfied);
    }

    // --- AC2: budget pressure chooses only artifacts already eligible -------------------------

    #[test]
    fn an_artifact_with_no_delete_quarantine_or_archive_action_is_never_selected_even_under_extreme_pressure()
     {
        let blocked = classified("blocked", 1_000_000, AuthorityLevel::Observe);
        let candidates = vec![blocked.clone()];
        // The artifact has an Action, but it is Observe, not Delete/Quarantine/Archive - build_actions'
        // own definition of "not eligible."
        let actions = vec![observe_action(&blocked)];

        let selection = select_under_budget_pressure(&candidates, &actions, u64::MAX, 0);
        assert!(
            selection.selected.is_empty(),
            "an Observe-only artifact must never be selected regardless of budget pressure"
        );
        assert_eq!(selection.freed_bytes, 0);
        assert!(
            !selection.satisfied,
            "pressure cannot be satisfied by an ineligible-only pool"
        );
    }

    #[test]
    fn a_candidate_absent_from_the_actions_list_entirely_is_never_selected() {
        let untracked = classified("untracked", 1_000_000, AuthorityLevel::Autopilot);
        let candidates = vec![untracked];
        let actions: Vec<Action> = Vec::new();

        let selection = select_under_budget_pressure(&candidates, &actions, u64::MAX, 0);
        assert!(selection.selected.is_empty());
    }

    #[test]
    fn selection_is_deterministic_across_repeated_calls_with_tied_sizes() {
        let a = classified("a", 50, AuthorityLevel::Govern);
        let b = classified("b", 50, AuthorityLevel::Govern);
        let candidates = vec![a.clone(), b.clone()];
        let actions = vec![delete_action(&a), delete_action(&b)];

        let first = select_under_budget_pressure(&candidates, &actions, 100, 60);
        let second = select_under_budget_pressure(&candidates, &actions, 100, 60);
        let first_ids: Vec<&str> = first
            .selected
            .iter()
            .map(|c| c.artifact.artifact_id.0.as_str())
            .collect();
        let second_ids: Vec<&str> = second
            .selected
            .iter()
            .map(|c| c.artifact.artifact_id.0.as_str())
            .collect();
        assert_eq!(first_ids, second_ids);
        // Tied sizes break by identity_token ascending - "a" before "b".
        assert_eq!(first_ids, vec!["a"]);
    }

    #[test]
    fn selection_saturates_freed_bytes_when_eligible_sizes_exceed_u64() {
        let first = classified("first", u64::MAX - 1, AuthorityLevel::Govern);
        let second = classified("second", 2, AuthorityLevel::Govern);
        let candidates = vec![first.clone(), second.clone()];
        let actions = vec![delete_action(&first), delete_action(&second)];

        let selection = select_under_budget_pressure(&candidates, &actions, u64::MAX, 0);

        assert_eq!(selection.selected.len(), 2);
        assert_eq!(selection.freed_bytes, u64::MAX);
        assert!(selection.satisfied);
    }
}

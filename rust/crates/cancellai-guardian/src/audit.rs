//! Guardian audit trail (E15-S04, AC2: "every action links detection evidence, policy decision,
//! and sealed plan"; `docs/architecture/GUARDIAN_MODEL.md`'s own "Audit" list: "detection
//! evidence; pressure/anomaly state; policy resolution; authority result; sealed plan ID if any;
//! execution result").
//!
//! Reuses `cancellai_store::EventLedger` (E13-S02) rather than a second, Guardian-owned ledger -
//! the same "one shared engine" principle `docs/PRODUCT.md` states for Guardian generally
//! ("Guardian and Desktop are later clients of the same engine"), applied here to persistence.
//! `EventLedger::append` already enforces, at the database layer, that a mutation-class event
//! (`PlanCreated`, `ActionBlocked`, ...) carries a non-empty `plan_id` and at least one
//! `EvidenceId` (`docs/architecture/PERSISTENCE_MODEL.md`'s "Mutation events carry plan/evidence
//! references") - [`record_guardian_decision`] cannot bypass that; it is the same `append` every
//! other mutation-class writer in this workspace uses, not a second path.
//!
//! [`GuardianPlanItem::action_class`] decides which kind is recorded: [`EventKind::PlanCreated`]
//! when Guardian's own planner granted enough authority to act
//! ([`ActionClass::Quarantine`]) - a real plan exists, even if execution/sealing has not
//! happened yet ("sealed plan ID if any" is deliberately `plan_id: String`, not yet a real
//! `cancellai_safety::SealedPlan::plan_id`, since this story's own planner does not seal plans -
//! see the module docs' "Residual" note); [`EventKind::ActionBlocked`] when it did not
//! ([`ActionClass::Observe`]) - Guardian considered the candidate and recorded why it did not
//! proceed, whether that is insufficient policy authority, insufficient pressure, or the kill
//! switch (`crate::killswitch`) having forced it there.

use cancellai_model::{ActionClass, EvidenceId};
use cancellai_store::ledger::{
    EventId, EventKind, EventLedger, EventMetadata, LedgerError, MutationReference, NewEvent,
};

use crate::remediation::GuardianPlanItem;

/// Records one Guardian plan item as a ledger event, linking detection evidence
/// (`evidence_ids`), the policy decision (`item`'s own `granted_authority`/`pressure_state`/
/// `binding_constraints`, carried in `EventMetadata::reason_code`/`policy_id`), and a plan
/// reference (`plan_id`) - see the module docs for why `plan_id` is not yet a real sealed plan
/// ID. `recorded_at` is caller-supplied seconds-since-epoch, matching `EventLedger`'s own
/// "production code never calls `SystemTime::now()` directly" convention.
///
/// `pub(crate)`, matching `GuardianPlanItem`'s own visibility (`remediation.rs`'s module docs).
/// `#[allow(dead_code)]` because no production orchestrator wires this yet (this crate's own
/// "primitive delivered, no orchestrator yet" pattern) - exercised directly by this module's own
/// tests.
#[allow(dead_code)]
pub(crate) fn record_guardian_decision(
    ledger: &mut EventLedger,
    item: &GuardianPlanItem,
    plan_id: String,
    evidence_ids: Vec<EvidenceId>,
    recorded_at: u64,
) -> Result<EventId, LedgerError> {
    let kind = if item.action_class == ActionClass::Quarantine {
        EventKind::PlanCreated
    } else {
        EventKind::ActionBlocked
    };
    let event = NewEvent {
        kind,
        recorded_at,
        metadata: EventMetadata {
            artifact_id: Some(item.artifact_id.clone()),
            provider_id: None,
            category: Some("guardian".to_string()),
            policy_id: Some(item.binding_constraints.join(",")),
            reason_code: Some(format!(
                "pressure={:?};granted_authority={:?}",
                item.pressure_state, item.granted_authority
            )),
        },
        mutation: Some(MutationReference {
            plan_id,
            evidence_ids,
        }),
    };
    ledger.append(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pressure::PressureState;
    use cancellai_model::{ArtifactId, AuthorityLevel};

    fn quarantine_item() -> GuardianPlanItem {
        GuardianPlanItem {
            artifact_id: ArtifactId::new("artifact-0001"),
            action_class: ActionClass::Quarantine,
            granted_authority: AuthorityLevel::Quarantine,
            pressure_state: PressureState::Orange,
            binding_constraints: vec!["provider_trust_authority"],
        }
    }

    fn observe_item() -> GuardianPlanItem {
        GuardianPlanItem {
            artifact_id: ArtifactId::new("artifact-0002"),
            action_class: ActionClass::Observe,
            granted_authority: AuthorityLevel::Recommend,
            pressure_state: PressureState::Yellow,
            binding_constraints: vec!["lifecycle_authority"],
        }
    }

    #[test]
    fn an_actionable_item_is_recorded_as_plan_created_with_plan_and_evidence_linked() {
        let mut ledger = EventLedger::open_in_memory().expect("open in-memory ledger");
        let item = quarantine_item();
        let evidence_ids = vec![EvidenceId::new("evidence-pressure-0001")];

        record_guardian_decision(
            &mut ledger,
            &item,
            "guardian-plan-0001".to_string(),
            evidence_ids.clone(),
            1_700_000_000,
        )
        .expect("recording an actionable item must succeed");

        let events = ledger.read_all().expect("read back");
        assert_eq!(events.len(), 1);
        let recorded = &events[0];
        assert_eq!(recorded.kind, EventKind::PlanCreated);
        assert_eq!(
            recorded.metadata.artifact_id,
            Some(item.artifact_id.clone())
        );
        let mutation = recorded
            .mutation
            .as_ref()
            .expect("a mutation-class event must carry its MutationReference");
        assert_eq!(mutation.plan_id, "guardian-plan-0001");
        assert_eq!(mutation.evidence_ids, evidence_ids);
    }

    #[test]
    fn a_non_actionable_item_is_recorded_as_action_blocked() {
        let mut ledger = EventLedger::open_in_memory().expect("open in-memory ledger");
        let item = observe_item();

        record_guardian_decision(
            &mut ledger,
            &item,
            "guardian-plan-0002".to_string(),
            vec![EvidenceId::new("evidence-anomaly-0001")],
            1_700_000_100,
        )
        .expect("recording a blocked item must succeed");

        let events = ledger.read_all().expect("read back");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::ActionBlocked);
    }

    #[test]
    fn the_policy_decision_is_recoverable_from_the_recorded_metadata() {
        let mut ledger = EventLedger::open_in_memory().expect("open in-memory ledger");
        let item = quarantine_item();

        record_guardian_decision(
            &mut ledger,
            &item,
            "guardian-plan-0003".to_string(),
            vec![EvidenceId::new("evidence-0001")],
            1_700_000_200,
        )
        .unwrap();

        let events = ledger.read_all().unwrap();
        let reason = events[0].metadata.reason_code.as_deref().unwrap();
        assert!(reason.contains("Orange"));
        assert!(reason.contains("Quarantine"));
        assert_eq!(
            events[0].metadata.policy_id.as_deref(),
            Some("provider_trust_authority")
        );
    }

    #[test]
    fn empty_evidence_ids_is_refused_by_the_ledger_itself_not_silently_accepted() {
        // EventLedger::append already enforces this (PERSISTENCE_MODEL.md's "Mutation events
        // carry plan/evidence references") - this test confirms record_guardian_decision does
        // not somehow route around that enforcement.
        let mut ledger = EventLedger::open_in_memory().expect("open in-memory ledger");
        let result = record_guardian_decision(
            &mut ledger,
            &quarantine_item(),
            "guardian-plan-0004".to_string(),
            Vec::new(),
            1_700_000_300,
        );
        assert!(result.is_err());
        assert!(ledger.read_all().unwrap().is_empty());
    }

    #[test]
    fn multiple_decisions_accumulate_in_append_order() {
        let mut ledger = EventLedger::open_in_memory().expect("open in-memory ledger");
        record_guardian_decision(
            &mut ledger,
            &quarantine_item(),
            "guardian-plan-a".to_string(),
            vec![EvidenceId::new("evidence-a")],
            1_700_000_400,
        )
        .unwrap();
        record_guardian_decision(
            &mut ledger,
            &observe_item(),
            "guardian-plan-b".to_string(),
            vec![EvidenceId::new("evidence-b")],
            1_700_000_500,
        )
        .unwrap();

        let events = ledger.read_all().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EventKind::PlanCreated);
        assert_eq!(events[1].kind, EventKind::ActionBlocked);
    }
}

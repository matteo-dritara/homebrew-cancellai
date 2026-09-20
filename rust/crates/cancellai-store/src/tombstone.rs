//! Purge tombstones (E12-S04, `docs/architecture/PERSISTENCE_MODEL.md`'s "Tombstones", SI-020
//! "irreversible actions are explicit and stronger-gated").
//!
//! ## No new schema, file or connection (same shape as [`crate::budget`])
//!
//! A tombstone is not a fourth persisted layer: it is exactly [`crate::ledger::EventLedger`]'s
//! own [`crate::ledger::EventKind::Purged`] event, already named in `docs/architecture/
//! PERSISTENCE_MODEL.md`'s Layer 2 event-kind list and already schema-pinned by
//! `crate::ledger::tests::ledger_events_schema_has_only_the_allowlisted_columns`. This module
//! adds a typed, narrower front door onto that existing capability - [`Tombstone`] and
//! [`record_purge_tombstone`] - rather than a new table a future reader would have to learn is
//! the "real" tombstone instead of the ledger.
//!
//! ## Contentless by construction (AC1: "Tombstones contain no prompts/source/file contents")
//!
//! [`Tombstone`] carries only the fields `docs/architecture/PERSISTENCE_MODEL.md`'s "Tombstones"
//! section names - opaque artifact ID, provider/category, purge time (the ledger's own
//! `recorded_at`), reason/policy ID, and action result/evidence references (plan ID + evidence
//! IDs) - the same closed set [`crate::ledger::EventMetadata`]/[`crate::ledger::
//! MutationReference`] already enforce at the SQLite layer. There is no path, prompt, or
//! content-typed field on this struct for a caller to populate even by mistake - unlike
//! [`crate::ledger::EventMetadata`]'s own residual (a `String` field can still be misused; see
//! that module's doc), this module does not widen what the ledger already accepts, only narrows
//! the door onto it. "size/reclaim observation," the one illustrative item this document's
//! "Tombstones" section names that the ledger's own schema has no column for, is a disclosed
//! residual (this crate's evidence packet) rather than a new column: `AnalyticalMemory`'s
//! `ReclaimableBytes` metric already tracks reclaimable bytes as an aggregate time series
//! (`docs/architecture/PERSISTENCE_MODEL.md`'s Layer 3), and widening the ledger's own
//! already-pinned schema for a per-artifact figure is a larger, separately reviewable change
//! this story's acceptance criteria do not require.
//!
//! ## Distinguishable from a reversible/conditionally-reversible outcome (AC2: "Irreversible
//! purge is distinguishable from vendor-native conditionally reversible operations")
//!
//! [`record_purge_tombstone`] refuses - writes nothing - unless it is given exactly
//! `ActionClass::Delete` with `Reversibility::Irreversible`. Every other combination this
//! crate's shared `cancellai_model::vocabulary` admits is refused, including
//! `Reversibility::VendorConditional` (the vocabulary's own name for a vendor-native
//! conditionally-reversible outcome) paired with any action class, and `Delete` paired with any
//! reversibility other than `Irreversible`. This is the same coupling
//! `cancellai_safety::authority::reversibility_allowed` already enforces before the real OS
//! call is ever attempted (SI-020) - restated here, at the point the retained record is
//! written, as a **strictly narrower** predicate (it accepts only the one combination that
//! predicate's own `Delete` arm accepts) so the two checks can never disagree, never as a
//! second, independent authorization decision: this function makes no authority decision at
//! all (it does not see or take an `AuthorityLevel`), and by the time a real caller would ever
//! reach it the OS mutation has already happened and already been authorized elsewhere. It only
//! decides whether the outcome it is asked to record is eligible to be labeled a purge
//! tombstone, refusing to let a `Quarantine`/`Archive`/`Restore` outcome, or a hypothetical
//! vendor-native soft-delete (which this system has no path to represent as anything but one of
//! those three, or `Observe`, since `ActionClass::Delete` is reserved for the one irreversible
//! class `authority.rs` defines), be mislabeled as one. `cancellai-store` deliberately does not
//! depend on `cancellai-safety` (see [`crate::local_state_root`]'s own module doc for why that
//! isolation is load-bearing elsewhere in this crate), so this predicate is expressed locally
//! against `cancellai_model`'s shared vocabulary rather than by calling
//! `reversibility_allowed` itself.
//!
//! ## No orchestrator yet
//!
//! Nothing in this workspace calls [`record_purge_tombstone`] from a real purge yet -
//! `cancellai_safety::mutation_executor::execute`'s `ActionClass::Delete` branch performs the
//! real OS deletion (E03-S05) but does not itself touch `cancellai-store`, matching every prior
//! E13 story's own "primitive delivered, no orchestrator yet" precedent (see `crate::budget`'s
//! own module doc). A crash between a real purge succeeding and a future orchestrator's call to
//! this function is therefore that future story's failure mode to close, not this one's.

use crate::ledger::{EventKind, EventLedger, EventMetadata, LedgerError, MutationReference};
use cancellai_model::{ActionClass, ArtifactId, EvidenceId, Reversibility};

pub use crate::ledger::EventId;

/// The allowlisted, contentless record retained after a permanent purge
/// (`docs/architecture/PERSISTENCE_MODEL.md`'s "Tombstones"). See this module's own doc for
/// exactly what is and is not covered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tombstone {
    pub artifact_id: ArtifactId,
    pub provider_id: Option<String>,
    pub category: Option<String>,
    pub reason_code: Option<String>,
    pub policy_id: Option<String>,
    pub plan_id: String,
    pub evidence_ids: Vec<EvidenceId>,
}

/// Why [`record_purge_tombstone`] refused to write a tombstone.
#[derive(Debug)]
pub enum PurgeTombstoneError {
    /// `action_class`/`reversibility` was not exactly `ActionClass::Delete` with
    /// `Reversibility::Irreversible` - this module's own doc, "Distinguishable from a
    /// reversible/conditionally-reversible outcome."
    NotAnIrreversiblePurge {
        action_class: ActionClass,
        reversibility: Reversibility,
    },
    /// The underlying ledger append itself refused (SI-020 aside, [`EventLedger::append`]'s
    /// own fail-closed contract for mutation-class events: an empty `plan_id`/`evidence_ids`
    /// is rejected there too - this variant is reached instead of `NotAnIrreversiblePurge` only
    /// when the action_class/reversibility check already passed).
    Ledger(LedgerError),
}

impl std::fmt::Display for PurgeTombstoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PurgeTombstoneError::NotAnIrreversiblePurge {
                action_class,
                reversibility,
            } => write!(
                f,
                "{action_class:?} with reversibility {reversibility:?} is not an irreversible \
                 purge - only ActionClass::Delete with Reversibility::Irreversible may be \
                 recorded as a purge tombstone"
            ),
            PurgeTombstoneError::Ledger(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PurgeTombstoneError {}

/// Records `tombstone` as a [`EventKind::Purged`] ledger event at `recorded_at` - refusing,
/// with nothing written, unless `action_class`/`reversibility` together name exactly the one
/// combination `docs/security/SAFETY_INVARIANTS.md`'s SI-020 reserves for a real irreversible
/// purge. See this module's own doc for why this check is a narrower restatement of that
/// invariant, never a second, independent one.
pub fn record_purge_tombstone(
    ledger: &mut EventLedger,
    action_class: ActionClass,
    reversibility: Reversibility,
    recorded_at: u64,
    tombstone: Tombstone,
) -> Result<EventId, PurgeTombstoneError> {
    if action_class != ActionClass::Delete || reversibility != Reversibility::Irreversible {
        return Err(PurgeTombstoneError::NotAnIrreversiblePurge {
            action_class,
            reversibility,
        });
    }

    let event = crate::ledger::NewEvent {
        kind: EventKind::Purged,
        recorded_at,
        metadata: EventMetadata {
            artifact_id: Some(tombstone.artifact_id),
            provider_id: tombstone.provider_id,
            category: tombstone.category,
            policy_id: tombstone.policy_id,
            reason_code: tombstone.reason_code,
        },
        mutation: Some(MutationReference {
            plan_id: tombstone.plan_id,
            evidence_ids: tombstone.evidence_ids,
        }),
    };
    ledger.append(event).map_err(PurgeTombstoneError::Ledger)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::EventLedger;

    fn tombstone(artifact: &str) -> Tombstone {
        Tombstone {
            artifact_id: ArtifactId::new(artifact),
            provider_id: Some("codex".to_string()),
            category: Some("session".to_string()),
            reason_code: Some("policy-expired".to_string()),
            policy_id: Some("policy-0001".to_string()),
            plan_id: "plan-0001".to_string(),
            evidence_ids: vec![EvidenceId::new("evidence-0001")],
        }
    }

    #[test]
    fn record_purge_tombstone_accepts_delete_plus_irreversible_and_appends_a_purged_event() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        record_purge_tombstone(
            &mut ledger,
            ActionClass::Delete,
            Reversibility::Irreversible,
            1_000,
            tombstone("artifact-0001"),
        )
        .expect("Delete + Irreversible must be accepted");

        let events = ledger.read_all().expect("read_all");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::Purged);
    }

    #[test]
    fn record_purge_tombstone_refuses_every_combination_except_delete_plus_irreversible() {
        // AC2's exhaustive falsification: every ActionClass x Reversibility pair this crate's
        // shared vocabulary admits, except the one SI-020 reserves for a real purge, must be
        // refused - including Reversibility::VendorConditional, the vocabulary's own name for a
        // vendor-native conditionally-reversible outcome, paired with every action class.
        let action_classes = [
            ActionClass::Observe,
            ActionClass::Quarantine,
            ActionClass::Archive,
            ActionClass::Delete,
            ActionClass::Restore,
        ];
        let reversibilities = [
            Reversibility::Rebuildable,
            Reversibility::Quarantinable,
            Reversibility::Archivable,
            Reversibility::VendorConditional,
            Reversibility::Irreversible,
            Reversibility::Unknown,
        ];

        let mut refused = 0;
        let mut accepted = 0;
        for &action_class in &action_classes {
            for &reversibility in &reversibilities {
                let mut ledger = EventLedger::open_in_memory().expect("open");
                let result = record_purge_tombstone(
                    &mut ledger,
                    action_class,
                    reversibility,
                    1_000,
                    tombstone("artifact-0001"),
                );
                let is_the_one_true_purge = action_class == ActionClass::Delete
                    && reversibility == Reversibility::Irreversible;
                if is_the_one_true_purge {
                    accepted += 1;
                    assert!(
                        result.is_ok(),
                        "Delete + Irreversible must be accepted, got {result:?}"
                    );
                    assert_eq!(ledger.read_all().expect("read_all").len(), 1);
                } else {
                    refused += 1;
                    assert!(
                        matches!(
                            result,
                            Err(PurgeTombstoneError::NotAnIrreversiblePurge {
                                action_class: got_class,
                                reversibility: got_rev,
                            }) if got_class == action_class && got_rev == reversibility
                        ),
                        "{action_class:?} + {reversibility:?} must be refused as \
                         NotAnIrreversiblePurge, got {result:?}"
                    );
                    assert!(
                        ledger.read_all().expect("read_all").is_empty(),
                        "{action_class:?} + {reversibility:?} must write nothing on refusal"
                    );
                }
            }
        }
        assert_eq!(accepted, 1, "exactly one combination must be accepted");
        assert_eq!(
            refused,
            action_classes.len() * reversibilities.len() - 1,
            "every other combination must be refused"
        );
    }

    #[test]
    fn record_purge_tombstone_refuses_empty_plan_id_and_empty_evidence_ids() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let mut empty_plan_id = tombstone("artifact-0001");
        empty_plan_id.plan_id = String::new();
        let result = record_purge_tombstone(
            &mut ledger,
            ActionClass::Delete,
            Reversibility::Irreversible,
            1_000,
            empty_plan_id,
        );
        assert!(
            matches!(result, Err(PurgeTombstoneError::Ledger(_))),
            "an empty plan_id must be refused by the ledger's own mutation-reference contract, \
             got {result:?}"
        );
        assert!(ledger.read_all().expect("read_all").is_empty());

        let mut empty_evidence = tombstone("artifact-0002");
        empty_evidence.evidence_ids = Vec::new();
        let result = record_purge_tombstone(
            &mut ledger,
            ActionClass::Delete,
            Reversibility::Irreversible,
            1_000,
            empty_evidence,
        );
        assert!(
            matches!(result, Err(PurgeTombstoneError::Ledger(_))),
            "empty evidence_ids must be refused by the ledger's own mutation-reference \
             contract, got {result:?}"
        );
        assert!(ledger.read_all().expect("read_all").is_empty());
    }

    #[test]
    fn record_purge_tombstone_round_trips_exactly_the_given_allowlisted_fields() {
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let given = tombstone("artifact-0001");
        record_purge_tombstone(
            &mut ledger,
            ActionClass::Delete,
            Reversibility::Irreversible,
            42,
            given.clone(),
        )
        .expect("accepted");

        let events = ledger.read_all().expect("read_all");
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.kind, EventKind::Purged);
        assert_eq!(event.recorded_at, 42);
        assert_eq!(event.metadata.artifact_id, Some(given.artifact_id));
        assert_eq!(event.metadata.provider_id, given.provider_id);
        assert_eq!(event.metadata.category, given.category);
        assert_eq!(event.metadata.policy_id, given.policy_id);
        assert_eq!(event.metadata.reason_code, given.reason_code);
        let mutation = event
            .mutation
            .as_ref()
            .expect("a Purged event always carries a MutationReference");
        assert_eq!(mutation.plan_id, given.plan_id);
        assert_eq!(mutation.evidence_ids, given.evidence_ids);
    }
}

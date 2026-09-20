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
//! Two independent review rounds rejected this module's earlier, wider field set
//! (`provider_id`, `category`, `reason_code`, `policy_id`, in addition to the linkage fields
//! below). Round 1 showed a column allowlist does not restrict what bytes a caller puts in the
//! `String` those columns hold; round 2 showed that a syntactic content-safety validator over
//! those bytes (an ASCII/hyphen/segment-count shape check) does not either, because a short,
//! ordinary phrase (`do-not-purge-this`) is exactly as "safe" by any such shape check as a real
//! ID, and is still meaningful retained content. No further syntactic tightening closes this: it
//! is a structural limit, not a bug in one predicate. **`Tombstone` therefore no longer carries
//! `provider_id`/`category`/`reason_code`/`policy_id` at all** - those were purely descriptive
//! annotations, not needed to identify what was purged, and the fields most likely to invite
//! free-text ("reason" is, definitionally, an invitation to explain in words). This is the same
//! response this crate's own review history already gave a repeatedly-defeated automated
//! mechanism (`docs/architecture/PERSISTENCE_MODEL.md`'s "Tombstones" cites E12's own
//! `recover_pending_moves` rounds 3-5: after three distinct genuine defects in successive
//! repairs, the mechanism was removed rather than patched a fourth time) - narrow the surface
//! instead of re-attempting the same kind of fix a third time.
//!
//! What remains - `artifact_id`, `plan_id`, `evidence_ids` - is retained because a purge record
//! naming neither what was purged nor which plan/evidence produced it is not a tombstone at all;
//! this is a structurally required minimum, not a lower-risk version of the same annotation
//! fields. **This is a disclosed residual, not a closed guarantee** - AC1 was narrowed to say so
//! explicitly by owner decision (`docs/adrs/0033-purge-tombstone-content-safety-is-a-disclosed-
//! residual.md`), after round 3 independent review held that a verifier cannot accept a residual
//! in place of an unqualified acceptance criterion on its own authority: these three fields are
//! still
//! caller-supplied `String`/`ArtifactId`/`EvidenceId` values, still validated by
//! [`validate_content_safe`] (the same shape check - short, ASCII, hyphen-joined, bounded
//! segments), and that check still cannot prove a short value carries no meaning, only that it
//! is not an obvious prompt/source/path (it lacks whitespace, `/`, `\`, quotes, braces, `.`,
//! `_`, or more than a few hyphenated segments). Closing this fully requires a real orchestrator
//! that sources these three values from the system's own already-validated purge/evidence
//! records rather than accepting them from an arbitrary public caller (this module's own "No
//! orchestrator yet" section, below) - that story can then re-derive `ArtifactId`/`EvidenceId`
//! from objects it already trusts instead of from caller-typed strings. Until then, this module
//! narrows the exposure (three fields instead of seven, no field whose ordinary use is
//! explanatory prose) and states the residual rather than a guarantee it cannot make.
//!
//! "size/reclaim observation," one item `docs/architecture/PERSISTENCE_MODEL.md`'s "Tombstones"
//! section names that the ledger's own schema has no column for, is a disclosed residual (this
//! crate's evidence packet) rather than a new column: `AnalyticalMemory`'s `ReclaimableBytes`
//! metric already tracks reclaimable bytes as an aggregate time series (`docs/architecture/
//! PERSISTENCE_MODEL.md`'s Layer 3), and widening the ledger's own already-pinned schema for a
//! per-artifact figure is a larger, separately reviewable change this story's acceptance
//! criteria do not require.
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

/// The narrowed, contentless-by-disclosed-residual record retained after a permanent purge
/// (`docs/architecture/PERSISTENCE_MODEL.md`'s "Tombstones"). See this module's own doc for
/// exactly what is and is not covered, and why `provider_id`/`category`/`reason_code`/
/// `policy_id` are deliberately absent rather than merely validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tombstone {
    pub artifact_id: ArtifactId,
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
    /// A field carried something other than a short, content-safe identifier - AC1
    /// "Tombstones contain no prompts/source/file contents." Named by field, never by value:
    /// echoing the rejected value back would hand the caller a second channel to retain
    /// exactly what this rejection exists to keep out of any log or error report.
    UnsafeField { field: &'static str },
    /// The underlying ledger append itself refused (SI-020 aside, [`EventLedger::append_purged`]'s
    /// own fail-closed contract for mutation-class events: an empty `plan_id`/`evidence_ids`
    /// is rejected there too - this variant is reached instead of `NotAnIrreversiblePurge` or
    /// `UnsafeField` only when both prior checks already passed).
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
            PurgeTombstoneError::UnsafeField { field } => write!(
                f,
                "tombstone field {field:?} is not identifier-shaped (at most {MAX_SEGMENTS} \
                 hyphen-separated segments of letters/digits, {MAX_SEGMENT_LEN} characters \
                 each, {MAX_FIELD_LEN} total) - refusing to risk retaining prompt, source, or \
                 path content in a purge tombstone"
            ),
            PurgeTombstoneError::Ledger(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PurgeTombstoneError {}

/// Longest value [`validate_content_safe`] accepts in total, and the longest it accepts for any
/// one hyphen-separated segment. Every artifact/plan/evidence ID this workspace actually
/// produces is a handful of short lowercase words and digits joined by single hyphens
/// (`plan-0001`, `evidence-0001`, ...); these bounds are sized for that shape, not merely for "a
/// short string."
const MAX_FIELD_LEN: usize = 64;
const MAX_SEGMENT_LEN: usize = 32;
/// Most hyphen-separated segments [`validate_content_safe`] accepts. Bounding segment count
/// narrows the character allowlist's blind spot (an attacker's message re-encoded with hyphens
/// standing in for spaces is, character-for-character, as "safe" as a real ID) but - see this
/// module's own doc, "Contentless by construction" - does not close it: round 2 independent
/// review found a short, ordinary phrase (`do-not-purge-this`) that fits within this bound and
/// is still meaningful content. This constant narrows exposure; it is not a proof of
/// contentlessness, which is exactly why `provider_id`/`category`/`reason_code`/`policy_id` were
/// removed rather than tightened further.
const MAX_SEGMENTS: usize = 4;

/// Judges whether `value` is a short, identifier-shaped string (AC1's disclosed residual - see
/// this module's own doc): empty, or ASCII letters/digits only joined by single hyphens (no
/// leading/trailing/doubled hyphen, so no empty segment), at most [`MAX_SEGMENTS`] segments of
/// at most [`MAX_SEGMENT_LEN`] characters each, [`MAX_FIELD_LEN`] characters total. This rejects
/// an obvious prompt, source line, or file path (each usually contains a character this
/// allowlist excludes: whitespace, `/`, `\`, quotes, braces, `.`, `_`, control characters, ...);
/// it does not and cannot reject every short, ordinary, hyphen-joined phrase, which is a
/// disclosed residual, not a gap in this function.
///
/// `pub(crate)` because [`crate::ledger::EventLedger::append_purged`] enforces this same
/// predicate on every [`EventKind::Purged`] event directly - round 3 independent review found
/// that this module's own check, applied only in [`record_purge_tombstone`], was reachable
/// around by any caller using the ledger's own public `append` to construct a `Purged` event
/// directly (round 4 additionally closed that route to `EventKind::Purged` entirely - see
/// `EventLedger::append`'s own doc). The actual database-writing function is the boundary AC1
/// needs enforced at; this module's own check stays as an earlier, friendlier error path (naming
/// which field failed), not the only gate.
pub(crate) fn is_identifier_shaped(value: &str) -> bool {
    value.is_empty()
        || (value.len() <= MAX_FIELD_LEN
            && !value.starts_with('-')
            && !value.ends_with('-')
            && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && {
                let segments: Vec<&str> = value.split('-').collect();
                segments.len() <= MAX_SEGMENTS
                    && segments
                        .iter()
                        .all(|s| !s.is_empty() && s.len() <= MAX_SEGMENT_LEN)
            })
}

fn validate_content_safe(field: &'static str, value: &str) -> Result<(), PurgeTombstoneError> {
    if is_identifier_shaped(value) {
        Ok(())
    } else {
        Err(PurgeTombstoneError::UnsafeField { field })
    }
}

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

    validate_content_safe("artifact_id", &tombstone.artifact_id.0)?;
    validate_content_safe("plan_id", &tombstone.plan_id)?;
    for evidence_id in &tombstone.evidence_ids {
        validate_content_safe("evidence_id", &evidence_id.0)?;
    }

    let event = crate::ledger::NewEvent {
        kind: EventKind::Purged,
        recorded_at,
        metadata: EventMetadata {
            artifact_id: Some(tombstone.artifact_id),
            provider_id: None,
            category: None,
            policy_id: None,
            reason_code: None,
        },
        mutation: Some(MutationReference {
            plan_id: tombstone.plan_id,
            evidence_ids: tombstone.evidence_ids,
        }),
    };
    ledger
        .append_purged(event)
        .map_err(PurgeTombstoneError::Ledger)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::EventLedger;

    fn tombstone(artifact: &str) -> Tombstone {
        Tombstone {
            artifact_id: ArtifactId::new(artifact),
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
        // provider_id/category/reason_code/policy_id are deliberately never populated by
        // record_purge_tombstone (see this module's own doc, "Contentless by construction") -
        // Tombstone no longer even offers them as fields to give a value for.
        assert_eq!(event.metadata.provider_id, None);
        assert_eq!(event.metadata.category, None);
        assert_eq!(event.metadata.policy_id, None);
        assert_eq!(event.metadata.reason_code, None);
        let mutation = event
            .mutation
            .as_ref()
            .expect("a Purged event always carries a MutationReference");
        assert_eq!(mutation.plan_id, given.plan_id);
        assert_eq!(mutation.evidence_ids, given.evidence_ids);
    }

    #[test]
    fn record_purge_tombstone_refuses_prompt_source_and_path_sentinels_in_every_remaining_field() {
        // Independent review round 1 finding: every retained field accepted and round-tripped
        // arbitrary prompt/source/path content unchanged (AC1, C-09, SI-020). Each sentinel here
        // contains at least one character validate_content_safe's allowlist excludes. Only
        // artifact_id/plan_id/evidence_id remain on Tombstone at all - provider_id/category/
        // reason_code/policy_id were removed outright (this module's own doc).
        let sentinels: [(&str, &str); 3] = [
            ("prompt", "please do not store this instruction verbatim"),
            (
                "source",
                "fn source() { /* do not store source contents */ }",
            ),
            ("path", "/private/provider/source.rs"),
        ];

        type FieldSetter = fn(&mut Tombstone, &str);
        let fields: [(&str, FieldSetter); 3] = [
            ("artifact_id", |t: &mut Tombstone, v: &str| {
                t.artifact_id = ArtifactId::new(v)
            }),
            ("plan_id", |t: &mut Tombstone, v: &str| {
                t.plan_id = v.to_string()
            }),
            ("evidence_id", |t: &mut Tombstone, v: &str| {
                t.evidence_ids = vec![EvidenceId::new(v)]
            }),
        ];

        for (field_name, set_field) in fields {
            for (sentinel_name, sentinel) in sentinels {
                let mut ledger = EventLedger::open_in_memory().expect("open");
                let mut poisoned = tombstone("artifact-0001");
                set_field(&mut poisoned, sentinel);
                let result = record_purge_tombstone(
                    &mut ledger,
                    ActionClass::Delete,
                    Reversibility::Irreversible,
                    1_000,
                    poisoned,
                );
                assert!(
                    matches!(
                        result,
                        Err(PurgeTombstoneError::UnsafeField { field }) if field == field_name
                    ),
                    "{field_name} with a {sentinel_name} sentinel must be refused as \
                     UnsafeField({field_name:?}), got {result:?}"
                );
                assert!(
                    ledger.read_all().expect("read_all").is_empty(),
                    "{field_name} with a {sentinel_name} sentinel must write nothing"
                );
            }
        }
    }

    #[test]
    fn record_purge_tombstone_refuses_the_codex_round1_reproduction() {
        // The exact independent-review round-1 reproduction, restricted to the fields Tombstone
        // still has: every remaining field poisoned at once with the same sentinel that
        // previously round-tripped through EventLedger intact.
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let poisoned = Tombstone {
            artifact_id: ArtifactId::new("PROMPT_SENTINEL_do_not_store_source_contents"),
            plan_id: "PROMPT_SENTINEL_do_not_store_source_contents".to_string(),
            evidence_ids: vec![EvidenceId::new(
                "PROMPT_SENTINEL_do_not_store_source_contents",
            )],
        };
        let result = record_purge_tombstone(
            &mut ledger,
            ActionClass::Delete,
            Reversibility::Irreversible,
            1_000,
            poisoned,
        );
        assert!(
            matches!(result, Err(PurgeTombstoneError::UnsafeField { .. })),
            "the round-1 reproduction must be refused, got {result:?}"
        );
        assert!(ledger.read_all().expect("read_all").is_empty());
    }

    #[test]
    fn record_purge_tombstone_accepts_a_short_ordinary_phrase_as_a_disclosed_residual() {
        // Independent review round 2 finding, deliberately preserved as a passing test rather
        // than papered over: `validate_content_safe` cannot distinguish a real identifier from
        // an ordinary short phrase re-encoded with hyphens, because none exists to distinguish
        // by syntax alone (this module's own doc, "Contentless by construction"). This is why
        // provider_id/category/reason_code/policy_id were removed rather than validated harder,
        // and why artifact_id/plan_id/evidence_ids remain a disclosed residual rather than a
        // closed guarantee until a future orchestrator sources them from already-trusted
        // records instead of an arbitrary public caller.
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let mut residual = tombstone("artifact-0001");
        residual.plan_id = "do-not-purge-this".to_string();
        let result = record_purge_tombstone(
            &mut ledger,
            ActionClass::Delete,
            Reversibility::Irreversible,
            1_000,
            residual,
        );
        assert!(
            result.is_ok(),
            "a short, identifier-shaped, ordinary phrase is accepted - documented residual, \
             not an oversight; got {result:?}"
        );
    }
}

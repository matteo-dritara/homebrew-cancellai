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
//! MutationReference`] already enforce at the SQLite layer. Restricting *which columns* exist
//! does not restrict *what bytes* a caller can put in the `String`/ID fields those columns hold
//! (an independent review round 1 finding: this module previously asserted contentlessness from
//! the column allowlist alone and left every field able to round-trip an arbitrary prompt,
//! source excerpt, or absolute path unchanged). [`record_purge_tombstone`] therefore validates
//! every caller-supplied field - `artifact_id`, `provider_id`, `category`, `reason_code`,
//! `policy_id`, `plan_id`, and each evidence ID - against [`validate_content_safe`] before it
//! writes anything: a short value shaped like every real ID this workspace produces - ASCII
//! letters/digits joined by single hyphens, a handful of short segments (`policy-expired`,
//! `plan-0001`) - is accepted, and anything else is refused with nothing appended to the
//! ledger. This is a positive allowlist over both *characters* and *shape*, not a blocklist of
//! known-bad patterns: prompt text, source code, and file paths usually contain a character the
//! allowlist excludes outright (whitespace, `/`, `\`, quotes, braces, `.`, `_`, ...), and the one
//! case that character check alone cannot catch - attacker-chosen words joined by hyphens
//! instead of spaces, which is exactly as "safe" character-for-character as a real ID - is
//! caught by the segment-count bound instead, since no real provider/category/reason/policy/
//! plan/evidence ID in this system is built from more than a few hyphenated words. "size/reclaim
//! observation," the one illustrative item this document's "Tombstones"
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
    /// A field carried something other than a short, content-safe identifier - AC1
    /// "Tombstones contain no prompts/source/file contents." Named by field, never by value:
    /// echoing the rejected value back would hand the caller a second channel to retain
    /// exactly what this rejection exists to keep out of any log or error report.
    UnsafeField { field: &'static str },
    /// The underlying ledger append itself refused (SI-020 aside, [`EventLedger::append`]'s
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
                "tombstone field {field:?} is not a content-safe identifier (at most \
                 {MAX_SEGMENTS} hyphen-separated segments of letters/digits, {MAX_SEGMENT_LEN} \
                 characters each, {MAX_FIELD_LEN} total) - refusing to risk retaining prompt, \
                 source, or path content in a purge tombstone"
            ),
            PurgeTombstoneError::Ledger(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PurgeTombstoneError {}

/// Longest value [`validate_content_safe`] accepts in total, and the longest it accepts for any
/// one hyphen-separated segment. Every provider/category/reason/policy/plan/evidence ID this
/// workspace actually produces is a handful of short lowercase words and digits joined by single
/// hyphens (`policy-expired`, `plan-0001`, `evidence-0001`, ...); these bounds are sized for
/// that shape, not merely for "a short string."
const MAX_FIELD_LEN: usize = 64;
const MAX_SEGMENT_LEN: usize = 32;
/// Most hyphen-separated segments [`validate_content_safe`] accepts. A character allowlist
/// alone does not distinguish a legitimate identifier from an attacker's message re-encoded
/// with hyphens standing in for spaces (`prompt-sentinel-do-not-store-source-contents` is,
/// character-for-character, as "safe" as `policy-0001`) - bounding segment count catches what
/// the allowlist cannot, the same way E13-S06 rejects a caller-suppliable proof of ownership on
/// structural grounds rather than trying to pattern-match forged content.
const MAX_SEGMENTS: usize = 4;

/// Refuses a tombstone field that is not a short, content-safe identifier (AC1). Emptiness is
/// not this function's concern - [`EventLedger::append`]'s own mutation-reference contract
/// already refuses an empty `plan_id`/`evidence_ids`, and an absent `Option` field is simply not
/// validated - so this judges shape: ASCII letters/digits only, joined by single hyphens (no
/// leading/trailing/doubled hyphen, so no empty segment), at most [`MAX_SEGMENTS`] segments of
/// at most [`MAX_SEGMENT_LEN`] characters each, [`MAX_FIELD_LEN`] characters total. A prompt, a
/// line of source code, or a file path either contains a character this allowlist excludes
/// (whitespace, `/`, `\`, quotes, braces, `.`, `_`, control characters, ...) or, once forced
/// into hyphen-joined words to dodge that, exceeds the segment-count bound real IDs never reach.
fn validate_content_safe(field: &'static str, value: &str) -> Result<(), PurgeTombstoneError> {
    if value.is_empty() {
        return Ok(());
    }
    let shape_safe = value.len() <= MAX_FIELD_LEN
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && {
            let segments: Vec<&str> = value.split('-').collect();
            segments.len() <= MAX_SEGMENTS
                && segments
                    .iter()
                    .all(|s| !s.is_empty() && s.len() <= MAX_SEGMENT_LEN)
        };
    if shape_safe {
        Ok(())
    } else {
        Err(PurgeTombstoneError::UnsafeField { field })
    }
}

/// [`validate_content_safe`] over an optional field - `None` carries nothing to validate.
fn validate_optional_content_safe(
    field: &'static str,
    value: &Option<String>,
) -> Result<(), PurgeTombstoneError> {
    match value {
        Some(v) => validate_content_safe(field, v),
        None => Ok(()),
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
    validate_optional_content_safe("provider_id", &tombstone.provider_id)?;
    validate_optional_content_safe("category", &tombstone.category)?;
    validate_optional_content_safe("reason_code", &tombstone.reason_code)?;
    validate_optional_content_safe("policy_id", &tombstone.policy_id)?;
    validate_content_safe("plan_id", &tombstone.plan_id)?;
    for evidence_id in &tombstone.evidence_ids {
        validate_content_safe("evidence_id", &evidence_id.0)?;
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

    #[test]
    fn record_purge_tombstone_refuses_prompt_source_and_path_sentinels_in_every_field() {
        // Independent review round 1 finding: every retained field accepted and round-tripped
        // arbitrary prompt/source/path content unchanged (AC1, C-09, SI-020). Each sentinel here
        // contains at least one character validate_content_safe's allowlist excludes.
        let sentinels: [(&str, &str); 3] = [
            ("prompt", "please do not store this instruction verbatim"),
            (
                "source",
                "fn source() { /* do not store source contents */ }",
            ),
            ("path", "/private/provider/source.rs"),
        ];

        type FieldSetter = fn(&mut Tombstone, &str);
        let fields: [(&str, FieldSetter); 7] = [
            ("artifact_id", |t: &mut Tombstone, v: &str| {
                t.artifact_id = ArtifactId::new(v)
            }),
            ("provider_id", |t: &mut Tombstone, v: &str| {
                t.provider_id = Some(v.to_string())
            }),
            ("category", |t: &mut Tombstone, v: &str| {
                t.category = Some(v.to_string())
            }),
            ("reason_code", |t: &mut Tombstone, v: &str| {
                t.reason_code = Some(v.to_string())
            }),
            ("policy_id", |t: &mut Tombstone, v: &str| {
                t.policy_id = Some(v.to_string())
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
    fn record_purge_tombstone_refuses_hyphen_joined_words_standing_in_for_a_sentence() {
        // A character allowlist alone cannot tell `policy-0001` from an attacker's message
        // re-encoded with hyphens instead of spaces - both are letters/digits/hyphens. This is
        // the independent review's own round-1 reproduction re-expressed with hyphens rather
        // than underscores, which the segment-count bound must still catch.
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let mut poisoned = tombstone("artifact-0001");
        poisoned.reason_code = Some("prompt-sentinel-do-not-store-source-contents".to_string());
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
                Err(PurgeTombstoneError::UnsafeField { field }) if field == "reason_code"
            ),
            "a long hyphen-joined message must be refused on segment count, got {result:?}"
        );
        assert!(ledger.read_all().expect("read_all").is_empty());
    }

    #[test]
    fn record_purge_tombstone_refuses_the_codex_round1_reproduction() {
        // The exact independent-review round-1 reproduction: every retained field poisoned at
        // once with the same sentinel that previously round-tripped through EventLedger intact.
        let mut ledger = EventLedger::open_in_memory().expect("open");
        let poisoned = Tombstone {
            artifact_id: ArtifactId::new("artifact-0001"),
            provider_id: Some("PROMPT_SENTINEL_do_not_store_source_contents".to_string()),
            category: Some("/private/provider/source.rs".to_string()),
            reason_code: Some("PROMPT_SENTINEL_do_not_store_source_contents".to_string()),
            policy_id: Some("PROMPT_SENTINEL_do_not_store_source_contents".to_string()),
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
}

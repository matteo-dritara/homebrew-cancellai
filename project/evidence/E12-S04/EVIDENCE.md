# Evidence Packet - E12-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex, round 1 - **FAIL** (`project/evidence/E12-S04-VERIFIER-REVIEW.md`);
  round 2 - **FAIL** (`project/evidence/E12-S04-VERIFIER-REVIEW-ROUND2.md`); round 3 - **FAIL**
  (`project/evidence/E12-S04-VERIFIER-REVIEW-ROUND3.md`), addressed below by closing a concrete
  bypass plus an owner decision narrowing AC1 ([ADR-0033](../../../docs/adrs/0033-purge-tombstone-content-safety-is-a-disclosed-residual.md)).
  Round 4 pending against the narrowed AC1 and closed bypass
- Change Risk: CR4 (declared CR4 at planning time in `project/epics/E12.json`, matching
  E12-S01/S02/S03's own level for SI-020; the diff adds no new mutation capability and no new
  cross-crate dependency, so no reclassification applies)
- Spec version/commit: `docs/architecture/PERSISTENCE_MODEL.md` "Tombstones"; `docs/security/
  SAFETY_INVARIANTS.md` SI-020 ("Irreversible actions are explicit and stronger-gated")

## Outcome

PASS against AC1 as narrowed by owner decision (ADR-0033) and with the round-3 ledger bypass
closed (see "Repair - round 3 finding" below). Verdict pending round 4.

## Scope

`cancellai_store::tombstone` (new module, `rust/crates/cancellai-store/src/tombstone.rs`): a
typed, narrower front door onto `cancellai_store::ledger::EventLedger`'s existing
`EventKind::Purged` event kind (E13-S02), adding no new schema, file, connection, or crate
dependency. `Tombstone` carries only opaque artifact ID and plan/evidence references
(`provider_id`/`category`/`reason_code`/`policy_id` were removed after round 2 - see Repair
section). `record_purge_tombstone(ledger, action_class, reversibility, recorded_at, tombstone)`
writes exactly one `Purged` ledger event, refusing - with nothing written - unless
`action_class == ActionClass::Delete && reversibility == Reversibility::Irreversible`. No
orchestrator wires this to a real purge yet (`mutation_executor::execute`'s `ActionClass::Delete`
branch, E03-S05, is unchanged); this story delivers the primitive, matching every prior E12/E13
story's own "primitive delivered, no orchestrator yet" precedent.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 (narrowed by ADR-0033) - "Tombstones carry no descriptive annotation fields ...; artifact_id/plan_id/evidence_ids are each validated as a short, identifier-shaped value ...; this is a disclosed residual against arbitrary short phrases, not a closed guarantee." | `Tombstone`'s field set is exactly `artifact_id`, `plan_id`, `evidence_ids` - `provider_id`/`category`/`reason_code`/`policy_id` were removed outright (round 2). All three remaining fields are validated by `validate_content_safe`/`is_identifier_shaped` (ASCII letters/digits joined by at most 4 single hyphens, ≤32 characters per segment, ≤64 total) - and, since round 3, this check is enforced inside `EventLedger::append` itself for `EventKind::Purged`, not only in `record_purge_tombstone`, closing the direct-ledger bypass round 3 found (see Repair section). `record_purge_tombstone_accepts_a_short_ordinary_phrase_as_a_disclosed_residual` deliberately demonstrates the residual the narrowed AC1 now names explicitly rather than hides. `record_purge_tombstone_round_trips_exactly_the_given_allowlisted_fields` proves a content-safe tombstone still reads back with exactly and only the given fields. | PASS (against narrowed AC1; residual disclosed, not closed) |
| AC2 - "Irreversible purge is distinguishable from vendor-native conditionally reversible operations." | `record_purge_tombstone_refuses_every_combination_except_delete_plus_irreversible` is the exhaustive falsification: all 5 `ActionClass` x 6 `Reversibility` combinations (30 total) `cancellai-model`'s shared vocabulary admits are exercised; only `Delete + Irreversible` is accepted and produces a `Purged` event, every other combination - including `Reversibility::VendorConditional` (the vocabulary's own name for a vendor-native conditionally-reversible outcome) paired with every action class, and `Quarantine`/`Archive`/`Restore`/`Observe` paired with every reversibility - is refused with `PurgeTombstoneError::NotAnIrreversiblePurge` and writes nothing. | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Privacy field allowlist test" | `record_purge_tombstone_round_trips_exactly_the_given_allowlisted_fields` (see AC1). | PASS |
| "Purge evidence tests" | `record_purge_tombstone_accepts_delete_plus_irreversible_and_appends_a_purged_event` and `record_purge_tombstone_refuses_every_combination_except_delete_plus_irreversible` (see AC2); `record_purge_tombstone_refuses_empty_plan_id_and_empty_evidence_ids` proves the ledger's own mutation-reference fail-closed contract (E13-S02) still applies at this API's boundary - an empty `plan_id`/`evidence_ids` is refused and writes nothing even when the action/reversibility check already passed. | PASS |

## Adversarial-cases pass (CR4)

Falsification axes worked before implementation (`adversarial-cases` skill):

| # | Axis | Case | Expected | Test |
| --- | --- | --- | --- | --- |
| 7 | Boundary | Empty `plan_id` / empty `evidence_ids` | Refused, nothing written | `record_purge_tombstone_refuses_empty_plan_id_and_empty_evidence_ids` |
| 8 | Second-path / SI-020 | Every `ActionClass` x `Reversibility` combination (30) except `Delete+Irreversible` | Refused, nothing written, including `VendorConditional` | `record_purge_tombstone_refuses_every_combination_except_delete_plus_irreversible` |
| 10 | Malformed/untrusted | Round-trip of a fully-populated, content-safe tombstone | Exactly and only the given fields persist | `record_purge_tombstone_round_trips_exactly_the_given_allowlisted_fields` |
| 10 | Malformed/untrusted (round 1 repair) | A prompt, source-text, or absolute/relative path sentinel in each of the 3 remaining fields (9 cases) | Refused as `UnsafeField`, nothing written | `record_purge_tombstone_refuses_prompt_source_and_path_sentinels_in_every_remaining_field` |
| 10 | Malformed/untrusted (round 1 repair) | Codex's exact round-1 reproduction, restricted to the fields that remain | Refused, nothing written | `record_purge_tombstone_refuses_the_codex_round1_reproduction` |
| 10 | Malformed/untrusted (round 2 finding, disclosed residual) | A short, ordinary, identifier-shaped phrase (`do-not-purge-this`) that fits every bound `validate_content_safe` enforces | **Accepted** - deliberately, as a documented residual, not a regression | `record_purge_tombstone_accepts_a_short_ordinary_phrase_as_a_disclosed_residual` |
| second-path (round 3 repair) | A caller constructs `EventKind::Purged` directly via public `EventLedger::append`, bypassing `record_purge_tombstone` entirely, with every removed annotation field populated with a sentinel | Refused by `append` itself, nothing written | `append_refuses_the_direct_public_bypass_round3_independent_review_found` (`ledger.rs`) |
| second-path (round 3 repair) | Direct `append` with no annotation fields but a non-identifier-shaped `artifact_id` | Refused by `append` itself | `append_refuses_a_purged_event_whose_linkage_fields_are_not_identifier_shaped` (`ledger.rs`) |
| second-path (round 3 repair) | Direct `append` with the exact shape `record_purge_tombstone` itself produces | Accepted - the boundary narrows what can be smuggled in, not the one legitimate shape | `append_accepts_a_purged_event_with_no_annotations_and_identifier_shaped_linkage` (`ledger.rs`) |
| 1,2,3,4,9,11 | Path/identity, partial reads, links/mounts, provider drift, platform, scale | N/A - this module never touches a path or the filesystem; it operates only on an already-open `&mut EventLedger` and already-decided `cancellai-model` enum values | - |
| 5 | Concurrency | Two concurrent `record_purge_tombstone` calls on one ledger | Impossible by construction - `&mut EventLedger` excludes aliasing at the borrow-checker level | - |
| 6 | Crash/retry | Crash between a real OS purge succeeding and a tombstone being written | Not this story's mechanism to close - no orchestrator calls `record_purge_tombstone` from a real purge yet (disclosed residual, below) | - |
| second-path | Does the local action_class/reversibility check duplicate `cancellai_safety::authority::reversibility_allowed` as an independent authorization decision? | No - it is a strictly narrower predicate (accepts only the one combination that check's `Delete` arm accepts), takes no `AuthorityLevel`, and decides only whether an already-completed, already-authorized outcome may be labeled a purge tombstone | Documented in `tombstone.rs`'s module doc and `docs/architecture/PERSISTENCE_MODEL.md` |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-020 ("Irreversible actions are explicit and stronger-gated") | Every non-`Delete+Irreversible` `ActionClass`/`Reversibility` combination, including `VendorConditional` | `record_purge_tombstone_refuses_every_combination_except_delete_plus_irreversible`: 29 of 30 combinations refused with `NotAnIrreversiblePurge` and zero ledger rows written; only `Delete+Irreversible` produces a `Purged` event | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                              # PASS
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings   # PASS
cd rust && cargo check --workspace --all-targets                          # PASS
cd rust && cargo test --workspace                                         # PASS - 0 failed (4 new cancellai-store::tombstone tests)
cd rust && cargo deny check                                               # PASS - advisories, bans, licenses, sources OK; no new dependency
python3 scripts/check_mutation_boundary.py check                          # PASS - only rust/crates/cancellai-platform/src/mutation.rs deletes anything
python3 scripts/check_schemas.py check                                    # PASS
python3 scripts/check_fixtures.py check                                   # PASS
python3 scripts/project_os.py check                                       # PASS
```

Not run: the Windows-target and Linux-target cross-compiled clippy passes AGENTS.md calls out for
changes to `cancellai-platform` or anything moving the workspace-wide lint surface - this story
touches neither `cancellai-platform` nor the workspace lint policy; CI's existing per-platform
`cargo check`/quality matrix covers `cancellai-store` on macOS, Linux and Windows as it already
does for E13-S01 through E13-S06. `pre-commit run --all-files` was not run standalone in this
session; the individual Python checks it bundles that are relevant to this diff's scope
(`check_mutation_boundary`, `check_schemas`, `check_fixtures`, `project_os check`) were run
directly above and passed - CI runs the full hook set before merge regardless.

## Repair - round 1 independent review finding

Codex's round-1 review (`project/evidence/E12-S04-VERIFIER-REVIEW.md`) FAILed AC1: `Tombstone`'s
fields were plain, unvalidated `String`/`EvidenceId` values, and `record_purge_tombstone` passed
them unchanged to `EventLedger`. The evidence packet's original AC1 argument ("no path, prompt,
or content-typed field exists on the struct") conflated a *schema-column* allowlist with
*content* validation of those columns' untyped `String` payloads - the same class of error, at a
different layer, as E13-S04's marker-mimicry gap. An independent reproduction stored
`PROMPT_SENTINEL_do_not_store_source_contents` in `provider_id`/`reason_code`/`policy_id`/
`plan_id`/an evidence ID and `/private/provider/source.rs` in `category`, and `EventLedger::read_all`
returned every sentinel unchanged.

Repair (round 1): `tombstone.rs` added `validate_content_safe`/`validate_optional_content_safe`,
called on every caller-supplied field before `EventLedger::append` (atomic no-write on refusal by
construction). A value was accepted only if empty or shaped like every real ID this system
produces: ASCII letters/digits joined by up to 4 single hyphens, ≤32 characters per segment, ≤64
total - a shape check, not only a character-class check, since a character allowlist alone cannot
distinguish `policy-0001` from an attacker's message re-encoded with hyphens.

## Repair - round 2 finding: scope reduction (not a third syntactic patch)

Codex's round-2 review (`project/evidence/E12-S04-VERIFIER-REVIEW-ROUND2.md`) FAILed AC1 again:
`validate_content_safe` accepts any 1-4-word ASCII hyphen-joined string, and the ordinary short
phrase `do-not-purge-this` is one of those - accepted and persisted through every field, exactly
as meaningful as before, just shaped differently. Round 2's own conclusion: "a lexical
length/character/segment predicate is insufficient because valid-looking short natural language
is content." This is a structural limit of syntactic validation over free text, not a bug in one
predicate's specific bounds - tightening the bounds again would only invite the same finding with
a shorter phrase.

Per the orchestrator's policy (at most two independent review rounds per approach before the next
attempt must be a different kind of solution, not a third patch of the same kind), this repair is
a scope reduction rather than a tighter validator: **`provider_id`, `category`, `reason_code`, and
`policy_id` are removed from `Tombstone` entirely.** These were purely descriptive annotation
fields, not structurally required to identify what was purged, and `reason_code` in particular is
definitionally an invitation to explain in words - exactly the shape of field this class of attack
targets. This mirrors this crate's own established precedent for a mechanism defeated repeatedly
in successive review rounds (`docs/architecture/PERSISTENCE_MODEL.md` cites E12's own
`recover_pending_moves` rounds 3-5): remove the mechanism rather than patch it a further time.

`artifact_id`, `plan_id`, and `evidence_ids` remain, because a purge record naming neither what was
purged nor which plan/evidence produced it is not a tombstone - these are structurally required,
not merely lower-risk. They keep the same `validate_content_safe` shape check (still narrows
obvious prompt/source/path content) but this is now explicitly documented, in `tombstone.rs`'s
module doc, this evidence packet, and `docs/architecture/PERSISTENCE_MODEL.md`, as a **disclosed
residual, not a closed guarantee**: `record_purge_tombstone_accepts_a_short_ordinary_phrase_as_a_disclosed_residual`
is a passing test that a short ordinary phrase (`do-not-purge-this`) is still accepted in
`plan_id`, kept deliberately rather than hidden, so the residual stays visible to a future reader
instead of silently regressing. Full closure needs a future orchestrator story that sources these
three values from the system's own already-validated purge/evidence records instead of accepting
them from an arbitrary public caller - out of this story's scope, since no orchestrator exists yet
(see "No orchestrator yet" below).

The per-field sentinel sweep and the round-1 reproduction test were narrowed to the three
remaining fields; the round-1 "hyphen-joined-words" test was removed because it asserted exactly
the property round 2 falsified (that segment-count bounding closes the gap) - it is replaced by
the residual-disclosure test above, which asserts the actual, honest current behavior.

## Repair - round 3 finding: close the public-ledger bypass, then narrow AC1 by owner decision

Codex's round-3 review (`project/evidence/E12-S04-VERIFIER-REVIEW-ROUND3.md`) found two separate
things, and both are addressed here rather than one being mistaken for a fix to the other:

1. **A concrete, fixable bypass.** `record_purge_tombstone`'s validation was reachable around:
   `cancellai_store::ledger::{EventLedger, NewEvent, EventMetadata, EventKind}` are all public, so
   a caller could construct an `EventKind::Purged` `NewEvent` directly and hand `append` the exact
   sentinels two prior rounds had already closed at the narrower API - reconstructing round 1's
   channel and doing so with none of `record_purge_tombstone`'s checks. **Fix:** `EventLedger::
   append` now enforces, for `EventKind::Purged` specifically, that `provider_id`/`category`/
   `policy_id`/`reason_code` are all absent and that `artifact_id`/`plan_id`/`evidence_ids` are
   each `crate::tombstone::is_identifier_shaped` (the same predicate `validate_content_safe`
   already used, extracted to a `pub(crate)` function both modules call) - refusing with nothing
   written otherwise. This is the actual persistence boundary every write path must call, so the
   check is no longer optional for any caller. New tests reproduce round 3's exact bypass and
   confirm refusal (`ledger.rs`, see Adversarial-cases table).
2. **A policy question requiring an owner decision, not more code.** Round 3 held, independently
   of the bypass, that a verifier cannot treat a "disclosed residual" as satisfying an unqualified
   AC1 on its own authority - correctly: a residual is an accepted gap, not proof the gap is
   closed, and only an owner can accept it against the acceptance criterion itself. Per round 3's
   own first offered path, the owner narrowed AC1 by decision, recorded as
   [ADR-0033](../../../docs/adrs/0033-purge-tombstone-content-safety-is-a-disclosed-residual.md):
   `project/epics/E12.json`'s AC1 now states exactly the property this primitive can make and
   test (no descriptive annotations at all; a disclosed, not closed, residual for the three
   linkage fields; full closure deferred to a future orchestrator story). This is not the
   executor asserting its own implementation is good enough - the decision is the owner's, made
   through the control plane and recorded in an ADR precisely because a verifier's own review
   record said it could not make this call.

## Compatibility

- No schema change, no new dependency, no wire-format change. `Tombstone` and
  `record_purge_tombstone` are new, additive public items; no existing public API changed
  signature or behavior. `cancellai-cli`/`cancellai-guardian` still do not call any of this from
  source - no live caller is wired yet, matching every prior E12/E13 story's own "primitive
  delivered, no orchestrator yet" precedent.

## Performance / operability

- `record_purge_tombstone` costs exactly one `EventLedger::append` call (one SQLite transaction,
  already the ledger's own established cost) plus a cheap enum comparison - no measurable
  additional cost.

## Documentation updated

- `docs/adrs/0033-purge-tombstone-content-safety-is-a-disclosed-residual.md`: new ADR recording
  the owner decision narrowing AC1.
- `project/epics/E12.json`: AC1 text narrowed; `ac_narrowing_note` added explaining why and
  pointing to ADR-0033.
- `docs/architecture/PERSISTENCE_MODEL.md`: rewrote the "Tombstones" implementation paragraph -
  the narrowed field set, why the independent-review findings led to removal rather than a third
  validator, the closed `EventLedger::append` bypass, and the ADR-0033 reference.
- `rust/crates/cancellai-store/src/tombstone.rs`: module doc references ADR-0033.
- `CHANGELOG.md`: updated the E12-S04 `### Added` entry under `[Unreleased]` to match.

## Method defects

- none

## Residual risks

- **AC1, as narrowed by ADR-0033, still carries a disclosed residual for `artifact_id`/`plan_id`/
  `evidence_ids`.** `validate_content_safe`/`is_identifier_shaped` narrows exposure (rejects an
  obvious prompt, source fragment, or path) but cannot prove the absence of all meaningful content
  - a short, ordinary, identifier-shaped phrase (`do-not-purge-this`) is accepted, and the same
  capacity could carry a short secret or instruction. This is now the *accepted* scope of AC1
  (not a gap against it): ADR-0033 records the owner decision to accept this residual until a
  future orchestrator story derives these three values from the system's own already-validated
  purge/evidence records instead of accepting them from an arbitrary public caller. Tracked here
  rather than in a new story ID, since no orchestrator work is currently scheduled to carry it;
  whoever schedules that orchestrator should also carry this closure.
- **`provider_id`/`category`/`reason_code`/`policy_id` are no longer recorded at all**, reducing
  the tombstone's audit/analytics usefulness the architecture doc's illustrative field list
  describes (e.g. "how many purges were `policy-expired`" is no longer answerable from this
  record). Reintroducing them needs a real closed vocabulary or an orchestrator-verified source,
  not caller-supplied free text - a larger, separately reviewable change this story's acceptance
  criteria do not require.
- **"Size/reclaim observation,"** one item `docs/architecture/PERSISTENCE_MODEL.md`'s "Tombstones"
  section names illustratively ("such as"), has no column in the ledger's already schema-pinned
  event table and is not carried by `Tombstone`. Widening `ledger_events`' pinned schema for a
  per-artifact figure is a larger, separately reviewable change this story's acceptance criteria
  do not require. `AnalyticalMemory`'s `ReclaimableBytes` metric already tracks reclaimable bytes
  as an aggregate time series (Layer 3), a different granularity (aggregate, not per-tombstone).
- **No orchestrator wires `record_purge_tombstone` to a real purge yet.** A crash between
  `mutation_executor::execute`'s `ActionClass::Delete` branch (E03-S05) succeeding and a future
  caller invoking this function to record the tombstone is that future orchestrating story's
  failure mode to close, not this one's - matching E13-S04's own "No CLI/TUI/Guardian surface
  calls any of this yet" precedent.

## Verifier verdict

Round 1: **FAIL** (Codex) - `project/evidence/E12-S04-VERIFIER-REVIEW.md`.
Round 2: **FAIL** (Codex) - `project/evidence/E12-S04-VERIFIER-REVIEW-ROUND2.md`. Addressed by
scope reduction (a new design, not a third patch of round 1/2's approach).
Round 3: **FAIL** (Codex) - `project/evidence/E12-S04-VERIFIER-REVIEW-ROUND3.md`. Confirmed the
scope reduction was genuine but found it reachable around via public `EventLedger::append`, and
held that a disclosed residual cannot itself satisfy an unqualified AC1 without an owner decision.
Addressed above: the bypass is closed in code, and AC1 is narrowed by owner decision
(ADR-0033) rather than by verifier or executor assertion.
Round 4 (of the fresh, at-most-two-round budget this repair opens) pending, against the narrowed
AC1 and closed bypass.

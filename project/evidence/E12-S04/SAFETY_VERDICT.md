# Safety Verdict - E12-S04

Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
Verifier: Codex

- Change: Purge tombstone record primitive
- Risk: CR4
- Review target: `f215d33..8afcebc`
- Independent verifier: Codex
- Verifier: Codex
- Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
- Date: 2026-09-20

## Verdict

`FAIL`

## Safety surface changed

The change adds a public persistence path for a record labeled `PURGED`, intended to retain only
minimal contentless evidence after an irreversible action. It does not wire a production purge
to that path yet.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-020 | An irreversible purge is explicit, stronger-gated, and not disguised as cleanup metadata. | The action/reversibility pairing is correctly restricted, but the retained `PURGED` metadata accepts and stores prompt/source/path sentinels through arbitrary strings. | FAIL |
| C-09 | Persistent state must not copy prompts, source code, secrets, or file contents by default. | Independent public-API reproduction stored `PROMPT_SENTINEL_do_not_store_source_contents` and `/private/provider/source.rs` in the tombstone event. | FAIL |

## Adversarial cases

- `Delete + Irreversible` with prompt sentinel in `provider_id`, `reason_code`, `policy_id`,
  `plan_id`, and evidence ID, and an absolute source-like path in `category`: accepted and read
  back unchanged from `EventLedger`.
- The executor's 30 action/reversibility combinations confirm distinguishability, but do not
  examine adversarial values in any retained field.

## Differential / compatibility evidence

- No Python reference behavior is changed.
- Rust format, clippy, check, workspace tests, and cargo-deny all passed; those gates did not
  detect the semantic privacy violation.

## Known residual risks

- Until repaired, any caller can permanently retain arbitrary payload in a purportedly
  contentless purge tombstone.
- A future integration must separately make real purge plus tombstone recording crash/retry safe;
  this unwired primitive cannot prove that end-to-end property.

## Rollback / recovery

Do not invoke or integrate `record_purge_tombstone` until content-safe fields and regressions are
implemented. The story is returned to `in_progress`; no existing provider mutation path is
changed by this review.

## Owner decision

`REJECT`

Owner note: Independent verifier rejection. Repair and re-review are required before a CR4
Safety Verdict may record a passing result.

## Round 3 independent review

- Review target: `1ed7d24^..1ed7d24`
- Verifier: Codex
- Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
- Date: 2026-09-20
- Verdict: `FAIL`

The reduction removes `provider_id`, `category`, `reason_code`, and `policy_id` from the typed
`Tombstone` struct, but it cannot discharge AC1/C-09: all three remaining references are
caller-constructed strings and the deliberately accepted `do-not-purge-this` phrase persists.
Further, `EventLedger::append(NewEvent { kind: EventKind::Purged, .. })` and its public
`EventMetadata` fields reconstruct every removed annotation directly, including the original
prompt/path sentinels, and bypass the typed helper's SI-020 pairing predicate.

| Invariant | Required property | Round 3 evidence | Result |
| --- | --- | --- | --- |
| SI-020 | A permanent purge record is explicit, stronger-gated, and not disguiseable as cleanup metadata. | The typed helper has the right pair check, but public direct `PURGED` ledger append has neither that check nor the narrowed metadata surface. | FAIL |
| C-09 | Persistent state does not copy prompts, source code, secrets, or file contents by default. | Public direct append persisted a prompt sentinel and `/private/provider/source.rs`; the typed helper also persists the documented short phrase in each remaining field. | FAIL |

Required disposition: keep E12-S04 `in_progress`. An owner must either explicitly redefine AC1
through the control plane or schedule an orchestrator/reference design that makes the three
references authority-bound and prevents direct construction of `PURGED` events outside that
boundary. A disclosed residual does not itself waive AC1.

## Executor note - repair for round 4 (not a verdict)

Two things changed since round 3, ahead of the next independent review round:

1. **The `EventLedger::append` bypass is closed.** `append` now enforces, for
   `EventKind::Purged` specifically, that `provider_id`/`category`/`policy_id`/`reason_code` are
   all absent and that `artifact_id`/`plan_id`/`evidence_ids` are each identifier-shaped - the
   identical predicate `record_purge_tombstone` already applied, now unbypassable because it
   lives in the one function every write path must call. New regression tests reproduce round
   3's exact direct-append reproduction and confirm it is now refused
   (`rust/crates/cancellai-store/src/ledger.rs`,
   `append_refuses_the_direct_public_bypass_round3_independent_review_found`).
2. **AC1 is narrowed by owner decision, recorded as `docs/adrs/0033-purge-tombstone-content-safety-is-a-disclosed-residual.md`.**
   Per round 3's own first offered path ("either an owner-approved control-plane change narrows
   AC1... or a new orchestrator/authority-bound-reference story"), the owner chose the former:
   `project/epics/E12.json`'s AC1 no longer reads as an unqualified guarantee. It now states
   exactly the property this primitive can make and test - no descriptive annotation fields, and
   a disclosed (not closed) residual for the three structurally required linkage fields, with
   closure explicitly deferred to a future orchestrator story. This is the owner decision round
   3 said a verifier cannot make on its own; it is made here, on the record, not asserted by the
   executor.

Round 4 should judge the current diff against the narrowed AC1 in `project/epics/E12.json` and
ADR-0033, not against the original unqualified reading rounds 1-3 correctly falsified.

## Round 4 independent review

- Review target: repair commit `4ce61c3` (`4ce61c3^..4ce61c3`); full story history `f215d33..4ce61c3`
- Verifier: Codex
- Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
- Date: 2026-09-20
- Verdict: `FAIL`

ADR-0033 is a legitimate, owner-authorized narrowing of AC1: its resulting property is explicit,
testable, and the remaining three caller-supplied linkage fields' short-phrase capacity is
accurately disclosed rather than treated as a closed guarantee. The repair also closes the
round-3 content/annotation bypass at the actual persistence boundary: a public direct
`EventLedger::append` carrying `provider_id`, `category`, `policy_id`, or `reason_code` now
refuses before writing. The supplied verifier brief remains checksum-valid but stale on the
original AC1 wording; this verdict therefore judges the authoritative current control-plane text
and ADR-0033, not the brief's superseded wording.

| Invariant | Required property | Round 4 evidence | Result |
| --- | --- | --- | --- |
| AC1 / C-09 (as narrowed) | No descriptive annotations on any persisted `PURGED` event; linkage fields receive the identical disclosed-residual shape check through either public route. | Independent public-API reproduction of the old direct-append sentinel route was refused and left the ledger empty. Source inspection confirms all four annotations are rejected in `EventLedger::append`; helper and direct append call the same `is_identifier_shaped` predicate. | PASS |
| AC2 / SI-020 | A `PURGED` event may only represent `ActionClass::Delete + Reversibility::Irreversible`, never a conditionally reversible action. | Independent public-API reproduction directly appended a well-shaped `EventKind::Purged` record successfully. `EventLedger::append` receives no `ActionClass` or `Reversibility` and therefore cannot enforce the helper's pairing predicate. The event is persisted as `PURGED` without proof it was an irreversible delete. | FAIL |

The direct append acceptance is the SI-020 part of the round-3 bypass that the repair did not
close. It remains a public alternate path that can label a vendor-native conditional operation as
an irreversible purge. The exact required repair is to make every public construction/write path
for `EventKind::Purged` carry and enforce the `Delete + Irreversible` proof, or to make direct
construction of `PURGED` impossible outside a controlled API that enforces that proof. Merely
checking metadata fields or identifier shape does not satisfy AC2/SI-020.

Empty `plan_id` and empty `evidence_ids` remain fail-closed before writing through both routes;
the pre-existing mutation-reference contract is intact. No production caller currently depends on
the removed annotation fields for a `PURGED` event; the workspace search found no production
`EventKind::Purged` construction outside this store implementation and its tests.

The story returns to `in_progress`. No code repair was attempted by this verifier.

## Owner decision — Round 4

`REJECT`

Owner note: Independent CR4 review rejects closure because the direct public `PURGED` write path
does not enforce AC2/SI-020. E12-S04 remains `in_progress` pending the exact repair stated above.

## Executor note - repair for round 5 (not a verdict)

Round 4's exact required repair is applied: `EventLedger::append` now refuses `EventKind::Purged`
unconditionally (not "unless well-shaped" - always), with no exception. The previous
`Purged`-specific content-shape check moved to a new crate-private `EventLedger::append_purged`,
reachable only from within `cancellai-store` - in practice, only from
`crate::tombstone::record_purge_tombstone`, which has already checked `ActionClass::Delete +
Reversibility::Irreversible` before calling it. There is now exactly one path to a written
`Purged` row anywhere in this crate, and it is gated by that pairing check before it is reached -
satisfying round 4's stated repair ("make every public construction/write path for
`EventKind::Purged` carry and enforce the `Delete + Irreversible` proof, or make direct
construction of `PURGED` impossible outside a controlled API that enforces that proof") via the
second option. New tests (`ledger.rs`) prove `append` refuses a well-shaped `Purged` event with no
pairing proof, and that `append_purged` still applies the content-shape check as defense in depth.

Round 5 should judge whether this closes AC2/SI-020's "no second path to the `PURGED` label"
property completely, and re-confirm AC1/ADR-0033 and the round-3 content bypass remain closed (no
change was made to that logic beyond moving it into `append_purged`).

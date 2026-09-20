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

## Round 5 independent review

- Review target: repair commit `31cf69d` (`31cf69d^..31cf69d`); full story history `f215d33..31cf69d` on `main`
- Verifier: Codex
- Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
- Date: 2026-09-20
- Verdict: `FAIL`

Round 4's exact public reproduction was independently re-run: a well-shaped `NewEvent` with
`EventKind::Purged`, absent annotations, and valid artifact/plan/evidence linkage passed to the
public `EventLedger::append` is now refused and leaves `read_all()` empty. An external integration
test that attempted `ledger.append_purged(event)` failed to compile with `E0624` because the
method is `pub(crate)`. A workspace search found the sole production `INSERT INTO ledger_events`
in the private `append_row`, and the sole call of `append_purged` in
`tombstone::record_purge_tombstone`; neither CLI nor Guardian references this ledger. Thus every
currently writable `PURGED` row reaches `record_purge_tombstone`'s prior
`ActionClass::Delete + Reversibility::Irreversible` check.

| Invariant | Required property | Round 5 evidence | Result |
| --- | --- | --- | --- |
| SI-020 / AC2 | A `PURGED` record cannot label a conditionally reversible or otherwise unproven operation as an irreversible purge. | Public `append` refuses `Purged` unconditionally; the only current writer is crate-private `append_purged`, reached solely after `record_purge_tombstone` checks `Delete + Irreversible`. | PASS |
| AC1 / C-09, narrowed by ADR-0033 | No descriptive annotations persist; linkage rejects obvious prompt/source/path forms while retaining the accepted short-phrase residual. | `append_purged` requires all four annotations absent and identifier-shaped linkage before `append_row`; its tests and the tombstone sentinel tests pass. `do-not-purge-this` remains deliberately accepted exactly as ADR-0033 records. | PASS_WITH_ACCEPTED_RESIDUAL |
| E13-S02 mutation-reference contract | Every mutation-class event requires a non-empty plan ID and at least one evidence ID. | Both public `append` and `append_purged` delegate to `append_row`, whose `EventKind::is_mutation` check rejects absent/blank plan IDs and empty evidence lists before insertion. | PASS |

All requested code-quality and evidence checks passed before the final state transition: `cargo fmt --check`; `cargo clippy --workspace --all-targets
--all-features -- -D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace`;
`cargo deny check`; `python3 scripts/project_os.py check`; `python3 scripts/check_process.py check`;
`python3 scripts/check_evidence.py check`; `python3 scripts/verifier_handoff.py check`; and
`python3 scripts/check_docs.py check`. `cargo deny` and the Python process/evidence checks report
only their pre-existing, recorded warnings. However, after setting E12-S04 to `done`,
`python3 scripts/project_os.py generate` failed: its `safety_verdict_passes` gate rejects an
append-only Safety Verdict file whenever any historical line is exactly `FAIL` or `REJECT`, even
when the final Round 5 verdict is passing. The protocol requires those historical rounds to remain.
This is a CR4 delivery-gate failure under C-16, so the story cannot close in this review.

## Owner decision — Round 5

`REJECT`

Owner note: The SI-020 second-path bypass is closed and ADR-0033's residual remains accepted, but
the required CR4 closure gate cannot accept an append-only verdict history. E12-S04 remains
`in_progress` pending a separately reviewed repair to the safety-verdict gate.

## Round 6 independent review

- Review target: `31cf69d..86ec71e` on `main`
- Verifier: Codex
- Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
- Date: 2026-09-20

`FAIL`

Round 5's AC1/ADR-0033, AC2/SI-020, and E13-S02 mutation-reference conclusions remain intact;
there is no `rust/` diff since `31cf69d`. E32-S01 fails independent review because its raw-text
parser lets a verdict-shaped line inside a fenced code block control the verdict. A genuine
current `REJECT` followed by a fenced illustrative `PASS` returns true. The gate therefore does
not yet read the newest attributable verdict and cannot authorize E12-S04 closure.

## Owner decision — Round 6

`REJECT`

Owner note: E12-S04 remains `in_progress`; the CR4 closing-gate parser defect in E32-S01 must not
be treated as a passing Safety Verdict.

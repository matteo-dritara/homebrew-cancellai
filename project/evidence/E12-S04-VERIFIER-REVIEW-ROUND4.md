# E12-S04 Verifier Review — Round 4

Review-Scope: story
Round: 4
Review target: repair commit `4ce61c3` (`4ce61c3^..4ce61c3`); full story history `f215d33..4ce61c3` on `main`
Verifier: Codex
Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
Date: 2026-09-20

## Verdict

`FAIL`

The round-3 content/annotation bypass is closed at the public persistence boundary, and
ADR-0033 is a legitimate owner decision that makes the narrowed AC1 explicit and testable. The
repair does not close the independent AC2/SI-020 part of round 3's direct-ledger finding:
`EventLedger::append` still accepts a directly constructed, well-shaped `EventKind::Purged`
event without any `ActionClass::Delete + Reversibility::Irreversible` proof. It takes neither
value, so it cannot distinguish a genuine irreversible deletion from an event a caller chooses to
label `PURGED`. This remains a public alternate persistent-record path around the typed helper's
pairing gate.

E12-S04 returns to `in_progress`. This is the final permitted fresh-budget round; no repair or
further review round was attempted.

## ADR-0033 judgment and verifier brief

ADR-0033 holds up on inspection. It records an actual owner decision, narrows AC1 to the exact
property a caller-facing primitive can test, identifies the remaining capacity for short ordinary
phrases as an accepted residual, and requires a future orchestrator to derive the linkage fields
from trusted records for full closure. That is a legitimate closure of the original AC1 concern;
the disclosed residual is not grounds for this FAIL.

`project/evidence/E12-S04/VERIFIER_BRIEF.md` is checksum-valid and still identifies the correct
story, CR4 risk, outcome, and SI-020 obligation, but its AC1 section has the original unqualified
wording. It is stale on that owner-authorized contract change. This review repeats its required
checksum while judging against the authoritative current `project/epics/E12.json` AC1 and
ADR-0033.

## Independent reproductions

An uncommitted external integration test, using only public `cancellai-store` and
`cancellai-model` APIs, was compiled, run, and then removed:

```text
cd rust && cargo test -p cancellai-store --test e12_s04_round4_verifier --offline
```

| Re-attempt | Outcome |
| --- | --- |
| Round-3 exact direct `EventLedger::append` reproduction: `PURGED` with all four former annotation fields populated (`PROMPT_SENTINEL_do_not_store_source_contents`, `/private/provider/source.rs`, `policy-free-text`, `do-not-purge-this`) | Refused by `append`; `read_all()` remained empty. The content/annotation bypass is closed. |
| Direct `append` with annotations absent and identifier-shaped artifact/plan/evidence linkage | Accepted and persisted as `PURGED`. This is correct for narrowed AC1, but the public API contains no `ActionClass`/`Reversibility` input and therefore no SI-020 pairing proof. |

## Further adversarial checks

- Both public paths use `crate::tombstone::is_identifier_shaped`: the helper uses it through
  `validate_content_safe`, and `EventLedger::append` calls it directly for a `PURGED` event's
  artifact (when present), plan, and every evidence ID. Obvious prompt/source/path forms are
  refused in either route; `do-not-purge-this` remains an accepted residual exactly as
  ADR-0033/AC1 disclose.
- All four former descriptive fields are genuinely unreachable in a persisted `PURGED` event:
  `EventLedger::append`, the sole row-writing API, refuses if any is `Some`; the helper emits
  `None` for each. Workspace search found no separate raw SQLite write path or production
  `PURGED` constructor.
- The E13-S02 mutation-reference contract remains intact: missing mutation, blank `plan_id`, or
  empty `evidence_ids` is refused before the `PURGED`-specific check and leaves no row.
- The helper's exhaustive 30-combination test remains meaningful only for its own API. It cannot
  establish AC2 for every persisted `PURGED` event while public direct append accepts the label
  without the pair. No existing production code relies on the four removed annotations.

## Required repair

Close the remaining public `PURGED` representation bypass. A direct public writer must either be
unable to construct `EventKind::Purged`, or it must provide a non-forgeable/validated proof of
`ActionClass::Delete + Reversibility::Irreversible` which the write boundary enforces. The repair
must demonstrate that a vendor-native conditionally reversible outcome cannot be persisted as
`PURGED`, including through the direct-ledger route. This is required by AC2 and SI-020.

## Safety obligations

| Obligation | Evidence | Result |
| --- | --- | --- |
| AC1 / C-09, narrowed by ADR-0033 | Direct public append refuses any populated `provider_id`, `category`, `policy_id`, or `reason_code`; all linkage fields follow the shared predicate. | PASS_WITH_ACCEPTED_RESIDUAL |
| AC2 / SI-020 | Direct public append writes a well-shaped `PURGED` event without receiving or checking the required irreversible-delete pair. | FAIL |
| E13-S02 mutation-reference contract | Missing mutation, blank plan, and empty evidence references refuse before write. | PASS |

## Gate status

| Command | Result |
| --- | --- |
| `cd rust && cargo test -p cancellai-store --test e12_s04_round4_verifier --offline` | PASS as independent public-API reproduction; temporary test removed. |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS (124 `cancellai-store` unit tests; workspace green). |
| `cd rust && cargo deny check` | PASS; pre-existing unmatched-license and duplicate warnings only. |
| `python3 scripts/project_os.py check` | PASS after status transition and regeneration. |
| `python3 scripts/check_process.py check` | PASS; documented baseline review-round warnings only. |
| `python3 scripts/check_evidence.py check` | PASS; documented baseline evidence warnings only. |
| `python3 scripts/verifier_handoff.py check` | PASS; accepts this Codex verdict and its brief checksum. |
| `python3 scripts/check_docs.py check` | PASS |

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`;
`project/epics/E12.json`; `project/evidence/E12-S04/VERIFIER_BRIEF.md`;
`project/evidence/E12-S04/EVIDENCE.md`; `project/evidence/E12-S04/SAFETY_VERDICT.md`;
`project/evidence/E12-S04-VERIFIER-REVIEW.md`;
`project/evidence/E12-S04-VERIFIER-REVIEW-ROUND2.md`;
`project/evidence/E12-S04-VERIFIER-REVIEW-ROUND3.md`;
`project/templates/SAFETY_VERDICT.md`;
`docs/adrs/0033-purge-tombstone-content-safety-is-a-disclosed-residual.md`;
`docs/architecture/PERSISTENCE_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`;
`docs/security/THREAT_MODEL.md`; `docs/development/ENGINEERING_SYSTEM.md`;
`docs/development/AGENT_PROTOCOL.md`; `rust/crates/cancellai-store/src/tombstone.rs`;
`rust/crates/cancellai-store/src/ledger.rs`; `rust/crates/cancellai-store/src/lib.rs`;
`rust/crates/cancellai-safety/src/authority.rs`; and
`rust/crates/cancellai-safety/src/mutation_executor.rs`.

## Overall verdict

`FAIL` — the narrowed AC1 is met with ADR-0033's accepted residual and the direct content bypass
is closed, but E12-S04 does not meet AC2/SI-020 until direct construction of a `PURGED` event is
bound to the irreversible-delete representation.

# E12-S04 Verifier Review — Round 3

Review-Scope: story
Round: 3 (fresh review of a structurally different design - not a third patch of round 1/2's approach)
Review target: repair commit `1ed7d24` (`1ed7d24^..1ed7d24`); full story history `f215d33..1ed7d24` on `main`
Verifier: Codex
Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
Date: 2026-09-20

## Verdict

`FAIL`

The field reduction is real at the `Tombstone` struct boundary: it now exposes only
`artifact_id`, `plan_id`, and `evidence_ids`; the four former descriptive fields are absent, and
`record_purge_tombstone` writes `None` for all four `EventMetadata` annotations. It is therefore
an honest reduction of that one public struct's surface, not a rename/default masquerading as a
removal.

It is not an honest closure of AC1. AC1 is an unqualified requirement that tombstones contain no
prompts/source/file contents; C-09 says the database does not copy those data classes by default.
Neither contract permits a record that knowingly accepts and persists a caller-supplied ordinary
phrase as a "disclosed residual." Documentation accurately labels the gap, but cannot convert an
unsatisfied acceptance criterion into a passing residual without a control-plane/owner decision.
The primitive cannot meet AC1 while `ArtifactId`, `EvidenceId`, and `plan_id` remain freely
constructible caller strings. Its own deliberately passing residual test proves the counterexample.

More importantly, the narrowing is reachable around: `Tombstone` is documented as exactly the
ledger's `EventKind::Purged` event, but `cancellai_store::ledger::{EventLedger, NewEvent,
EventMetadata, MutationReference, EventKind}` and `EventLedger::append` are public. An external
caller can append `EventKind::Purged` directly, populate all four supposedly removed fields with
arbitrary strings, and supply a non-empty plan/evidence reference. This bypasses both the typed
front door's shape predicate and its `Delete + Irreversible` condition. Thus it reconstructs the
round-1 channel and can label a vendor-conditional outcome as `PURGED`; AC1 and AC2/SI-020 are
not closed at the actual persisted-record boundary.

E12-S04 must remain `in_progress`. The required next step is not another lexical validator:
either an owner-approved control-plane change narrows AC1 to an explicitly stated, testable
primitive-only property, or a new orchestrator/authority-bound-reference story must close AC1.
The latter must make artifact/plan/evidence references non-forgeable to arbitrary callers and
must prevent direct public construction/appending of a `PURGED` event outside the controlled
factory/orchestrator. Until then, no primitive that accepts arbitrary public identifier strings
can satisfy AC1's closed contentlessness guarantee. This is a CR4 gate failure, not a residual
that a verifier may accept on its own.

## Independent reproductions

An uncommitted external integration test using only public `cancellai-store` and
`cancellai-model` APIs was compiled and run, then removed. It established all outcomes below.

| Re-attempt | Outcome |
| --- | --- |
| Round-1 sentinel, adapted to `artifact_id`, `plan_id`, and `evidence_ids`: `PROMPT_SENTINEL_do_not_store_source_contents` | Refused by `record_purge_tombstone`; `read_all()` was empty. The executor's narrow-API claim is correct. |
| Round-2 phrase `do-not-purge-this`, placed separately in each of the three remaining fields | Accepted and read back as a `PURGED` event in all three cases. This confirms the documented residual is real, but also disproves AC1. |
| Direct public ledger construction of a `PURGED` event with the old metadata fields | Accepted and read back unchanged: `provider_id = PROMPT_SENTINEL_do_not_store_source_contents`, `category = /private/provider/source.rs`, `policy_id = policy-free-text`, and `reason_code = do-not-purge-this`. This reconstructs the removed fields through `EventMetadata` and bypasses the typed API. |

The test command was:

```text
cd rust && cargo test -p cancellai-store --test e12_s04_round3_verifier --offline
```

It passed because it asserts the observed refusals, residual acceptance, and public-ledger bypass;
the temporary test file was deleted before this review commit and all repository gates.

## Further adversarial checks

- `EventMetadata` consumers and constructors were searched across `rust/`. There is no current
  production caller of `record_purge_tombstone`, but that does not make its public alternate
  `EventLedger::append` route unavailable; the public module exports permit it now. The typed
  helper itself sets the four metadata options to `None`, but does not restrict other callers.
- AC2's typed helper is correct in isolation: its exhaustive test permits only
  `ActionClass::Delete + Reversibility::Irreversible`. The public ledger route accepts
  `EventKind::Purged` without either value, so that helper cannot establish AC2 for all persisted
  purge records. This violates SI-020's requirement that permanent purge cannot be disguised as
  cleanup metadata.
- Empty `plan_id` and empty `evidence_ids` remain fail-closed: the helper delegates to
  `EventLedger::append`, whose mutation-reference contract rejects both without a row; existing
  tombstone tests and workspace tests passed. This part of the E13-S02 ledger contract remains
  intact.
- `mutation_executor::execute` was inspected. Its actual delete path retains its independent
  `Govern` authority and `Irreversible` reversibility checks. No current purge calls the
  tombstone helper, so this review finding does not create a new OS deletion path; it concerns
  the claimed contentless audit record and its public representation.

## Safety obligations

| Obligation | Evidence | Result |
| --- | --- | --- |
| AC1 / C-09 | A short meaningful phrase persists through each remaining caller-supplied field; direct public ledger append additionally persists prompt/path strings in all four removed annotations. | FAIL |
| AC2 / SI-020 | Typed helper accepts only `Delete + Irreversible`, but direct public `EventKind::Purged` append has neither pairing check nor metadata restriction. | FAIL |
| E13-S02 mutation-reference contract | Empty plan/evidence references still refuse before append. | PASS |

## Gate status

| Command | Result |
| --- | --- |
| `cd rust && cargo test -p cancellai-store --test e12_s04_round3_verifier --offline` | PASS as an adversarial reproduction: verified the documented narrow-API outcomes and the direct public-ledger bypass. Temporary test removed. |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS (121 `cancellai-store` unit tests; workspace green) |
| `cd rust && cargo deny check` | PASS after sandbox escalation required to acquire Cargo's advisory-DB lock; pre-existing unmatched-license and duplicate warnings only. |
| `python3 scripts/project_os.py generate`; `python3 scripts/project_os.py check`; `python3 scripts/check_process.py check`; `python3 scripts/check_evidence.py check`; `python3 scripts/verifier_handoff.py check` | PASS after the `in_progress` transition and regeneration. Process/evidence tools reported their documented baseline warnings only. |
| `gh run list --branch main --limit 5` | UNKNOWN: GitHub API connection failed at session start, so no CI state is claimed green. |

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`;
`project/epics/E12.json`; `project/evidence/E12-S04/VERIFIER_BRIEF.md`;
`project/evidence/E12-S04/EVIDENCE.md`; `project/evidence/E12-S04-VERIFIER-REVIEW.md`;
`project/evidence/E12-S04-VERIFIER-REVIEW-ROUND2.md`;
`project/evidence/E12-S04/SAFETY_VERDICT.md`; `project/templates/SAFETY_VERDICT.md`;
`docs/architecture/PERSISTENCE_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`;
`docs/security/THREAT_MODEL.md`; `docs/development/ENGINEERING_SYSTEM.md`;
`docs/development/AGENT_PROTOCOL.md`; `rust/crates/cancellai-store/src/tombstone.rs`;
`rust/crates/cancellai-store/src/ledger.rs`; `rust/crates/cancellai-store/src/lib.rs`;
`rust/crates/cancellai-model/src/agent_artifact.rs`;
`rust/crates/cancellai-safety/src/authority.rs`; and
`rust/crates/cancellai-safety/src/mutation_executor.rs`.

## Overall verdict

`FAIL` — E12-S04 returns to `in_progress`, pending an owner-controlled AC1 decision or a
structurally closed orchestrator/reference design. No further review round or implementation
repair was attempted.

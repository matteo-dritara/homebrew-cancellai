# E13 Independent Verifier Review — Round 3

- Review-Target: `920776d..557bf25`, restricted to `rust/crates/cancellai-store/` and `project/evidence/E13-*`
- Verifier: Codex
- Date: 2026-09-19
- Review-Scope: epic
- Round: 3

All four scoped stories were confirmed `ready_for_review` in `project/epics/E13.json` before
this review began.

## Brief provenance

- Story: E13-S02 | Brief-Checksum: `b12e1ca2c0b20a2679c2ceedaf147b6e482fddbe0160590a507519b90187defe` | Verifier: Codex
- Story: E13-S03 | Brief-Checksum: `4d52843da365e7b247ff727671d9426b379e55a56cf7c8b3a905759f1a344adf` | Verifier: Codex
- Story: E13-S04 | Brief-Checksum: `29d1f9f58400ce38b5ee0394cd6ab5c300c0ef55fa93474a069cc5a440356e73` | Verifier: Codex
- Story: E13-S05 | Brief-Checksum: `d78bbc149e2b33b351a7d72fff727d853f1efb6b165ae73c0c0bfa8a1a5d105d` | Verifier: Codex

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E13-S02 | PASS | Reproduced the round-2 `None` versus `Some("")` metadata case through `compact_range_digest_distinguishes_absent_metadata_from_empty_metadata`; it passes. The current preimage encodes all six nullable fields with an explicit absence/presence tag followed by a length-prefixed value, while the non-null fields remain length-prefixed. This closes the original non-injective `IFNULL` digest preimage. |
| E13-S03 | PASS | The rollup retention behavior remains intact: the independently selected cascade test `jumping_past_every_window_in_one_compact_call_still_reaches_long_term_without_skipping` passes, as does the workspace suite. The only round-3 change in `rollup.rs` is the reset/open ownership check associated with E13-S04; no contrary retention or contentless-aggregation behavior was reproduced. |
| E13-S04 | FAIL | The new `rebuild_within_current_state_budget` is a tested, single-call rebuild-plus-pressure-enforcement path, so the self-review's uncalled-function finding is closed. However, the claimed ownership check is forgeable: each layer accepts a static marker string stored in its own source as proof of cancellAI ownership. An independent synthetic-provider reproduction created a matching schema, correct `PRAGMA user_version`, and the documented marker for each of current state, ledger, and analytical memory. `CurrentStateStore::open`/`EventLedger::open`/`AnalyticalMemory::open` each accepted it; their public `reset` methods then erased the provider rows. |
| E13-S05 | PASS | Reproduced the round-2 cross-identity key case through `set_invalidation_key_rejects_a_key_whose_identity_does_not_match_the_persisted_row`; it passes. The current transaction reads and deserializes the stored `AgentArtifact`, rejects a mismatched `identity_token` before `UPDATE`, and rolls back on that error. `rebuild` also clears cache columns, so an earlier key cannot survive a new row. |

## E13-S04 failure: static marker is not ownership authority

### Reproduction

I added and then removed a temporary integration test, leaving the working tree unchanged. It
created three separate SQLite files representing provider-owned payload, each with the exact
schema expected by its corresponding public opener, its completed `user_version`, one provider
row, and the fixed marker compiled into the crate:

- `cancellai-current-state-store-v1` for `CurrentStateStore`;
- `cancellai-event-ledger-v1` for `EventLedger`;
- `cancellai-analytical-memory-v1` for `AnalyticalMemory`.

`cargo test -p cancellai-store --test marker_mimicry_adversarial -- --nocapture` passed. That
passing test is the failure reproduction: all three `open` calls accepted the marker-bearing
mimic and all three `reset` calls reduced the synthetic provider table's row count from one to
zero. The marker constants are fixed literals in `src/lib.rs`, `src/ledger.rs`, and
`src/rollup.rs`; they are neither secret nor bound to a retained cancellAI-owned root, so a
schema-mimicking provider file can carry them.

The pre-migration checks do close the narrower self-review case of a mimic lacking a marker and
avoid mutating an intermediate-version file before refusing it. They do not establish ownership
of a file that asserts the fixed marker itself.

### Required repair

Replace the public arbitrary-path construction of reset-capable stores with handles derived only
from an opaque, cancellAI-owned local-state-root capability. Database filenames must be bound
relative to that established root; a provider or caller-supplied SQLite path must not be enough
to obtain a handle on which `reset` is available. The static marker may be retained as a
corruption/migration check, but cannot be the authorization to erase a file. Add adversarial
regressions for marker-bearing mimicry against all three layers.

### Violated obligations

This violates E13-S04 AC2 (`reset --local-state cannot target provider roots`), SI-026, and
Constitution C-10. The public API presently turns a provider-root file into a reset target when
it mimics a known SQLite schema and embeds an easily copied marker.

## Gates actually run

| Command | Result |
| --- | --- |
| `python3 scripts/project_os.py check` | PASS — `governance OK: 24 decisions, 32 epics, 165 stories` |
| `python3 scripts/project_os.py review` | PASS — E13-S02 through E13-S05 listed for independent review |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS — including 106 `cancellai-store` unit tests; scheduled benchmarks remained intentionally ignored |
| `cd rust && cargo deny check` | PASS — advisories, bans, licenses, and sources OK; pre-existing unmatched-license and duplicate-dependency warnings reported |
| `cd rust && cargo test -p cancellai-store compact_range_digest_distinguishes_absent_metadata_from_empty_metadata` | PASS |
| `cd rust && cargo test -p cancellai-store set_invalidation_key_rejects_a_key_whose_identity_does_not_match_the_persisted_row` | PASS |
| `cd rust && cargo test -p cancellai-store rebuild_within_current_state_budget_wires_the_write_path_to_enforcement` | PASS |
| `cd rust && cargo test -p cancellai-store jumping_past_every_window_in_one_compact_call_still_reaches_long_term_without_skipping` | PASS |
| `cd rust && cargo test -p cancellai-store --test marker_mimicry_adversarial -- --nocapture` | PASS — independent temporary adversarial test confirmed the E13-S04 failure; test file removed after execution |

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/development/WORK_ITEM_MODEL.md`
- `docs/development/RELEASE_GATES.md`
- `docs/architecture/PERSISTENCE_MODEL.md`
- `docs/architecture/DOMAIN_MODEL.md`
- `docs/architecture/JSON_CONTRACTS.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/security/HAZARD_ANALYSIS.md`
- `docs/adrs/0019-dependency-rings-per-crate.md`
- `docs/BACKLOG.md`
- `project/epics/E13.json`
- `project/evidence/E13-S02/VERIFIER_BRIEF.md`
- `project/evidence/E13-S03/VERIFIER_BRIEF.md`
- `project/evidence/E13-S04/VERIFIER_BRIEF.md`
- `project/evidence/E13-S05/VERIFIER_BRIEF.md`
- `project/evidence/E13-VERIFIER-REVIEW-ROUND2.md`
- `project/evidence/E13-SELF-REVIEW-ROUND2.md`
- `project/evidence/E13-S04/EVIDENCE.md`
- `rust/crates/cancellai-store/src/lib.rs`
- `rust/crates/cancellai-store/src/budget.rs`
- `rust/crates/cancellai-store/src/ledger.rs`
- `rust/crates/cancellai-store/src/rollup.rs`

## Overall verdict and cost-ceiling disposition

**FAIL.** E13-S02, E13-S03, and E13-S05 pass. E13-S04 fails because a fixed SQLite marker is
not an ownership capability and permits reset of a marker-bearing provider-root mimic.

This is the owner-authorized ADR-0025 cost-ceiling round. There must be no fourth review round.
The surviving E13-S04 ownership-forgery defect must be recorded as accepted residual risk and
carried by a new backlog work item before a repair is undertaken; no new carrier story ID exists
in the current control plane, so it must be assigned by the owner/executor rather than invented
in this verifier record. Until then, the residual is explicitly tracked against E13-S04.

# E13 Independent Verifier Review — Round 2

- Review-Target: `aca7485..920776d`
- Verifier: Codex
- Date: 2026-09-19
- Review-Scope: epic
- Round: 2

## Brief provenance

- Story: E13-S02 | Brief-Checksum: `b12e1ca2c0b20a2679c2ceedaf147b6e482fddbe0160590a507519b90187defe` | Verifier: Codex
- Story: E13-S03 | Brief-Checksum: `4d52843da365e7b247ff727671d9426b379e55a56cf7c8b3a905759f1a344adf` | Verifier: Codex
- Story: E13-S04 | Brief-Checksum: `29d1f9f58400ce38b5ee0394cd6ab5c300c0ef55fa93474a069cc5a440356e73` | Verifier: Codex
- Story: E13-S05 | Brief-Checksum: `d78bbc149e2b33b351a7d72fff727d853f1efb6b165ae73c0c0bfa8a1a5d105d` | Verifier: Codex

All four E13 stories under review were `ready_for_review` in `project/epics/E13.json` before this pass. E13-S01 is background only, as directed.

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E13-S02 | FAIL | `EventLedger::compact_range` applies `IFNULL(field, '')` before the length-prefixed digest encoding. An event with `provider_id = None` and the same event with `provider_id = Some("")` are semantically distinct `EventMetadata` values but produce the same digest. The isolated public-API reproduction printed `null_empty_digest_equal=true`. The round-1 delimiter test does not exercise optional-value presence. |
| E13-S03 | PASS | Reviewed all three promotion cutoffs and cascade transaction. Each now uses `checked_sub`, so `now < window` does not turn into a zero cutoff; promotion/deletion remains transactional and ordered raw → hourly → daily → long-term. `cargo test --workspace` passed, including the E13-S03 exact-boundary, pre-window, idempotence, backdated-sample, and cascade tests. No independent AC violation was reproduced. |
| E13-S04 | FAIL | (1) A budget is not enforced for Layer 1: `check_current_state_budget` only reports `OverBudget`; unrestricted `CurrentStateStore::rebuild` can continue growing the table and has no budget admission/compaction/refusal path. (2) reset handles are not restricted to cancellAI-owned storage. An isolated public-API reproduction created a provider-owned SQLite file with the current-state schema and `user_version=2`, opened it with `CurrentStateStore::open`, called `reset`, and printed `provider_rows_after_reset=0`. The adjacent-file tests do not exercise this target-selection path. |
| E13-S05 | FAIL | `set_invalidation_key` never binds `key.identity_token` to the `AgentArtifact` identity in the stored row. The isolated public-API reproduction rebuilt a row for `identity-A`, attached a complete key for `identity-B`, then queried with fresh `identity-B`; it printed `mismatched_identity_hint=ReuseForReading`. The added tests only compare a key to itself and therefore miss this cross-identity cache reuse. |

## Required repairs for FAILs

### E13-S02 — non-injective compaction digest preimage

Reproduction: append two otherwise identical `Discovered` events in separate ledgers, one with `EventMetadata { provider_id: None, .. }`, the other with `provider_id: Some(String::new())`; compact each singleton range. Both summaries have the same `digest_hex` because the query maps both values to `""` before hashing.

Required repair: make the digest preimage encode optional-field presence separately from contents (for example a tag plus a length-prefixed value), for every optional metadata field. Add a regression that proves `None` and `Some("")` yield different summary digests, alongside the existing cross-field-boundary test.

Violation: E13-S02 AC1. The compacted summary is intended to be the hashed record that preserves audit attribution; the current digest does not distinguish two different committed event records.

### E13-S04 — unenforced current-state budget and unbounded reset target

Reproduction A: construct a one-row current-state budget, rebuild the store with two artifacts, and call `check_current_state_budget`. It reports `OverBudget`; nothing in the write path compacts, degrades, or refuses the over-limit state. `BudgetLimits` is absent from `CurrentStateStore::rebuild` and `check_current_state_budget` has no action beyond observation.

Required repair A: put current-state writes behind an enforced budget admission/orchestration path. If current facts cannot safely be compacted, the path must explicitly refuse the expansion or trigger the documented safe degradation before the state store grows beyond its configured limit. A status-only check is not enforcement. Add a low-limit regression proving a rebuild/write cannot leave Layer 1 above the configured limit without an explicit, documented safe outcome.

Violation A: E13-S04 AC1 and Constitution C-11.

Reproduction B: create a SQLite file representing provider payload with `PRAGMA user_version = 2` and an `agent_artifacts` table/row; pass its path to public `CurrentStateStore::open`; invoke `reset`. The row is deleted. The isolated reproduction printed `provider_rows_after_reset=0`.

Required repair B: establish cancellAI-owned local-state storage as an explicit, opaque capability/root and permit reset only on handles derived from that owned location. A bare arbitrary `Path` must not be sufficient to obtain a reset-capable handle. Add an adversarial provider-database target test, not only a provider file adjacent to a known state database.

Violation B: E13-S04 AC2, SI-026, and Constitution C-10.

### E13-S05 — cache key detached from cached artifact identity

Reproduction: rebuild an `AgentArtifact` row whose `identity_token` is `identity-A`; call `set_invalidation_key` for the same `ArtifactId` with an otherwise complete key whose identity is `identity-B`; call `cache_read_hint` with fresh `identity-B` and the same other axes. It returns `ReuseForReading`, although the cached artifact facts are for `identity-A`.

Required repair: atomically bind an invalidation key to the exact persisted artifact identity, rejecting (or returning `Revalidate` for) a key whose identity differs from the stored row's `AgentArtifact::identity_token`. Add the cross-identity regression above; equality only between a persisted key and the supplied fresh key is insufficient.

Violation: E13-S05 AC2. The cache can reuse facts across an identity change. The store currently has no direct mutation-executor dependency, so this reproduction does not demonstrate a current execution bypass; it is nevertheless a broken cache-invalidation boundary that must not be carried into an orchestrator.

## Gates actually run

| Command | Result |
| --- | --- |
| `python3 scripts/project_os.py check` | PASS — `governance OK: 24 decisions, 32 epics, 165 stories` |
| `python3 scripts/project_os.py review` | PASS — E13-S02 through E13-S05 listed as awaiting independent review |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS — including 96 `cancellai-store` unit tests; scheduled benchmarks remained intentionally ignored |
| `cd rust && cargo deny check` | PASS — advisories, bans, licenses, and sources OK; pre-existing unmatched-license and duplicate-dependency warnings reported |

I also ran an isolated temporary public-API adversarial reproduction outside the repository. It produced exactly: `mismatched_identity_hint=ReuseForReading`, `null_empty_digest_equal=true`, and `provider_rows_after_reset=0`.

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
- `project/epics/E13.json`
- `project/evidence/E13-S02/VERIFIER_BRIEF.md`
- `project/evidence/E13-S03/VERIFIER_BRIEF.md`
- `project/evidence/E13-S04/VERIFIER_BRIEF.md`
- `project/evidence/E13-S05/VERIFIER_BRIEF.md`

## Overall verdict

FAIL. E13-S02, E13-S04, and E13-S05 require repair. E13-S03 passes this round.

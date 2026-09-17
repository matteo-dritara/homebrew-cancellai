# Independent Verifier Review — E13, Round 1

Review-Scope: epic
Round: 1
Verifier: Codex (/root)
Date: 2026-09-17

## Scope

- Epic: E13 — Local State, Event Ledger, Analytical Memory.
- Review target: `aca7485..2b37ffd`.
- `f7f9fcf` is in the range but is the unrelated E12-S01 Windows quarantine-name repair. It is out of scope and was not judged against E13.

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E13-S01 | PASS | Rebuild is a single transaction that deletes and re-inserts exactly the supplied set; migration application advances `PRAGMA user_version` in its transaction. I independently ran the store and workspace suites, including drop/rebuild, malformed-row, migration-failure, and provider-artifact boundary tests. Production `cancellai-store` has no raw delete call or safety-executor capability reference; `check_mutation_boundary.py` passed. Brief-Checksum: 55688326affe56d8bdb58b6b74dd1948d4c340b4d58031d6743d41c98d89f444 |
| E13-S02 | FAIL | Two public-API reproductions fail. (1) Compacting one event with `provider_id="a\\u{1}b", category="c"` produces the same SHA-256 (`6f5b98cc5935ec63859982f2525416592bda874613035ffc0cb165aacc653e7c`) as the distinct event `provider_id="a", category="b\\u{1}c"`; the delimiter encoding does not commit to field boundaries. (2) `compact_range(EventId(i64::MIN), EventId(i64::MAX), 0)` panics at `ledger.rs:471` on signed subtraction overflow rather than returning a rejection. Brief-Checksum: b12e1ca2c0b20a2679c2ceedaf147b6e482fddbe0160590a507519b90187defe |
| E13-S03 | FAIL | With `RetentionPolicy::new(10, 20, 30)`, a sample recorded at `0`, and `compact(..., now=5)`, raw-sample count becomes `0`; age is five seconds, so the ten-second recent window has not elapsed. `saturating_sub` turns the cutoff into zero and prematurely promotes timestamp-zero samples. Brief-Checksum: 4d52843da365e7b247ff727671d9426b379e55a56cf7c8b3a905759f1a344adf |
| E13-S04 | FAIL | Following the documented pre-append sequence with a five-event ledger limit (`enforce_ledger_budget`, then `append`, six times) leaves six raw events. At exactly the limit the check is a no-op, so the next write grows past it; `append` has no budget admission path. The same design only observes current-state overrun and lets unaged raw samples grow if time retention cannot compact them. Reset and in-memory paths themselves held: no reset API takes a target path, and SI-019 passed. Brief-Checksum: 29d1f9f58400ce38b5ee0394cd6ab5c300c0ef55fa93474a069cc5a440356e73 |
| E13-S05 | FAIL | A complete row persisted and queried with equal identity/mtime/knowledge but `provider_fingerprint: None` returns `ReuseForReading`, not `Revalidate`. An unavailable fingerprint cannot establish that a provider layout is unchanged; the same equality rule also accepts unavailable mtime and knowledge-version axes. The production dependency graph still prevents this crate from calling the safety executor, but the five-way cache invalidation claim is false. Brief-Checksum: d78bbc149e2b33b351a7d72fff727d853f1efb6b165ae73c0c0bfa8a1a5d105d |

## Required repairs

### E13-S02

The compaction digest must use an injective canonical encoding: length-prefix every nullable/string field (including an explicit null marker), or hash a rigorously canonical structured encoding. Add the collision reproduction above and boundary/property tests. Reject non-positive/unrepresentable `EventId` inputs and use checked range arithmetic so all public inputs return `LedgerError`, never panic.

This violates the AC permitting removal only into a correctly signed/hashed summary record and the crash/boundary aspect of the verification contract.

### E13-S03

Do not treat a sample as eligible when `now` precedes its tier window. Compute cutoffs with `checked_sub` (no eligible rows when it returns `None`) and add time-travel tests for `now < recent_window`, `now == recent_window`, and the corresponding hourly/daily windows.

This violates AC1: raw samples expire according to policy.

### E13-S04

Replace optional after-the-fact checking with an admission/enforcement path that knows the prospective write: compact/rotate before accepting an optional ledger or analytical sample, and refuse/degrade that write when safe compaction cannot meet the limit. Current-state overflow needs an explicit bounded outcome rather than observation alone; it must not silently discard safety-critical facts. Make the public/orchestration write path use this admission atomically and add a stress test that never observes a count above its limit.

This violates AC1, “Budget overrun triggers compaction before growth continues.” The reset construction remains compliant with SI-026.

### E13-S05

Return `Revalidate` unless all five invalidation axes are positively known and exactly equal, with both completeness values `Complete`; absent/empty identity, mtime/metadata, provider fingerprint, or knowledge version is uncertainty, not equality. Add cases for every unavailable axis as well as changed axes, partial/unknown completeness, and stale rows after a rebuild.

This violates AC2 and SI-024: an uncertain persistent cache row is reusable even though it cannot establish current provider/knowledge state. Fresh mutation preconditions are not presently wired to this primitive, but that absence does not repair the unsafe cache contract.

## Gate status

- Passed: `cargo fmt --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace`; and `cargo deny check` (exit 0; existing duplicate/unmatched-allowance warnings only).
- Passed: `python3 -m pytest tests -v` — 665 tests and 556 subtests.
- Passed: `env -u FORCE_COLOR pre-commit run --all-files`, and every repository checker in AGENTS.md's Python suite: docs, governance, fixtures, schemas, characterization/diff/parity, Rust topology/mutation boundary, provider/platform, process/release, evidence, risk, toolchain, handoff, safety oracle, EARS, and gate sensitivity.
- `env -u FORCE_COLOR python3 scripts/check_skill_content.py check` passed with the already-recorded partial-inspection residual. The documented `FORCE_COLOR` parsing issue was not re-proposed.
- Direct `python3 -m ruff check .`, `python3 -m ruff format --check .`, and `python3 -m mypy ...` could not run because this interpreter lacks those modules; their isolated pre-commit gates passed.
- Remote CI for `2b37ffd` is unknown: `origin/main` remains at pre-E13 `c6a9af4`. The latest visible rust failure is for that pre-E13 commit and is the Windows separator defect repaired by excluded `f7f9fcf`; governance, tests, and CodeQL on that run succeeded.

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/BACKLOG.md`
- `docs/architecture/PERSISTENCE_MODEL.md`
- `docs/architecture/JSON_CONTRACTS.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/adrs/0019-dependency-rings-per-crate.md`
- `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/development/WORK_ITEM_MODEL.md`
- `docs/development/RELEASE_GATES.md`
- `docs/development/VERIFICATION_STRATEGY.md`
- `project/epics/E13.json`

## Overall verdict

FAIL. Four of five judged stories have material defects (80% finding and rejection yield), so ADR-0025 requires a repair cycle and independent round 2. E13-S01 is done; E13-S02, E13-S03, and E13-S05 return to `in_progress`; E13-S04 is blocked on the failed same-epic dependencies while retaining its recorded repair requirement. The epic remains `in_progress` and no release is due.

<!-- SELF-REVIEW (not independent). This document does not satisfy AGENTS.md's/
AGENT_PROTOCOL.md's independent-review requirement: it was written by the same session
(Claude) that executed the fixes it examines. Per AGENTS.md ("A review by the agent that
executed the work is a self-review: it may find and repair defects, it may not close a
CR3/CR4 story on its own") and `docs/development/AGENT_PROTOCOL.md`'s "What 'independent'
can mean here," this pass cannot close E13-S02 (CR2), E13-S03 (CR2), E13-S04 (CR3) or
E13-S05 (CR3), and issues no CR4 Safety Verdict. AGENTS.md names Codex as the standing
independent reviewer for this epic; that review is still required before any of these
stories may move past `ready_for_review`. No story or epic status was changed by this
document or its author. -->

# E13 Self-Review, Round 2 (not independent)

- Review-Target: uncommitted working-tree changes on top of `96f645e` (the three repairs made
  in response to `project/evidence/E13-VERIFIER-REVIEW-ROUND2.md`: `rust/crates/cancellai-store/
  src/budget.rs`, `src/ledger.rs`, `src/lib.rs`). Not yet committed at the time of this review.
- Reviewer: Claude (same session that authored the repairs under review - self-review, not
  independent)
- Date: 2026-09-19
- Review-Scope: epic (E13-S02 through E13-S05; E13-S01 background only)
- Round: self-review pass between independent round 2 and independent round 3

## Method note

This pass re-derived the requirement from `project/epics/E13.json`, the four `VERIFIER_BRIEF.md`
files, `docs/architecture/PERSISTENCE_MODEL.md`'s "Self-budget" and "Incremental reuse" sections,
and `project/evidence/E13-VERIFIER-REVIEW-ROUND2.md`, then read the actual diff and current code
independently rather than trusting the repair descriptions handed in. For each of the three
claimed repairs, a standalone reproduction was built and run against the real code (added as a
temporary `#[test]`, executed, and then reverted so the working tree matches the executor's diff
exactly - confirmed by `git diff` after revert). Two of the three reproductions confirmed the
claimed fix; the process of building them surfaced one unaddressed part of the claimed E13-S04
fix and one unfixed variant of the same root cause elsewhere in the crate.

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E13-S02 | PASS | `EventLedger::compact_range`'s digest preimage now tags each nullable metadata field with a presence byte (`0` = `None`, `1` + length-prefix = `Some`) before hashing, replacing the `IFNULL(x,'')` that erased the distinction. Independently reproduced round 2's exact case (`provider_id: None` vs `provider_id: Some(String::new())`, otherwise identical `Discovered` events, singleton-range `compact_range` on each) via `cargo test -p cancellai-store compact_range_digest_distinguishes_absent_metadata_from_empty_metadata` - passes. Traced the encoding by hand: every one of the six nullable fields is now self-delimiting (tag, then length-prefix for `Some`), and `kind`/`evidence_ids` (both `NOT NULL`) keep their own unconditional length-prefix outside the loop - the whole concatenation is injective, so this is not a narrow patch of the one reported pair but closes the general non-injective-preimage class round 1 and round 2 both hit pieces of. |
| E13-S03 | PASS (carried forward, unchanged) | No file this fix pass touched (`rollup.rs`) is in this diff; `git diff --stat` confirms only `budget.rs`, `ledger.rs`, `lib.rs`, and the four `VERIFIER_BRIEF.md` headers (checksum/date only, contract text unchanged) moved. Round 2's PASS stands; this pass did not re-run S03's own falsification, since nothing about it changed. |
| E13-S04 | FAIL (repair incomplete) | Two required repairs; one closes its reported reproduction, the other does not, and a variant of the same class round 2 found was left open elsewhere in the crate. See "E13-S04 findings" below. |
| E13-S05 | PASS | `set_invalidation_key` now runs the read-check-write inside one `rusqlite::Transaction`, loads the persisted row's `AgentArtifact` and rejects (`Err`, no write) a key whose `identity_token` differs from `artifact.identity_token` before ever touching the `UPDATE`. Independently reproduced round 2's exact case (row persisted for `identity-A`, key stamped `identity-B` supplied) via `cargo test -p cancellai-store set_invalidation_key_rejects_a_key_whose_identity_does_not_match_the_persisted_row` - passes, and confirmed by hand that a rejected call leaves `cache_read_hint` at `Revalidate` (no invalidation key was ever written, matching the transaction's rollback-on-error semantics: the `Err` returns before `tx.execute`/`tx.commit`, so the transaction's implicit `Drop` rolls back regardless). Checked the adjacent surface: `rebuild`'s `INSERT` never sets the `cache_*` columns, so a rebuild still always wipes any previously attached key to `NULL` before a new one could be attached - no stale cross-identity key can survive a rebuild by a separate path. No adjacent variant found. |

## E13-S04 findings

### Repair A (Layer 1 budget enforcement) - not closed

Round 2's required repair: *"put current-state writes behind an enforced budget admission/
orchestration path... A status-only check is not enforcement."*

The fix adds `cancellai_store::budget::enforce_current_state_pressure(store, memory, limits,
degraded_raw_sample_ceiling, policy, now)`, which calls `check_current_state_budget` and, if
`OverBudget`, forces `AnalyticalMemory::compact_if_over` on the caller-supplied ceiling. Read in
isolation this is a real, working consequence (verified: `cargo test -p cancellai-store
enforce_current_state_pressure_degrades_rollup_when_current_state_is_over_budget` passes, and by
hand the assertions show `raw_sample_count` going from 6 to 0 while `store.row_count()` stays at
2, i.e., Layer 1's own rows are correctly never the thing discarded).

The gap: **nothing calls this function from the write path.** `CurrentStateStore::rebuild`
(`rust/crates/cancellai-store/src/lib.rs:385`) - the only way to write to Layer 1 - takes no
`BudgetLimits`, no `&mut AnalyticalMemory`, and calls no budget-related function internally;
confirmed by reading its full body (`grep -n "pub fn rebuild" -A 30 src/lib.rs`): it is `DELETE
FROM agent_artifacts` + per-row `INSERT`, nothing else. `enforce_current_state_pressure` is a
free function a caller must remember to invoke separately, exactly the same shape
`check_current_state_budget` already had before this fix - the only change is that the function
you must remember to call now also *does* something, not that calling it is any less optional.
`rebuild` can still be called any number of times growing Layer 1 without limit and with zero
consequence, which is the literal defect round 2 reported ("unrestricted `CurrentStateStore::
rebuild` can continue growing the table... has no budget admission/compaction/refusal path")
still being true after the fix.

The module's own new doc comment concedes this directly: *"A live caller (Guardian) wiring this
into an ongoing process... is still future work."* That is the same "no orchestrator yet"
framing round 2 explicitly rejected for this exact story (it accepted the equivalent framing for
S01-S03's own primitives, but not for S04's compaction AC1). Classification: **implementation
gap, not a spec gap** - `docs/architecture/PERSISTENCE_MODEL.md`'s "Self-budget" section already
states the AC1 obligation and already named Layer 3 degradation as the correct mitigation before
this fix; what was missing was wiring, and a callable-but-uncalled function does not supply it.
Note also that `docs/architecture/PERSISTENCE_MODEL.md` itself was not updated by this fix and
still reads, unchanged, "wiring that cross-layer degradation decision needs a live (Guardian)
caller this workspace does not have yet" - consistent with, and further evidence for, the wiring
still not existing.

Required repair: either (a) give `CurrentStateStore` a write entry point that takes budget
limits and an `&mut AnalyticalMemory` and calls `enforce_current_state_pressure` (or equivalent)
internally, with `rebuild` either becoming that entry point or being demoted/documented as the
unenforced primitive `enforce_...` composes - matching the `append_within_ledger_budget`/
`record_sample_within_rollup_budget` precedent this same module doc already cites for Layer 2/3,
which close exactly this "checked before, not after" gap by wrapping the write itself; or (b) if
wiring truly is out of scope for this crate (no live caller exists), say so explicitly as a
residual risk in the story's evidence packet rather than recording AC1 as satisfied. A regression
proving a `rebuild` call alone (with no separate `enforce_current_state_pressure` call) cannot
leave Layer 1 above a configured limit is still missing.

### Repair B (reset cannot target provider roots) - closes the reported case; one adjacent gap found by reproduction, one broader gap found elsewhere in the crate

Round 2's reproduction (hand-crafted SQLite file, `agent_artifacts` table+row, `PRAGMA
user_version = 2`, opened via `CurrentStateStore::open`, then `reset`) is closed: reproduced
independently via `cargo test -p cancellai-store
open_refuses_a_provider_owned_file_that_only_mimics_this_crates_schema` (passes), and confirmed
by hand that `verify_store_identity` correctly fails this file (no `cancellai_store_identity`
table exists on it, since `apply_migrations` skipped migration 0 - the only migration that
creates it - because `user_version` already read as 2).

**Adjacent gap (own reproduction, not in the diff's tests):** the identity marker lives inside
migration 0's own SQL, and `apply_migrations` runs every migration whose index is at or past the
file's current `user_version`, unconditionally, *before* `verify_store_identity` ever runs. A
file at an intermediate `user_version` (not fully caught up) that happens to already carry a
schema matching an earlier migration's output gets the *remaining* migrations executed against
it - a real mutation - before being refused. Built and ran a temporary test (added, executed,
reverted; not part of the committed diff): a hand-crafted file with `agent_artifacts` in exactly
migration 0's shape (plus its two indexes) and `PRAGMA user_version = 1` was passed to
`CurrentStateStore::open`. `open` still correctly returned `Err` (no identity marker - migration
0 was skipped), but migration 1 (`ALTER TABLE agent_artifacts ADD COLUMN cache_identity_token
TEXT` and the other four `cache_*` columns) had already been executed against the file first:
`SELECT COUNT(*) FROM pragma_table_info('agent_artifacts') WHERE name = 'cache_identity_token'`
against the file afterward returned `1`, not `0`. This is schema mutation, not data loss - no
row was deleted or altered, and `open`'s refusal is still correct - but it is still an
unauthorized write to a file this crate does not own, which the fix's own reasoning ("fails the
open closed rather than silently granting a reset-capable handle") does not fully deliver: the
handle is correctly refused, but the file was touched first. Classification: **implementation
gap** (narrower than round 2's reported case - it is very unlikely a real provider file
independently matches migration 0's exact table/index names at exactly `user_version = 1`, but
it is a real gap in the "opaque, cancellai-owned root" property the fix's own doc comment claims
to establish, and it is worth a low-cost repair: verify identity as the very first statement
`open` runs against an existing file, before calling `apply_migrations`, and only run migrations
once ownership - or absence of any file at all - is established).

**Broader gap (own reproduction, elsewhere in the same crate - not addressed by this fix at
all):** the round 2 finding's root cause - `open` trusting `PRAGMA user_version`/schema shape
alone as proof of ownership, with no identity check before running migrations or before `reset`
- was fixed only for `CurrentStateStore` (Layer 1). `EventLedger::open`/`EventLedger::reset`
(Layer 2, `src/ledger.rs:329,741`) and `AnalyticalMemory::open`/`AnalyticalMemory::reset` (Layer
3, `src/rollup.rs:803,1042`) have no analogous check - confirmed by `grep -n "verify_store_identity\|identity" src/ledger.rs src/rollup.rs`, which found nothing. `budget::reset_local_state`
(`docs/architecture/PERSISTENCE_MODEL.md`'s own description: "sequencing
`CurrentStateStore::reset`, `EventLedger::reset` and `AnalyticalMemory::reset`") therefore still
sequences two resets with no ownership check into the SI-026 boundary this same story is
supposed to enforce for all three layers, not just one. Built and ran a temporary test (added,
executed, reverted; not part of the committed diff): a hand-crafted SQLite file at `PRAGMA
user_version = 1` was given `ledger_events`/`ledger_control`/`ledger_compactions` tables matching
`EventLedger`'s own migration output exactly, plus one inserted row representing foreign content.
`EventLedger::open` accepted it without error, and `.reset()` succeeded and destroyed the row -
`SELECT COUNT(*) FROM ledger_events` on the file afterward returned `0`. This is the *same*
reproduction shape round 2 used against `CurrentStateStore`, applied one file over, and it
succeeds identically. Classification: **implementation gap - the same root cause as round 2's
S04 finding B, left open for two of the three layers this story's own outcome ("Enforce
cancellAI state/log budgets and provide reset of cancellAI state only") names.** Required repair:
extend `verify_store_identity` (or an equivalent per-module marker/check, since each layer keeps
its own migration history and connection, per this crate's own "why layers stay independent"
precedent) to `EventLedger::open` and `AnalyticalMemory::open` as well, with the same open-refuses
guarantee `CurrentStateStore` now has.

## Reproductions performed (all reverted; working tree matches the executor's diff exactly - confirmed by `git diff` after each revert)

1. `compact_range_digest_distinguishes_absent_metadata_from_empty_metadata` (existing, unmodified) - ran, passed.
2. `set_invalidation_key_rejects_a_key_whose_identity_does_not_match_the_persisted_row` (existing, unmodified) - ran, passed.
3. `open_refuses_a_provider_owned_file_that_only_mimics_this_crates_schema` (existing, unmodified) - ran, passed.
4. Temporary test: `CurrentStateStore::open` against a hand-crafted file at `user_version = 1` matching migration 0's schema exactly - `open` correctly refused it, but the file's schema had already been altered (migration 1 ran) before the refusal. Added, run (failed on the assertion documenting the finding, as intended), reverted.
5. Temporary test: `EventLedger::open` + `.reset()` against a hand-crafted foreign file at `user_version = 1` matching the ledger's own migrated schema, with one inserted row - `open` accepted it and `.reset()` destroyed the row with no identity check at all. Added, run (passed, confirming the gap - no such check exists to fail), reverted.

## Gates run (in addition to the reproductions above)

| Command | Result |
| --- | --- |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo test -p cancellai-store` | PASS - 101 tests |
| `cd rust && cargo test --workspace` | PASS |
| `git diff` (after each temporary reproduction, following its revert) | confirmed byte-identical to the executor's own diff for `lib.rs` and `ledger.rs` (piped through `diff` against the original patch capture; reported `IDENTICAL`) |

`cargo deny check` was not re-run in this pass (no dependency changed since round 2's PASS on
it); `python3 scripts/project_os.py check`/`review` were not re-run either, since this document
changes no story/epic status and the orchestrating session owns that transition.

## Documents opened

- `project/epics/E13.json`
- `project/evidence/E13-S02/VERIFIER_BRIEF.md`
- `project/evidence/E13-S04/VERIFIER_BRIEF.md`
- `project/evidence/E13-S05/VERIFIER_BRIEF.md`
- `project/evidence/E13-VERIFIER-REVIEW-ROUND2.md`
- `docs/architecture/PERSISTENCE_MODEL.md` ("Self-budget", "Layer 2: Operational Event Ledger" sections)
- `rust/crates/cancellai-store/src/lib.rs` (full diff and surrounding `rebuild`/`set_invalidation_key`/`apply_migrations`/`verify_store_identity` code)
- `rust/crates/cancellai-store/src/budget.rs` (full diff, `enforce_current_state_pressure` and its module doc)
- `rust/crates/cancellai-store/src/ledger.rs` (full diff, `compact_range`, and - for the S04 variant finding - `EventLedger::open`/`reset`, unmodified by this fix)
- `rust/crates/cancellai-store/src/rollup.rs` (`AnalyticalMemory::open`/`reset`, unmodified by this fix, read to confirm the same gap)

## Round verdict

**FAIL** (self-review, not independent). E13-S02 and E13-S05 close what round 2 found and no
adjacent variant was found for either. E13-S03 carries forward its round 2 PASS unchanged.
**E13-S04 does not close round 2's finding**: repair A (budget enforcement) leaves the only
write path to Layer 1 (`rebuild`) with no enforcement behind it at all, and repair B (reset
boundary), while closing round 2's exact reproduction against `CurrentStateStore`, leaves the
identical root cause open and independently reproducible against `EventLedger` and (by the same
reasoning, unreproduced but structurally identical) `AnalyticalMemory`. This document changes no
story or epic status; per AGENTS.md, that transition belongs to the orchestrating session and,
for the stories this pass finds PASS-worthy, still requires the independent reviewer named in
AGENTS.md (Codex) before any of E13-S02/S03/S05 may move to `done`, and E13-S04 requires further
repair before it is ready for that independent pass at all.

## Executor follow-up (written by the orchestrating session, after this self-review)

Both E13-S04 gaps this self-review found were repaired:

- **Repair A closed**: `budget::rebuild_within_current_state_budget(store, memory, artifacts,
  limits, degraded_raw_sample_ceiling, policy, now)` composes `CurrentStateStore::rebuild` with
  `enforce_current_state_pressure` into one call - the same "wrap the write itself" shape
  `append_within_ledger_budget`/`record_sample_within_rollup_budget` already give Layer 2/3.
  Regression: `budget::tests::rebuild_within_current_state_budget_wires_the_write_path_to_enforcement`
  proves a single such call landing Layer 1 over budget itself degrades Layer 3, with no separate
  `enforce_current_state_pressure` call required.
- **Repair B's two gaps closed**: (1) `verify_store_identity_before_migrating`/
  `verify_ledger_identity_before_migrating`/`verify_rollup_identity_before_migrating` now check
  identity *before* `apply_migrations` runs anything further when `user_version > 0`, so a
  non-owned file at an intermediate version is refused before any schema mutation, not after -
  regression: `tests::open_refuses_before_mutating_a_file_at_an_intermediate_user_version_with_no_marker`
  (`lib.rs` and `ledger.rs`, one each). (2) The same identity-marker check this fix pass had given
  only `CurrentStateStore` is now applied identically to `EventLedger`
  (`cancellai_ledger_identity`/`LEDGER_IDENTITY_MARKER`) and `AnalyticalMemory`
  (`cancellai_rollup_identity`/`ROLLUP_IDENTITY_MARKER`) - regressions:
  `ledger::tests::open_refuses_a_provider_owned_file_that_only_mimics_this_ledgers_schema` and
  `rollup::tests::open_refuses_a_provider_owned_file_that_only_mimics_this_modules_schema`.
  Fixing this also surfaced and repaired a real bug the new ledger marker table introduced:
  `EventLedger::reset()` drops and replays every migration unconditionally but had not been
  taught to drop `cancellai_ledger_identity` first, so every `reset()` call failed with "table
  already exists" until the `DROP TABLE IF EXISTS cancellai_ledger_identity` was added alongside
  the pre-existing three.

All 106 `cancellai-store` tests pass (up from 101), plus workspace-wide `cargo fmt --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo check --workspace
--all-targets`, `cargo test --workspace`, and `cargo deny check` all pass. This follow-up is still
executor work on the executor's own repair, not an independent pass - it does not change this
document's FAIL-for-independent-review-purposes status, and E13-S04 (along with E13-S02/S03/S05)
still requires the independent reviewer named in AGENTS.md before any of these four stories may
move past `ready_for_review`.

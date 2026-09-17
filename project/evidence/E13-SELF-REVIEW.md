# E13 SELF-REVIEW (not independent)

Per `.claude/skills/epic-verifier/SKILL.md`'s standing note and `docs/development/
AGENT_PROTOCOL.md`'s "Self-review": `AGENTS.md` names Codex as the independent reviewer and
Claude as the executor. This review was run as the `epic-verifier` skill inside a forked Claude
subagent with no access to the executor's private reasoning - only the committed story
contracts (`project/epics/E13.json`), the generated verifier briefs, `docs/architecture/
PERSISTENCE_MODEL.md`, `docs/security/SAFETY_INVARIANTS.md` (SI-024, SI-026), and the final
diff/working tree. Context isolation removes priming; it does not remove self-preference bias,
which is a property of the model family (Claude reviewing Claude's own prior repair), not of the
conversation. **This document must not be treated as satisfying the independent-review
requirement.** No E13 story may move to `done` on the strength of this document alone; that
requires the actual Codex independent round 2 this repository has not yet run.

- Epic: E13 - Local State, Event Ledger, Analytical Memory
- Stories under review: E13-S02 (Append-only operational ledger, CR2), E13-S03 (Analytical
  rollups and retention, CR2), E13-S04 (Self-budget and local reset, CR3), E13-S05 (Incremental
  inventory reuse, CR3). E13-S01 is `done` (Codex round 1 PASS) and out of scope - its
  neighbours were still moving at review time, so it is not re-judged here.
- Review target: `2970ee3..HEAD` - Claude's own round 1 repair commit (`fix(store): repair every
  defect round 1 independent review found in E13`) plus the further fix this self-review itself
  made on top of it. `2970ee3` was written in response to `project/evidence/
  E13-VERIFIER-REVIEW.md` (Codex, round 1, FAIL on all four of these stories); this document
  reviews whether that repair actually closed what round 1 found, and hunts for adjacent
  defects in the same code before handing the epic to round 2.
- Verifier: Claude (forked subagent, no memory of writing commit `2970ee3`)
- Date: 2026-09-17
- Round: pre-round-2 self-review (round 1 was Codex's, independent, already recorded and FAIL)

## Method

Reconstructed each story's requirement from `project/epics/E13.json`'s acceptance criteria,
`docs/architecture/PERSISTENCE_MODEL.md`, and SI-024/SI-026, deliberately read before re-reading
round 1's own required repairs, then read the full implementation
(`rust/crates/cancellai-store/src/{lib.rs,ledger.rs,rollup.rs,budget.rs}`) end to end - not only
the lines round 1's diff touched - looking specifically for **variants** of round 1's four
findings (an off-by-one/boundary or unchecked-arithmetic class of bug tends to recur near its own
repair, not only at the exact reported line) and for any new defect the repair itself introduced.

## Verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E13-S02 | PASS (self-review) | Reproduced round 1's exact collision (`provider_id="a\u{1}b", category="c"` vs. `provider_id="a", category="b\u{1}c"`) against the repaired length-prefixed digest via `compact_range_digest_does_not_collide_across_a_field_boundary` - now `assert_ne!`, confirmed still passing. Reproduced round 1's `compact_range(EventId(i64::MIN), EventId(i64::MAX), 1)` panic reproduction - returns `Err` via `checked_sub`/`checked_add`, confirmed. Read every other arithmetic site in `ledger.rs`: `compact_oldest_to_fit`'s `EventId(min_id.0 + excess_i64 - 1)` is unchecked addition/subtraction on the same `i64` type round 1's finding was about, but is not independently reachable with adversarial values the way `compact_range`'s public `from`/`to` parameters are - `min_id.0` only ever advances by SQLite's own `AUTOINCREMENT` (one per real `append`), so reaching a value near `i64::MAX` needs on the order of 9.2e18 prior appends, not a single crafted call. Flagged below as a residual, not fixed (diff discipline: this is a different, unreported line, and the probability is not in the same class as round 1's single-call panic). |
| E13-S03 | PASS (self-review) | Reproduced round 1's exact case (`RetentionPolicy::new(10, 20, 30)`, sample at `t=0`, `compact(now=5)`) against all three promotion stages (`promote_raw_to_hourly`/`_hourly_to_daily`/`_daily_to_long_term`), each independently patched with the same `checked_sub`/early-`Ok(0)` pattern - confirmed the sample is retained, not wrongly promoted, at each tier's own boundary. Read the cascade's grouping/merge logic (`Accumulator`/`upsert_bucket`) for a similar "wrong cutoff" or double-counting variant: none found - `compacting_twice_over_the_same_data_is_idempotent` and `jumping_past_every_window_in_one_compact_call_still_reaches_long_term_without_skipping` both still pass and directly exercise the idempotence/cascade properties a boundary-arithmetic slip would most likely break. |
| E13-S04 | **FAIL, repaired in this self-review** | `record_sample_within_rollup_budget` - the exact primitive round 1 required to close the ledger's "before-only" gap - gated its own pre-write compaction on `enforce_rollup_budget`, whose underlying `compact_if_over` only compacts when the raw-sample count is *strictly over* `max_raw_samples`. Once any admission call has run once, the tier's steady state sits at *exactly* the limit (every successful admission already enforces `count < limit` afterward), so that "strictly over" gate was a no-op in the normal case, and the function refused every subsequent write with `SampleAdmissionError::BudgetExceeded` even when every held sample had already aged past `RetentionPolicy`'s recent window and a real `compact()` call would have freed all of it. Reproduced directly: 1s recent window, three samples recorded at t=0/1/2, `max_raw_samples=3`, a fourth admission at `now=1000` (every held sample ~1000x past its 1s eligibility) - returned `Err(BudgetExceeded { count: 3, limit: 3 })` instead of compacting and admitting. This defeats round 1's own required repair ("refuse ... that write when safe compaction cannot meet the limit" - here compaction *could* meet the limit, but was never attempted) and AC1 ("budget overrun triggers compaction before growth continues" - here growth was blocked even when compaction, the AC's own named remedy, would have worked). Repaired: the function now calls `AnalyticalMemory::compact` directly and unconditionally before checking room, rather than through `enforce_rollup_budget`'s "over" gate. Regression test added: `record_sample_within_rollup_budget_compacts_and_admits_at_exactly_the_limit_when_every_held_sample_is_eligible`. Full detail in `project/evidence/E13-S04/EVIDENCE.md`'s own "Self-review before round 2" section. |
| E13-S05 | PASS (self-review) | Reproduced round 1's exact case (a complete row with `provider_fingerprint: None` on both sides) against the repaired `cache_read_hint`: `modified_confirmed_unchanged`/`provider_fingerprint_confirmed_unchanged`/`knowledge_version_confirmed_unchanged` each require `matches!((persisted, fresh), (Some(p), Some(f)) if p == f)` - `None` on either side can never satisfy this, confirmed against `falsifier_unavailable_provider_fingerprint_is_never_treated_as_unchanged` and its `knowledge_version`/`modified` counterparts, all still passing. Checked the one field this fix does not touch, `identity_token` (mandatory `String`, not `Option`): correctly compared by plain equality with no ambiguity to resolve, since a caller with no identity token at all has nothing to construct a `CacheInvalidationKey` from in the first place. No adjacent variant found in this file. |

## Falsification attempted beyond the committed suite

- **Adjacent-arithmetic sweep** (the class round 1 found twice - unchecked/saturating math near
  boundary or extreme values): grepped every arithmetic operation in `ledger.rs`/`rollup.rs`/
  `budget.rs` outside the lines round 1's own diff touched. Found one further unchecked site
  (`compact_oldest_to_fit`'s `min_id.0 + excess_i64 - 1`, discussed above under E13-S02) that is
  not practically reachable through the public API at realistic scale; recorded as a residual,
  not fixed, per diff discipline (a different, unreported line, at a materially lower
  probability than round 1's own finding).
- **"At exactly the limit" sweep** (round 1's ledger finding was specifically about the
  before/after asymmetry at the exact limit boundary): checked every "at/over budget" decision
  point in `budget.rs` for the same shape. Found the E13-S04 defect above in the rollup
  admission path; the ledger's own admission path (`append_within_ledger_budget`) does not share
  it, because `compact_oldest_to_fit` has no time gate - its "after" compaction always succeeds
  regardless of what the "before" gate did, so the ledger's "over," not "at-or-over," pre-check
  threshold is harmless there. This asymmetry between the two primitives (documented in the
  code's own updated doc comment after the fix) is why the same repair shape produced a working
  primitive for one layer and a broken one for the other.
- **Concurrency**: `reset_under_a_held_write_lock_fails_closed_without_corrupting_existing_data`
  re-run and confirmed still passing after this session's edits (unrelated file, but exercises
  the same crate's transaction discipline this session's fix also relies on).
- **Stress re-verification**: `ledger_self_budget_stress_test_growth_never_exceeds_the_configured_
  limit` and `rollup_self_budget_stress_test_growth_never_exceeds_the_configured_limit` (500
  iterations each) re-run after this session's fix; both still assert the count never exceeds
  the configured limit at any observed point, including immediately after the fix changed what
  the rollup variant does internally.
- **Mutation-boundary/dependency check**: `scripts/check_mutation_boundary.py check` re-run after
  the fix - `budget.rs`'s edit added no new SQL, no new `Connection`, no reference to the raw
  filesystem-delete or `SystemMutationExecutor` capability; the fix is a call to an existing,
  already-reviewed method (`AnalyticalMemory::compact`) on an already-open handle, changing
  nothing about SI-019's structural constraint.

## Gate commands run and results

| Command | Result |
| --- | --- |
| `cargo fmt --check` (rust/) | PASS (no diff) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS (no warnings) |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS: every crate `test result: ok`, 0 failed; `cancellai-store`: 96 passed (was 95 after round 1's repair, 88 before it) |
| `cargo deny check` | PASS: advisories ok, bans ok, licenses ok, sources ok |
| `python3 scripts/check_mutation_boundary.py check` | PASS: only `cancellai-platform/src/mutation.rs` deletes anything; only that file and `cancellai-safety/src/mutation_executor.rs` reference the mutation capability |
| `python3 scripts/check_rust_workspace.py check` | PASS: 13 crates match TARGET.md, acyclic, model/safety isolated |
| `python3 scripts/check_evidence.py check` | PASS: 148 packets checked; no new warning for any E13-S0x packet |
| `python3 scripts/check_docs.py check` | PASS: 400 Markdown files; local links/safety IDs consistent |
| `python3 scripts/gen_docs.py --check` | PASS: `docs/CLI.md` up to date (unaffected - Rust-only change) |
| `python3 scripts/check_process.py check` | PASS: pre-existing E00/E07/E12 round-ceiling warnings only, none new |
| `python3 scripts/check_risk_classification.py check` | PASS: no new story below its floor |
| `python3 scripts/verifier_handoff.py check` | PASS |
| `python3 scripts/safety_oracle.py check` | PASS |
| `python3 scripts/check_ears.py check` | PASS: pre-existing baseline warnings on unrelated stories only |
| `python3 scripts/gate_sensitivity.py check` | PASS: 11 mutants, 11 killed |
| `python3 scripts/release.py check` | PASS: v1.14.0 consistent |
| `python3 scripts/check_repository_topology.py check` | PASS |
| `python3 scripts/check_agent_skills.py check` | PASS |
| `python3 scripts/project_os.py check` | PASS: `governance OK: 24 decisions, 32 epics, 165 stories` |

`mypy`/`pytest`/`ruff` were not re-run: no Python source file changed by this self-review's fix
(only `rust/crates/cancellai-store/src/budget.rs`, `project/evidence/E13-S04/EVIDENCE.md`, and
this document) - matching E13-S02/S03/S04's own recorded precedent for the same situation; CI
runs them regardless.

`gh run list --branch main` shows the most recent `rust` CI failure is on commit `c6a9af4`
(pre-E13, still the current tip of `origin/main` - local `main` is 8 commits ahead, not yet
pushed) and is the Windows quarantine-destination-name defect `f7f9fcf` already repairs; this is
the same explained-and-fixed-but-not-yet-pushed situation round 1's own review already recorded,
not a new finding.

## Documents actually opened, by path

- `project/epics/E13.json`
- `docs/architecture/PERSISTENCE_MODEL.md` (all three layers, Self-budget, Ephemeral mode
  sections)
- `docs/security/SAFETY_INVARIANTS.md` (SI-024, SI-026)
- `docs/development/AGENT_PROTOCOL.md` ("What independent can mean here", "Self-review")
- `project/evidence/E13-VERIFIER-REVIEW.md` (Codex round 1)
- `project/evidence/E13-S02/EVIDENCE.md`, `E13-S03/EVIDENCE.md`, `E13-S04/EVIDENCE.md`,
  `E13-S05/EVIDENCE.md`
- `rust/crates/cancellai-store/src/lib.rs` (full file)
- `rust/crates/cancellai-store/src/ledger.rs` (full file)
- `rust/crates/cancellai-store/src/rollup.rs` (full file, including its test module)
- `rust/crates/cancellai-store/src/budget.rs` (full file, before and after this self-review's fix)
- commits `5196274` (round 1 review record), `2970ee3` (round 1 repair) via `git show`

## Round verdict

Pre-round-2 self-review. Found and repaired one real defect (E13-S04) in round 1's own repair,
using an actual reproduction (not a hypothetical) before writing the fix. E13-S02/S03/S05's
round 1 repairs hold under re-reproduction of the exact original findings plus an adjacent-defect
sweep; no further defect found in those three. This does **not** close the epic or any story:
all four stories (E13-S02 through E13-S05) remain `ready_for_review` pending the actual
independent (Codex) round 2 this repository has not yet run. The one residual noted above
(`compact_oldest_to_fit`'s unchecked `EventId` arithmetic at astronomically large event counts)
is disclosed for round 2's own judgment, not treated as blocking here.

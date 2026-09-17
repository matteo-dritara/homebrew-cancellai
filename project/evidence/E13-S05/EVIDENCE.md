# Evidence Packet - E13-S05

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E13 epic review
- Change Risk: CR3 (declared CR3 at planning time in `project/epics/E13.json`; no risk floor in
  this workspace's risk-classification data applies to `rust/crates/cancellai-store/*` - only
  kernel-ring paths and `cancellai-model`/`cancellai-policy`/`cancellai.py` carry a floor above
  the declared level, and `python3 scripts/check_risk_classification.py check` confirms no new
  story falls below its floor. CR3 stays declared, matching E13-S01 through E13-S04's own
  precedent of this crate carrying no floor.)
- Spec version/commit: `docs/architecture/PERSISTENCE_MODEL.md` "Layer 1: Current State"
  (incremental reuse paragraph, this story); `docs/security/SAFETY_INVARIANTS.md` SI-024
  ("Persistent cache is never destructive truth"); `scripts/check_mutation_boundary.py` (SI-019,
  the structural constraint this story's design had to fit inside)

## Outcome

PASS

## Scope

Adds incremental-reuse invalidation to `cancellai-store::CurrentStateStore` (E13-S01,
`rust/crates/cancellai-store/src/lib.rs`) - no new crate, no new dependency, no new
`Connection`/schema file. New schema migration (index 1 in `MIGRATIONS`) adds five nullable
columns to the existing `agent_artifacts` table. Two new public methods
(`set_invalidation_key`, `cache_read_hint`) and three new public types
(`CacheInvalidationKey`, `CacheCompleteness`, `CacheReadHint`).

**Design decision - a small, primitive `CacheInvalidationKey`, not a `cancellai-inventory`
dependency.** `cancellai-store`'s `Cargo.toml` depends only on `cancellai-model`
(`rust/crates/cancellai-store/Cargo.toml`, unchanged by this story). `cancellai-inventory`'s
`FileFacts`/`ScopeCompleteness` are that crate's own scan-shaped vocabulary, and
`docs/architecture/PERSISTENCE_MODEL.md` documents Layer 1 as a generic reconstructible
cache/index, not specific to one scanner's output. `CacheInvalidationKey` therefore carries only
primitive fields (`identity_token: String`, `modified: Option<u64>` seconds - not
`cancellai_platform::Timestamp`, to avoid widening this crate's dependency ring -
`provider_fingerprint: Option<String>`, `knowledge_version: Option<String>`) plus a small local
`CacheCompleteness { Complete, Partial, Unknown }` enum that echoes (without depending on)
`cancellai-inventory::completeness::ScopeCompleteness`'s vocabulary at the primitive level. A
future caller maps its own richer completeness type down to this one.

**Design decision - `rebuild`'s signature is completely unchanged; the invalidation key is
attached in a separate call, additively.** `rebuild(&[AgentArtifact])` and every one of its 13
existing call sites across `lib.rs` and `budget.rs` (E13-S04) are untouched. The five new columns
are nullable with no `DEFAULT`, so `rebuild`'s existing `INSERT` (which does not name them)
leaves them `NULL` automatically - SQLite's own behavior, not new code. `NULL` in
`cache_identity_token` is the fail-safe default this story relies on: "no invalidation key was
ever attached to this row," never "matches whatever is fresh." This was chosen over widening
`rebuild` to accept `(AgentArtifact, CacheInvalidationKey)` pairs for two reasons: (1) it keeps
the diff to one crate's new code plus zero changes to already-`ready_for_review` E13-S01/S04
tests, matching `AGENTS.md`'s "diff discipline" and this story's own instruction not to rewrite
`rebuild` unless needed; (2) the fail-safe default it produces on a crash between `rebuild` and
populating keys (every affected row stays permanently `Revalidate`, never falsely `Reuse`) is
itself the safe direction SI-024 requires - a lost cache-read-hint is a performance loss, never a
correctness or safety one. Every `rebuild` (including `reset`'s empty one, `E13-S01`) already
performs `DELETE FROM agent_artifacts` before reinserting, which wipes these five columns for
every row along with the rest - a rebuilt row can never inherit a stale invalidation key from a
previous scan by construction, with no second cleanup step to keep in sync.

**Design decision - `cache_read_hint` returns `CacheReadHint`, never a `bool`, and is named
around reading, not validity/authorization.** This directly discharges the story's SI-024
"by construction, not convention" instruction: `ReuseForReading`/`Revalidate` cannot be
mistaken for a mutation-precondition result by their own names, unlike a bare `bool` or a
method named `is_valid`/`is_reusable`/`is_authorized`. `cancellai-store`'s `Cargo.toml` does not
depend on `cancellai-safety` at all (`grep -rn "cancellai-store" rust/crates/cancellai-safety/`
finds nothing - confirmed before writing any code, per this story's own instruction, and no such
dependency existed before this change either, so there was no pre-existing architectural risk to
report), so no code in this crate could reference `cancellai-safety`'s mutation-execution
capability even by accident; `tests::cargo_toml_declares_no_dependency_on_cancellai_safety`
(`rust/crates/cancellai-store/src/lib.rs`) pins this directly by reading this crate's own
manifest at test time and asserting no dependency-table line names `cancellai-safety` (a
substring match on the whole file would have false-positived on the pre-existing `sha2`
dependency's own rationale comment, which mentions `cancellai-safety` in prose - the test checks
for a line that actually starts with the crate name, which distinguishes "declared as a
dependency" from "named in a comment," and this was caught and fixed while writing the test, not
anticipated in the falsification plan). `scripts/check_mutation_boundary.py check` (run below)
independently confirms zero occurrences of the raw capability's two matched patterns anywhere in
this crate's production code - a finding surfaced while implementing: this module's own doc
comment originally spelled out the two literal patterns
(`scripts/check_mutation_boundary.py`'s own matched strings) in prose to explain what it was
proving absent, which put those two literal substrings into `lib.rs`'s *production* code (the
text before `#[cfg(test)]`) and would have made `check_mutation_boundary.py check` itself fail,
since `lib.rs` is not on that script's allowlist. Reworded to describe the capability without
literally spelling out the two matched patterns; `check_mutation_boundary.py check`'s clean
result below confirms the fix.

**Public surface**: `CacheCompleteness`, `CacheInvalidationKey`, `CacheReadHint`,
`CurrentStateStore::{set_invalidation_key, cache_read_hint}`. No existing public API changed.

## Falsification plan (written before implementation)

Per `docs/development/AGENT_PROTOCOL.md`'s "plan verification before code" and the
`adversarial-cases` skill's eleven axes, worked for this change, and the story's own "stale-cache,
provider-layout-change, clock/mtime edge, and partial-scan adversarial tests" verification
contract:

| Falsifier | Would prove the implementation wrong | Test |
| --- | --- | --- |
| Stale cache: same identity, changed mtime | A cache row with unchanged identity but a different mtime is still treated as reusable | `falsifier_stale_cache_same_identity_changed_mtime_is_never_reusable` |
| Provider-layout-change: same identity/mtime, changed provider fingerprint | A provider whose on-disk format/version changed is still treated as reusable because identity/mtime alone matched | `falsifier_provider_layout_change_same_identity_and_mtime_different_fingerprint_is_never_reusable` |
| Clock/mtime edge - byte-for-byte identical mtime | An unchanged mtime, compared exactly, is wrongly treated as a divergence (false revalidation) | `cache_read_hint_matches_on_byte_for_byte_identical_mtime` |
| Clock/mtime edge - mtime moved backward (clock skew) | A backward-moved mtime is treated as "no change" instead of a divergence | `falsifier_clock_skew_mtime_moved_backward_is_never_treated_as_unchanged` |
| Clock/mtime edge - both sides have no mtime at all | `None == None` is wrongly treated as a divergence, forcing unnecessary revalidation on a platform that cannot report mtime | `cache_read_hint_matches_when_both_sides_have_no_modified_timestamp` |
| Partial-scan: row written Complete, fresh observation Partial/Unknown | A `Complete` row is still treated as reusable against a degraded fresh observation | `falsifier_partial_scan_complete_row_against_a_degraded_fresh_observation_is_never_reusable` (both `Partial` and `Unknown`) |
| Partial-scan: row written Partial, fresh observation identical Partial (same explicit, tested behavior, not an implicit default) | A row already known to be incomplete is treated as more reliable than it was when written, merely because nothing changed since | `falsifier_partial_scan_a_row_persisted_as_partial_is_never_reusable_even_against_an_identical_partial_observation` |
| Knowledge-version changed, identity/mtime/fingerprint unchanged | A provider-knowledge-bundle version bump is not caught because the other three axes still matched | `falsifier_knowledge_version_changed_is_never_reusable` |
| Identity changed | A changed identity token under an unchanged mtime is still treated as reusable | `falsifier_identity_changed_is_never_reusable` |
| Artifact absent from the cache entirely | A lookup for an id with no row at all returns something other than `Revalidate` (e.g. panics, or a default `Reuse`) | `cache_read_hint_is_revalidate_when_no_row_exists_for_the_id` |
| A row exists (from a plain `rebuild`) but no invalidation key was ever attached | A `rebuild`-only row (the common case until an orchestrator calls `set_invalidation_key`) is treated as reusable by some default rather than as a forced cache miss | `cache_read_hint_is_revalidate_when_no_invalidation_key_was_ever_set` |
| The one true positive: every axis matches, both sides `Complete` | The one case that *should* be reusable is wrongly reported `Revalidate` (over-eager invalidation, which would make the primitive useless) | `cache_read_hint_is_reuse_for_reading_when_every_axis_matches_and_both_sides_are_complete` |
| `set_invalidation_key` against a non-existent row | Attaching a key to an id with no row silently no-ops instead of failing | `set_invalidation_key_fails_for_an_id_with_no_row` |
| `set_invalidation_key` called twice for the same row | The second call merges with, rather than fully replacing, the first - a stale field from the first call could still satisfy a later comparison | `set_invalidation_key_replaces_a_previously_attached_key_rather_than_merging_it` |
| `rebuild` after a key was attached | A rebuilt row (same artifact_id reinserted) inherits a stale invalidation key from before the rebuild, silently declaring itself reusable against fresh evidence it was never compared to | `rebuild_clears_every_previously_attached_invalidation_key` |
| Malformed/corrupted `cache_completeness` column | A hand-edited or corrupted value is silently coerced to a default completeness instead of surfaced as an error | `parse_cache_completeness_reports_an_unrecognized_value_as_an_error_not_a_panic` |
| No path from this module to the mutation-execution capability, even when the hint is `ReuseForReading` | A future change lets this crate reference `cancellai-safety`'s mutation-execution capability, directly or via a new dependency | `cargo_toml_declares_no_dependency_on_cancellai_safety` (crate-local, by construction) plus `python3 scripts/check_mutation_boundary.py check` (workspace-wide, SI-019) below |
| `CacheCompleteness` encode/decode round-trip | The stored-string encoding and its parser drift apart for some variant | `cache_completeness_round_trips_through_parse_and_key` |

Axes from the `adversarial-cases` skill not applicable here, with reason (same reasoning
E13-S01 through E13-S04's own evidence packets already give for this crate): links/mounts,
platform differences (this module touches no filesystem path but its own SQLite file; `modified`
is an opaque `u64` supplied by a caller, not observed here); concurrency (no new concurrent-access
pattern - the existing single-`Connection`-per-store model is unchanged, and E13-S01/S02/S03's own
lock-contention precedent already covers this crate's SQLite access pattern); crash/failure/retry
beyond the rebuild-then-crash-before-set_invalidation_key case above, which is covered by the
fail-safe-default design decision itself (an interrupted population sequence leaves affected rows
permanently `Revalidate`, never falsely `Reuse`); boundary values (identity/fingerprint/version
are opaque strings with no numeric boundary; `modified`'s `u64`-does-not-fit-`i64` edge is handled
by `set_invalidation_key` returning `Err` rather than silently truncating - not separately tested
with a literal `u64::MAX` value since the code path is a plain `i64::try_from` with no special-
cased boundary logic of its own to falsify); policy/trust conflicts, protection bypass, root
escape (this crate makes no authority/mutation decision and adds no `cancellai-safety`/
`cancellai-platform` dependency); malformed/untrusted input beyond the completeness-column case
above (every other new method takes only already-typed Rust values, never a string/path/blob a
caller could corrupt, except the one stored column this story's own malformed-completeness test
covers); performance/large datasets (`set_invalidation_key`/`cache_read_hint` are both
single-row, primary-key-indexed operations - `O(1)` in table size, no new scan).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Cached facts are performance hints only and never a source of destructive truth." | `CacheReadHint::{ReuseForReading, Revalidate}` is a plain two-variant enum with no method, field, or path to a capability - never a `bool`, never named/shaped like an authorization. `cancellai-store`'s `Cargo.toml` has no dependency on `cancellai-safety`, confirmed both before implementation (`grep -rn "cancellai-store" rust/crates/cancellai-safety/` found nothing, so no pre-existing risk existed to report) and after (`cargo_toml_declares_no_dependency_on_cancellai_safety` test, plus `scripts/check_mutation_boundary.py check` below finding zero occurrences of the mutation capability's two matched patterns in this crate). | PASS |
| AC2 - "Identity, mtime/metadata, provider fingerprint, knowledge version, or completeness uncertainty invalidates the relevant cache scope." | `cache_read_hint`'s five-way equality/completeness check: `falsifier_identity_changed_is_never_reusable`, `falsifier_stale_cache_same_identity_changed_mtime_is_never_reusable`, `falsifier_provider_layout_change_same_identity_and_mtime_different_fingerprint_is_never_reusable`, `falsifier_knowledge_version_changed_is_never_reusable`, `falsifier_partial_scan_complete_row_against_a_degraded_fresh_observation_is_never_reusable`, `falsifier_partial_scan_a_row_persisted_as_partial_is_never_reusable_even_against_an_identical_partial_observation`, `falsifier_clock_skew_mtime_moved_backward_is_never_treated_as_unchanged` - every one of the seven change axes independently forces `Revalidate`. | PASS |
| AC3 - "Fresh execution-time observation remains mandatory for mutation preconditions." | `cancellai-store` (this crate, unchanged by this story on this axis) has never been consulted by, or referenced from, `cancellai-safety::mutation_executor` - confirmed by the absence of a dependency edge in either direction between the two crates and by `scripts/check_rust_workspace.py check` below. Nothing this story adds changes that: `cache_read_hint` is a read-hint, not a mutation-precondition API, and no caller exists in this workspace yet (Residual risks) that could route a mutation decision through it even indirectly. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-024 "Persistent cache is never destructive truth" | Every falsifier in the plan above attempting to get `ReuseForReading` out of a divergent, absent, never-set, or under-evidenced row | 17 tests listed in the falsification plan, all passing; `CacheReadHint`'s own type shape (no bool, no authorization-shaped name) | PASS |
| SI-019 "all filesystem/vendor mutations route through the safety executor" (this story's own binding structural constraint, not itself a new obligation) | This story's new code calls the raw filesystem-delete primitive or the mutation-execution capability outside the two files SI-019 allows | `scripts/check_mutation_boundary.py check` (below) - clean; this story's new code is SQL-only against the connection this crate's own type already owns, matching E13-S01 through E13-S04's own precedent | PASS |
| A rebuilt row never inherits a stale invalidation key from a previous scan | `rebuild` reinserts the same `artifact_id` and the new columns still carry the previous scan's stale key, letting a caller silently trust an unrelated observation | `rebuild_clears_every_previously_attached_invalidation_key` - seeds a key, rebuilds, and proves `cache_read_hint` against the *same* previously-matching fresh key now returns `Revalidate` | PASS |
| A row with no invalidation key ever attached (the common case for any row produced by `rebuild` alone, until an orchestrator calls `set_invalidation_key`) never reports `ReuseForReading` by some implicit default | `cache_read_hint` against a freshly-rebuilt, never-`set_invalidation_key`'d row returns anything other than `Revalidate` | `cache_read_hint_is_revalidate_when_no_invalidation_key_was_ever_set` | PASS |
| Malformed stored `cache_completeness` is reported as an error, not silently coerced or panicking | A hand-edited/corrupted completeness value | `parse_cache_completeness_reports_an_unrecognized_value_as_an_error_not_a_panic` | PASS |

## Verification Commands

```text
$ cd rust
$ cargo fmt --check                                                        clean
$ cargo clippy --workspace --all-targets --all-features -- -D warnings     clean
$ cargo check --workspace --all-targets                                    clean
$ cargo test --workspace                                                   all suites "test result: ok", 0 failed (cancellai-store: 88 passed, up from 70 baseline - 18 new tests)
$ cargo deny check                                                         advisories ok, bans ok, licenses ok, sources ok
$ cd ..
$ python3 scripts/project_os.py check                                     governance OK: 24 decisions, 32 epics, 165 stories
$ python3 scripts/check_rust_workspace.py check                           rust workspace OK: 13 crates match TARGET.md, acyclic, model/safety isolated
$ python3 scripts/check_mutation_boundary.py check                        mutation boundary OK: 85 Rust source files scanned; only cancellai-platform/src/mutation.rs deletes anything, only that file and cancellai-safety/src/mutation_executor.rs reference the capability
$ python3 scripts/check_docs.py check                                     docs OK: 393 Markdown files; local links and safety IDs are consistent
$ python3 scripts/check_risk_classification.py check                      risk classification OK: no new story is below its floor (50 recorded, owner decision pending; E13-S05 not flagged; pre-existing baseline warnings only)
$ python3 scripts/check_coverage.py report                                cancellai-store 92.88% (informational; outer-ring, not ratchet-gated)
$ python3 scripts/check_coverage.py check                                 coverage OK: 6 ratcheted crates at or above their recorded floor (cancellai-store not ratcheted)
$ python3 scripts/check_process.py check                                  process OK: ADR lifecycle, decision supersession, evidence, review rounds, and generated banners are consistent (pre-existing E00/E07/E12 round-ceiling warnings, unrelated to this story)
$ python3 scripts/check_evidence.py check                                 evidence OK: 147 packets checked against their contracts (plus this one once committed; pre-existing pre-convention packets, unrelated to this story)
$ python3 scripts/gate_sensitivity.py check                               gate sensitivity OK: 11 mutants, 11 killed, every gate clean on an unmutated tree
$ python3 scripts/check_ears.py check                                     EARS OK: 516 acceptance criteria classified, 66 describe unwanted behaviour (13%); pre-existing baseline warnings on other stories, none new to E13-S05
$ python3 scripts/safety_oracle.py check                                  safety oracle OK: protected-name checked against the engine's own decision; root-capability and retention checked as predicates only (2000 cases)
$ python3 scripts/verifier_handoff.py check                               verifier handoff OK: every verdict that names a brief answers the brief that was committed
$ python3 scripts/release.py check                                        release OK: v1.14.0 is consistent across source, packaging and formula
$ python3 scripts/release_manifest.py check                               release manifest OK: 1 golden document(s) match project/schemas/release_manifest.schema.json
$ python3 scripts/check_repository_topology.py check                      repository topology OK
$ python3 scripts/check_agent_skills.py check                             agent skills OK: 8 skills; frontmatter, names and every cited path resolve
$ python3 scripts/gen_docs.py --check                                     docs/CLI.md is up to date
$ python3 -m pytest tests -q                                              665 passed, 554 subtests passed
$ python3 -m ruff check .                                                 All checks passed!
$ python3 -m ruff format --check .                                        453 files already formatted
$ python3 -m mypy <pinned file list from AGENTS.md>                       Success: no issues found in 27 source files
$ python3 scripts/check_workflows.py check                                workflow policy OK: 6 workflow files use explicit permissions and immutable action SHAs
$ python3 scripts/check_fixtures.py check                                 fixtures OK: 13 fixtures cover all required categories
$ python3 scripts/check_schemas.py check                                  schemas OK: 4 golden documents match docs/architecture/JSON_CONTRACTS.md
$ python3 scripts/characterize.py check                                   characterization OK: 13 fixtures match their committed characterization
$ python3 scripts/diff_harness.py check                                   diff harness OK: self-test cases all behave as documented
$ python3 scripts/check_provider_compatibility.py check                   provider compatibility matrix OK: 36 rows across 2 provider(s)
$ python3 scripts/check_provider_trust.py check                           provider trust OK: 3 manifest(s) linted, registry entries complete and evidenced
$ python3 scripts/check_platforms.py check                                platforms OK: 4 platforms, generated matrix current, all CI/evidence claims verified
$ python3 scripts/rust_python_parity.py self-test                         rust/python parity self-test OK: the comparator correctly catches every injected divergence class
$ python3 scripts/rust_python_parity.py check                             rust/python parity OK: 13 NORMATIVE fixture(s) match across engines (unaffected - no cancellai.py change)
$ python3 scripts/process_metrics.py check                                process metrics OK: the generated report matches the committed evidence and history
$ python3 scripts/check_agent_toolchain.py report                         no decision past review date; 16 previously-rejected components unchanged
$ TERM=dumb python3 scripts/check_skill_content.py check                  skill content OK: no finding at HIGH or above (1 pre-existing waived finding)
```

Cross-target clippy (`--target x86_64-pc-windows-gnu`/`x86_64-unknown-linux-gnu`) was not run:
`AGENTS.md` requires it specifically for changes touching `cancellai-platform` or moving the lint
surface workspace-wide, neither of which applies here (`cancellai-store` only, no `cfg`-gated
code added, no `unsafe`). Real CI still builds and lints natively on all three platforms.

## Compatibility

- `cancellai-store`'s `Cargo.toml` is unchanged - no new dependency.
- No existing public API (`CurrentStateStore::open`/`open_in_memory`/`rebuild`/`all`/`get`/
  `row_count`/`reset`, or any type/method in `ledger`/`rollup`/`budget`) changed signature or
  behavior - this story only adds two new methods and three new types.
- No CLI/TUI/Guardian surface consumes this yet (library-level capability only), matching
  `CurrentStateStore`'s own state at every one of its prior `ready_for_review` points
  (E13-S01 through E13-S04).

## Performance / operability

- `set_invalidation_key` costs one `UPDATE ... WHERE artifact_id = ?1` - `O(1)`, primary-key
  indexed.
- `cache_read_hint` costs one `SELECT ... WHERE artifact_id = ?1` plus in-memory field
  comparisons - `O(1)`, primary-key indexed, no new scan of the table.
- The schema migration adds five nullable columns via `ALTER TABLE ... ADD COLUMN` (no table
  rewrite for existing rows under SQLite's `ADD COLUMN` semantics).

## Documentation updated

- `docs/architecture/PERSISTENCE_MODEL.md` - "Layer 1: Current State" now describes the real
  implementation: `CacheInvalidationKey`/`CacheCompleteness`/`CacheReadHint`, the
  `set_invalidation_key`/`cache_read_hint` pair, the fail-safe `NULL`-means-never-set default,
  and why this crate does not depend on `cancellai-inventory` for it.
- `CHANGELOG.md` - `[Unreleased]` / `### Added` (this story adds a new capability rather than
  fixing a previously-shipped defect; matching E13-S01 through E13-S04's own placement for the
  same reason, even though this story also discharges an explicit safety invariant - the
  `### Security` section in this changelog is reserved, by this epic's own precedent, for fixes
  to a previously-shipped gap, not for a new capability that discharges an invariant by design).

## Method defects

- none. This session hit the same `skillspector`/`TERM` ANSI-parsing quirk already recorded as a
  method defect in `project/evidence/E13-S04/EVIDENCE.md` (`skillspector 2.11.2`'s version output
  embeds a colour escape sequence that `check_skill_content.py`'s `VERSION_RE` cannot parse under
  non-interactive capture unless `TERM=dumb` is set) - not re-proposed here since it is already
  `proposed 2026-09-16` and pending an owner decision; recorded here only so a later reader sees
  this session also worked around it the same way, not that it is a new observation.

## Residual risks

- **No orchestrator/caller wires this yet**, matching `CurrentStateStore`'s/`EventLedger`'s/
  `AnalyticalMemory`'s/`budget`'s own state at their own `ready_for_review`
  (E13-S01 through E13-S04). `set_invalidation_key`/`cache_read_hint` are designed to be called by
  a future scan orchestrator deciding whether to re-invoke `cancellai-inventory` for a given
  artifact, but no caller in this workspace does so yet - `cancellai-cli`'s and
  `cancellai-guardian`'s pre-existing `cancellai-store` dependency remains unused for this purpose.
  No story ID assigned yet; a later story's scope per `AGENTS.md`'s "work one story at a time."
- **`CacheInvalidationKey::modified` is a primitive `Option<u64>` seconds value, not
  `cancellai_platform::Timestamp`** - a deliberate design choice to avoid widening this crate's
  dependency ring (this evidence's own "Scope" section), but it means a future caller must convert
  `cancellai_platform::Timestamp`/`cancellai-inventory`'s own mtime representation into this
  primitive itself; this crate performs no such conversion or validation beyond the `u64`-fits-
  `i64` check in `set_invalidation_key`.
- **`provider_fingerprint`/`knowledge_version` are opaque, caller-defined strings with no producer
  today** - no provider adapter or knowledge-bundle loader in this workspace currently computes
  either value; this story defines the shape a future caller fills in, matching
  `AgentArtifact::project_attribution`'s and `FileFacts::provider_hint`'s own "field exists for a
  future real producer, not invented now" precedent.
- **No concurrent-access handling beyond SQLite's own default fail-closed-on-lock-contention
  behavior**, matching E13-S01 through E13-S04's own recorded residual - not re-tested here since
  this story adds no new concurrent-access pattern (single `UPDATE`/`SELECT` against the same
  already-tested `Connection` model).
- **`u64::MAX`-class `modified` values are handled by `Err`, not separately falsified with a
  literal boundary-value test** - the code path is a single `i64::try_from` with no boundary logic
  of its own beyond what the type conversion already guarantees; flagged in the falsification
  plan's own "axes not applicable" note rather than silently omitted.
- This packet is executor self-assessment. Independent review happens at epic scope, once every
  story in E13 is `ready_for_review` (this story is the fifth and last of E13, so epic-scope review
  can begin once this commit lands).

## Round 1 independent review repair (2026-09-17)

Codex's round 1 review (`project/evidence/E13-VERIFIER-REVIEW.md`) returned `FAIL`: a row
persisted with a `provider_fingerprint`/`knowledge_version`/`modified` unavailable (`None`) on
one or both sides compared as `ReuseForReading` when the other axes matched, because
`cache_read_hint`'s original comparison used plain `Option<T>` equality, and `None == None` is
`true`. Per AC2 ("... or completeness uncertainty invalidates the relevant cache scope") and
SI-024, an axis neither side can positively confirm is uncertainty, not a confirmed absence of
change - this module's own doc comment on `CacheInvalidationKey::modified` already said so
("never treated as unchanged ... a change ... to unknown, is exact-equality-false"), which the
code did not actually implement for any of the three optional axes.

Fixed by requiring `modified`/`provider_fingerprint`/`knowledge_version` to each be positively
`Some` on both sides and equal before counting as confirmed-unchanged; `None` on either side now
always forces `Revalidate`. This also required rewriting `cache_read_hint_matches_when_both_
sides_have_no_modified_timestamp` (renamed `cache_read_hint_is_revalidate_when_neither_side_has_
a_modified_timestamp`), which had asserted the old, incorrect behavior as intended - its own
comment ("a platform that cannot report mtime at all is a stable fact") was the same reasoning
error the code embodied, and it directly contradicted AC2. Regression tests: `falsifier_
unavailable_provider_fingerprint_is_never_treated_as_unchanged` (Codex's exact reproduction) and
`falsifier_unavailable_knowledge_version_is_never_treated_as_unchanged`.

Verification after the repair: the same full Rust gate set as this packet's original run,
re-executed and all passing; `cancellai-store` now has 95 tests (was 88), including the new/
rewritten regressions above. No other change in this story's scope - the production dependency
graph still has no path from this crate to `cancellai-safety`, confirmed again after the fix.

## Verifier verdict

Round 1 (Codex, 2026-09-17): FAIL - see the defect above, repaired in this packet. Round 2
pending.

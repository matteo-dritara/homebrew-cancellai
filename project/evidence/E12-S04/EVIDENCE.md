# Evidence Packet - E12-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex, round 1 - **FAIL** (`project/evidence/E12-S04-VERIFIER-REVIEW.md`),
  repaired below; round 2 pending
- Change Risk: CR4 (declared CR4 at planning time in `project/epics/E12.json`, matching
  E12-S01/S02/S03's own level for SI-020; the diff adds no new mutation capability and no new
  cross-crate dependency, so no reclassification applies)
- Spec version/commit: `docs/architecture/PERSISTENCE_MODEL.md` "Tombstones"; `docs/security/
  SAFETY_INVARIANTS.md` SI-020 ("Irreversible actions are explicit and stronger-gated")

## Outcome

PASS after repair (see "Repair - round 1 independent review finding" below)

## Scope

`cancellai_store::tombstone` (new module, `rust/crates/cancellai-store/src/tombstone.rs`): a
typed, narrower front door onto `cancellai_store::ledger::EventLedger`'s existing
`EventKind::Purged` event kind (E13-S02), adding no new schema, file, connection, or crate
dependency. `Tombstone` is the allowlisted record `docs/architecture/PERSISTENCE_MODEL.md`'s
"Tombstones" section names (opaque artifact ID, provider/category, reason/policy ID, plan ID,
evidence IDs). `record_purge_tombstone(ledger, action_class, reversibility, recorded_at,
tombstone)` writes exactly one `Purged` ledger event, refusing - with nothing written - unless
`action_class == ActionClass::Delete && reversibility == Reversibility::Irreversible`. No
orchestrator wires this to a real purge yet (`mutation_executor::execute`'s `ActionClass::Delete`
branch, E03-S05, is unchanged); this story delivers the primitive, matching every prior E12/E13
story's own "primitive delivered, no orchestrator yet" precedent.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "Tombstones contain no prompts/source/file contents." | `Tombstone`'s field set is exactly `artifact_id`, `provider_id`, `category`, `reason_code`, `policy_id`, `plan_id`, `evidence_ids`, and `record_purge_tombstone` now validates every one of those caller-supplied values with `validate_content_safe`/`validate_optional_content_safe` before any ledger write: each non-empty value must be ASCII letters/digits joined by at most 4 single hyphens, at most 32 characters per segment and 64 total - the shape every real ID in this system already has - or the call is refused with `PurgeTombstoneError::UnsafeField` and nothing is written. This closes round 1's finding that a column allowlist alone does not restrict field *content* (see Repair section). `record_purge_tombstone_round_trips_exactly_the_given_allowlisted_fields` proves a *content-safe* tombstone still reads back with exactly and only the given fields. | PASS |
| AC2 - "Irreversible purge is distinguishable from vendor-native conditionally reversible operations." | `record_purge_tombstone_refuses_every_combination_except_delete_plus_irreversible` is the exhaustive falsification: all 5 `ActionClass` x 6 `Reversibility` combinations (30 total) `cancellai-model`'s shared vocabulary admits are exercised; only `Delete + Irreversible` is accepted and produces a `Purged` event, every other combination - including `Reversibility::VendorConditional` (the vocabulary's own name for a vendor-native conditionally-reversible outcome) paired with every action class, and `Quarantine`/`Archive`/`Restore`/`Observe` paired with every reversibility - is refused with `PurgeTombstoneError::NotAnIrreversiblePurge` and writes nothing. | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Privacy field allowlist test" | `record_purge_tombstone_round_trips_exactly_the_given_allowlisted_fields` (see AC1). | PASS |
| "Purge evidence tests" | `record_purge_tombstone_accepts_delete_plus_irreversible_and_appends_a_purged_event` and `record_purge_tombstone_refuses_every_combination_except_delete_plus_irreversible` (see AC2); `record_purge_tombstone_refuses_empty_plan_id_and_empty_evidence_ids` proves the ledger's own mutation-reference fail-closed contract (E13-S02) still applies at this API's boundary - an empty `plan_id`/`evidence_ids` is refused and writes nothing even when the action/reversibility check already passed. | PASS |

## Adversarial-cases pass (CR4)

Falsification axes worked before implementation (`adversarial-cases` skill):

| # | Axis | Case | Expected | Test |
| --- | --- | --- | --- | --- |
| 7 | Boundary | Empty `plan_id` / empty `evidence_ids` | Refused, nothing written | `record_purge_tombstone_refuses_empty_plan_id_and_empty_evidence_ids` |
| 8 | Second-path / SI-020 | Every `ActionClass` x `Reversibility` combination (30) except `Delete+Irreversible` | Refused, nothing written, including `VendorConditional` | `record_purge_tombstone_refuses_every_combination_except_delete_plus_irreversible` |
| 10 | Malformed/untrusted | Round-trip of a fully-populated, content-safe tombstone | Exactly and only the given fields persist | `record_purge_tombstone_round_trips_exactly_the_given_allowlisted_fields` |
| 10 | Malformed/untrusted (round 1 repair) | A prompt, source-text, or absolute/relative path sentinel in each of the 7 retained fields (21 cases) | Refused as `UnsafeField`, nothing written | `record_purge_tombstone_refuses_prompt_source_and_path_sentinels_in_every_field` |
| 10 | Malformed/untrusted (round 1 repair) | Attacker words joined by hyphens instead of spaces, character-for-character as "safe" as a real ID | Refused on segment-count, not merely character class | `record_purge_tombstone_refuses_hyphen_joined_words_standing_in_for_a_sentence` |
| 10 | Malformed/untrusted (round 1 repair) | Codex's exact round-1 reproduction (`PROMPT_SENTINEL_do_not_store_source_contents` in five fields, a path in `category`) | Refused, nothing written | `record_purge_tombstone_refuses_the_codex_round1_reproduction` |
| 1,2,3,4,9,11 | Path/identity, partial reads, links/mounts, provider drift, platform, scale | N/A - this module never touches a path or the filesystem; it operates only on an already-open `&mut EventLedger` and already-decided `cancellai-model` enum values | - |
| 5 | Concurrency | Two concurrent `record_purge_tombstone` calls on one ledger | Impossible by construction - `&mut EventLedger` excludes aliasing at the borrow-checker level | - |
| 6 | Crash/retry | Crash between a real OS purge succeeding and a tombstone being written | Not this story's mechanism to close - no orchestrator calls `record_purge_tombstone` from a real purge yet (disclosed residual, below) | - |
| second-path | Does the local action_class/reversibility check duplicate `cancellai_safety::authority::reversibility_allowed` as an independent authorization decision? | No - it is a strictly narrower predicate (accepts only the one combination that check's `Delete` arm accepts), takes no `AuthorityLevel`, and decides only whether an already-completed, already-authorized outcome may be labeled a purge tombstone | Documented in `tombstone.rs`'s module doc and `docs/architecture/PERSISTENCE_MODEL.md` |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-020 ("Irreversible actions are explicit and stronger-gated") | Every non-`Delete+Irreversible` `ActionClass`/`Reversibility` combination, including `VendorConditional` | `record_purge_tombstone_refuses_every_combination_except_delete_plus_irreversible`: 29 of 30 combinations refused with `NotAnIrreversiblePurge` and zero ledger rows written; only `Delete+Irreversible` produces a `Purged` event | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                              # PASS
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings   # PASS
cd rust && cargo check --workspace --all-targets                          # PASS
cd rust && cargo test --workspace                                         # PASS - 0 failed (4 new cancellai-store::tombstone tests)
cd rust && cargo deny check                                               # PASS - advisories, bans, licenses, sources OK; no new dependency
python3 scripts/check_mutation_boundary.py check                          # PASS - only rust/crates/cancellai-platform/src/mutation.rs deletes anything
python3 scripts/check_schemas.py check                                    # PASS
python3 scripts/check_fixtures.py check                                   # PASS
python3 scripts/project_os.py check                                       # PASS
```

Not run: the Windows-target and Linux-target cross-compiled clippy passes AGENTS.md calls out for
changes to `cancellai-platform` or anything moving the workspace-wide lint surface - this story
touches neither `cancellai-platform` nor the workspace lint policy; CI's existing per-platform
`cargo check`/quality matrix covers `cancellai-store` on macOS, Linux and Windows as it already
does for E13-S01 through E13-S06. `pre-commit run --all-files` was not run standalone in this
session; the individual Python checks it bundles that are relevant to this diff's scope
(`check_mutation_boundary`, `check_schemas`, `check_fixtures`, `project_os check`) were run
directly above and passed - CI runs the full hook set before merge regardless.

## Repair - round 1 independent review finding

Codex's round-1 review (`project/evidence/E12-S04-VERIFIER-REVIEW.md`) FAILed AC1: `Tombstone`'s
fields were plain, unvalidated `String`/`EvidenceId` values, and `record_purge_tombstone` passed
them unchanged to `EventLedger`. The evidence packet's original AC1 argument ("no path, prompt,
or content-typed field exists on the struct") conflated a *schema-column* allowlist with
*content* validation of those columns' untyped `String` payloads - the same class of error, at a
different layer, as E13-S04's marker-mimicry gap. An independent reproduction stored
`PROMPT_SENTINEL_do_not_store_source_contents` in `provider_id`/`reason_code`/`policy_id`/
`plan_id`/an evidence ID and `/private/provider/source.rs` in `category`, and `EventLedger::read_all`
returned every sentinel unchanged.

Repair: `tombstone.rs` adds `validate_content_safe`/`validate_optional_content_safe`, called on
every caller-supplied field before `EventLedger::append` (atomic no-write on refusal by
construction - the function returns before touching the ledger). A value is accepted only if it
is empty (deferred to the ledger's own existing empty-`plan_id`/`evidence_ids` refusal, unchanged
by this repair) or shaped like every real ID this system produces: ASCII letters/digits joined by
up to 4 single hyphens, ≤32 characters per segment, ≤64 total. This is deliberately a shape check,
not only a character-class check: a character allowlist alone cannot distinguish `policy-0001`
from an attacker's message re-encoded as `prompt-sentinel-do-not-store-source-contents` (both are
letters/digits/hyphens), so segment count is bounded too, since no real ID here is built from more
than a few hyphenated words. Three new adversarial tests cover this (per-field sentinel sweep, the
hyphen-joined-words variant, and Codex's exact reproduction) - see the Adversarial-cases table
above.

## Compatibility

- No schema change, no new dependency, no wire-format change. `Tombstone` and
  `record_purge_tombstone` are new, additive public items; no existing public API changed
  signature or behavior. `cancellai-cli`/`cancellai-guardian` still do not call any of this from
  source - no live caller is wired yet, matching every prior E12/E13 story's own "primitive
  delivered, no orchestrator yet" precedent.

## Performance / operability

- `record_purge_tombstone` costs exactly one `EventLedger::append` call (one SQLite transaction,
  already the ledger's own established cost) plus a cheap enum comparison - no measurable
  additional cost.

## Documentation updated

- `docs/architecture/PERSISTENCE_MODEL.md`: new paragraphs under "Tombstones" naming the
  implementation, the AC1/AC2 discharge, and the size/reclaim-observation residual.
- `CHANGELOG.md`: `### Added` entry under `[Unreleased]`.

## Method defects

- none

## Residual risks

- **"Size/reclaim observation," one item `docs/architecture/PERSISTENCE_MODEL.md`'s "Tombstones"
  section names illustratively ("such as"), has no column in the ledger's already schema-pinned
  event table and is not carried by `Tombstone`.** Widening `ledger_events`' pinned schema for a
  per-artifact figure is a larger, separately reviewable change this story's acceptance criteria
  (which name only the privacy-allowlist and distinguishability contracts) do not require.
  `AnalyticalMemory`'s `ReclaimableBytes` metric already tracks reclaimable bytes as an aggregate
  time series (Layer 3), which is a different granularity (aggregate, not per-tombstone). No
  story currently carries adding a per-tombstone size column.
- **No orchestrator wires `record_purge_tombstone` to a real purge yet.** A crash between
  `mutation_executor::execute`'s `ActionClass::Delete` branch (E03-S05) succeeding and a future
  caller invoking this function to record the tombstone is that future orchestrating story's
  failure mode to close, not this one's - matching E13-S04's own "No CLI/TUI/Guardian surface
  calls any of this yet" precedent.

## Verifier verdict

Round 1: **FAIL** (Codex) - `project/evidence/E12-S04-VERIFIER-REVIEW.md`. Repaired above; round 2
pending.

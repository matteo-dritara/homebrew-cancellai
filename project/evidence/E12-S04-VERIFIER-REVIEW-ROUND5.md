# E12-S04 Verifier Review — Round 5

Review-Scope: story
Round: 5
Review target: repair commit `31cf69d` (`31cf69d^..31cf69d`); full story history `f215d33..31cf69d` on `main`
Verifier: Codex
Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
Date: 2026-09-20

## Verdict

`FAIL`

The round-4 `PURGED` representation bypass is closed. Public `EventLedger::append` now refuses
every `EventKind::Purged` event before `append_row`, including the exact well-shaped event round 4
used to demonstrate that `append` had no `ActionClass`/`Reversibility` proof. The sole current
writer of a `PURGED` row is the crate-private `append_purged`, called only by
`record_purge_tombstone` after its `ActionClass::Delete + Reversibility::Irreversible` check.

The outcome retains ADR-0033's accepted residual: a short, identifier-shaped ordinary phrase such
as `do-not-purge-this` remains possible in structurally required linkage fields. This is the
owner-authorized, disclosed AC1 limitation, not a new finding or an SI-020 bypass. The verdict is
nevertheless `FAIL` because the required closing governance gate cannot process the mandated
append-only Safety Verdict history.

## Brief and contract judgment

`project/evidence/E12-S04/VERIFIER_BRIEF.md` is checksum-valid and correctly identifies the
story, CR4 risk, outcome, and SI-020 obligation. Its original unqualified AC1 wording is stale;
the authoritative current contract is `project/epics/E12.json` together with accepted ADR-0033.
That ADR explicitly narrows AC1 to absent descriptive annotations plus identifier-shaped linkage,
and explicitly accepts the short-phrase residual pending an orchestrator.

## Independent reproductions and path search

| Check | Independent outcome |
| --- | --- |
| Round-4 exact reproduction: public `append` of a well-shaped `Purged` `NewEvent`, with no annotations and valid linkage but no action/reversibility inputs | Refused; `read_all()` stayed empty. Ran as an uncommitted external integration test: `cd rust && cargo test -p cancellai-store --test verifier_round5_public_append`. The test passed and was removed. |
| External visibility of `append_purged` | Refused at compile time. An uncommitted integration test calling only public APIs failed with `E0624: method append_purged is private`, pointing at its `pub(crate)` declaration. Ran `cd rust && cargo test -p cancellai-store --test verifier_append_purged_visibility`; the expected compile failure was observed and the test was removed. |
| Other path to a written `PURGED` row | `rg` search across `cancellai-store`, `cancellai-guardian`, and `cancellai-cli` found one production `INSERT INTO ledger_events`, in private `append_row`, and one `append_purged` call, in `tombstone::record_purge_tombstone`. No other `Purged` writer, raw SQL writer, externally visible crate-private method, or non-test raw-connection escape exists. CLI and Guardian contain no `EventLedger`/tombstone reference. |
| Test-only leakage | The raw SQLite connection is a private method under `#[cfg(test)]`; no `cfg_attr` or test-only public writer exposes it in a non-test build. |

`append_purged` remains callable to other modules inside `cancellai-store` by Rust's crate
visibility, but the current complete source has one call site. Any future internal call is still
subject to its own annotation/linkage gate and must be reviewed as a new route; none exists in the
reviewed target.

## AC1 and round-3 content-safety regression

The AC1/ADR-0033 behavior is unchanged after moving the boundary:

- `append_purged` rejects a `Purged` event if any of `provider_id`, `category`, `policy_id`, or
  `reason_code` is populated, or if artifact/plan/evidence linkage is not identifier-shaped.
- `record_purge_tombstone` emits every annotation as `None` and validates all linkage before it
  calls that boundary. The tombstone tests reject prompt, source, and path sentinels in each
  remaining linkage field.
- The `do-not-purge-this` short phrase remains accepted by a passing regression test, precisely
  matching ADR-0033's disclosed residual. It is the sole residual supporting this verdict rather
  than a reason to reject AC1.

## Mutation-reference contract

The refactor preserved the E13-S02 contract. `append_row`, the only raw insert site, checks every
`EventKind::is_mutation()` kind for a non-blank `plan_id` and a non-empty `evidence_ids` list.
Both public `append` and crate-private `append_purged` delegate to it. `Purged` additionally has
the earlier unconditional public refusal, and its controlled tombstone route is tested to refuse
an empty plan ID and empty evidence list with no write. Thus no mutation kind has lost the shared
reference requirement.

## Safety obligations

| Obligation | Evidence | Result |
| --- | --- | --- |
| AC1 / C-09, as narrowed by ADR-0033 | Structural absence of all descriptive annotations; shared identifier-shape gate at the actual `PURGED` insert boundary; prompt/source/path sentinel tests pass. | PASS_WITH_ACCEPTED_RESIDUAL |
| AC2 / SI-020 | The public route now rejects `Purged` unconditionally. The only current write route follows `record_purge_tombstone`'s `Delete + Irreversible` proof; every other action/reversibility pairing is refused. | PASS |
| E13-S02 mutation evidence | Shared `append_row` rejects mutation events without a non-empty plan ID and evidence list. | PASS |

## Closing-gate failure and required repair

The reviewed code meets AC1 as narrowed by ADR-0033 and meets AC2/SI-020; no remaining path to a
written `PURGED` row bypassing `Delete + Irreversible` was found. Closure nevertheless failed at
the required governance step:

```text
python3 scripts/project_os.py generate
GOVERNANCE ERROR: E12-S04: cannot be done - no committed Safety Verdict records PASS or PASS_WITH_RESIDUALS, and at least one records FAIL/REJECT
```

`scripts/project_os.py::safety_verdict_passes` searches the entire Safety Verdict for any line
that is exactly `FAIL` or `REJECT`. That contradicts the explicit append-only requirement for
round history: the preserved rounds 1–4 failures make a later passing Round 5 uncloseable.

Required repair: a separately owned governance story must change and test the Safety Verdict gate
to evaluate the newest attributable round verdict (or an equally explicit current-verdict field),
while preserving historical failures as history. It must include a regression with an append-only
CR4 verdict containing historical failures followed by a passing final round. This is not an
AC1/AC2 implementation failure; it violates C-16's evidence-gated delivery requirement because a
valid independent CR4 verdict cannot pass the mandatory closing gate. E12-S04 is returned to
`in_progress`; no code repair was attempted by this verifier.

## Gate status

| Command | Result |
| --- | --- |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS (125 `cancellai-store` unit tests; workspace green) |
| `cd rust && cargo deny check` | PASS; pre-existing unmatched-license and duplicate-dependency warnings only |
| `python3 scripts/project_os.py check` | PASS before transition; closing `project_os.py generate` FAILS as documented above, and `check` passes again after returning the story to `in_progress` |
| `python3 scripts/check_process.py check` | PASS; recorded historical review-round warnings only |
| `python3 scripts/check_evidence.py check` | PASS; recorded historical evidence-packet warnings only |
| `python3 scripts/verifier_handoff.py check` | PASS before this verdict; re-run after recording below |
| `python3 scripts/check_docs.py check` | PASS |

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`;
`project/epics/E12.json`; `project/evidence/E12-S04/VERIFIER_BRIEF.md`;
`project/evidence/E12-S04/EVIDENCE.md`; `project/evidence/E12-S04/SAFETY_VERDICT.md`;
`project/evidence/E12-S04-VERIFIER-REVIEW-ROUND4.md`;
`project/templates/SAFETY_VERDICT.md`; `docs/adrs/0033-purge-tombstone-content-safety-is-a-disclosed-residual.md`;
`docs/architecture/PERSISTENCE_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`;
`docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`;
`rust/crates/cancellai-store/src/lib.rs`; `rust/crates/cancellai-store/src/ledger.rs`; and
`rust/crates/cancellai-store/src/tombstone.rs`.

## Overall verdict

`FAIL` — the implementation closes SI-020's direct-ledger bypass and satisfies AC1 to ADR-0033's
owner-approved boundary, but the mandatory C-16 CR4 closing gate fails on the protocol-required
append-only Safety Verdict history. E12-S04 cannot be marked done until that governance defect is
repaired and independently re-reviewed.

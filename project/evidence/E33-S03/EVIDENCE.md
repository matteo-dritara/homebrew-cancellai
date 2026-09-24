# Evidence Packet - E33-S03

- Commit/PR: the E33-S03 commit on `main`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR4
- Spec version/commit: `project/epics/E33.json` at this commit; ADR-0040 (owner decision 2026-09-24)

## Outcome

PASS

## Why this story exists

E06 review rounds 8 and 9 found that the JSONL containment history decided from one read and
appended in a separate write: concurrent refreshes wrote duplicate lines that broke replay, then -
after a chain-digest/nonce redesign and a lock database - still wrote losing lines, and a crash
inside the one `write` could leave a torn line. The design consultation with Codex
(`project/evidence/E06-S04/DESIGN_CONSULTATION.md`) and ADR-0040 replace the mechanism: the ledger
is a SQLite table read, decided and appended in one transaction.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - install, refresh and lift read, decide and insert at most one row in one transaction | `cancellai-store::containment_state::transact` (`BEGIN IMMEDIATE`, read rows, caller's `Decision`, one `INSERT`, commit); `cancellai-cli::containment::install_text`/`decide_install` and `cmd_lift` decide inside it, `cmd_refresh` goes through `install_text`. `the_decision_sees_the_history_it_appends_to`, `concurrent_decisions_on_one_history_append_exactly_one_event` (16 threads) | PASS |
| AC2 - a refusal, an already-current notice or a lost race inserts no row | `a_kept_decision_inserts_nothing`; CLI `every_refused_notice_leaves_the_history_byte_identical` and `every_unusable_feed_leaves_the_history_byte_identical` now compare the event rows; `concurrent_refreshes_of_one_notice_...` (16 processes) asserts one row, `concurrent_installs_of_different_notices_...` (8 processes) one row per accepted install | PASS |
| AC3 - an interrupted transaction leaves no part of the event | Store: `an_uncommitted_event_leaves_no_trace` (panic inside `before_commit`). CLI: `an_install_killed_before_commit_leaves_no_partial_event` kills `containment install` at the new `containment-before-commit` kill point, after its insert and before its commit; the rows are unchanged, `list` works, the retry succeeds. The `kill-harness` CI job runs it on macOS, Linux and Windows | PASS |
| AC4 - an existing ledger that cannot be opened, read or replayed caps authority and is not written | `a_corrupt_or_foreign_database_is_unreadable_and_refuses_writes` (bytes unchanged), `an_event_of_unknown_kind_makes_the_ledger_unreadable`, `an_oversized_ledger_is_unreadable_not_truncated`; CLI `an_unreadable_or_unverifiable_history_caps_everything_at_recommend` over a truncated database, garbage, an unsigned install row and an unknown-kind row | PASS |

Mutation checks, each killed: committing before `before_commit` (the CLI kill test and the store
uncommitted test fail); a deferred instead of an immediate transaction (the 16-thread test fails).

## Safety Evidence

- **SI-022:** the table holds opaque text; every row is re-verified through
  `ContainmentLedger::replay` on every load and inside every decision. SQLite decides nothing.
- **SI-029:** nothing inserted by a refusal; expiry, replay and rollback refusals are decided on
  the same rows the append would continue.
- **SI-019:** no file is removed by this code. SQLite's rollback journal is created and removed
  by SQLite itself, as it is for the existing `cancellai-store` ledger.

## Round 10 repair (Codex; refused by the harness, `project/evidence/E06-S14/ROUND10_FINDINGS.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| `transact` ran `CREATE TABLE IF NOT EXISTS` before deciding, so an existing database without an `events` table - unreadable to `load_events` - was silently initialized by the next install | The table is created only in a ledger that is no database yet (a zero-byte file: the one `transact` itself creates, or one a crash left), inside the `IMMEDIATE` transaction; a first decision that keeps the history commits only the table. Any existing database without the table is refused and left byte-identical. `load_events` treats a zero-byte file as missing, consistently | Codex's `an_existing_database_without_events_must_not_be_initialized_on_install` (kept, now also asserting the bytes are unchanged), `a_zero_byte_ledger_is_no_database_and_starts_empty`; the 16-thread and 16-process concurrency tests still pass |

## Verification Commands

```text
cargo test -p cancellai-store containment                                   -> 11 passed
CANCELLAI_CHANNEL=stable cargo test -p cancellai-cli --features kill-points,test-curl --test containment -> 16 passed
```

## Residual risks

- **Same-user tampering** (ADR-0039) is unchanged: a process running as the owner can delete or
  edit the database, as it could the JSONL file.
- **A fresh installation's first refusal creates an empty ledger file.** E06-S07's criterion that a
  refused install leaves "the persisted ledger byte-identical" holds for every existing ledger - a
  refusal writes nothing - but where no ledger existed, the table is now created before deciding.
- **A reader during the instant between file creation and table creation** sees an unreadable
  ledger and caps authority at `Recommend` until any containment command completes: fail closed.
- **Durability rests on SQLite's default rollback journal with `synchronous=FULL`** and on the
  filesystem honouring `fsync`.

## Verifier verdict

pending

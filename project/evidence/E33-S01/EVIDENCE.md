# Evidence Packet - E33-S01

- Commit/PR: the E33-S01 commit on `main`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR4
- Spec version/commit: `project/epics/E33.json` at this commit; owner decisions 2026-09-23 (system curl; repository file)

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - a reachable feed is ingested through the same verification and persistence path as install | `cmd_refresh` fetches, then calls `install_text`, the function `cmd_install` now also uses. `refresh_installs_a_published_notice_once_and_is_current_after`: installed once, the second refresh reports "already current" and leaves the history byte-identical, `list` shows the incident. | PASS |
| AC2 - unreachable, oversized, malformed, replayed, rolled-back or untrusted feed: history byte-identical, authority from local state | `every_unusable_feed_leaves_the_history_byte_identical`: curl failure, HTTP 500, a body over 256 KiB (refused before parsing, "larger than" asserted), malformed, a rolled-back sequence, an untrusted signer - each exit 4 with the history compared byte for byte; HTTP 404 is "nothing published", exit 0, unchanged. `a_missing_curl_is_unavailability_not_a_notice`; `a_feed_override_that_is_not_https_is_refused`. | PASS |
| AC3 - never lift, narrow or loosen a containment from feed content | The feed reaches the ledger only through `ContainmentLedger::install`, whose notice vocabulary has no lift, and records are append-only (E17-S07). | PASS |

## Round-6 repairs (Codex, `project/evidence/E06-VERIFIER-REVIEW-ROUND6.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| A notice already present in an unverifiable history made refresh answer "already current", exit 0 | The history is replayed and verified before "already current" is ever said; an unusable ledger is refused (exit 4) | `an_unverifiable_history_is_never_already_current` |
| A `curl` supplied through `PATH` was executed | Only the system curl at its fixed location runs (`/usr/bin/curl` or `/bin/curl`; `C:\Windows\System32\curl.exe`); the fake-curl test hook exists only in `--features test-curl` builds | `a_curl_on_path_is_never_executed` (runs in every build); the fake-curl tests run in `rust.yml`'s stable-channel job with the feature |

## Round-7 repair

| Finding | Repair | Evidence |
| --- | --- | --- |
| After sequences 1 and 2, a feed serving the older sequence 1 was "already current" | Refresh compares the feed's sequence with the ledger's last verified sequence from that publisher: older is refused as a rollback (exit 4, history unchanged); only the newest installed notice is current | `a_feed_serving_an_older_installed_notice_is_refused_as_a_rollback` |

## Round-8 redesign (owner decision 2026-09-24)

| Finding | Repair | Evidence |
| --- | --- | --- |
| Sixteen concurrent refreshes of one notice appended duplicate sequences and broke replay | Optimistic concurrency without a lock file (removing one would be a mutation outside the boundary): every history line names the chain digest of the history it was decided on (`prev`) and a nonce; readers accept only lines that continue the chain, so of concurrent appends exactly one takes effect; the appender re-reads, and a loser is told to retry - or, for a refresh whose identical notice the winner installed, told it is current. The decision and the append use one snapshot of the history, so a decision made on a stale history is never applied. | `concurrent_refreshes_of_one_notice_leave_one_accepted_install_and_a_replayable_history` (16 processes, stress-run 10 times), `concurrent_installs_of_different_notices_never_break_the_history` (8 processes), `a_line_that_lost_a_concurrent_append_is_skipped_and_its_nonce_is_not_accepted` (store) |

## Round-9 repair (Codex, `project/evidence/E06-VERIFIER-REVIEW-ROUND9.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| A losing append wrote its line before learning it had lost: sixteen concurrent refreshes left two physical lines for one accepted notice, a refresh that reported `already current` had changed the ledger's bytes, and stale lines consumed the 16 MiB history budget | `append_event` is a compare-and-append: it takes an exclusive SQLite transaction on `containment_lock.sqlite3` in the state root (created once, never written or removed; released by the OS if the holder dies), re-reads the chain head, and writes only if it is still the head the caller decided on; otherwise it returns `None` and writes nothing. Waiting is bounded (30 s); a timeout is an error with nothing written | `a_stale_append_writes_nothing` (byte identity), `concurrent_appends_decided_on_one_history_write_exactly_one_line` (16 threads, one physical line; fails 2 of 3 runs with the lock removed); the 16-process `concurrent_refreshes_of_one_notice_...` and 8-process `concurrent_installs_...` CLI tests now also assert the physical line count |

The chain digest and nonce stay as a second barrier: a line that does not continue the chain is
still skipped on load.

## Superseded by E33-S03 (ADR-0040, 2026-09-24)

The round-9 repair above - a lock database around the JSONL append - is replaced: the history is
now a SQLite table read, decided and appended in one transaction (E33-S03), and `refresh`'s
rollback and "already current" checks moved inside that transaction (`decide_install`). This
story's second criterion now reads "the ledger's ordered event rows and their raw signed payloads
shall stay identical", which the refusal tests assert.

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-022 | Transport-supplied content trusted | Transport authenticity is not relied on; every notice is signature-verified | PASS |
| SI-029 | A feed lifting or weakening containment | Refusal cases above; install path append-only | PASS |
| Byte cap | A server with no Content-Length | Enforced in Rust by reading at most 256 KiB + 1 from curl's stdout, not only `--max-filesize` | PASS |

## Verification Commands

```text
cargo test -p cancellai-cli --test containment                       -> 11 passed (nightly and stable builds)
cargo clippy --workspace (host + x86_64-pc-windows-gnu) -D warnings  -> clean
```

## Residual risks

- **The fake-curl tests are Unix-only**; on Windows `refresh` is exercised only by its override
  refusal test. The real network path is not tested in CI (no network dependence in tests).
- **`curl` is resolved from `PATH`**: a hostile `curl` can only return bytes, which are signature-verified.
- **No scheduled refresh** until the Guardian loop exists (E33-S02).
- **A crash in the middle of the one `write`** can still leave a torn last line, which makes the
  history unreadable - fail closed, as before; the lock does not change that.

## Verifier verdict

pending

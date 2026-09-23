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

## Verifier verdict

pending

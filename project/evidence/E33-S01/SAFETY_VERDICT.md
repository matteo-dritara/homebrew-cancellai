# Safety Verdict - E33-S01

Verifier: Codex
Brief-Checksum: bb614fb4de0d31b814e7497488c4e83eac0c298b389fc899b433a3d92b1854a9
Date: 2026-09-24
Risk: CR4
Reviewed tree: `2531746`

## Verdict and safety surface

`containment refresh` adds a network-facing path into the persisted incident ledger. It must accept only a valid newer signed notice, and failure must leave local authority computed from a usable local ledger. This first independent pass is **FAIL**. Details and eleven-axis coverage are in `project/evidence/E06-VERIFIER-REVIEW-ROUND6.md`.

| Invariant | Independent evidence | Result |
| --- | --- | --- |
| SI-022, signed knowledge is data | Normal fetched text calls the same `install_text` verification as file install; oversized/malformed/untrusted inputs in the stable suite did not append. A PATH-supplied executable named `curl` ran instead of the system curl, bypassing the claimed executable/protocol selection. | FAIL |
| SI-029, tamper/rollback fails closed | With a parseable log event containing an invalid signed notice, a matching invalid feed body made refresh exit 0 `already current`; `containment list` refused the same log as unverifiable. The raw-event shortcut skips replay and trust checks. | FAIL |
| SI-030, release channel bounds deletion | The stable-channel CLI suite passed, including containment and kill-points tests; the local default-channel build remains bounded by its compiled channel. No direct authority increase from fetched content was reproduced. | PASS within tested scope |
| Feed acceptance criteria | Valid newer notices use `install_text`; a 256 KiB plus 10 byte response, HTTP override, missing curl, 500/404, malformed, rolled-back and untrusted bodies were exercised. The corrupted-history fast path and PATH hijack are missing cases. | FAIL |

### Adversarial cases

- Wrote `{"install":"{\"not\":\"a signed notice\"}"}` to a synthetic `containment_log.jsonl`; a fake curl returned that body with status 200. Refresh said `already current` and exited 0; `containment list` exited 4 because event 1 failed verification. History bytes were unchanged but the success report was false.
- Put a temporary executable called `curl` first in PATH. Refresh ran it and accepted its 404 output. The fake program could perform arbitrary work or fetch non-HTTPS content despite the arguments meant for genuine curl.
- The stable-channel integration suite exercised valid installation once, repeated current response, server and curl errors, malformed/untrusted/replayed/rolled-back notices, a >256 KiB response, absent curl, and an HTTP override.
- Source inspection confirmed genuine curl is passed `--proto =https`, `--proto-redir =https`, `--max-time 30`, and the body read uses `MAX_NOTICE_BYTES + 1` before parsing. No real external feed was contacted by this verifier.

### Gates

| Gate | Result |
| --- | --- |
| Rust fmt, workspace Clippy, Windows-target Clippy, workspace tests, stable-channel CLI with kill-points | PASS locally |
| Python pytest | PASS: 705 passed, 3 skipped |
| `release.py check`, `project_os.py check`, `check_process.py check` | PASS at baseline |
| `verifier_handoff.py check` | FAIL at baseline due to E06-S04's older verdict missing the newly rendered brief checksum; post-write check is recorded in the round review |
| Native Linux/Windows feed reproduction | NOT RUN: no native hosts in this verifier workspace |
| Latest main CI | PASS: governance, tests, rust, and CodeQL for `2531746`; the counterexamples remain untested by those workflows |

### Required repair and residuals

Replay and verify the local history/trust before reporting `already current`, and require the exact notice to be the verified current publisher sequence. Resolve a trusted system curl executable independent of untrusted PATH, and test both counterexamples. The owner-accepted same-user ability to edit the ledger remains outside this finding; the first counterexample is an incorrect success report for a state the kernel itself calls unknown. Concurrent refresh decision/append behavior deserves a targeted regression because the current path does not serialize it.

### Owner decision

PENDING

FAIL

## Round 7

Verifier: Codex
Brief-Checksum: bb614fb4de0d31b814e7497488c4e83eac0c298b389fc899b433a3d92b1854a9

Second and final owner-authorized independent pass over repair commit `23bb14e`. The full reproduction and eleven-axis audit are in `project/evidence/E06-VERIFIER-REVIEW-ROUND7.md`.

| Invariant / obligation | Adversarial evidence | Result |
| --- | --- | --- |
| SI-022, verified notice only | The fixed system-curl path and HTTPS-only arguments remain; PATH hijack test passes, and `test-curl` injection is feature-gated. Valid new notices still use `install_text`. | PASS within tested scope |
| SI-029, rollback/tamper fails closed | A valid older notice (publisher sequence 1) already in history, followed by valid sequence 2, returned exit 0 `already current` when sequence 1 was served again. History bytes stayed unchanged, but a rolled-back feed was falsely accepted. | FAIL |
| SI-030, channel bounds deletion | Stable-channel integration suite passed with `kill-points,test-curl`; no authority lift from fetched content was reproduced. | PASS within tested scope |
| Feed refusal contract | Corrupt history now refuses before the `already current` path; HTTP override, malformed/untrusted content, oversized body and curl failures refuse in the integration suite. The exact old signed text remains a rollback gap. | FAIL |

### Adversarial cases

- Added a temporary integration test using a synthetic home and trusted test publisher. Installed signed sequences 1 and 2, served the exact sequence 1 through test curl, and compared history bytes. Refresh said `already current` with exit 0 rather than safety refusal. The test failed as expected and was removed; the tracked source is unchanged.
- Rechecked the round 6 malformed-history regression and PATH-hijack test through the stable-channel suite; both pass.
- Source inspected the 256 KiB plus one read, fixed curl executable candidates, HTTPS initial/redirect options, and the raw `events.contains` shortcut.

### Gates

| Gate | Result |
| --- | --- |
| Rust fmt, workspace Clippy, Windows-target Clippy, workspace tests, stable-channel CLI with `kill-points,test-curl` | PASS locally; temporary old-sequence test failed as expected |
| Python pytest | PASS: 714 passed, 3 skipped |
| `release.py check`, `project_os.py check`, `verifier_handoff.py check`, `check_process.py check` | PASS at baseline |
| Native Linux/Windows refresh | NOT RUN: no native host here |
| Latest main CI | All four workflows PASS for both `23bb14e` and `bfce0cd`; reproduced counterexamples are outside those gates |

### Owner decision

PENDING

FAIL

## Round 8

Verifier: Codex
Brief-Checksum: bb614fb4de0d31b814e7497488c4e83eac0c298b389fc899b433a3d92b1854a9

Owner-authorized independent pass over `351295d`. Full findings and eleven-axis audit: `project/evidence/E06-VERIFIER-REVIEW-ROUND8.md`.

| Invariant / obligation | Adversarial evidence | Result |
| --- | --- | --- |
| SI-022, verified notice only | A separate temporary integration test installed valid signed sequences 11 and 12. Exact old 11 refused; exact current 12 said `already current`; corrupt trust refused. Production curl remains fixed-path with HTTPS-only switches. | PASS within tested scope |
| SI-029, rollback and available local ledger | Sixteen concurrent refreshes of the same valid sequence 1 wrote two install events. Replay refused event 2, `containment list` exited 4, and subsequent refreshes refused the unusable history. Authority fails closed, but a valid feed action can persistently corrupt the ledger. | FAIL |
| SI-030, channel bounds deletion | Stable-channel suite passed with `kill-points,test-curl`; no fetched notice raised local destructive authority. | PASS within tested scope |
| Feed refusal contract | HTTP override, malformed/untrusted notice, oversized body, failed curl, corrupt trust and exact old sequence refuse in tests. Concurrent identical valid input remains unsafe for ledger availability. | FAIL |

### Adversarial cases

- Independently installed signed publisher sequences 11 and 12 in a synthetic home; fake curl served exact 11. Refresh exited 4, called it older, and kept history byte-identical. Serving 12 returned `already current`. Corrupting the owner trust file then made refresh exit 4 with unchanged history.
- Independently launched sixteen refresh processes against one synthetic fake curl serving the same signed sequence 1. The resulting ledger had two install lines; replay and `containment list` exited 4. The temporary regression failed as intended and was removed without changing tracked test code.
- Source and stable-suite inspection confirmed `/usr/bin/curl` or `/bin/curl` on Unix, a fixed Windows system path, `--proto =https`, `--proto-redir =https`, and a `MAX_NOTICE_BYTES + 1` read. PATH hijack, HTTP override and over-cap cases passed the stable suite.

### Gates

| Gate | Result |
| --- | --- |
| Rust fmt, workspace Clippy, Windows-target Clippy, workspace tests, stable-channel CLI with `kill-points,test-curl` | PASS locally; separate race test FAILED as expected |
| Python pytest | PASS: 719 passed, 3 skipped |
| `release.py check`, `project_os.py check`, `verifier_handoff.py check`, `check_process.py check` | PASS at baseline |
| Native Linux/Windows refresh | NOT RUN: macOS verifier host only |
| Post-evidence governance gates | Recorded in the round 8 epic review |

### Owner decision

PENDING

FAIL

## Round 9 — 2026-09-24

Verifier: Codex
Brief-Checksum: bb614fb4de0d31b814e7497488c4e83eac0c298b389fc899b433a3d92b1854a9
Review target: `351295d..7e35a2a`

| Invariant / obligation | Independent evidence | Result |
| --- | --- | --- |
| SI-022, SI-030 | Signed notices still enter through the shared install verification path; the fixed system curl, HTTPS protocol restrictions, byte cap and compiled release-channel authority were inspected, and the stable-channel containment suite passed. | PASS within local scope |
| SI-029, byte-identical refusal/current state | Sixteen concurrent refresh processes produced two physical log lines for one signed notice. One containment was accepted and replayable, but a process told `already current` left an extra line. A deterministic stale-head append also changed raw bytes. | FAIL |
| Crash/retry and platform coverage | A lost-race line consumes the 16 MiB cap even when skipped. No native Linux/Windows feed runtime or crash injection for the new append protocol was observed. | INCOMPLETE |

### Adversarial case and exact repair

A temporary raw-line-count assertion in the existing 16-process integration test failed `left: 2, right: 1`; the original replay assertions passed. A temporary store test comparing bytes before and after a stale second append also failed. Both temporary edits were removed. Serialize the replay/current decision with the durable append, or implement equivalent compare-and-append that writes no losing line, then test physical bytes, line count, replay and crash/retry. Full reproduction and gates: `project/evidence/E06-VERIFIER-REVIEW-ROUND9.md`.

### Known residuals, rollback and owner decision

Unknown/corrupt history caps authority at Recommend, but a valid concurrent refresh should not create ignored physical history. No release or user-data mutation occurred. Owner decision: PENDING. This round rejects the story and accepts no new residual.

FAIL

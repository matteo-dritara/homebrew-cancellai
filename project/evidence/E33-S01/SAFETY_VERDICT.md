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

# E00 Independent Verifier Review

- Baseline: `4b2df0130e62d83e3a10caaae73daa456211f92d`
- Review target: uncommitted working tree
- Date: 2026-08-27
- Verifier: Codex

## Verdicts

| Story | Verdict | Evidence |
| --- | --- | --- |
| E00-S01 | FAIL | Protected symlink-name barrier is bypassed and the link is unlinked. |
| E00-S02 | FAIL | Common `auth.json` + `sessions/` names grant a generic custom directory high destructive authority. |
| E00-S03 | PASS | All aggressive paths retain the cutoff check; dry-run and clean share mutation-plan selection. |
| E00-S04 | FAIL | A root fingerprint that changes between planning and execution raises uncaught `SafetyError` from `cmd_clean`, rather than documented exit 4/JSON. |
| E00-S05 | FAIL | Multiple observation paths swallow errors without a `Scan`; status can silently report zero/empty totals. |
| E00-S06 | FAIL | Stream rewrite normalizes retained CRLF bytes to LF, violating malformed-line preservation. |
| E00-S08 | FAIL | `history.jsonl` is labelled `cleanable` although no rule selects a standalone history file. |

## Concrete reproductions

1. Run the three failing independent tests in `IndependentVerifierAdversarialTests`.
2. Patch `fingerprint_root` to return high authority during `build_plan` and unknown authority at `execute_plan`; `main(["clean", ...])` raises `SafetyError` from `cmd_clean` instead of producing the documented blocked result.
3. `coverage_state("history.jsonl", "claude") == "cleanable"`, while a Claude-only `build_plan` with only that file produces no matching action.
4. Inspect unrecorded `OSError` paths: `read_codex_parent_session_id` (line 559), `count_claude_history_matches` (line 925), and `root_entry_sizes` (lines 1370-1378).

## Gate results

- `python3 -m pip install -r requirements-dev.txt`: system Python refused the externally managed environment; pinned tools were installed in `/private/tmp/aiclean-verifier-venv` after approval.
- Full pytest: 62 passed, 3 verifier adversarial tests failed.
- Ruff check / format: pass.
- Mypy: pass.
- `scripts/gen_docs.py --check`, project governance, docs, and workflow checks: pass.

## Out-of-story findings, prioritised

1. **P1 implementation bug:** the `ready_for_review` evidence check tests only for an arbitrary Markdown filename. A blank or unrelated `E00-*.md` batch file satisfies every story handoff, so the evidence gate is vacuous. `done` does not use the batch fallback, so it is not a direct closure loophole, but handoff evidence is not validated by story, required fields, or content.
2. **P2 spec gap:** E00-S08 is a well-formed CR1 observation story and belongs in E00 because it exposes provider-layout uncertainty, but its `cleanable` definition needs an explicit mode/condition rule. It currently conflicts with the requested honesty check for conditional history trimming.
3. **P1 architecture decision:** robust custom-root authentication cannot safely rest on plausible provider filenames. Decide whether destructive custom roots require provider-native identity, a user-confirmed capability token, or are disabled in Python v1.

## Documentation review

README, CHANGELOG, and AS_IS overclaim the failed protections (symlink barrier, credible custom-root protection, complete error reporting, and byte-preserving history rewrite). The exit-code table also omits the execution-time SafetyError path. These claims must be corrected with the implementation.

## Test isolation / performance

All tests use temporary synthetic roots; no real provider home path, transcript, prompt, or secret was found. Status still performs repeated walks (plan discovery plus `root_entry_sizes`, and `latest_mtime`/`directory_size` pairs), consistent with the documented deferred single-pass inventory risk.

# Executor Evidence - E00 partial remediation (2026-08-27) (round 1)

> **Superseded by round 2.** Independent review on 2026-08-27 rejected six of the seven
> stories recorded here. This file is kept as the round-1 record; the current state is in
> [`E00-VERIFIER-REVIEW.md`](E00-VERIFIER-REVIEW.md) and
> [`E00-EXECUTOR-ROUND2.md`](E00-EXECUTOR-ROUND2.md).

- Executor: Claude
- Independent verifier: **not yet performed**
- Stories covered: E00-S01 (CR4), E00-S02 (CR4), E00-S03 (CR3), E00-S04 (CR3), E00-S05 (CR4), E00-S06 (CR3), E00-S08 (CR1)
- Stories still open: E00-S07 (CR2), which closes the epic once review passes

## Outcome

PASS for the seven implemented stories, all moved to `ready_for_review`. None is eligible to be marked `done`: the three CR4 stories (E00-S01, E00-S02, E00-S05) each require an owner-visible Safety Verdict from the independent reviewer per `docs/development/RELEASE_GATES.md`, and `ready_for_review` is by definition the executor's exit state.

## What changed

| Story | Change |
| --- | --- |
| E00-S01 | `protected_component()` enforces the protected-name lists at plan assembly and again inside `safe_remove()`, which now takes the applicable set as a required argument. |
| E00-S03 | `discover_claude_aux()` applies the retention cutoff to Claude legacy roots and rebuildable cache files. |
| E00-S04 | `normalize_argv()` resolves a leading flag to `status` and leaves an unknown verb to argparse. `cmd_clean()` returns the documented exit taxonomy; `CleanResult` carries `blocked_tools`/`deferred`. |
| E00-S02 | `fingerprint_root()` scores a root against structural provider markers and returns a `RootAuthority` with `default`/`high`/`low`/`unknown` confidence. Destructive planning and execution require `default` or `high`; inspection is unaffected. |
| E00-S05 | A `Scan` completeness channel is threaded through every filesystem observation helper and every discovery function. `Plan` exposes `scan_complete` / `incomplete_scopes` / `scan_errors`. An incomplete scope withholds destructive authority for that whole tool. |
| E00-S06 | `trim_claude_history()` streams, re-identifies the source before `os.replace`, and returns a status. `execute_plan()` never rewrites `history.jsonl` while a Claude process is live. |
| E00-S08 | `coverage_state()` / `coverage_report()` classify top-level provider entries; `status --coverage` and `status --json` expose them. No discovery path reads them. |

## Verification commands

```text
python3 -m pytest tests -v
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
```

All gates pass locally. The Python test suite grew from 18 to 47 tests; the new `TrustFloorTests` class holds the P0 regression cases.

## Safety evidence

| Invariant | Counterexample tested | Result |
| --- | --- | --- |
| SI-001 / SI-006 | Protected action injected directly into `execute_plan`; nested protected paths for both tools; a scanner patched to emit a protected root | refused, PASS |
| SI-005 / SI-012 | Fresh legacy directory and cache file under `--aggressive`; cutoff boundary at -1s/+1s | not selected, PASS |
| SI-007 | Every common flag without a subcommand; unrecognized verb | resolves to `status` / usage error, PASS |
| SI-014 | Blocked run via patched `active_processes`, text and `--json` | exit 4 and `blocked_tools`, PASS |
| SI-011 / SI-015 | Live Claude process under `--allow-running`; concurrent append injected between copy and replace | trim skipped / rewrite abandoned, no temp file left, PASS |
| SI-004 / SI-009 / SI-010 | Unclassified provider directory under `--aggressive` | reported as unknown, no action emitted, PASS |
| SI-002 / SI-004 | Ordinary project directory containing `tmp/` and `log/` as `CODEX_HOME`; single-marker root; empty default root; plan hand-built around the gate | refused with exit 4, default root still accepted, PASS |
| SI-008 / SI-009 / SI-010 | Directory chmod 000 inside a scanned scope; plan hand-built with an incomplete `Scan`; vanished path; 75 recorded errors | authority withheld, execution refused, race not counted, error list bounded at 50, PASS |

## Incidental repairs

`scripts/project_os.py` as delivered failed the repository's own lint and type gates (3x RUF005, 1 unformatted region, 1 mypy `no-redef`). Fixed in the same work item; the `lint` CI job would otherwise have been red on first push.

## Residual risks

- Scan completeness is per tool, not per scope: one unreadable path anywhere under `~/.codex` withholds all Codex cleanup for that run. Measured on a real machine (3,423 directories under `~/.codex`, 353 under `~/.claude`) this produced zero errors, so the practical cost is low, but the granularity is coarse and belongs to the Rust inventory engine (E04).
- Root fingerprinting is structural, not cryptographic. A directory deliberately populated with `auth.json` and `sessions/` will be accepted as a Codex root. It defends against misconfiguration, not against an attacker who already controls the filesystem.
- The protected-name barrier is component-name based. It is deliberately over-inclusive: a candidate whose path contains a protected component anywhere is refused. Failure direction is non-destructive.
- Exit codes 3 and 4 are a breaking change for any existing automation that treated non-zero as failure only.
- Coverage classification is a static vocabulary in this build. It reports drift; it does not resolve it.

## Verifier verdict

PENDING - no independent review has been performed. Codex is the standing reviewer; start from:

```sh
python3 scripts/project_os.py review
python3 scripts/project_os.py brief E00-S01 --role verifier
```

The three CR4 stories additionally require a Safety Verdict before they can move to `done`.

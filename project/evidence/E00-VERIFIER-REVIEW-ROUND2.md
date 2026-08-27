# E00 Independent Verifier Review - Round 2

- Review target: PR #3 / `HEAD`
- Verifier: Codex
- Date: 2026-08-27

## Verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E00-S01 | FAIL | Case-variant `Plugins` does not trigger the protected barrier; unsafe on case-insensitive APFS. |
| E00-S02 | FAIL | Valid JSON/config/rollout lookalike gains authority with `--allow-custom-root`. |
| E00-S04 | FAIL | `cmd_clean()` catches only `SafetyError`; an execution `OSError` escapes. |
| E00-S05 | FAIL | `Path.exists()` discovery pre-checks collapse access failures before `Scan.record`. |
| E00-S06 | FAIL | History trimming follows and replaces a `history.jsonl` symlink. |
| E00-S08 | FAIL | `projects` is labelled cleanable even when its contents cannot be selected. |
| E00-S09 | FAIL | Any parseable unrelated `ps` line marks the observation complete. |

## New adversarial tests

`RoundTwoIndependentVerifierTests` contains six failing tests: APFS-case protection, validated lookalike custom root, absent-self `ps`, history symlink rewrite, uncaught execution `OSError`, and memory-only project coverage.

## Cross-cutting

- Dry-run and clean share `build_plan`, but that does not repair unrecorded scan errors or unsafe authority.
- The adapted generic-root test is stricter only for invalid marker content; it misses valid lookalikes. New process tests do not prove completeness when unrelated output parses.
- Temporary roots and patched `default_home` keep tests away from real provider data.

## Out-of-story findings, prioritised

1. **P1 implementation bug:** `.gitattributes` sets `* text=auto eol=lf` while claiming CRLF history fixtures will not be rewritten. Runtime byte tests survive, but future committed text fixtures can be normalized.
2. **P1 architecture decision:** ADR-0012 correctly rejects filename heuristics but its adopted structure-plus-intent scheme is not positive provider identity and conflicts with SI-002 for acknowledged lookalikes. Decide between disabling custom-root mutation in Python v1 and provider-native verification.
3. **P2 implementation bug:** the evidence gate accepts 400 bytes of filler containing a story ID; this is only weak anti-vacuity, not substantive evidence.
4. **P2 spec gap:** coverage needs a conditional state for `projects`/`sessions`, whose contents, age, and policy decide cleanability.

## Gate status

The full suite fails on the six new verifier counterexamples. Ruff, formatting, type checks, generated docs, governance, documentation, and workflow checks were rerun separately.

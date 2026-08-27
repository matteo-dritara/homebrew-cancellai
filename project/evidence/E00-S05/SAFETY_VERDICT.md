# Safety Verdict - E00-S05

- Risk: CR4
- Independent verifier: Codex
- Date: 2026-08-27, round 2

## Verdict

`FAIL`

| Invariant | Evidence | Result |
| --- | --- | --- |
| SI-008 | Discovery can treat an unreadable root as absent through `Path.exists()` before a scan records it. | FAIL |
| SI-009 | The empty result can leave a plan looking complete. | FAIL |
| SI-010 | Several discovery pre-checks retain this error-collapse path. | FAIL |

The new tests cover selected helper exceptions, but `discover_codex_sessions`, `discover_claude_sessions`, `discover_aged_top_entries`, `count_claude_history_matches`, and `directory_size` still have unrecorded `Path.exists()` pre-checks.

## Owner decision

`REJECT`

# Safety Verdict - E00-S05

- Change: scan-completeness authority gate
- Risk: CR4
- Commit/PR: working tree against `4b2df0130e62d83e3a10caaae73daa456211f92d`
- Independent verifier: Codex
- Date: 2026-08-27

## Verdict

`FAIL`

## Safety surface changed

Filesystem observation errors now affect destructive authority and status diagnostics.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 | Partial inventory cannot authorize deletion | Some observations do not receive a `Scan`, so a plan can remain complete after an observation failure. | FAIL |
| SI-009 | Missing evidence is non-destructive | `read_codex_parent_session_id()` catches `OSError` and returns `None`, changing lineage evidence without marking the scan partial. | FAIL |
| SI-010 | Scan errors are visible | `root_entry_sizes()` calls `safe_lstat_size()` without a scan and returns zero/empty data silently; `count_claude_history_matches()` also swallows `OSError`. | FAIL |

## Adversarial cases

- Source audit of every `except OSError` and `suppress(OSError)` found the unrecorded paths above. Existing chmod fixtures are not sufficient proof because they do not cover these helpers or all OS permission semantics.

## Differential / compatibility evidence

- Existing partial-scan tests pass, but only exercise `os.walk` paths that are explicitly passed a scan object.

## Known residual risks

- A status total may be silently wrong, and missing Codex lineage evidence may be treated as an independent session rather than incomplete safety evidence.

## Rollback / recovery

- Keep the story open; route every observation error through its scope and make status expose partial totals.

## Owner decision

`REJECT`

Owner note: incomplete-observation semantics remain incomplete.

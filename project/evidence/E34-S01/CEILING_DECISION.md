# E34-S01 - Review ceiling decision

- Story: E34-S01 (CR2)
- Date: 2026-09-24
- Decided by: project owner, by standing instruction - at most two independent review rounds per
  story; recorded by the executor (Claude), who does not thereby become a verifier.
- Related: ADR-0025, `E34-VERIFIER-REVIEW-ROUND1.md`, `E34-VERIFIER-REVIEW-ROUND2.md`.

## What the rounds found

| Round | Verifier | Verdict | Finding |
| --- | --- | --- | --- |
| 1 | Codex | FAIL | A staged rename reported only its destination, so a deleted production source escaped the tier check; import overwrote a record created in the main tree after the run was checked. |
| 2 | Codex | FAIL | A record replaced after the check, path list unchanged, was imported: import re-ran neither the record check nor the model attribution. Required repair: re-run the record and attribution checks on the exact bytes to import, bind the run metadata to the checked output, refuse any post-check change. |

## The other solution taken

Both repairs were made to their prescription and every counterexample is pinned in `tests/test_review_round.py`:

| Counterexample | Regression |
| --- | --- |
| Round 1: staged rename out of `scripts/` | `test_a_rename_reports_its_source_and_the_source_is_refused` |
| Round 1: record appeared in the target after the check | `test_a_record_that_appeared_since_the_base_is_never_overwritten`, `test_a_target_changed_since_the_base_is_refused` |
| Round 2: record edited after the check, path list unchanged | `test_a_record_edited_after_the_check_is_refused` |
| Round 2: forged record with a run file rewritten to match | `test_a_forged_record_fails_even_when_the_run_file_is_rewritten_to_match` |
| Round 2: attribution not re-derived at import | `test_an_opencode_run_is_reattributed_from_its_log_at_import` |

Each repair was mutation-checked: reverting it fails its regression.

The story is CR2: it closes on this decision plus a forked self-review
(`E34-SELF-REVIEW.md`), not a third independent round. The independent reviewer may still reopen it
if a later round over E34 or a story that uses the harness finds the repair unsound.

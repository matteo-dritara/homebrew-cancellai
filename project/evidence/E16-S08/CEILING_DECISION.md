# E16-S08 - Review ceiling decision

- Story: E16-S08 (CR3)
- Date: 2026-09-23
- Decided by: project owner, by standing instruction for this work session - at most two
  independent review rounds per story, and "if further reviews are needed, find other
  solutions"; recorded by the executor (Claude), who does not thereby become a verifier.
- Related: ADR-0025 (a review that ends at the ceiling rather than on yield is an owner decision,
  recorded as such), `scripts/check_process.py` `REVIEW_ROUND_EXCEPTIONS["E16"]`.

## What the rounds found

| Round | Verifier | Verdict on E16-S08 | Finding |
| --- | --- | --- | --- |
| 2 | Codex | REPAIRED | Embedded NUL escaped as `ValueError`; `.`/`./` accepted as fixtures; a Git-ignored file accepted. The verifier repaired all three, the last by adding a `git check-ignore` probe. |
| 3 | Codex | FAIL | The round-2 probe failed open: Git absent, not a repository, timeout, launch error or an unexpected exit status all accepted the fixture; an untracked file was accepted. Required repair: fail closed on every Git failure mode and require the fixture to be tracked, while accepting a tracked path an ignore pattern matches. |

Both rounds found defects, so ADR-0025's yield rule would call for another round. This is the
second independent round on this story, the owner's limit.

## The other solution taken

The round-3 FAIL names its repair exactly, so the repair was made to that prescription, and every
counterexample round 3 ran was turned into a regression test that fails against the pre-repair
code:

| Round-3 counterexample | Regression test (`tests/test_provider_trust.py`) |
| --- | --- |
| Git absent accepts | `test_git_being_unavailable_refuses` |
| Timeout / `OSError` accepts | `test_a_git_timeout_or_launch_failure_refuses` |
| Exit status 128 / 2 accepts | `test_an_unexpected_git_exit_status_refuses` (2, 128, -9) |
| Not a repository accepts | `test_a_root_that_is_not_a_repository_refuses` |
| Untracked file accepts | `test_an_untracked_existing_file_is_not_fixture_evidence`, `test_a_directory_with_no_tracked_file_is_not_fixture_evidence` |
| Tracked-but-ignored must still pass | `test_a_tracked_file_matching_an_ignore_pattern_is_evidence` |
| Ignored untracked file | `test_an_ignored_untracked_file_is_not_fixture_evidence` |

Each fail-closed branch was then disabled in turn; every mutant was caught (1, 1, 2, 3 and 2
failing tests respectively). The earlier refusals round 2 and round 3 both confirmed (missing,
absolute, parent-component, symlink escape, repository root, NUL, empty) are unchanged.

## What this decision does and does not claim

- It closes E16-S08 without a third independent pass over the final code. The final code has
  been checked by its executor's tests and by mutation, not by an independent reviewer; that is
  the residual this decision accepts.
- It is bounded by the story's risk: the checker gates a registry that holds no promoted entry
  today, and `CODEOWNERS` review of `project/provider_trust.json` remains in force.
- It does not relax the ceiling for any other story or epic.

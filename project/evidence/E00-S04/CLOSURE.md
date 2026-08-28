# Closure Record - E00-S04

- Story: E00-S04 - Remove ambiguous destructive CLI escalation
- Risk: CR3
- Closed by: project owner, on the executor's evidence, without a fourth review round
- Date: 2026-08-28

## Outcome

`PASS_WITH_RESIDUALS`. Independent review examined this story twice: round 1 rejected an
execution-time refusal that escaped as an uncaught exception, round 2 rejected an `OSError`
that still escaped `cmd_clean()`. Both are repaired and the taxonomy is now total.

## What changed

Flags without a subcommand resolve to the read-only `status` view instead of `clean`, and an
unrecognized verb is a usage error. Exit codes are stable and documented: `0` completed,
`1` declined at the prompt, `2` invalid usage or a refused configuration root, `3` mutation
failure, `4` safety blocked, withheld or deferred the work. `CleanResult` carries
`blocked_tools` and `deferred`, and `--json` exposes `exit_code`.

`main()` converts any escaping exception into exit `3`. Leaving one uncaught would surrender
the process to Python's own exit code `1`, which automation cannot distinguish from the
operator declining the confirmation prompt - a crash would read as a safe no-op.

## Verification

- `TrustFloorTests.test_flags_without_a_subcommand_never_normalize_to_clean` - every common
  flag, plus `--version`, plus an unknown verb;
- `test_unknown_verb_is_a_usage_error_not_a_cleanup`;
- `test_exit_code_distinguishes_blocked_from_success_and_usage`;
- `test_blocked_json_run_reports_its_exit_code`;
- `ReviewResponseTests.test_execution_time_root_refusal_becomes_exit_blocked` - the boundary
  fires between planning and execution;
- `RoundTwoIndependentVerifierTests.test_cmd_clean_converts_execution_oserror_to_documented_failure`
  - the reviewer's counterexample, retained;
- `RoundTwoResponseTests.test_unexpected_exception_never_collides_with_the_cancelled_code`.

Documented in `README.md`, `docs/architecture/AS_IS.md` and `CHANGELOG.md`, each of which was
corrected after review found the earlier wording overclaiming.

## Residual risks

- The exit taxonomy is a breaking change for automation that treated any non-zero code as a
  single failure condition.
- `argparse` still owns exit `2` for its own usage errors. That is consistent with the
  documented meaning of `2`, but the code is produced by a component this story does not
  control.
- No independent verifier examined the final state of this story.

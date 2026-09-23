# E06-S08 - Review ceiling decision

- Story: E06-S08 (CR1)
- Date: 2026-09-23
- Decided by: project owner, by standing instruction - at most two independent review rounds per
  story; recorded by the executor (Claude), who does not thereby become a verifier.
- Related: ADR-0025, `scripts/check_process.py` `REVIEW_ROUND_EXCEPTIONS["E06"]`.

## What the rounds found

| Round | Verifier | Verdict | Finding |
| --- | --- | --- | --- |
| 3 | Codex | FAIL | A timed command that exited non-zero was timed as if it had done the work. |
| 4 | Codex | FAIL | After the round-3 repair, a command that exited 0 and printed anything non-blank still counted. Required repair: validate each timed command's result against the generated corpus; add a successful, non-blank, zero-work regression. |

## The other solution taken

The round-4 repair was made to its prescription: `result_matches_corpus` checks every timed run of
both engines against the corpus it was built for (Rust `status` per-provider artifact counts,
`inspect` artifact total, `plan` delete count, dry `clean` delete-candidate count; the reference's
candidate count in each of its outputs). The counterexamples are pinned:

| Counterexample | Regression (`tests/test_cutover_benchmark.py`) |
| --- | --- |
| Round 3: non-zero exit timed | `test_a_timed_command_that_fails_is_refused_not_timed` |
| Round 3: empty output timed | `test_a_timed_command_that_prints_nothing_is_refused_not_timed` |
| Round 4: exit 0, `x` printed, timed | `test_a_successful_non_blank_run_that_did_no_work_is_refused` |
| Each engine's real output shape accepted, a wrong count refused | `test_result_matches_corpus_on_each_engines_real_output_shapes` |

The story is CR1: it closes on this decision plus a forked self-review, not a third independent
round. The independent reviewer may still reopen it if its round on E06-S13 finds the benchmark
gate unsound.

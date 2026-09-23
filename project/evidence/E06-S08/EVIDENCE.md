# Evidence Packet - E06-S08

- Commit/PR: the E06-S08 commit on `main`
- Executor: Claude
- Independent verifier: pending - reviewed with the rest of E06's cutover stories
- Change Risk: CR1
- Spec version/commit: `project/epics/E06.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - wall time and peak RSS of status, inspect, plan and a dry clean, both engines, one corpus | `scripts/cutover_benchmark.py` pairs each Rust command with the reference command answering the same question, on one generated `$HOME`, median of N runs, peak RSS from `os.wait4` of the child alone. Local run (macOS, 2000 sessions per provider, 3 runs): status 0.257s vs 0.498s, inspect 0.273s vs 0.518s, plan 0.273s vs 0.460s, dry clean 0.269s vs 0.453s; Rust peak RSS 9.5-27.7 MiB vs the reference's 31.7 MiB. | PASS |
| AC2 - slower beyond tolerance, or over the RSS ceiling, fails on Linux CI | `budget_violations` (ratio 1.10 plus 0.05s start-up slack; 64 MiB ceiling), pinned by `tests/test_cutover_benchmark.py` at, just inside and just outside each boundary. A wrapper that adds 0.5s to the release binary made `check` fail on all four commands with exit 1. `rust.yml`'s `cutover-budget` job runs `check` on ubuntu-latest. | PASS |
| AC3 - a corpus that resolved no artifacts fails rather than reporting a fast empty run | `prove_corpus_is_live` requires both engines to propose exactly `2 * (sessions - keep_latest)` deletions before anything is timed. `test_a_rust_engine_that_sees_nothing_is_refused_before_anything_is_timed` and `test_the_reference_sees_exactly_the_corpus_it_was_built_for` pin both directions. | PASS |
| AC4 - macOS and Windows reported, not gated | The same job runs `report` on macos-latest and windows-latest; report mode prints violations as notes and exits 0 unless the corpus is not seen. RSS is reported as unmeasured on Windows, never guessed. | PASS (first CI run pending) |

## Round-3 repair (Codex, `project/evidence/E06-VERIFIER-REVIEW-ROUND3.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| A timed command that exits non-zero (or prints nothing) was timed as if it had done the work; a wrapper exiting 7 for status/inspect/dry clean still passed `check` | `failed_run` refuses any timed run that did not exit 0 or printed nothing; `measure` returns the error instead of a timing | `test_a_timed_command_that_fails_is_refused_not_timed` (Codex's exact reproduction: real plan, exit 7 elsewhere), `test_a_timed_command_that_prints_nothing_is_refused_not_timed`, `test_failed_run_accepts_only_a_successful_non_empty_run` |

## Round-4 repair

| Finding | Repair | Evidence |
| --- | --- | --- |
| A run exiting 0 with non-blank output that did no work was still timed | Every timed run of both engines must describe the generated corpus (`result_matches_corpus`) | `test_a_successful_non_blank_run_that_did_no_work_is_refused`, `test_result_matches_corpus_on_each_engines_real_output_shapes`; see `CEILING_DECISION.md` |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| Tests never touch real provider data | A child inheriting the real `HOME`/`CLAUDE_CONFIG_DIR`/`CODEX_HOME` | `child_env` sets `HOME`/`USERPROFILE` to the temporary tree and removes both overrides; the tree is removed in a `finally`. | PASS |

## Verification Commands

```text
python3 scripts/cutover_benchmark.py check --sessions 2000 --runs 3   -> cutover performance budget OK
python3 scripts/cutover_benchmark.py check --rust-bin <slowed>         -> 4 BUDGET violations, exit 1
python3 -m pytest tests/test_cutover_benchmark.py                      -> 9 passed
python3 -m mypy --strict scripts/cutover_benchmark.py                  -> no issues
python3 scripts/check_workflows.py check                               -> OK
```

## Residual risks

- **The measured commands are read-only.** A real `clean` is not timed: it would need a fresh
  corpus per run and measures deletion syscalls more than the engine. E06-S09 exercises the real
  mutation path for correctness, not speed.
- **The Windows leg is unproven until its first CI run**, including whether the reference sees the
  corpus there through `USERPROFILE`.
- **The tolerance is a judgement.** 10% plus 50 ms is generous for commands that take a quarter of a
  second; it exists to keep a shared runner from failing the gate on jitter.

## Verifier verdict

pending

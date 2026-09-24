# Evidence Packet - E34-S01

- Commit/PR: the E34 commit on `main`
- Executor: Claude
- Independent verifier: round 1 FAIL (Codex, E34-VERIFIER-REVIEW-ROUND1.md); round 2 FAIL (Codex, E34-VERIFIER-REVIEW-ROUND2.md); round 3 FAIL (Codex, E34-VERIFIER-REVIEW-ROUND3.md); repaired
- Change Risk: CR2
- Spec version/commit: `project/epics/E34.json` at this commit; owner decision 2026-09-24 (OpenCode + OpenRouter reviewer, PD-028)

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - prompt built from committed briefs; a story with no committed brief refuses to start | `committed_brief` reads `VERIFIER_BRIEF.md` from `HEAD` via `git show`, never from the working tree, and requires its `Brief-Checksum`; `render_prompt` quotes each brief under its checksum. `test_a_story_without_a_committed_brief_is_refused`, `test_the_prompt_quotes_each_committed_brief_by_checksum` | PASS |
| AC2 - the record's filename is chosen by the harness; an existing file refuses the run | `record_name` + `next_number` (numbering after every existing epic record, story-scoped ones included); `cmd_run` refuses an existing record and an existing worktree. `test_record_names_by_tier`, `test_the_next_number_follows_every_existing_record`, `test_an_existing_review_record_can_never_be_changed` | PASS |
| AC3 - a second model or an Anthropic model in the stream makes the round non-independent and unimportable | `streamed_models` reads OpenCode's `message=stream providerID= modelID=` log lines; `stream_problems` flags any model other than the declared one, any Anthropic model as self-review, and a log naming no model; `cmd_run` refuses to declare an executor-family model at all. `StreamAttributionTests` (6 tests) | PASS |
| AC4 - a path outside the tier's allowed set refuses the round | `allowed_paths` / `path_problems`: pre-review may change only its record; a formal round its record, `SAFETY_VERDICT.md` (append-only, `append_only_problems`), story JSON, generated docs and new tests - never production code, executor evidence or briefs. `test_a_pre_review_may_change_only_its_own_record`, `test_a_formal_round_may_not_touch_production_code_or_executor_evidence`, `test_appending_passes_and_rewriting_is_refused` | PASS |
| AC5 - a passing round imports exactly its changed allowed paths | `cmd_import` refuses a failed run, a worktree that drifted after the check, and deletions, then copies only the allowed changed paths | PASS (by inspection; not exercised end to end in a test - see residual risks) |

## Repairs after the first real run (2026-09-24)

The first OpenCode pre-review (E06-S04 + E33-S01) ran 18 minutes and ended on repeated
`503 provider_overloaded` from the free endpoint before writing its record. The harness failed it
closed ("the reviewer wrote no E06-PRE-REVIEW-1.md") and imported nothing - the intended
behaviour - but the configuration audit that followed found three things the run did not expose:

| Finding | Repair | Evidence |
| --- | --- | --- |
| A failed run said only that no record was written, hiding a provider outage | The run records `the reviewer exited N: <last logged error>` | `test_a_failed_run_says_why` |
| OpenCode loaded the owner's personal `~/.claude/skills` (unmanaged prompt content) into the reviewer | `reviewer_env` sets `OPENCODE_DISABLE_CLAUDE_CODE=1`; `opencode.json` loads back only `.claude/skills` | `opencode debug skill` under that environment lists the built-in skill and the eight repository skills only |
| A reviewer allowed `python3` could reach the owner's git and `gh` credentials | `reviewer_env`: no credential helper, `core.sshCommand=false`, invalid `origin` push URL, empty `GH_CONFIG_DIR`, no `SSH_AUTH_SOCK`/tokens, no auto-update or LSP download | `test_a_reviewer_cannot_push_to_origin` (a real push to a local bare remote fails) |

## Method defect

`test_the_prompt_quotes_each_committed_brief_by_checksum` read the real repository's `HEAD` via
`git show`. It passed locally and in `pytest` CI, and failed `gate_sensitivity.py check` in CI,
whose control run executes the suite on a copy without `.git`. The pre-commit hook for that gate
is scoped to its own script, so it never ran for this change locally. The test now supplies the
committed brief through a mock; `gate_sensitivity.py check` passes locally (11/11). Lesson: a test
that shells out to git against the real repository is not hermetic here - build a temporary repo
(as `AppendOnlyTests` does) or mock `git`.

## Repairs after independent round 1 (2026-09-24)

`E34-VERIFIER-REVIEW-ROUND1.md` (Codex) failed this story on AC4 and AC2/AC5. Both
counterexamples are pinned as regression tests and each was mutation-checked: reverting the repair
makes its test fail.

| Finding | Repair | Evidence |
| --- | --- | --- |
| A staged rename `scripts/protected.py -> tests/test_new.py` reported only the destination, so the deleted production source never reached the tier check (AC4) | `changed_paths` runs `git status --porcelain=v1 -z --no-renames`: a rename is its deleted source plus its destination, and `-z` gives paths verbatim | `test_a_rename_reports_its_source_and_the_source_is_refused`, `test_a_path_with_spaces_or_quotes_is_reported_verbatim`; dropping `--no-renames` fails the first |
| `import` trusted the run file and overwrote a record created in the main tree after the run was checked (AC2, AC5) | `import_problems` repeats the path, existing-record and append-only checks at import time, and refuses - before copying any byte - a target that appeared or changed in the main tree after the run's `base`; `cmd_import` copies nothing unless it returns no problem | `ImportTests` (5 cases: clean import, record appeared, target changed, tampered run file, failed run); disabling the base comparison fails two of them |

Not repaired, recorded: the harness writes its logs to `.opencode-run/` inside the review
worktree, and a reviewer running the full suite there sees `check_docs` fail on the prompt's
relative links (the round-1 reviewer moved the directory aside to run it). It is a usability
defect of the harness, not an acceptance-criterion failure; backlog candidate for E34.

## Repairs after independent round 2 (2026-09-24)

`E34-VERIFIER-REVIEW-ROUND2.md` (Codex) failed AC5: a record replaced after the run's check with
`wrong header / Verifier: forged`, path list unchanged, was imported; import re-ran neither the
record check nor the model attribution.

| Repair | Evidence |
| --- | --- |
| The run binds what it checked: `digests` holds the sha256 of every changed file and of the stream log; `import` refuses any other bytes | `test_a_record_edited_after_the_check_is_refused` |
| `import` re-runs `check_record` and, for OpenCode, the stream attribution on the bytes it is about to copy, so a run file rewritten to match forged output still fails | `test_a_forged_record_fails_even_when_the_run_file_is_rewritten_to_match`, `test_an_opencode_run_is_reattributed_from_its_log_at_import` |

Mutation check: dropping the digest comparison fails two tests; dropping the record re-check fails one.

## Repairs after the forked self-review (`E34-SELF-REVIEW.md`, 2026-09-24)

The self-review (not independent) failed this story again on two routes round 2 had not tried.

| Finding | Repair | Evidence |
| --- | --- | --- |
| `import` took tier, record, reviewer, model and digests from `.review-run.json`, which sat in the reviewer-writable worktree: rewriting it relabelled an Anthropic-model round, a pre-review as formal, or an OpenCode run as Codex | The run record is written beside the worktree (`<worktree>.review-run.json`), out of reach of a reviewer confined to its worktree; a run refuses to start when one already exists there | `test_the_run_record_is_outside_the_worktree` |
| A background job the reviewer started could outlive it and change the tree after the check | The reviewer runs as the leader of its own process group, which the harness kills when the reviewer exits, before any check | `test_nothing_the_reviewer_started_outlives_it` (fails with the kill removed) |
| Writes outside the worktree were never seen (OpenCode ran unsandboxed) | Owner decision 2026-09-24: an OS sandbox, carried by E34-S06 | E34-S06 |

## Round 3 (Codex, `E34-VERIFIER-REVIEW-ROUND3.md`)

Round 3 failed this story on the same route as the self-review: a `.review-run.json` rewritten to
say `reviewer: codex` with matching digests skipped the model check. The repair above - the run
record beside the worktree, not in it - is the one it requires; the round ran on the commit before
it. `test_the_run_record_is_outside_the_worktree` pins it, and E34-S06's sandbox keeps an OpenCode
reviewer from writing beside its worktree at all.

## Verification Commands

```text
python3 -m pytest tests/test_review_round.py -q     -> 24 passed (after the round-1 repairs)
python3 -m mypy --strict scripts/review_round.py    -> no issues
python3 -m ruff check / format --check              -> clean
```

## Residual risks

- **`cmd_run`/`cmd_import` have no end-to-end test**: they spawn a real reviewer. Their parts
  (naming, attribution, path and append-only checks) are unit-tested; the first real pre-review run
  is the integration evidence.
- **Attribution rests on OpenCode's log format.** If a future OpenCode changes the `message=stream`
  line, `streamed_models` finds nothing and the run fails closed ("names no model"), never open.
- **Codex runs are not model-attributed** from logs; the record's `Verifier:` line is trusted for
  Codex, as before this story.

## Verifier verdict

closed by owner decision at the ceiling, see E34-CLOSURE.md

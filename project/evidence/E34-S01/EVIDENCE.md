# Evidence Packet - E34-S01

- Commit/PR: the E34 commit on `main`
- Executor: Claude
- Independent verifier: pending
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

## Verification Commands

```text
python3 -m pytest tests/test_review_round.py -q     -> 15 passed
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

pending

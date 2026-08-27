# Executor Evidence - E00 round 3 (response to the second independent review)

- Executor: Claude
- Independent reviewer: Codex - [round 1](E00-VERIFIER-REVIEW.md), [round 2](E00-VERIFIER-REVIEW-ROUND2.md)
- Previous executor records: [round 1](E00-EXECUTOR-SUMMARY.md), [round 2](E00-EXECUTOR-ROUND2.md)
- Baseline: `4b2df0130e62d83e3a10caaae73daa456211f92d`
- Stories at `ready_for_review`: E00-S01, E00-S02, E00-S04, E00-S05, E00-S06, E00-S08, E00-S09
- Closed: E00-S03 (`done`)
- Open: E00-S07, the epic's closing evidence packet

## Outcome

PASS for the seven stories, none of which the executor may close. Round 2 rejected all
seven; every finding is repaired, each with a regression test that fails against the
round-2 implementation.

## What round 2 taught, recorded because it should change how the next epic is executed

Every round-1 repair closed the reported **instance** and left the defect **class** open.
Round 2 was scoped to falsify classes, and found one in each story. Round-3 repairs were
therefore written class-first, and the regression tests assert the class:

| Story | Round-2 instance | Class actually closed |
| --- | --- | --- |
| E00-S01 | `Plugins` on case-insensitive APFS | name matching is case-folded for every protected name, both providers, both path views |
| E00-S02 | a validated lookalike root | no non-default root is mutable at all (ADR-0013) |
| E00-S04 | `OSError` escaping `cmd_clean` | no exception escapes `main()`; the exit taxonomy is total |
| E00-S05 | `Path.exists()` guards | every discovery guard goes through `observe()`, which cannot conflate absent with unreadable |
| E00-S06 | `history.jsonl` symlink | shared provider metadata is never rewritten through an indirection we did not create |
| E00-S08 | `projects` labelled cleanable | the vocabulary has no state meaning "deleted as it stands", because no top-level entry is |
| E00-S09 | an unrelated parseable `ps` line | a listing that does not contain this process is not a listing |

## Repairs

### E00-S01 - case-insensitive filesystems

`protected_component()` compares case-folded names and returns the canonical protected
name. On a case-sensitive filesystem this is over-inclusive, which is the safe direction.

Regression: `RoundTwoResponseTests.test_protected_barrier_matches_any_case_variant`
(three spellings, both providers) and
`test_case_variant_protected_path_is_refused_at_deletion`, which asserts the refusal at
`safe_remove` rather than only in the predicate.

### E00-S02 - the third answer to the same question

The reviewer's finding was that structure plus intent is not positive identity and
conflicts with SI-002. That is correct and is not repairable by strengthening the
heuristic, so the decision was retaken rather than patched:
[ADR-0013](../../docs/adrs/0013-custom-provider-roots-are-inspection-only-in-python-v1.md)
supersedes ADR-0012, and PD-020 supersedes PD-019.

Only `~/.codex` and `~/.claude` may be mutated. `--allow-custom-root` is **removed**: a
switch that cannot make an unsafe operation safe would only move responsibility for an
unsolved identity problem onto the operator. `structurally_credible` survives as reported
information and is documented as non-authoritative.

This is a capability regression and is recorded as a breaking change. It is the option the
reviewer named, and the alternative - provider-native identity - needs the E05 capability
contract that this reference does not have.

Regression: `RoundTwoResponseTests.test_every_custom_root_shape_is_refused` (empty, weak
and structurally perfect roots) and `test_configure_refuses_a_custom_claude_root`, which
covers the non-`clean` mutation path.

### E00-S04 - a total exit taxonomy

`cmd_clean()` now separates `SafetyError` (blocked, exit 4) from `OSError` (mutation
failure, exit 3), and `main()` converts any other escaping exception into exit 3.

The last point matters beyond tidiness: an uncaught exception leaves Python's own exit
code 1, which is the code for "the operator declined the confirmation prompt". Automation
reading exit 1 as "user said no" would silently treat a crash as a safe no-op.

Regression: `RoundTwoResponseTests.test_unexpected_exception_never_collides_with_the_cancelled_code`.

### E00-S05 - the guard was the leak

`Path.exists()` returns False both for "absent" and for "the parent denied me", so using it
as a discovery guard reintroduced the exact collapse the scan channel exists to prevent -
before any error could be recorded. `observe()` replaces every such guard with an `lstat`
that distinguishes the two and records the second.

Guards replaced: `directory_size`, `iter_files`, `root_entry_sizes`,
`discover_aged_top_entries`, `discover_codex_sessions`, `discover_claude_sessions`,
`discover_claude_aux` (legacy roots and cache files), `count_claude_history_matches`,
`protected_codex_db_entries`, and the before/after sizing in `execute_plan`.

Regression: `RoundTwoResponseTests.test_unreadable_root_is_recorded_rather_than_read_as_empty`
and `test_observe_separates_absent_from_unreadable`.

### E00-S06 - no rewrite through an indirection

`trim_claude_history()` refuses a symlinked `history.jsonl`. `os.replace` would have
swapped the link for a regular file and silently detached whatever it pointed at.

Regression: `RoundTwoResponseTests.test_history_symlink_is_left_alone_in_both_directions`,
which also asserts the target's bytes are untouched, and
`test_execution_reports_a_skipped_history_trim`, which asserts the operator is told.

### E00-S08 - a container is not a candidate

`projects/`, `sessions/`, `archived_sessions/`, `log/`, `tmp/` and the Claude retention
directories are containers: entries inside them are selected by age and policy and the
container itself is never deleted. The vocabulary is now `selective` /
`selective-aggressive` / `aggressive-only` / `trimmed` / `protected` / `reported` /
`unknown`, and `cleanable` is gone because nothing qualified for it.

Regression: `RoundTwoResponseTests.test_no_selective_container_is_ever_deleted_whole`,
which ages four containers past any cutoff and asserts none is selected even under
`--aggressive`.

### E00-S09 - a listing contains its reader

`active_processes()` marks the observation complete only if the output contains this
process. Filtered, truncated, sandboxed or stubbed output is not a full listing.

Regression: `RoundTwoResponseTests.test_observation_requires_seeing_this_process`, over
empty, unrelated-only, target-without-self and unparsable output.

## Out-of-story findings

1. **`.gitattributes` (P1).** The comment claimed protection the setting did not provide.
   `tests/fixtures/** -text` now exempts committed fixtures from normalization, and the
   comment states what is actually guaranteed.
2. **Evidence gate (P2).** Size plus a story id was still filler-shaped. The gate now
   requires the file to state an outcome and how it was established; a missing
   residual-risk statement is a warning rather than a hard failure, because a genuine PASS
   may have none and forcing the phrase would only teach people to paste it. The first
   thing the warning found was a real gap in the E00-S03 closure record, addressed by an
   attributed addendum rather than by editing the reviewer's text.
3. **CR4 closure gate (found while responding).** `done` required a Safety Verdict *file*
   but never read it, so a story could close while a committed `FAIL` sat beside it. The
   gate now requires a recorded PASS or PASS_WITH_RESIDUALS with no FAIL/REJECT in the same
   file. This gate currently blocks E00 closure, which is the correct behaviour.
4. **Coverage conditional state (P2 spec gap).** Addressed by the `selective` vocabulary
   above.

## Verification commands

```text
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_process.py check
```

All pass, with no governance warnings. The suite is 98 tests, including the nine
counterexamples written by the independent reviewer across both rounds.

## Residual risks

- **Custom roots are unusable for cleanup.** An operator who relocated a provider root has
  no supported path until the Rust core ships provider-native identity. Documented, but a
  real loss.
- **Scan completeness is per tool.** One unreadable path anywhere under a provider root
  withholds all cleanup for that provider in that run. Deferred to E04's inventory engine.
- **Case-folded protection is over-inclusive on case-sensitive filesystems.** A genuinely
  distinct directory named `Plugins` on a case-sensitive volume cannot be cleaned. The
  failure direction is non-destructive.
- **Process detection remains best-effort on success.** Exact-name matching cannot prove no
  writer exists; it now fails closed on observation failure, which is a different property.
- **Two review rounds found a class defect in every story they examined.** The base rate
  argues for a third round rather than against it. Nothing here should be read as evidence
  that the remaining code is defect-free.

## Reviewer verdict

PENDING - third independent review not performed. E00-S01, E00-S02, E00-S05 and E00-S09 are
CR4 and additionally require a Safety Verdict recording PASS or PASS_WITH_RESIDUALS before
they can move to `done`; `scripts/project_os.py` now enforces that rather than trusting the
presence of a file.

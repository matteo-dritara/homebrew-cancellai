# Executor Evidence - E00 round 2 (response to independent review)

- Executor: Claude
- Independent reviewer (round 1): Codex - [`E00-VERIFIER-REVIEW.md`](E00-VERIFIER-REVIEW.md)
- Round-1 record: [`E00-EXECUTOR-SUMMARY.md`](E00-EXECUTOR-SUMMARY.md)
- Baseline: `4b2df0130e62d83e3a10caaae73daa456211f92d`
- Stories at `ready_for_review`: E00-S01, E00-S02, E00-S04, E00-S05, E00-S06, E00-S08, E00-S09
- Story closed in round 1: E00-S03 (`done`, PASS)
- Still open: E00-S07, which closes the epic once every other story passes

## Outcome

Every defect the review found is repaired, with a regression test that fails against the
round-1 implementation. Nothing has been marked `done` by the executor.

## Round-1 findings and their repairs

### E00-S01 - FAIL: protected symlink could be unlinked

`protected_component()` resolved the path before computing the name components. A protected
entry that was itself a symlink out of the root fell out of the `relative_to()` computation
and lost its protection entirely.

Repair: the name is checked against two views of the path - lexical (`os.path.normpath`,
which collapses `..` without following links) and resolved. A protected component in either
view refuses the deletion. Resolution failure is still treated as protected.

Regression: `ReviewResponseTests.test_protected_symlink_pointing_outside_the_root_is_refused`
(both providers) and `test_protected_name_reached_through_a_dot_dot_path_is_refused`. The
reviewer's own `test_protected_symlink_name_cannot_be_unlinked` is retained unmodified.

### E00-S02 - FAIL: filenames were treated as identity

The reviewer escalated this beyond the bug: a filename heuristic is not authentication. The
question was routed to an owner decision rather than patched.

Repair: [ADR-0012](../../docs/adrs/0012-custom-provider-roots-require-structure-and-intent.md)
and PD-019. A non-default root now needs two independent conditions - content-validated
provider structure (`auth.json` parsing as a JSON object, a real `rollout-<uuid>.jsonl`
under `sessions/`, a real `<uuid>.jsonl` under `projects/`, and so on) **and** an explicit
`--allow-custom-root`. Structural probing is capped by `MAX_ROOT_PROBE_ENTRIES` because it
runs before authority is granted. `configure` routes through the same boundary.

The reviewer's counterexample is retained and was made **stricter**: it now asserts the
generic directory is refused even when the operator acknowledges it
(`destructive_allowed(acknowledged=True)` is false).

Regression: `RootAuthorityTests.test_marker_filenames_without_provider_content_do_not_identify_a_root`,
`test_credible_custom_root_needs_explicit_intent`, `test_acknowledged_custom_root_can_still_be_cleaned`.

### E00-S04 - FAIL: execution-time refusal escaped as an uncaught exception

The `execute_plan()` boundary re-check raised `SafetyError` past `cmd_clean()`, so a root
that lost its fingerprint between planning and execution produced a traceback instead of the
documented exit 4 and JSON result.

Repair: `cmd_clean()` catches `SafetyError` from execution and converts it into a blocked
`CleanResult`, so the documented exit taxonomy and `--json` shape hold on that path too.

Regression: `ReviewResponseTests.test_execution_time_root_refusal_becomes_exit_blocked`,
which flips the fingerprint between planning and execution and asserts exit 4 plus a
populated `deferred` list in the JSON payload.

### E00-S05 - FAIL: three observation paths still swallowed errors

`read_codex_parent_session_id()`, `count_claude_history_matches()` and `root_entry_sizes()`
converted `OSError` into `None`/`0`/`[]` without a `Scan`.

Repair: all three accept and populate a scan. `protected_codex_db_entries()` was found to
have the same shape during a full re-audit of every `except OSError` and
`contextlib.suppress(OSError)` in the file, and was fixed with them. `status` now sizes each
root through a scan and prints partial totals as lower bounds rather than as facts. The
remaining swallowing sites are fail-closed by construction and carry a comment saying so:
marker validators can only reduce confidence, `protected_component()` returns "protected" on
resolution failure, and `prune_empty_dirs()` is post-mutation cosmetics.

An unreadable `history.jsonl` now returns a distinct `unreadable` status instead of looking
like "nothing to trim", and that is surfaced to the user.

Regression: `ReviewResponseTests.test_unreadable_codex_lineage_marks_the_scan_partial`,
`test_unreadable_history_marks_the_scan_partial`,
`test_status_reports_root_totals_as_lower_bounds_when_partial`.

### E00-S06 - FAIL: retained CRLF bytes were normalised

The rewrite streamed in text mode, so universal-newline translation rewrote retained bytes.

Repair: the rewrite streams in binary. Lines are decoded only to test the session id; the
bytes written back are the bytes read.

Regression: `ReviewResponseTests.test_history_trim_preserves_crlf_and_missing_trailing_newline`,
which also covers a non-UTF-8 byte and a missing trailing newline. The reviewer's
`test_history_trim_preserves_retained_bytes_including_crlf` is retained unmodified.

### E00-S08 - FAIL: `cleanable` overclaimed

`history.jsonl` was labelled `cleanable` although no rule deletes a standalone history file,
and aggressive-only categories were indistinguishable from unconditional ones.

Repair: the vocabulary is now `cleanable` / `aggressive-only` / `trimmed` / `protected` /
`reported` / `unknown`, each with a printed legend. `cleanable` means a rule selects the
entry as it stands.

Regression: `ReviewResponseTests.test_coverage_states_match_what_cleanup_actually_reaches`
and `test_no_standalone_history_file_is_ever_selected_for_deletion`.

### E00-S09 - new story, found during remediation

`active_processes()` returned an empty mapping when `ps` was missing, failed or timed out,
which is indistinguishable from "no provider is running" - a fail-open safety signal in the
middle of a trust-floor epic. Recorded as its own CR4 story rather than folded in silently.

Repair: `ProcessObservation` carries `complete` alongside `pids`. An incomplete observation
blocks every target tool unless `--allow-running` is given, `status` prints `unknown`, and
`--json` exposes `running.observed`.

Regression: `ReviewResponseTests.test_unknown_process_activity_blocks_cleanup`,
`test_unusable_ps_output_is_reported_as_incomplete`,
`test_successful_ps_output_is_reported_as_complete`.

## Out-of-story finding: the evidence gate was vacuous

The reviewer found that `ready_for_review` accepted any Markdown filename, making the
handoff requirement ceremonial. `evidence_is_substantive()` now requires the file to exist,
carry at least `MIN_EVIDENCE_BYTES`, and name the story it is offered for. Covered by
`tests/test_project_os.py::ProjectOSTests::test_evidence_gate_rejects_an_empty_or_unrelated_file`.

A second brittle test was found and fixed while re-running the suite: the control-plane test
asserted an exact decision count, so adding PD-019 broke it. It now asserts the shape of the
register instead of a number that has to be edited to accept new data.

## Verification commands

```text
python3 -m pytest tests -v
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
```

All gates pass. The suite is 80 tests, including the three counterexamples written by the
independent reviewer.

## Documentation updated

- `README.md` - the safety-model bullets and exit-code table that the review found
  overclaiming, plus the new flag;
- `CHANGELOG.md` - both breaking changes and each corrected claim;
- `docs/architecture/AS_IS.md` - safety-critical core, data model, exit taxonomy, and a
  defect table that now records the round-1 verdicts;
- `docs/adrs/0012-custom-provider-roots-require-structure-and-intent.md` and PD-019;
- `docs/CLI.md` regenerated.

## Residual risks

- Scan completeness is per tool, not per scope: one unreadable path anywhere under a
  provider root withholds all cleanup for that provider in that run. Measured on a real
  machine this produced zero errors, but the granularity is coarse and belongs to the Rust
  inventory engine (E04).
- Structural fingerprinting defends against misconfiguration, not against an adversary who
  already controls the filesystem and can fabricate provider-shaped content. Provider-native
  identity is the target for the Rust core and is recorded as such in ADR-0012.
- `--allow-custom-root` is a breaking change for anyone who has relocated a provider root.
- Process detection remains best-effort on success: exact-name matching cannot prove that no
  writer exists. It now fails closed on observation failure, which is a different property.
- `status` still traverses provider roots more than once. Tracked as the deferred single-pass
  inventory work (E04), not regressed by this round.

## Reviewer verdict

PENDING - second independent review not yet performed. The three CR4 stories (E00-S01,
E00-S02, E00-S05) additionally require a Safety Verdict before they can move to `done`.

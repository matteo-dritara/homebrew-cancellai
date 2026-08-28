# Closure Record - E00-S08

- Story: E00-S08 - Report provider layout coverage
- Risk: CR1
- Closed by: project owner, on the executor's evidence, without a fourth review round
- Date: 2026-08-28

## Outcome

`PASS_WITH_RESIDUALS`. Independent review rejected the first vocabulary for overclaiming:
`projects/` was labelled `cleanable` although no rule deletes it, and the reviewer filed the
missing conditional state as a spec gap rather than an implementation bug. The story contract
was corrected accordingly before the implementation.

## Why the story exists

Provider layouts had already drifted and nothing measured the drift. On a real machine
`status --coverage` reports roughly 260 MB under `~/.codex` that this build cannot classify
at all, including a hidden `.tmp/` holding plugin staging state. That gap is the honest scope
of the current tool, and it is now visible rather than silent.

## What changed

The vocabulary is `selective`, `selective-aggressive`, `aggressive-only`, `trimmed`,
`protected`, `reported`, `unknown`, each with a printed legend. There is deliberately no
state meaning "deleted as it stands", because no top-level provider entry is treated that
way: `projects/`, `sessions/` and the retention directories are containers whose *contents*
are selected by age and policy, and `history.jsonl` is trimmed rather than deleted.

No discovery path reads the classification, so no state here can create a cleanup candidate.

## Verification

- `TrustFloorTests.test_unknown_provider_entries_are_reported_and_never_cleaned` and
  `test_coverage_classifies_protected_selective_and_reported_state`;
- `test_status_coverage_output_lists_unknown_entries`;
- `ReviewResponseTests.test_coverage_states_match_what_cleanup_actually_reaches` and
  `test_no_standalone_history_file_is_ever_selected_for_deletion`;
- `RoundTwoIndependentVerifierTests.test_coverage_does_not_call_memory_only_projects_cleanable`
  - the reviewer's counterexample, retained;
- `RoundTwoResponseTests.test_no_selective_container_is_ever_deleted_whole` - four containers
  aged well past any cutoff, none selected even under `--aggressive`.

## Residual risks

- The classification is a static vocabulary compiled into this build. It reports drift; it
  does not resolve it, and a provider that adds a directory tomorrow shows up as `unknown`
  until someone updates the lists.
- `selective` describes what the rules can reach, not what they will reach in a given run:
  age, keep-latest and `--aggressive` still decide. The legend says so; a reader who skips it
  could over-read the state.
- No independent verifier examined the final state of this story.

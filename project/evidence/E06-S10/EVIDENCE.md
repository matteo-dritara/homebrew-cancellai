# Evidence Packet - E06-S10

- Commit/PR: the E06-S10 commit on `main`
- Executor: Claude
- Independent verifier: pending - reviewed with the rest of E06's cutover stories
- Change Risk: CR3
- Spec version/commit: `project/epics/E06.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - `--keep-claude-history` accepted, `history.jsonl` untouched | `clean_leaves_claude_history_untouched_with_and_without_keep_claude_history` (flag given): the session is deleted, `history.jsonl` is byte-identical, exit 0. | PASS |
| AC2 - without the flag, `history.jsonl` still untouched and the output says so | Same test, flag absent: byte-identical `history.jsonl`, and stdout carries "history.jsonl was left unchanged"; with the flag the note is absent. The note is printed only on human-readable runs that deleted a Claude artifact; the JSON result schema is unchanged. | PASS |
| AC3 - `--verbose` reports each action without changing which run | `clean_verbose_reports_each_action_and_changes_nothing_it_does`: the same tree cleaned with and without `--verbose` deletes the same two artifacts and prints the same totals line; only the verbose run prints one `succeeded` line per action. | PASS |
| AC4 - the other gaps stay refused and disclosed | `verbose_and_keep_claude_history_are_refused_outside_clean` (exit 2 on `plan`); `status --paths/--coverage/--top` and `--aggressive` are untouched and still refused by clap. `docs/CLI_RUST.md` "Known gaps" lists them as divergences, and `CHANGELOG.md` states the new flags. | PASS |

## Round-3 repair (Codex, `project/evidence/E06-VERIFIER-REVIEW-ROUND3.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| A stable `clean --yes --json` deleted a Claude session and disclosed the untouched `history.jsonl` on neither stream | The note is emitted on every run that deleted a Claude artifact without `--keep-claude-history`: stdout for human output, stderr for `--json`, so stdout stays one document | `a_json_clean_discloses_the_untouched_history_on_stderr_only`, with and without the flag, parsing stdout as JSON |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 | A flag that adds a second mutation path (rewriting `history.jsonl`) | No mutation code was added: the flag only suppresses a printed note, and the history test asserts byte-identity with and without it. `scripts/check_mutation_boundary.py check` passes. | PASS |
| SI-007 | A flag that widens what `clean` does | `--verbose` is print-only; the action set is asserted equal. Neither flag is accepted by a read-only command. | PASS |

## Mutation check

- Removing the `history.jsonl` note fails the history test on the no-flag leg.
- Removing the per-action print fails the verbose test's `per_action == 2` assertion.

## Verification Commands

```text
cargo fmt --check                                                   -> clean
cargo clippy --workspace --all-targets --all-features -- -D warnings -> clean
cargo test --workspace                                              -> see commit; cli_behavior 44 passed
cargo deny check                                                    -> ok
pre-commit run --all-files                                          -> passed
```

## Compatibility

- `clean --help` gains two options (golden snapshot updated). No JSON schema change.

## Residual risks

- **The history trim itself remains a divergence.** Users of the canonical engine will keep lines
  for deleted Claude sessions in `history.jsonl`. Closing that needs a provider-file rewrite
  primitive inside the safety boundary, which is its own CR4 story, not part of this one.

## Verifier verdict

pending

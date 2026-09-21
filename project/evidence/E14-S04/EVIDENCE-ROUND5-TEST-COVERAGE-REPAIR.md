# Evidence Packet - E14-S04 (round 5, test-coverage repair)

- Commit/PR: repairs the finding in `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND7.md` (`FAIL`)
- Executor: Claude
- Independent verifier: Codex (pending)
- Change Risk: CR4
- Spec version/commit: `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-
  observation-type.md` ("Round 5, fourth self-correction" section)

## Outcome

PARTIAL (repair complete, awaiting independent review)

## Context

Round 7 review confirmed the errno-discrimination production code from the prior repair was
correct, but found its regression test closed the bound root's descriptor *before*
`list_child_names` was called at all - the method failed at the earlier `try_clone` step, never
reaching `readdir`'s own null/errno handling or the `DT_UNKNOWN` fallback the test claimed to
verify. The test would pass unchanged even if the errno-checking logic were removed.

## Repair

`rust/crates/cancellai-sealedfs/src/lib.rs`: `list_child_names` is now a thin wrapper over a new
private `list_child_names_with_hook(&self, after_entry: impl FnMut(usize, RawFd))`, matching the
test-hook shape this crate (`establish_with_hook`) and `cancellai-platform::mutation`
(`confirmed_delete_file_inner`'s `between_open_and_unlink`) already use elsewhere. The hook fires
once per real entry, with the exact raw duplicated descriptor `readdir` is reading from. A new
test closes that descriptor mid-enumeration (after enough real entries - 500 - to force
`readdir`'s own internal buffering past a single syscall, since a smaller directory can return a
correct end-of-stream answer already known from one buffered read regardless of what happens to
the descriptor afterward) and confirms the method now returns `Err` for that genuine
mid-enumeration failure, not a truncated `Ok`.

The earlier, narrower test (closing the descriptor before any call) is retained as a distinct,
real regression for a different failure point (`try_clone` itself), renamed for honesty about
what it covers.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| A genuine mid-enumeration `readdir` failure is reported as an error, not a truncated success | `list_child_names_reports_a_readdir_failure_after_a_real_entry_not_a_truncated_ok`, run 5x consecutively with no flakes | PASS |
| No regression to existing behavior | Full `cancellai-sealedfs` suite (27 tests) and full workspace suite re-run: 0 failures | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                                    -> PASS
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings -> PASS
cd rust && cargo test --workspace                                               -> PASS (0 failed)
cd rust && cargo deny check                                                     -> PASS
python3 scripts/check_rust_workspace.py check                                   -> PASS
python3 scripts/check_mutation_boundary.py check                                -> PASS
python3 scripts/project_os.py check                                             -> PASS
```

## Documentation updated

- `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-observation-type.md`
  ("Round 5, fourth self-correction" section added)
- New method doc on `list_child_names_with_hook` in `rust/crates/cancellai-sealedfs/src/lib.rs`

## Method defects

- **What happened**: a regression test was written and locally executed to confirm it passed,
  without independently checking that it would *fail* against the pre-repair code (the standard
  "red before green" discipline `adversarial-cases`/`risk-gate` both name). Had that check been
  done, the test's failure to reach `readdir` at all would have been visible immediately.
  **Prevented by**: `docs/development/AGENT_PROTOCOL.md`/the `adversarial-cases` skill already
  state this discipline; it was not followed for this specific test before submitting for review.
  **Disposition**: proposed - no process change needed, since the rule already exists; this is a
  personal-discipline note for future CR4 regression tests in this session.

## Residual risks

- Unchanged from prior evidence files in this directory.

## Verifier verdict

PENDING - awaiting independent review by Codex.

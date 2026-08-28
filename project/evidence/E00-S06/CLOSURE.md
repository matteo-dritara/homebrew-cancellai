# Closure Record - E00-S06

- Story: E00-S06 - Protect concurrent Claude metadata rewrites
- Risk: CR3
- Closed by: project owner, on the executor's evidence, without a fourth review round
- Date: 2026-08-28

## Outcome

`PASS_WITH_RESIDUALS`. Independent review examined this story twice: round 1 rejected a
rewrite that normalized retained CRLF bytes, round 2 rejected one that followed and replaced
a `history.jsonl` symlink. Both are repaired.

## What changed

The rewrite streams bytes rather than loading and re-encoding the file, so retained lines
keep their exact bytes - CRLF endings, invalid UTF-8, a missing trailing newline. Lines are
decoded only to test the session id; what is written back is what was read.

The source is re-identified by `(st_dev, st_ino, st_size, st_mtime_ns)` immediately before
the atomic replace, and the rewrite is abandoned if a provider wrote concurrently. A
symlinked `history.jsonl` is refused outright: `os.replace` would have swapped the link for a
regular file and silently detached whatever it pointed at.

Trimming is skipped entirely while a Claude process is running, even under `--allow-running`,
and skipped when activity cannot be determined. A failed or skipped trim is reported through
`deferred` rather than looking like "nothing to do".

## Verification

- `TrustFloorTests.test_history_is_not_rewritten_while_claude_is_running`;
- `test_history_trim_abandons_rewrite_on_concurrent_write` - an append injected between the
  copy loop and the replace, asserting no temp file survives;
- `test_history_trim_removes_only_deleted_sessions` - malformed lines preserved;
- `ReviewResponseTests.test_history_trim_preserves_crlf_and_missing_trailing_newline`;
- `RoundTwoIndependentVerifierTests.test_history_trim_preserves_retained_bytes_including_crlf`
  and `test_history_symlink_is_not_replaced` - the reviewer's counterexamples, retained;
- `RoundTwoResponseTests.test_history_symlink_is_left_alone_in_both_directions` and
  `test_execution_reports_a_skipped_history_trim`.

## Residual risks

- The concurrency guard detects a change, it does not prevent one. A writer that modifies the
  file and restores its size and mtime nanoseconds would go unnoticed; no filesystem this
  build targets makes that plausible, but it is not proof.
- The parent directory is not checked for being a symlink. Containment and the provider-root
  boundary cover the path that reaches here, but the check is on the file, not the chain.
- Skipping the trim leaves `history.jsonl` referencing deleted sessions. The operator is told,
  and the state is inconsistent-but-safe rather than lossy.
- No independent verifier examined the final state of this story.

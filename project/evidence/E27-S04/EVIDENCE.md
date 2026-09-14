# Evidence Packet - E27-S04

- Commit/PR: the recursive-deletion refusal on `rework/codebase-health`
- Executor: Claude
- Independent verifier: **not performed.** CR3 in the shipped deletion path; branch work awaiting the owner.
- Change Risk: CR3, the floor `project/risk_floors.json` sets for `cancellai.py` ("the Python reference still performs real deletion; it is frozen, not inert")
- Spec version/commit: `project/epics/E27.json` at this commit
- Freeze basis: a **safety/security fix**, which `AGENTS.md`'s Python reference freeze lists as an accepted change

## Outcome

PASS

## The finding

`safe_remove` deletes a directory with `shutil.rmtree(path)`. Everything protecting the approved
root before that line is path-based: `is_within`, `path.resolve()`, the config-root comparison.
None of it survives the walk. Once `rmtree` begins, a subdirectory can be replaced with a symlink
and the removal follows it out of the root.

Python closes this only where the platform supports fd-relative removal, and reports whether it
did: `shutil.rmtree.avoids_symlink_attacks`. **Nothing in this repository had ever read that flag.**
On macOS - the only platform v1 supports - it is `True`, so the shipped tool was protected by a
property of the platform rather than by a decision anyone made or recorded. The README says Linux
is untested; on a platform where the flag is `False`, the protection silently was not there.

This is the same gap [ADR-0017](../../../docs/adrs/0017-sealed-root-handle-for-configuration-writes.md)
built an entire crate to close for the Rust engine, in its own words: "a path re-checked
immediately before use is not enough to close this class of gap; only a *retained* capability is."
The frozen reference cannot grow a sealed root. It can refuse.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - refuse rather than delete anyway | `safe_remove` raises `SafetyError` naming the platform property and what the user can do instead. `test_a_directory_is_refused_where_the_platform_is_not_safe` flips the flag under test, which is the only way to reach this path on macOS where it is genuinely `True`. | PASS |
| AC2 - a refusal is not a half-deletion | The check precedes the call, so nothing has been unlinked when it fires. The test asserts the file *inside* the directory still exists after the refusal, rather than only that an exception was raised. | PASS |
| AC3 - single-file deletion still works there | The refusal is scoped to the recursive branch. A path already proven `S_ISREG` is unlinked in one call and is not exposed to a walk. Widening the refusal would stop the tool working for no gain. `test_a_plain_file_is_still_removed_where_the_platform_is_not_safe`. | PASS |
| AC4 - unchanged where removal is safe | `characterize.py check` (13 fixtures), `diff_harness.py check`, and `rust_python_parity.py check` (13 NORMATIVE fixtures across both root-origin scenarios) all pass unchanged. That is what proves a frozen reference did not move. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 | A symlink swapped in mid-walk redirecting deletion outside the approved root | Not reachable on macOS, which is why it was never found by testing. The defence is now a stated precondition rather than an accident of the platform, and the refusal path is exercised by forcing the flag. | PASS |
| SI-019 | The guard weakening an existing barrier | It adds a refusal and removes none. Every prior check runs first and in the same order; 88 tests in `test_cancellai.py` pass, three of them new. | PASS |
| C-01 (unknown state is non-destructive) | Treating "cannot delete safely" as "delete anyway" | That was the behaviour. It is now the constitution's answer instead: refuse, say why, and let the user act. | Corrected |

## Verification Commands

```text
python3 -m pytest tests/test_cancellai.py -q   -> 88 passed (3 new)
python3 scripts/characterize.py check          -> 13 fixtures match
python3 scripts/diff_harness.py check          -> self-test cases behave as documented
python3 scripts/rust_python_parity.py check    -> 13 NORMATIVE fixtures agree across engines
python3 -m mypy cancellai.py                   -> clean
```

## Compatibility

- **Behaviour is unchanged on macOS and Linux**, where `avoids_symlink_attacks` is `True`. On a
  platform where it is `False`, `clean` now refuses directory removal with exit code 4 (safety
  blocked) instead of performing it. That is a user-visible change on a platform this release does
  not claim to support.

## Residual risks

- **The refusal is coarse.** It refuses all recursive removal on such a platform rather than
  removing the tree safely, because removing it safely means fd-relative traversal and that is a
  sealed-root implementation the freeze does not permit here.
- **Nothing tests the real unsafe platform.** Both directions are exercised by flipping the flag,
  which tests this repository's logic and not Python's. No CI runner reports `False`.
- **The rest of the walk is still path-based.** `prune_empty_dirs` and the scan traverse with
  `os.walk(followlinks=False)`, which is correct for reading and would be a separate story to
  harden if it ever mutated.
- **This is a CR3 change to the shipped deletion path with no independent review**, made on a
  branch. The characterization passing is evidence that behaviour did not move; it is not a review.

## Verifier verdict

pending - branch work

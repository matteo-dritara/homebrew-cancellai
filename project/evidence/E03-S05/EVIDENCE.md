# Evidence Packet - E03-S05

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E03)
- Change Risk: CR4
- Spec version/commit: `rust/crates/cancellai-platform/src/mutation.rs`, `rust/crates/cancellai-safety/src/mutation_executor.rs`, `scripts/check_mutation_boundary.py` as added in this change

## Outcome

PASS

## This is the first story to perform a real filesystem mutation

Every prior E03 story (S01-S04) was read-only or pure computation. E03-S05 adds the first
code in this workspace that actually calls `std::fs::remove_file`/`remove_dir_all`/`rename` -
`cancellai-platform`'s new `MutationExecutor` seam (`mutation.rs`), mirroring the
`Clock`/`FsObserver`/`IdentityObserver`/`PathResolver` pattern the prior four stories
established: a real, OS-backed `SystemMutationExecutor` and a test-only
`SyntheticMutationExecutor` for fault injection. `cancellai-safety`'s new
`mutation_executor.rs` (`execute`/`execute_all`) is the orchestration that composes
everything the epic built so far - `SealedPlan`/`revalidate` (E03-S02), `BoundedPath`
(E03-S03) - into the one call path from an approved plan to that capability.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Provider/UI crates cannot directly unlink/rmtree/delete | New static check `scripts/check_mutation_boundary.py` (verification contract's "static dependency checks") scans every crate's production `src/**/*.rs` for `std::fs::remove_file`/`remove_dir`/`remove_dir_all` and fails if any file other than `cancellai-platform/src/mutation.rs` calls one. Verified the check both passes today (`mutation boundary OK: 24 Rust source files scanned`) and actually catches a violation: temporarily injected `std::fs::remove_dir_all(...)` into `cancellai-cli/src/main.rs`, confirmed the check reported it by exact file/line, then restored the original file (`git status` confirms no residual change). Also verified it does *not* false-positive on doc comments that merely *mention* these function names in prose (several files, including `mutation.rs`/`mutation_executor.rs` themselves, explain the boundary using the literal function names) - the check ignores `//`/`///`/`//!` comment lines. | PASS |
| AC2 - Executor revalidates all plan preconditions immediately before mutation | `execute` calls `observer.observe(target.path())` and `revalidate` (E03-S02, reused, not reimplemented) *before* ever constructing a `MutationOperation` or calling `executor.mutate`. `execute_never_calls_mutate_on_a_stale_plan` proves this isn't just "the result is ignored": a `SyntheticMutationExecutor` configured to return an error carrying the message "this must never be observed" for the target path is never triggered - the assertion is on `SafelyBlocked`, and the injected fault would have surfaced as `Failed` had `mutate` been called at all. | PASS |
| AC3 - Partial failures produce explicit per-action results and never hide skipped work | `execute_all` is `plans.iter().map(execute).collect()` - structurally incapable of skipping an element (unlike a hand-written loop with an early `return`/`?`) or stopping after the first failure. `execute_all_never_short_circuits_and_never_drops_a_result` drives three plans through one call with a mixed outcome (one `Succeeded`, one `SafelyBlocked` via injected staleness, one `Failed` via injected OS fault) and asserts the result vector has exactly 3 entries, each matching its corresponding fault - not fewer, not reordered, not collapsed to a single "batch failed" verdict. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 (one mutation boundary) | Any crate other than the one allowed file calling a deletion primitive directly | `scripts/check_mutation_boundary.py` (see AC1) | PASS |
| SI-019 / SI-013 (revalidate immediately before mutation) | Identity changed since planning; artifact became absent since planning | `execute_blocks_a_stale_plan_instead_of_mutating`, `execute_never_calls_mutate_on_a_stale_plan` (the latter also proves the OS call was never reached, not merely that its result was discarded) | PASS |
| SI-020 (irreversible actions are explicit, not disguised) | A non-mutating action class (`Observe`) reaching the executor; an unmapped action class (`Quarantine`/`Archive`, no OS-primitive defined yet) | `execute_refuses_a_non_mutating_action_class` - `Observe` is refused outright, never silently treated as a successful no-op mutation. `Quarantine`/`Archive` are refused with an explicit reason naming the action class, not attempted against a best-guess operation. | PASS |
| Not vacuously fail-closed | A genuinely valid plan against a genuinely unchanged, real file | `execute_deletes_when_identity_still_matches`, `execute_deletes_a_directory_tree_when_target_kind_is_directory` (the latter also proves the file-vs-directory `MutationOperation` selection is correct, not merely "some deletion happened") | PASS |
| Real OS mutation actually works, not just the synthetic double | Delete a real file via `SystemMutationExecutor`; delete a nonexistent target | `system_executor_deletes_a_real_file`, `system_executor_reports_the_os_error_for_a_missing_target` (`cancellai-platform`'s own tests) | PASS |

## Verification Commands

```text
# Python governance (repository-wide)
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py scripts/diff_harness.py scripts/check_rust_workspace.py scripts/check_mutation_boundary.py
python3 scripts/check_docs.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/project_os.py check

# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# Cross-platform compile verification (see E03-S01's evidence for why)
cargo check --target x86_64-pc-windows-gnu --all-targets
cargo check --target x86_64-unknown-linux-gnu --all-targets
cargo clippy --target x86_64-pc-windows-gnu --all-targets --all-features -- -D warnings
```

All passed. `cargo test -p cancellai-platform` now runs 23 unit tests (19 prior + 4 new
`mutation` tests, including one that deletes a real file on the real filesystem and confirms
it is gone). `cargo test -p cancellai-safety` runs 37 (30 prior + 7 new
`mutation_executor` tests). `scripts/check_mutation_boundary.py` passes with all 24 scanned
production source files clean.

A real bug surfaced and was fixed during development, worth recording: the mutation-executor
test helper originally built each temporary target directory from a fixed label plus process
id, identical across every call - since the Rust test runner runs tests in parallel threads
within one process, and the helper is called multiple times both within one test and across
different tests, this produced directory-name collisions (one test's `Drop` cleanup deleting
another still-in-use test's directory), observed as five spurious `RootIdentityUnavailable`
failures. Fixed with a per-call atomic counter added to the generated name; rerun clean
(`cargo test -p cancellai-safety`: 37 passed, 0 failed). This is a test-infrastructure defect,
not a production-code one - `execute`/`execute_all` themselves were never at fault - but it is
recorded here rather than silently fixed, since a flaky-looking failure in a CR4 story's own
test suite is exactly the kind of thing a reviewer should be told was investigated and
explained, not merely "eventually passed."

## Compatibility

- `MutationExecutor`'s production implementation (`SystemMutationExecutor`) uses
  `std::fs::remove_file`/`remove_dir_all`/`rename`, available identically on macOS, Linux,
  and Windows; no platform-specific branch exists in this seam. `execute`'s file-vs-directory
  selection depends on E03-S01's `IdentityToken`, which is `Unsupported` off-Unix today (see
  E03-S01's evidence) - so on a non-Unix target, `execute` never reaches the point of
  building a `BoundedPath` to mutate in the first place; this story does not change that
  posture.

## Performance / operability

- `execute` is one `observe` call plus (on the `Proceed` path) one `mutate` call - both O(1)
  relative to the target, no recursion or batching logic beyond `execute_all`'s single `map`.

## Documentation updated

- `docs/architecture/TARGET.md` - "Forbidden dependency direction" section now states both
  the E03-S05 mutation-boundary implementation and (retroactively, since it was missed at the
  time) the E03-S02 `cancellai-safety` -> `cancellai-platform` dependency-check change, since
  this is the file that actually documents that rule's list (the story's declared
  documentation impact).

## Residual risks

- `ActionClass::Quarantine`/`ActionClass::Archive` are refused outright by `execute`, not
  implemented - `SealedPlan` (E03-S02) does not carry a quarantine destination, and this
  story does not invent one. `MutationExecutor::mutate`'s `Quarantine` operation exists and
  is implemented (`SystemMutationExecutor` handles it via `std::fs::rename`) but has no
  production caller yet - a future story adding a destination field to `SealedPlan` is what
  wires `execute`'s `ActionClass::Quarantine` arm to it; recorded as scope for that story, not
  created here.
- `ActionResult::Succeeded` does not carry `observed reclaimed bytes` or post-action
  reconciliation state, both named in `docs/architecture/DOMAIN_MODEL.md`'s `Results` section
  - no inventory/reconciliation subsystem exists yet (E04) to supply either; recorded as
  future scope on the same type, not stubbed out now.
- `scripts/check_mutation_boundary.py`'s "text before the first `#[cfg(test)]`" heuristic for
  distinguishing production from test code assumes this codebase's existing one-test-module-
  per-file convention. A file that deliberately placed production code *after* its test
  module would defeat the heuristic (its later banned call would be treated as test code and
  missed). This is a documented assumption in the script's own docstring, not a silent gap;
  no file in this workspace currently violates the convention.
- `check_mutation_boundary.py`'s comment-line exemption is line-based (`//`-prefix), which is
  correct for this codebase (no `/* */` block comments exist anywhere in `rust/crates/*/src/`,
  verified by search) but would not correctly exempt a block comment if one were introduced
  later without also updating this script.
- `pre-commit` itself is not installed in this environment; the new hook
  (`mutation-boundary-check`) was verified by running the underlying command directly
  (`python3 scripts/check_mutation_boundary.py check`) and by validating
  `.pre-commit-config.yaml`'s YAML syntax, not by an actual `pre-commit run`. CI runs the full
  pre-commit hook set and will exercise it for real.

## Verifier verdict

PENDING - epic E03 review runs once every story in E03 is `ready_for_review` (at most twice per epic, per ADR-0014).

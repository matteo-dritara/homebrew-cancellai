# E03 Independent Verifier Review - Round 1

- Review target: `808ffd3..f2a4080562410ec49673de7d1c21e1364a30bc0c`
- Verifier: Codex
- Date: 2026-08-29

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E03-S01 | PASS_WITH_RESIDUALS | Unix `device`/`inode`/kind/mtime observation detects file, directory, symlink, and synthetic mount-target replacement; absent/unreadable/unsupported observations fail closed in revalidation. Windows remains explicitly unsupported and non-destructive. |
| E03-S02 | FAIL | `SealedPlan` has no policy explanation or provider capability required by SI-016, and its `RootFingerprint` has no type/API connection to the `BoundedPath` submitted to `execute`. A plan recorded for `root-a` executes against a separately established `root-b`. |
| E03-S03 | FAIL | The public `cancellai_platform::SystemMutationExecutor::mutate(&Path, MutationOperation)` accepts an unconstrained raw path. An external-consumer probe deleted a temporary file directly, bypassing `ApprovedRoot`, `BoundedPath`, plan, and safety executor. |
| E03-S04 | PASS_WITH_RESIDUALS | The named minimum is deterministic, retains all ties, caps protected/unknown/active/partial states at `Recommend`, and cannot be raised past the artifact ceiling by user authority. E03-S05 currently fails to enforce this result. |
| E03-S05 | FAIL | Four verifier probes succeeded: raw mutation bypass; `Delete` with `Observe` authority and `Quarantinable` reversibility; plan-root/target-root mismatch; and target replacement after observation but before `remove_file`, which deleted the replacement. |

## Failures and required repair

### E03-S02 — sealed plans are incomplete and not root-capability-bound

Reproduction: construct two `ApprovedRoot`s. Bind a target under root B, construct a
`SealedPlan` with root fingerprint A and the target's identity, then call `execute` with a
matching identity observer and synthetic executor. It returns `Succeeded`; neither
`SealedPlan` nor `BoundedPath` carries a shared, verified root-capability identity for the
executor to compare.

Required repair: make sealing consume a verified `ApprovedRoot`/bound target association and
require the executor to verify that association; retain and revalidate root identity as part of
execution. Add policy explanation, provider capability, and explicit execution-precondition
fields (or update the approved story/invariant before claiming compliance), and make arbitrary
caller construction unable to create an executable destructive plan without those facts.

Violates E03-S02 AC1 and SI-013/SI-016.

### E03-S03 — raw mutation API bypasses root and boundary capabilities

Reproduction: an external crate imports public `SystemMutationExecutor` and the public
`MutationExecutor` trait, creates a temporary file, and calls
`SystemMutationExecutor.mutate(&raw_path, MutationOperation::DeleteFile)`. The deletion
succeeds without an `ApprovedRoot`, `BoundedPath`, or sealed plan.

Required repair: keep the raw OS primitive private to the platform/safety implementation or
make its capability unconstructable/inaccessible to provider and UI crates; expose only a
safety-kernel operation that requires a valid sealed plan and root-bound target. Extend the
static/API test so a consumer crate cannot compile a direct destructive call.

Violates E03-S03 AC1/AC2 and SI-002/SI-003/SI-018.

### E03-S05 — executor does not enforce authority or eliminate check/use replacement

Reproduction:

- A matching plan with `ActionClass::Delete`, `AuthorityLevel::Observe`, and
  `Reversibility::Quarantinable` returns `Succeeded`.
- An `IdentityObserver` that first observes the original object, then atomically renames it
  away and creates a replacement before returning its observation, causes `execute` with
  `SystemMutationExecutor` to return `Succeeded` and delete the replacement.
- The public raw mutation call described for E03-S03 bypasses the executor altogether.

Required repair: make effective authority and reversibility mandatory, validated execution
preconditions (delete needs its explicit irreversible authority threshold); bind each plan to
its exact root capability/target; and replace path-based revalidate-then-delete with an
OS-specific handle/descriptor-based operation that proves the mutated object is the observed
one, or otherwise refuse where that guarantee cannot be established. Keep the raw primitive
private and add permanent external-consumer and post-observation-swap regression tests.

Violates E03-S05 AC1/AC2, SI-013, SI-019, and SI-020.

## Gate status

- PASS: `cargo fmt --check`
- PASS: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- PASS: `cargo check --workspace --all-targets`
- PASS: `cargo test --workspace`
- PASS: `cargo deny check` (initial sandbox run could not lock Cargo's advisory database; rerun with approved cache access passed, with only unmatched-license warnings).
- PASS: `python3 -m pytest tests -v` — 179 passed, 22 subtests.
- PASS: Ruff check/format and mypy via `uv run` (the system Python lacks Ruff).
- PASS: generated-docs, project OS, docs, workflows, fixtures, schemas, characterization,
  differential harness, Rust-workspace, mutation-boundary, process, and release checks.
- PASS: `git diff --check` before review-record/status edits; rerun after generation below.
- NOT LOCALLY EXECUTED: Windows/MSRV CI matrix. The current Windows identity posture is fail-closed.

## Overall verdict

FAIL. E03-S01 and E03-S04 are moved to `done` with owner-visible passing Safety Verdicts.
E03-S02 and E03-S03 return to `in_progress`; E03-S05 is `blocked` because it depends on both
failed stories and also has independent CR4 defects. The epic remains `in_progress` for the
executor repair cycle and one remaining independent review round.

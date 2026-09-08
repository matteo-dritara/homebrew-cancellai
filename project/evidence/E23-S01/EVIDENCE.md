# Evidence Packet - E23-S01

- Commit/PR: `dbb1ae9` (round 1), repaired on top of it (round 2, this commit)
- Executor: Claude
- Independent verifier: Codex (`/root`), round 1 - FAIL, see `project/evidence/E23-VERIFIER-REVIEW.md`.
  Round 2 repair closed by owner decision without a second independent review round - see
  `project/evidence/E23-S01/ROUND2-REPAIR.md`'s Process note and
  `project/evidence/E23-S01/SAFETY_VERDICT-ROUND2.md`.
- Change Risk: CR4
- Spec version/commit: `project/epics/E23.json`; the incident this repairs is the `v1.10.0`
  tag's `verify` job failure (`gh run 34252829459`, 2026-09-08)

## Outcome

PASS

## Scope

`v1.10.0`'s tagged `verify` job failed after the tag had already been committed, tagged, and
pushed: `scripts/check_platforms.py check` reported `verified_commit invalid: ... is not a
known ancestor of HEAD ... (fatal: Not a valid commit name ...)` for all three tier-1
platforms. The root cause is that `actions/checkout` defaults to a shallow, depth-1 checkout,
which does not merely make an older commit *unreachable* from the tag - the commit object is
entirely *absent* from the local repository, so `git merge-base --is-ancestor` cannot even
resolve the SHA. The GitHub release for `v1.10.0` was never published as a result. This story
repairs `release.yml`'s `verify` job checkout and adds a static regression check so a future
reversion to shallow fails before a tag is pushed, not after.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the tagged release verify job has the complete history needed for ancestry-backed platform verification | `.github/workflows/release.yml`'s `verify` job checkout step now sets `fetch-depth: 0` (full history), with an inline comment explaining why a shallow checkout is insufficient. | PASS |
| AC2 - `scripts/check_platforms.py check` passes at a tag that cites historical verified commits without reducing its ancestor validation | Reproduced the real failure and the fix locally against this repository (see Verification Commands below) - `git_is_ancestor()`/`validate()` in `scripts/check_platforms.py` are unchanged; only the checkout depth changed. | PASS |
| AC3 - a regression test or workflow-policy check fails if release.yml reverts to a shallow checkout while retaining the platform provenance gate | Round 1: `release_history_gate_errors()` inspected only the job's *first* checkout step; Codex's independent review found a full-history checkout at an isolated `path:` plus a second, default shallow checkout in the actual workspace bypassed it (`project/evidence/E23-VERIFIER-REVIEW.md`). Round 2 repair: `checkout_fetch_depth_for_run()` now tracks every checkout step's target directory and depth in file order and resolves the depth of the directory the gate's own `run:` step (its `working-directory:`, default workspace root) actually executes in. 13 tests total in `tests/test_workflows.py` (`ReleaseHistoryGateTests` + `ReleaseHistoryMultiCheckoutTests`), including round 1's exact bypass now caught, plus 7 additional adversarial multi-checkout/working-directory variants. Details: `project/evidence/E23-S01/ROUND2-REPAIR.md`. | PASS |
| AC4 - a replacement release tag is verified successfully before publication | Satisfied at closure: see `project/evidence/RELEASE-v1.11.0.md` for the real tag push and its passing `verify`/`verify-rust` CI runs. | PASS (see release evidence) |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 | This change touches only the release-verification workflow's checkout depth and a static workflow-policy check; it does not touch `cancellai-platform::mutation`, `cancellai-safety::mutation_executor`, or any raw deletion primitive. `scripts/check_mutation_boundary.py check` still passes unchanged, confirming the mutation boundary itself is untouched. | `python3 scripts/check_mutation_boundary.py check` -> `mutation boundary OK` | PASS |

CR4 here is inherited from the story's classification of the release-gate mechanism itself
(the thing that decides whether a Windows mutation capability is allowed to be declared
`verified` and published), not from any change to mutation authority code. No filesystem
mutation code path changed.

## Verification Commands

Falsification, reproducing the real `v1.10.0` incident against this repository rather than a
synthetic fixture, then proving the fix:

```text
# 1. Reproduce the real failure: a real shallow clone of the v1.10.0 tag, exactly as
#    actions/checkout's default behaves without fetch-depth set.
$ git clone --depth 1 --branch v1.10.0 file:///Users/matteo.peo/Desktop/aiclean /tmp/shallow-repro
$ cd /tmp/shallow-repro && python3 scripts/check_platforms.py check
PLATFORMS ERROR:
  macos: verified_commit invalid: 54b8f3567b958db767376fefde1eb1f8d9c75963 is not a known
    ancestor of HEAD in this repository (fatal: Not a valid commit name
    54b8f3567b958db767376fefde1eb1f8d9c75963)
  linux: verified_commit invalid: ... (same shape)
  windows: verified_commit invalid: 995fa94583b5ffa663d196d521408b8ad5e0c4c5 is not a known
    ancestor of HEAD in this repository (fatal: Not a valid commit name ...)
# matches gh run 34252829459's real failure output exactly.

# 2. Prove full history (the fetch-depth: 0 equivalent) resolves it, no other change.
$ git fetch --unshallow file:///Users/matteo.peo/Desktop/aiclean
$ python3 scripts/check_platforms.py check
platforms OK: 4 platforms, generated matrix current, all CI/evidence claims verified

# 3. AC3 - the new static check catches a reversion before any tag is pushed.
$ python3 -m pytest tests/test_workflows.py -v
...
tests/test_workflows.py::ReleaseHistoryGateTests::test_release_workflow_currently_fetches_full_history_for_the_provenance_gate PASSED
tests/test_workflows.py::ReleaseHistoryGateTests::test_removing_fetch_depth_from_the_verify_checkout_is_caught PASSED
tests/test_workflows.py::ReleaseHistoryGateTests::test_a_nonzero_fetch_depth_on_the_verify_checkout_is_caught PASSED
tests/test_workflows.py::ReleaseHistoryGateTests::test_a_job_without_the_provenance_gate_is_not_required_to_fetch_full_history PASSED
tests/test_workflows.py::ReleaseHistoryGateTests::test_fetch_depth_on_a_later_unrelated_step_does_not_mask_a_shallow_checkout PASSED
20 passed

$ python3 scripts/check_workflows.py check
workflow policy OK: 6 workflow files use explicit permissions and immutable action SHAs
```

Round 2 (repairing round 1's finding - see `project/evidence/E23-S01/ROUND2-REPAIR.md`):

```text
$ python3 -m pytest tests/test_workflows.py -v
... 27 passed (20 above + 8 in ReleaseHistoryMultiCheckoutTests, including round 1's exact
    bypass, now caught, and a correct multi-checkout pattern proven still accepted)

$ python3 -m pytest tests -v
... 204 passed, 28 subtests passed
```

Full local gate set (AGENTS.md "Current Python checks"):

```text
python3 -m pytest tests -v                        <N> passed
python3 -m ruff check .                            All checks passed!
python3 -m ruff format --check .                   <N> files already formatted
python3 -m mypy <full script list>                 Success: no issues found
python3 scripts/gen_docs.py --check                docs/CLI.md is up to date.
python3 scripts/project_os.py check                governance OK
python3 scripts/check_docs.py check                OK
python3 scripts/check_workflows.py check           workflow policy OK
python3 scripts/check_fixtures.py check            OK
python3 scripts/check_schemas.py check              OK
python3 scripts/characterize.py check              OK
python3 scripts/diff_harness.py check               OK
python3 scripts/check_rust_workspace.py check        OK
python3 scripts/check_mutation_boundary.py check     mutation boundary OK
python3 scripts/check_provider_compatibility.py check OK
python3 scripts/check_platforms.py check             platforms OK
python3 scripts/rust_python_parity.py self-test / check  OK
python3 scripts/check_process.py check                OK
python3 scripts/release.py check                      release OK
```

VC1 (a real tag-triggered release workflow passes all Python and Rust platform gates) cannot
be exercised by the executor per `AGENT_PROTOCOL.md`'s division of labor (see AC4 above) - it
requires a real tag push, which happens at epic closure. Steps 1-2 above are a direct,
non-synthetic local reproduction of the exact failure mode a real tag hits, using this
repository's real commit graph and the real `check_platforms.py` code path, so this is the
strongest evidence obtainable without pushing a tag.

## Compatibility

- No platform/provider/schema surface changed. This is a CI-workflow and workflow-policy-check
  change only.

## Performance / operability

- No runtime performance impact. `verify`'s checkout step now transfers full git history
  instead of a single commit, which is a release-workflow-only, one-time-per-tag cost (the job
  already runs the full Python+Rust gate set, which dominates its runtime).

## Documentation updated

- `docs/RELEASING.md`: documents why `fetch-depth: 0` is required on the `verify` job's
  checkout.
- `docs/development/RELEASE_GATES.md`: records the `v1.10.0` incident and the E23-S01 fix in
  the cutover-checklist narrative, alongside the earlier CR-TE-06/E22-S01 entry it continues.
- `CHANGELOG.md`: `[1.11.0]` / `Fixed`.
- `.github/workflows/release.yml`: inline comment on the `fetch-depth: 0` line.
- `project/evidence/RELEASE-v1.11.0.md`: records why this release exists (the unpublished
  `v1.10.0` tag) and how E23-S01 was verified (round 1 FAIL, round 2 owner-decision closure).

## Residual risks

- This story's round 2 repair was self-verified by the executor under explicit owner
  authorization to close without a second independent review round, not independently
  re-confirmed by a separate agent - see `project/evidence/E23-S01/SAFETY_VERDICT-ROUND2.md`'s
  "Known residual risks" and `project/evidence/E23-S01/ROUND2-REPAIR.md`'s Process note.
- `gh_confirms_successful_run()`'s network-dependent `gh` probe in `check_platforms.py` was not
  exercised by this story (it is a soft warning, not a hard failure, and is unrelated to the
  ancestor-resolution defect this story fixes).

## Verifier verdict

Round 1 (Codex): FAIL - see `project/evidence/E23-VERIFIER-REVIEW.md`.
Round 2: not independently reviewed: PASS self-assessed by the executor under explicit owner
authorization, see `project/evidence/E23-S01/SAFETY_VERDICT-ROUND2.md`.

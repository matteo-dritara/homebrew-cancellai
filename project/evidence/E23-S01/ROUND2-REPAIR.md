# E23-S01 - Round 2 repair (independent verifier review round 1 finding)

- Story: E23-S01
- Round: repair after `project/evidence/E23-VERIFIER-REVIEW.md` (round 1, FAIL)
- Date: 2026-09-08

## Verdict this repairs

Round 1 verdict: FAIL. `release_history_gate_errors()` inspected only the `verify` job's
*first* `actions/checkout` step. A workflow with a full-history checkout aimed at an isolated
`path: full-history-copy`, followed by a second, default (shallow) checkout populating the
actual job workspace before `python3 scripts/check_platforms.py check` ran, returned `[]` -
the ancestry gate would still run shallow in the real workspace, reproducing the v1.10.0
failure under a different layout. Reproduction (from `project/evidence/E23-VERIFIER-REVIEW.md`):

```yaml
jobs:
  verify:
    steps:
      - uses: actions/checkout@<40-hex-sha>
        with:
          fetch-depth: 0
          path: full-history-copy
      - uses: actions/checkout@<40-hex-sha>
      - run: python3 scripts/check_platforms.py check
```

## What changed

`scripts/check_workflows.py`:

- Replaced `checkout_fetch_depth()` (first-checkout-only) with `job_steps()` +
  `checkout_fetch_depth_for_run()`. `job_steps()` splits a job body into ordered per-step text
  blocks at the `steps:` list's own indentation. `checkout_fetch_depth_for_run()` walks those
  steps in file order, tracking a `{directory: fetch-depth}` map keyed by each
  `actions/checkout` step's `path:` (default: the job workspace root - `""` after
  normalization); a later checkout re-populating a directory overwrites the depth an earlier
  one left there, matching real `actions/checkout` semantics. When it reaches the `run:` step
  whose command equals `python3 scripts/check_platforms.py check`, it resolves that specific
  step's own `working-directory:` (default: workspace root) against the tracked map and
  returns the depth in effect there - not "the first checkout in the job."
- `_normalize_step_dir()` treats `path:`/`working-directory:` values that are absent, `""`,
  `.`, or `./`-prefixed as the same workspace-root key, and strips quotes/trailing slashes, so
  `path: ./full-history-copy` and `working-directory: full-history-copy` are recognized as the
  same directory rather than failing to match on a literal-string difference.
- `release_history_gate_errors()` now calls `checkout_fetch_depth_for_run()` instead of the
  removed first-checkout-only helper; its error message reports the resolved depth and states
  explicitly that the check is scoped per checkout-target-directory and per the gate step's own
  working directory.
- Fixed a bug caught while writing this repair, before it ever reached a verifier: the new
  `STEP_MARKER_RE` was defined without `re.MULTILINE`, so `job_steps()` returned zero steps for
  every real job body (which starts with job-level comment lines, not a step marker at offset
  0) and `checkout_fetch_depth_for_run()` silently returned `None` even for the correctly-fixed
  real `release.yml` - caught locally by `test_release_workflow_currently_fetches_full_history_for_the_provenance_gate`
  failing immediately after the rewrite, before any commit.

`tests/test_workflows.py`: new `ReleaseHistoryMultiCheckoutTests` (7 tests) covers, against
synthetic workflow text (not the real file):

- the exact round-1 bypass (full-history checkout at an isolated path, shallow default
  checkout in the workspace) - now caught;
- the correct multi-checkout pattern (a later checkout at the workspace root overwriting an
  earlier shallow one with `fetch-depth: 0`) - accepted, proving the fix does not just reject
  every multi-checkout workflow;
- the inverse ordering (full history at the root first, a later unrelated shallow checkout
  clobbering it before the gate runs) - caught;
- the gate's own `working-directory:` pointed at a directory that *was* checked out full -
  accepted, even though the workspace root itself stays shallow;
- the gate's own `working-directory:` pointed at a directory *never* checked out at all -
  caught;
- a leading `./` on `path:` vs. a bare `working-directory:` naming the same directory -
  recognized as equal, not a false failure;
- a full-history checkout that only happens *after* the gate step already ran - not credited
  retroactively.

## Verification

```text
$ python3 -m pytest tests/test_workflows.py -v
... 27 passed (20 pre-existing + 7 new adversarial multi-checkout tests)

$ python3 -m pytest tests -v
... 204 passed, 28 subtests passed

$ python3 -m ruff check . && python3 -m ruff format --check .
All checks passed! / 234 files already formatted

$ python3 -m mypy <full script list per AGENTS.md>
Success: no issues found in 14 source files

$ python3 scripts/check_workflows.py check
workflow policy OK: 6 workflow files use explicit permissions and immutable action SHAs
```

Manually reproduced the round-1 bypass and its fix directly against `checkout_fetch_depth_for_run()`
before formalizing it as a test (see conversation evidence); confirmed `depth: 1` for the
bypass (caught) and `depth: 0` for the real, fixed `release.yml`.

Full local gate set (AGENTS.md "Current Python checks") re-run and green: `gen_docs.py --check`,
`project_os.py check`, `check_docs.py check`, `check_workflows.py check`, `check_fixtures.py
check`, `check_schemas.py check`, `characterize.py check`, `diff_harness.py check`,
`check_rust_workspace.py check`, `check_mutation_boundary.py check`,
`check_provider_compatibility.py check`, `check_platforms.py check`, `rust_python_parity.py
self-test`/`check`, `check_process.py check`, `release.py check`. No Rust source file changed
by this repair; `rust/` gates were unaffected (Codex's round 1 review already confirmed
`cargo fmt --check` / `clippy -D warnings` / `cargo test` / `cargo deny check` pass on this
diff and no Rust file has changed since).

## Process note - no second independent review round

Per `docs/development/AGENT_PROTOCOL.md`, a story's independent verification is normally
performed by an agent other than its executor, and closing a CR4 story requires an
independent, owner-visible Safety Verdict the executor does not write for itself. For this
specific repair, the owner explicitly instructed (in-session, 2026-09-08): implement the fix
Codex's round 1 review specified, re-verify it, and close the story directly without spending
a second independent review round - "Al termine, non è necessaria un'ulteriore review, puoi
mettere il task in done."

This repository has an existing precedent for this exact sequence: v1.9.0's E22 closure notes
that the epic "was then closed by owner decision without spending the second independent
review round ADR-0014 permits... these repairs carry no independent re-confirmation beyond the
executor's own re-run of every gate the round-1 review specified" (`CHANGELOG.md`, `[1.9.0]`).
This closure follows the same pattern: `project/evidence/E23-S01/SAFETY_VERDICT-ROUND2.md`
records a PASS verdict self-assessed by the executor (Claude) against round 1's exact finding,
with adversarial tests reproducing that finding plus five additional counterexamples the
executor generated independently of Codex's report, not a second independent agent's
falsification pass. This is recorded as the residual risk it is, not represented as
independent re-confirmation.

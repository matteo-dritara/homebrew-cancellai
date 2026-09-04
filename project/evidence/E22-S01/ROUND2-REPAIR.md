# E22-S01 - Round 2 repair (independent verifier review round 1 findings)

- Story: E22-S01
- Round: repair after `project/evidence/E22-VERIFIER-REVIEW.md` (round 1, FAIL)
- Date: 2026-09-04

## Verdict this repairs

Round 1 verdict: FAIL. `release_gate_drift_errors()` was a literal-command presence check
against `.pre-commit-config.yaml` only; pytest is not a pre-commit hook and the remote
ruff/mypy hooks have no repository-owned `entry:`, so six independent adversarial variants
against the real `release.yml` (removing pytest, ruff-check, mypy; dropping Windows from
either matrix; disabling `verify-rust`; making clippy non-blocking) all returned an empty
error list.

## What changed

`scripts/check_workflows.py`:

- `agents_md_python_gate_commands()` parses AGENTS.md's "Current Python checks" fenced `sh`
  block - the actual documented contract for what main enforces - and every command it lists
  (pytest, ruff check, ruff format, the full mypy target list, and every `scripts/*.py check`)
  is now required verbatim in `release.yml`'s `verify` job. This is what makes the
  pytest/ruff/mypy removal variants fail: those commands have no pre-commit `entry:` to
  compare against, but they are literal lines in AGENTS.md.
- `blocking_job_errors()` rejects `continue-on-error: true` anywhere in `verify`, `verify-rust`
  (release.yml), or `quality` (rust.yml), and rejects any `if:` condition in those job bodies -
  this is what catches "disable_verify_rust" (an `if: false` guard) and "nonblocking_clippy"
  (a `continue-on-error: true` step) even though the literal command text is unchanged.
- `matrix_values()` + a new comparison assert `verify-rust`'s `os:` matrix equals `quality`'s -
  this catches Windows (or any platform) being dropped from either side independently of
  command text.
- A new check asserts `release.yml`'s `publish` job depends on both `verify` and
  `verify-rust` in its `needs:` list, so a gate cannot be silently detached from what actually
  gates the release artifact.
- `release.yml`'s own `mypy` invocation was itself missing several files AGENTS.md's canonical
  list requires (`check_fixtures.py`, `check_schemas.py`, `characterize.py`, `diff_harness.py`,
  `check_rust_workspace.py`, `check_mutation_boundary.py`, `check_provider_compatibility.py`,
  `rust_python_parity.py`) - a real, if narrower, instance of the same drift class this story
  exists to close. Corrected to match AGENTS.md exactly.

## Verification

`tests/test_workflows.py::ReleaseGateDriftTests` gained one regression test per round-1
counterexample (`test_removing_pytest_from_release_yml_is_caught`,
`test_removing_ruff_check_from_release_yml_is_caught`,
`test_removing_mypy_from_release_yml_is_caught`,
`test_removing_windows_from_either_matrix_is_caught`,
`test_disabling_verify_rust_with_an_if_condition_is_caught`,
`test_nonblocking_clippy_via_continue_on_error_is_caught`,
`test_dropping_verify_rust_from_publish_needs_is_caught`), each reproducing the exact mutation
the round-1 review demonstrated against the real workflow file and asserting a non-empty error
list where the previous checker returned `[]`. All pass; `python3 -m pytest tests -v` and
`python3 scripts/check_workflows.py check` both pass against the unmodified repository.

## Residual risk

The story's own verification bullets also call for a real tag-run dry-run of the repaired
`release.yml` (including a controlled Windows-clippy-failure replay). That requires the target
commit to exist on GitHub and a tag push, which this round's local repair work does not
perform in isolation - it is covered by cutting the real `v1.9.0` release this epic's closure
requires (ADR-0014), whose run is recorded in `project/evidence/RELEASE-v1.9.0.md`. The
static-analysis strengthening above is what makes `scripts/check_workflows.py` itself
trustworthy going forward; it does not by itself prove the workflow executes correctly on
GitHub's Windows runners, only that removing/weakening any of its gates is now detected.

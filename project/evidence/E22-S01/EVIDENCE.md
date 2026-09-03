# Evidence Packet - E22-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E22 epic review round 1
- Change Risk: CR1
- Spec version/commit: `docs/audits/2026-09-03-CODE_REVIEW.md`, finding `CR-TE-06`; reproduced
  in `project/evidence/RELEASE-v1.8.0.md`'s correction section

## Outcome

PASS

## Scope

`.github/workflows/release.yml` stated that it re-runs every gate at the tagged commit but ran
no Rust check at all, and omitted the differential parity gate, the fixture/schema/
characterization checks, the mutation-boundary and provider-compatibility checks, and
`scripts/release.py check`. This was not hypothetical: at v1.8.0 the release workflow reported
`success` for tag `v1.8.0` while `rust.yml`'s `quality (windows-latest)` job failed on the same
commit (`b5d83bd`, an unused-import lint on `#[cfg(unix)]`-only test code).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - `release.yml` runs the full checker set AGENTS.md lists, including the differential parity gate, fixture/schema/characterization checks, mutation-boundary/provider-compatibility checks, and `scripts/release.py check` | `verify` job now runs `check_fixtures.py check`, `check_schemas.py check`, `characterize.py check`, `diff_harness.py check`, `check_rust_workspace.py check`, `check_mutation_boundary.py check`, `check_provider_compatibility.py check`, `rust_python_parity.py self-test` and `check`, and `release.py check`, in addition to the checks it already ran. A Rust toolchain step was added since `check_provider_compatibility.py`/`rust_python_parity.py` both invoke `cargo build`/`cargo run` internally. | PASS |
| AC2 - `release.yml` runs the Rust quality set: `cargo fmt --check`, clippy with denied warnings, `cargo test`, `cargo deny check` | New `verify-rust` job, matrixed over `macos-latest`/`ubuntu-latest`/`windows-latest` (mirroring `rust.yml`'s `quality` job), runs all four; `publish` now depends on `[verify, verify-rust]`. | PASS |
| AC3 - `scripts/check_workflows.py` fails when the release gate set drifts from the pre-commit gate set | New `release_gate_drift_errors()` derives the required Python gate set from `.pre-commit-config.yaml`'s local hooks (`entry:`, excluding `commit-msg`-staged hooks) and the required Rust gate set from `rust.yml`'s `quality` job, then fails if `release.yml`'s `verify`/`verify-rust` jobs are missing any of them. Wired into `validate_workflows()`. | PASS |

## Safety Evidence

Not safety-bearing (CR1, observational: verifies gates re-run, does not change what any gate
permits).

## Verification Commands

Falsification, matching the story's Verification Contract exactly:

```text
$ python3 scripts/check_workflows.py check          # baseline: OK
workflow policy OK: 6 workflow files use explicit permissions and immutable action SHAs

# AC3 / VC1 - a deliberately removed gate in release.yml makes check_workflows.py fail
$ sed -i '' '/scripts\/release.py check/d' .github/workflows/release.yml
$ python3 scripts/check_workflows.py check
WORKFLOW ERROR: .github/workflows/release.yml: job 'verify' is missing pre-commit gate
'release-consistency' ('python3 scripts/release.py check'); the release workflow must re-run
every gate pre-commit enforces on main
$ # restored, re-checked OK

$ sed -i '' '/cargo deny check/d' .github/workflows/release.yml
$ python3 scripts/check_workflows.py check
WORKFLOW ERROR: .github/workflows/release.yml: job 'verify-rust' is missing Rust quality gate
'cargo deny check' from rust.yml's 'quality' job
$ # restored, re-checked OK
```

Committed as regression tests in `tests/test_workflows.py::ReleaseGateDriftTests`
(`test_a_removed_precommit_gate_in_release_yml_is_caught`,
`test_a_removed_rust_quality_gate_in_release_yml_is_caught`), which reconstruct the same
scenario against a temporary `release.yml` rather than mutating the real file.

VC2 (a dry run of the release workflow on a tag reaches every added step) and VC3 (replaying
the v1.8.0 Windows-clippy failure makes the release workflow fail) cannot be exercised locally
- both require a real GitHub Actions run on a pushed tag / matrix runner. `verify-rust` is a
direct copy of `rust.yml`'s already-proven `quality` job (same matrix, same steps, same pinned
action SHAs), so the same job definition that currently reports `quality (windows-latest)`
failures on `rust.yml` will report them as `verify-rust (windows-latest)` on `release.yml`
once a tag is pushed - this is a structural argument, not a substitute for VC2/VC3, and is
flagged as a residual below.

Full local gate set:

```text
python3 -m pytest tests -v                     183 passed, 26 subtests passed
python3 -m ruff check . && ruff format --check  All checks passed! / 203 files already formatted
python3 -m mypy <full script list>              Success: no issues found in 15 source files
python3 scripts/gen_docs.py --check             docs/CLI.md is up to date.
python3 scripts/project_os.py check             governance OK
python3 scripts/check_docs.py check             docs OK
python3 scripts/check_workflows.py check        workflow policy OK
python3 scripts/check_fixtures.py check         fixtures OK
python3 scripts/check_schemas.py check          schemas OK
python3 scripts/characterize.py check           characterization OK
python3 scripts/diff_harness.py check           diff harness OK
python3 scripts/check_rust_workspace.py check   rust workspace OK
python3 scripts/check_mutation_boundary.py check  mutation boundary OK
python3 scripts/check_provider_compatibility.py check  provider compatibility matrix OK
python3 scripts/rust_python_parity.py self-test   self-test OK
python3 scripts/rust_python_parity.py check       12 NORMATIVE fixtures OK
python3 scripts/check_process.py check          process OK (pre-existing E00 exception only)
python3 scripts/release.py check                release OK: v1.8.0 consistent

cd rust
cargo fmt --check           (clean)
cargo clippy --workspace --all-targets --all-features -- -D warnings   (clean)
cargo check --workspace --all-targets                                  (clean)
cargo test --workspace      327 tests, all passed
cargo deny check            advisories ok, bans ok, licenses ok, sources ok
```

## Compatibility

- No behavioural change to `cancellai.py` or the Rust engine; this is CI/release-tooling only.
- `release.yml`'s `verify` job now needs a Rust toolchain (added via the same
  `dtolnay/rust-toolchain` action `rust.yml` already uses, pinned to the same SHA) because two
  of the newly-added Python checks shell out to `cargo`.

## Performance / operability

- `release.yml` now takes longer to run (a full Rust build/test/clippy/deny pass across three
  platforms, in addition to the existing Python checks), which is the intended trade: the tag
  is not published faster than it can be verified.

## Documentation updated

- `docs/RELEASING.md` - describes the full gate set the tag now faces and the drift check.
- `docs/development/RELEASE_GATES.md` - `CR-TE-06` marked closed under the Rust cutover gate
  status (G4) section, `E06-S04` blocker note removed.
- `.github/workflows/release.yml` - `verify` job extended, new `verify-rust` job added,
  `publish` depends on both.

## Residual risks

- VC2/VC3 (a real tagged dry run reaching every step, and reproducing the Windows-clippy
  failure through an actual Actions run) are not exercised by this evidence packet - they
  require pushing a tag, which is outside an executor's authority and this story's CR1 scope.
  The structural argument above (the new `verify-rust` job is a literal copy of `rust.yml`'s
  already-CI-proven `quality` job) is the best available substitute; an independent verifier
  or the next real release should confirm VC2/VC3 empirically.
- `scripts/check_workflows.py`'s drift check compares literal `run:` command strings. A gate
  whose pre-commit `entry:` or `rust.yml` command text changes cosmetically (e.g. reordered
  flags with identical meaning) would read as "missing" and require a one-line update to
  `release.yml`, which is the intended failure mode (loud, not silent) rather than a defect.

## Verifier verdict

pending

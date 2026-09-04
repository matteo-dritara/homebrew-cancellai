# E22 Independent Verifier Review - Round 1

- Review target: `d0df840..0c0a100`
- Verifier: Codex (`/root`), independent verifier
- Date: 2026-09-04
- Epic: E22 - Engineering System Hardening

All six stories were `ready_for_review` before review began. Expected behavior was
reconstructed from the story contracts, linked engineering/security documents, and the final
diff. Executor reasoning was not treated as evidence.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E22-S01 | FAIL | The release workflow currently contains the listed commands and a three-OS Rust matrix, but `release_gate_drift_errors()` is only a literal-command presence check. Independent temporary variants removed pytest, Ruff, mypy, or Windows, disabled the whole Rust job, or made clippy non-blocking; every variant returned `[]`. No target-version tag run exists to satisfy the dry-run/replay contract. |
| E22-S02 | FAIL | Static configuration adds Cargo Dependabot and a Rust CodeQL job with the shared `security-events: write` permission. However, the latest real CodeQL run is run `33749220786` at `d0df840`, not the target, and its job list contains only `Python reference security analysis`. No real non-empty Rust analysis or synthetic-outdated-pin Dependabot proposal exists. |
| E22-S03 | FAIL | Help/version and ordinary irrelevant-flag refusal work, clap is confined to `cancellai-cli`, SI-007 mutation probes remained non-destructive, and `cargo deny check` passed. Counterexample: `cancellai-cli status --help --dry-run` exits 0 and silently accepts the irrelevant `--dry-run`. The claimed golden tests are also substring assertions, not exact golden snapshots. |
| E22-S04 | FAIL | `cargo llvm-cov -p cancellai-policy --lib --summary-only` reproduced 95.58% line coverage, but the new mixed-age-tree test pins the opposite of `cancellai.py::choose_codex_old_sessions`: Python selects nothing when a recent child makes the tree's effective mtime recent, while Rust emits one delete for the stale parent. The green direct test therefore creates false confidence around the required boundary case. |
| E22-S05 | FAIL | A narrower CR3 wiring is not safely available: the existing safety executor both authorizes and immediately invokes its sole filesystem mutation, so invoking `codex delete` outside it would create a second mutation path and extending it would be CR4 authority-boundary work. The disclosure branch is therefore correct in principle, and all four detection outcomes pass. But E22-S03 replaced `## Known gaps versus the Python reference` with `## Argument parsing`; the disclosure is now a list item under that section while `CLI_RUST.md`, `PROVIDERS.md`, and `CHANGELOG.md` still refer to a nonexistent “Known gaps” section. AC1 specifically requires that location. |
| E22-S06 | PASS | The regex counts both epic- and two-digit story-scoped records against the captured epic, E00/E07 are explicit reasoned exceptions, the synthetic third record fails, and its error names all three records. The real repository passes with only the two documented warnings. |

## FAIL reproductions and required repairs

### E22-S01 - effective release-gate drift is not detected

Against an unmodified checkout, `python3 scripts/check_workflows.py check` passes. I then
copied `release.yml` to a temporary file, patched `RELEASE_WORKFLOW` to point at it, and tested
six independent regressions. Each printed an empty error list:

```text
remove_pytest: []
remove_ruff_check: []
remove_mypy: []
remove_windows: []
disable_verify_rust: []
nonblocking_clippy: []
```

The first three show that the checker does not cover the full Python gate set it claims to
derive: pytest is not a pre-commit hook, and the remote Ruff/mypy hooks have no literal `entry:`
for `precommit_gate_commands()` to collect. The latter three show that command text remains
accepted even when the v1.8.0 Windows failure would again be invisible or non-blocking.

Required repair: make one machine-readable source define the full release-required Python and
Rust gates, or otherwise compare all of pytest/Ruff/format/mypy and repository checkers; verify
that required steps/jobs cannot be conditionally skipped or marked `continue-on-error`; and
compare/assert the tier-1 Rust OS matrix, not only command strings. Add regressions for these
counterexamples. Then run the repaired workflow on a tag (including a controlled failing
Windows clippy case) and record the real job/step evidence.

This violates AC3 (“fails when the release gate set drifts ... cannot silently regress”) and
leaves the two tag-run verification requirements unmet. AC1/AC2 describe commands present in
the current YAML, but presence alone is not the non-regression guarantee the story requires.

### E22-S02 - required real service evidence does not exist

GitHub Actions query on 2026-09-04:

```text
latest codeql run: 33749220786
head: d0df840fb49aa516846901602be8e7eb754bce1e
jobs: [Python reference security analysis]
target head: 0c0a100480c4d66e830b2402b2591519b18d9385
open Cargo Dependabot alerts returned: []
```

The target commits are local `main` commits while `origin/main` remains `d0df840`; consequently
GitHub has never parsed or executed the added Rust job or Cargo Dependabot entry. A YAML claim
cannot establish that CodeQL produced a non-empty Rust database/upload or that Dependabot can
resolve this workspace.

Required repair: expose the target to GitHub through the normal reviewed branch/push workflow,
record a successful `analyze-rust` job whose build and analyze/upload steps execute, verify its
Rust analysis is non-empty in code scanning, and perform the contracted temporary outdated-pin
exercise that results in a Cargo Dependabot proposal (then remove/close the synthetic pin/PR as
appropriate). This is required evidence for AC1-AC3, especially AC3's security-events delivery
claim, and directly violates both verification-contract items until supplied.

### E22-S03 - strict argument rejection and golden contract gaps

The built binary produced:

```text
cancellai-cli status --dry-run              -> exit 2
cancellai-cli status --help --dry-run       -> exit 0, status help printed
cancellai-cli --json clean --yes            -> exit 2, status parser rejects `clean`
cancellai-cli clean --json                  -> exit 2, explicit-intent error
cancellai-cli cle --yes                     -> exit 2, unknown subcommand
```

Clap's help action short-circuits validation of following arguments. This is non-destructive,
so SI-007 itself remains preserved, but it falsifies the unqualified AC3 statement that flags
irrelevant to a command are rejected. Separately, the new “golden” tests only check that help
contains a usage prefix/command names and version contains the crate version; many output
regressions would still pass, so the exact golden-output verification contract is not met.

Required repair: define and enforce error precedence so irrelevant trailing arguments are
rejected even when help/version is present, or amend the story contract explicitly if help is
intended to be an exception. Commit exact reviewed help/version snapshots (including each
subcommand) and exercise them on the tier-1 CI matrix. This violates AC3 and the golden CLI
snapshot verification requirement; no SI-007 mutation bypass was found.

### E22-S04 - mixed-age Codex trees violate the reference selector

For one root at mtime `0`, its child at `9 * 86400`, cutoff `3 * 86400`, `keep_latest=0`, and
filesystem fallback, direct invocation of the frozen Python selector returned:

```text
python_selected=[]
```

This follows `choose_codex_old_sessions`: tree effective mtime is the maximum member mtime;
when it is at/above cutoff, the entire tree is skipped. The new Rust test
`codex_tree_members_that_disagree_in_age_are_deleted_individually_when_the_tree_is_not_kept`
passes while asserting one deletion, because `resolve_codex` computes `effective_mtime` only
for keep-latest ordering and classifies staleness per member. Its own module contract says the
cutoff applies per tree and a recent descendant protects an old parent.

Required repair: apply the age cutoff to the tree's effective mtime before producing delete
actions. If the whole tree is old and the backend is filesystem-level, individual old members
may remain separate actions, matching the reference; if any member is recent, no member is
eligible. Reverse the mixed-age boundary test to expect zero deletes and add the scenario to
the differential fixture corpus so M6 catches the existing semantic divergence. Re-run the
two mutation-style spot checks against tests that assert the correct reference behavior.

This violates AC1's ported-reference rule coverage, the explicitly required mixed-age-tree
boundary case, and the migration differential contract. The named SI-005/SI-012 probes do not
show a separate dry-run/execution or category-expansion bypass; the failure is the retention
semantics the story was required to pin.

### E22-S05 - disclosure is outside the section the contract names

`rg '^## ' docs/CLI_RUST.md` reports only Commands, Shared flags, Exit codes, and Argument
parsing. The base revision had `## Known gaps versus the Python reference (tracked, not
silent)`, but E22-S03 replaced that heading while retaining every gap bullet beneath the new
Argument parsing section. E22-S05 then edited the Codex bullet in place without restoring the
heading. Cross-references now point to a section that does not exist.

I also independently traced the narrow-wiring alternative. `execute_with_system_capabilities`
calls `execute`, which performs root, identity, process, authority, action-class, and
reversibility checks and immediately calls `MutationExecutor::mutate`; `MutationOperation`
contains only `DeleteFile`. There is no authorization-only result that the outer CLI could
safely consume before invoking a vendor command. Calling the provider command beside or after
that route would either double-mutate or bypass C-07/SI-019. Adding a vendor operation to this
kernel seam is an irreversible mutation/authority-boundary change and therefore CR4, not a
narrow CR3 wiring.

Required repair: restore a real `## Known gaps versus the Python reference (tracked, not
silent)` section before the Codex and remaining gap bullets, keep the non-wiring rationale
there, and reconcile “permanent divergence” with the evidence packet's statement that wiring
remains wanted future work. This violates E22-S05 AC1's explicit disclosure-location and
truthfulness requirement. The no-new-mutation-path decision preserves SI-004, SI-019, and
SI-021; the six synthetic native-delete tests cover Supported, Unsupported/nonzero,
BinaryNotFound, ProbeFailed/timeout, and large-output behavior.

## Additional counterexamples checked

- CLI: missing subcommand, leading bare flags, misspelled `clean`, `--json` without destructive
  intent, irrelevant ordinary flags, negative/beyond-`u32` numeric inputs, and `--` placement.
  No ambiguity resolved toward mutation.
- Dependency graph: `cargo tree -i clap` shows clap reaches only `cancellai-cli`; kernel crates
  gained no E22 dependency. Cargo-deny accepted all resolved licenses/sources/advisories.
- Retention: cutoff equality, keep-latest zero/above count, protected-plus-pinned state,
  running/unknown process liveness, unobservable mtime, and tool scoping all passed. The
  mixed-age tree remains a real Python/Rust semantic disagreement despite those greens.
- Native delete: all four support outcomes remain distinguishable. No production call to the
  provider command exists, and the static mutation-boundary checker still reports one path.
- Review ceiling: epic/story filenames, exact two-round boundary, third-round diagnostics,
  and the E00/E07 exception list were inspected and exercised independently.

## Gates executed

| Command | Result |
| --- | --- |
| `python3 -m pytest tests -v` | PASS: 184 tests, 26 subtests |
| `.venv/bin/python -m ruff check .` | PASS |
| `.venv/bin/python -m ruff format --check .` | PASS: 209 files formatted |
| `.venv/bin/python -m mypy` over every AGENTS.md target | PASS: 13 source files |
| `python3 scripts/gen_docs.py --check` | PASS |
| `python3 scripts/project_os.py check` | PASS before verdict recording |
| `python3 scripts/check_docs.py check` | PASS |
| `python3 scripts/check_workflows.py check` | PASS, but adversarial false negatives above make E22-S01 FAIL |
| `python3 scripts/check_fixtures.py check` | PASS |
| `python3 scripts/check_schemas.py check` | PASS |
| `python3 scripts/characterize.py check` | PASS |
| `python3 scripts/diff_harness.py check` | PASS |
| `python3 scripts/check_rust_workspace.py check` | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| `python3 scripts/check_provider_compatibility.py check` | PASS |
| `python3 scripts/rust_python_parity.py self-test` | PASS |
| `python3 scripts/rust_python_parity.py check` | PASS: 12 NORMATIVE fixtures, both root origins; corpus lacks the mixed-age counterexample |
| `python3 scripts/check_process.py check` | PASS with only the documented E00/E07 exception warnings |
| `python3 scripts/release.py check` | PASS before verdict recording: v1.8.0 consistent |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS: 346 tests/doc-tests; two scheduled benchmarks ignored |
| `cargo deny check` | PASS after sandbox-approved advisory DB access; only three unmatched-license-allowance warnings |
| `cargo llvm-cov -p cancellai-policy --lib --summary-only` | PASS: `retention.rs` 95.58% line coverage |
| Real target CodeQL Rust run / Cargo Dependabot proposal | FAIL: target is not present on GitHub; latest run is the Python-only pre-target commit |
| Real tag dry-run / controlled Windows-clippy replay of repaired release workflow | FAIL: no target workflow run/tag exists |

The initial system-Python Ruff/mypy invocations failed because those modules are not installed
there; the pinned repository `.venv` supplied the successful required gates. The initial
sandboxed cargo-deny run could not lock the read-only advisory database and passed when rerun
with the required approval.

## Overall verdict

**FAIL - round 1 of at most 2.** E22-S01, E22-S02, and E22-S04 return to `in_progress`;
E22-S03 and E22-S05 also have their own required repairs but are `blocked` by their failed
dependencies, as the control-plane dependency rules require. E22-S06 is done. The epic remains
open with one review round remaining; no release is cut in this round.

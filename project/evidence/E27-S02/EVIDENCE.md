# Evidence Packet - E27-S02

- Commit/PR: the coverage ratchet on `rework/codebase-health`
- Executor: Claude
- Independent verifier: none - branch work, reviewed by the owner before merge
- Change Risk: CR1
- Spec version/commit: `project/epics/E27.json` at this commit

## Outcome

PASS

## The measurement this story exists to hold

First per-crate coverage in the project's history. The workspace average was already known from
E27-S01; the distribution was not, and the distribution is the part that matters.

| Crate | Regions | Gated |
| --- | --- | --- |
| `cancellai-safety` | 98.23% | yes |
| `cancellai-policy` | 98.16% | yes |
| `cancellai-platform` | 95.83% | yes |
| `cancellai-model` | 95.79% | yes |
| `cancellai-inventory` | 95.09% | yes |
| **`cancellai-sealedfs`** | **92.54%** | yes - lowest of the gated set, and the crate holding every `unsafe` block |
| `cancellai-provider-api` | 95.37% | no |
| `cancellai-tui` | 94.86% | no |
| `cancellai-cli` | 90.61% | no |
| `cancellai-provider-codex` | 88.76% | no |
| `cancellai-provider-claude` | 83.88% | no |
| `cancellai-guardian` | 0.00% | no - a declared 16-line skeleton (E02-S01) whose `main` prints "not yet implemented" |

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - recorded and compared against a real measurement | `project/coverage_baseline.json`, written by `check_coverage.py record` from a real `cargo llvm-cov --json` run. The gate re-measures; it never reads a number a human typed. | PASS |
| AC2 - a fall is refused, with both numbers | Shown failing before shown passing: setting the sealedfs floor to 99.0 produced `cancellai-sealedfs: region coverage fell to 92.54% from a recorded 99.00%`, and re-recording cleared it. `test_a_real_fall_is_refused_and_names_both_numbers`. | PASS |
| AC3 - improvement does not tighten the floor silently | An above-floor crate produces a note suggesting `record`, never an automatic rewrite. A ratchet that tightens itself is a number nobody chose; one that loosens itself is worse. `test_improvement_is_reported_rather_than_recorded_silently`. | PASS |
| AC4 - a missing crate is refused | `test_a_ratcheted_crate_that_stopped_building_is_refused`. A crate that vanished from the report has not achieved 100%; it has stopped being measured, which is the reading a naive gate gets wrong. | PASS |
| AC5 - tolerance tested from both sides | 0.5 points. Region counts shift when a function is split or a match arm reordered, with no test lost, and failing on that teaches reflexive re-recording - which turns a ratchet into a rubber stamp. `test_noise_below_the_tolerance_is_not_a_regression` and `test_the_tolerance_is_not_a_way_through` pin 0.4 and 0.6. | PASS |
| AC6 - a declared skeleton is not gated | `cancellai-guardian` is 16 lines printing "not yet implemented". Its 0% is correct, and `test_a_declared_skeleton_is_not_ratcheted` keeps it out of the gated set - a gate that called that a crisis would be measuring the wrong thing. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | The gate passing because it could not measure | `measure()` raises when `cargo` is absent or `cargo llvm-cov` fails, and `main` returns 2 on that error rather than reporting success. A coverage gate that cannot measure and passes anyway is the same defect this repository fixed in the risk gate for shallow clones. | PASS |
| n/a | Gating the wrong crates | The gated set is ADR-0019's kernel ring plus the two crates that decide eligibility and completeness. The presentation and adapter crates are recorded and not gated: spending refusal on a rendering regression buys nothing and trains people to re-record. | PASS |

## Verification Commands

```text
python3 scripts/check_coverage.py record   -> 12 crates recorded from a real llvm-cov run
python3 scripts/check_coverage.py check    -> 6 gated crates at or above their floor
  (with a floor raised to 99.0)            -> error naming 92.54% against 99.00%
python3 -m pytest tests/test_coverage.py   -> 13 passed, 10 subtests
python3 scripts/check_workflows.py check   -> workflow policy intact with the new job
```

## Compatibility

- New gate and new CI job. No product behaviour, no schema, nothing in the shipped artifact.

## Performance / operability

- CI cost: one Linux job with an instrumented rebuild plus a `cargo install cargo-llvm-cov`. It is
  deliberately not in the three-OS `quality` matrix and deliberately not in `pre-commit`: at over a
  minute it would be disabled by the person it protects.

## Residual risks

- **A ratchet cannot say what coverage is worth having.** It only forbids getting worse. The
  sealedfs figure is the lowest of the gated set and this story does nothing to raise it; it
  freezes it, which is a smaller claim than it may look.
- **Region coverage is not behaviour coverage.** A line executed by a test that asserts nothing
  counts. `gate_sensitivity.py`'s seeded mutants are the check that catches that, and they cover
  four invariants of thirty-one.
- **The tolerance is a judgement.** 0.5 points was chosen to absorb refactoring noise, and nothing
  proves that number is right - only that both sides of it are tested.
- **CI installs `cargo-llvm-cov` from crates.io on every run**, which is a supply-chain surface
  `cargo deny` does not cover because it is tooling rather than a dependency. Pinned to an exact
  version and `--locked`, which is the mitigation, not a solution.

## Verifier verdict

pending - branch work

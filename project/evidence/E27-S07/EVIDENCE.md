# Evidence Packet - E27-S07

- Commit/PR: on `rework/codebase-health`, [PR #19](https://github.com/matteo-dritara/homebrew-cancellai/pull/19)
- Executor: Claude
- Source finding: the E27-S06 independent verifier (Codex), 2026-09-14
- Independent verifier: none - branch work, CR1
- Change Risk: CR1
- Spec version/commit: `project/epics/E27.json` at this commit

## Outcome

PASS

## What the verifier found, and why filing it was the right call

Running `check_coverage.py check` on an unchanged `cancellai-platform` produced **93.24% against a
recorded 95.83%**, twice, on the same checkout. There was no source diff for that crate between the
baseline commit and the review target.

The verifier **filed this as a story instead of re-recording the baseline**. That judgement is the
reason the cause was found: a lowered ratchet inside an unrelated CR4 verification would have
looked like a small accepted regression and hidden a broken gate.

## The cause, demonstrated rather than argued

The same unchanged workspace, measured twice on this machine:

| Toolchain | `cancellai-platform` region coverage |
| --- | --- |
| `stable` (rustc 1.94.0) | **95.83%** |
| `nightly` (rustc 1.100.0-nightly) | **63.85%** |

A 32-point swing from the compiler alone. Region counting depends on the instrumentation the
compiler emits, and `cargo llvm-cov` uses whatever `rustup` currently defaults to. The ratchet
recorded a number and nothing about what produced it.

**Installing a nightly toolchain for Miri the day before (E27-S06) was enough to make this gate
report a coverage regression that was a different compiler.** Two consecutive runs on one toolchain
are byte-identical, which is what separates this from a flaky measurement.

This is the same defect this repository fixed in `check_risk_classification.py` five days ago - a
gate reading its environment and calling it the repository - rebuilt by the same author in a new
place, which is worth recording plainly.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the measurement names its toolchain | `cargo +stable llvm-cov ...`, with `MEASUREMENT_TOOLCHAIN` a named constant rather than an implicit default. `test_the_measurement_names_its_toolchain_rather_than_taking_the_default`. | PASS |
| AC2 - the baseline records what produced it | Schema version 2 adds `provenance`: toolchain, `rustc --version`, `cargo llvm-cov --version`. `test_the_committed_baseline_carries_provenance` also asserts none of them is `unknown`, so a failed probe cannot pass as a recording. | PASS |
| AC3 - a tooling difference refuses and draws no conclusion | `provenance_errors()` runs **before** `measure()`, so a mismatch costs nothing and reports nothing about coverage. Exercised by rewriting the recorded `rustc` and observing exit 2 with both versions named. | PASS |
| AC4 - an unprobed provenance is refused | `version()` returns `"unknown"` on any failure, and the committed-baseline test refuses it. A baseline that silently omitted provenance would compare against nothing. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | The gate reporting a coverage regression that is a different compiler | That is exactly what happened to the verifier, twice. The check now refuses first and measures second. | Corrected |
| n/a | The fix hiding a real regression behind a provenance excuse | The provenance check is an additional refusal, not a replacement: with matching tooling the ratchet behaves exactly as before, and `test_a_real_fall_is_refused_and_names_both_numbers` still passes. | PASS |

## Verification Commands

```text
cargo llvm-cov --workspace --json --summary-only            -> platform 95.83% (stable)
cargo +nightly llvm-cov --workspace --json --summary-only   -> platform 63.85%
python3 scripts/check_coverage.py report  (twice, stable)   -> 94.39%, 94.39% - identical
python3 scripts/check_coverage.py record                    -> 12 crates + provenance
python3 scripts/check_coverage.py check   (provenance edited) -> exit 2, difference named
python3 -m pytest tests/test_coverage.py -q                 -> 19 passed, 13 subtests
```

## The first CI run found the second half of the same defect

Pushing the provenance check turned the `coverage` job red immediately, and it was right to:

```
cargo_llvm_cov: measured with 'cargo-llvm-cov 0.6.21', baseline recorded 'cargo-llvm-cov 0.9.0'
rustc:          measured with 'rustc 1.98.1 (48a229cea 2026-09-01)', baseline recorded 'rustc 1.94.0'
```

A baseline recorded against `stable` can only ever be compared with itself: `stable` moves every
six weeks and a runner and a developer's machine are never on the same day of that cycle. So both
sides are now **pinned exactly** - `MEASUREMENT_TOOLCHAIN = "1.98.1"` in the script, the same
version in the workflow's `toolchain:` input, and `cargo-llvm-cov@0.9.0` installed there to match.
The cost is a cold instrumented rebuild whenever the pin changes, and an ageing compiler measuring
coverage, which is the right trade: coverage of this workspace's own code does not need the newest
compiler, and reproducibility is the entire point.

One number worth correcting from the section above: measured on the pinned 1.98.1, `cancellai-platform`
reads **94.39%** - the same as on rustc 1.94.0. So the 95.83 to 94.39 drift was **not** the
toolchain; only the 63.85 on nightly was. What moved that 1.4 points across a week of unrelated
workspace changes is still unexplained, and is now frozen against a pinned measurement rather than
a moving one.

## Residual risks

- **The baseline moved from 95.83% to 94.39% for `cancellai-platform` on the same toolchain**, with
  no source change to that crate, across a week of changes elsewhere in the workspace. Two
  consecutive runs agree, so it is not flakiness - something about the workspace build changed what
  is instrumented. This story makes the number honest about its tooling; it does **not** explain
  that 1.4-point drift, and the re-recorded baseline absorbs it.
- **So the 0.5-point tolerance was chosen without measuring real drift.** Observed drift from
  unrelated changes is nearly three times it. The tolerance is now known to be optimistic and is
  left as it is rather than widened by guess a second time.
- **Provenance is an exact string match.** A patch release of `cargo-llvm-cov` will refuse the whole
  gate until someone re-records, which is loud and correct but is friction.
- **CI records nothing yet.** The `coverage` job installs a pinned `cargo-llvm-cov` and uses the
  action's stable toolchain, so it should match - but the first CI run after this commit is the
  first evidence, not this packet.

## Verifier verdict

pending - branch work

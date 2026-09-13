# E10 independent verifier review

- Review target: `b7fd880^..b7fd880`, evaluated on current `main`
- Verifier: Codex (independent of Claude executor/self-review)
- Date: 2026-09-13

## Verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E10-S01 | FAIL | Released CR2 declaration crossed CR4 unsafe-sealedfs floor and had no ADR for the added `statfs` role. Repaired and recorded as E10-S03 / ADR-0027. Conservative estimator behaviour itself passes. |
| E10-S02 | PASS_WITH_RESIDUALS | Linux RSS budget has a real assertion and non-degenerate workload; local macOS cannot execute its Linux-only module. |
| E10-S03 | PASS_WITH_RESIDUALS | Independent CR4 correction, ADR authority, Safety Verdict and native tests completed; Miri component unavailable locally. |

## Acceptance criteria

| Story | AC | Reproduction / evidence | Result |
| --- | --- | --- | --- |
| E10-S01 | AC1 | `estimate_reclaim` is Verified only for known allocations plus `NotKnownToShare`; unknown allocation, unsupported observation, APFS, and unknown filesystem tests downgrade. | PASS |
| E10-S01 | AC2 | Unknown/reflink/shared cases produce named `Estimated` reasons and do not substitute logical bytes. | PASS |
| E10-S02 | AC1 | Linux module asserts non-empty discovery then `VmHWM <= 128 MiB`; a lower injected budget would fail that assertion. | PASS_WITH_RESIDUALS |
| E10-S02 | AC2 | Ignored scheduled benchmark emits machine-readable latency/RSS trend values without applying a noisy RSS threshold. | PASS |
| E10-S03 | AC1 | `risk_floors.json` records E10-S01 CR4 independent classification. | PASS |
| E10-S03 | AC2 | ADR-0027 authorizes the sole unsafe-boundary observation and names limits. | PASS |
| E10-S03 | AC3 | NUL, missing path, signed bytes, invalid UTF-8 and conservative fallback were inspected/tested. | PASS_WITH_RESIDUALS |

## Findings and repair

F-A: `b7fd880` touched CR2 inventory, CR3 platform, and CR4 sealedfs paths while its stories
declared CR2/CR1. E10-S01 is independently CR4. F-B: the `statfs` FFI is ABI-correct for its
documented use and conservative in failure handling, but ADR-0017 did not authorize its second
role. ADR-0027 now carries that authority. F-C: E25-S15 repairs ambiguous history attribution:
multi-story commits are checked at their combined floor, and pre-trailer history is explicit
baseline data rather than omitted. The E10 batch links to this correction.

## Counterexamples exercised

- Unknown allocated size, unrecognised filesystem, clone-capable filesystem, unsupported
  detector, and empty set all downgrade or state their limits correctly.
- Missing path, interior NUL, signed `c_char`, invalid UTF-8, and symlink semantics were checked
  against the macOS `statfs` wrapper; missing paths error and invalid names cannot become known.
- Mixed-filesystem scope is inapplicable by design: scope is device-bounded (SI-018). A vanished
  file is handled by scan completeness upstream and is not fabricated into allocation accounting.
- Linux RSS gate has an executable threshold but could not run on this macOS host; Linux CI is
  required evidence. Crash/retry/provider drift and large dataset axes are inapplicable to the
  pure estimator; scheduled performance coverage owns the large-dataset axis.

## Mutation results

The risk-gate repair has three focused mutants/counterexamples: lowering a CR1/CR4 batch to the
lowest declaration is caught, a reasoned historical baseline is visible, and an empty baseline
reason is rejected. No code mutant survived. Existing estimator tests catch logical-size
substitution, removal of clone downgrade, and removal of unsupported-detector downgrade.

## Gates

- Focused reclaim/platform/sealedfs/TUI tests: PASS.
- Workspace fmt/clippy/check/test, `cargo deny`, and Windows/Linux cross-target clippy: PASS;
  deny emits existing non-fatal duplicate/unmatched-license warnings.
- Full Python suite: PASS (500 tests). Repository gates and pre-commit are rerun after final
  record generation and release preparation.
- `cargo miri test -p cancellai-sealedfs --lib macos_filesystem`: not runnable because Miri is
  unavailable on the active stable macOS toolchain; no toolchain was installed.

## Documents opened

`AGENTS.md`, `docs/INDEX.md`, `docs/CONSTITUTION.md`, `docs/development/ENGINEERING_SYSTEM.md`,
`docs/development/AGENT_PROTOCOL.md`, `docs/development/WORK_ITEM_MODEL.md`,
`docs/development/RELEASE_GATES.md`, `docs/architecture/PLATFORM_MODEL.md`,
`docs/architecture/PERSISTENCE_MODEL.md`, `docs/security/SAFETY_INVARIANTS.md`,
`docs/security/THREAT_MODEL.md`, `docs/adrs/0015-rust-workspace-toolchain-and-repository-layout.md`,
`docs/adrs/0017-sealed-root-handle-for-configuration-writes.md`, `project/epics/E10.json`,
`project/epics/E25.json`, `project/evidence/E10-SELF-REVIEW.md`, and
`project/evidence/E09-SELF-REVIEW.md`.

## Limitations

Miri and the Rust 1.88 toolchain are unavailable locally by owner policy. Windows is explicitly
unsupported for filesystem-name observation; Linux-specific RSS execution is deferred to CI.

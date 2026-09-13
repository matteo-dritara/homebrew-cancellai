# E09 independent verifier review

- Review target: `7fc6cd2^..c0549a8`, evaluated on `main` after ratatui 0.30.2 upgrade
- Verifier: Codex (independent of Claude executor/self-review)
- Date: 2026-09-13

## Verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E09-S01 | PASS | `cancellai-tui` has no direct filesystem/provider dependency or source access; current terminal, navigation, ASCII, colour and small-frame tests pass. |
| E09-S02 | PASS | Atlas renders logical and reclaimable totals separately and displays incomplete scans without zeroing their observed totals. |
| E09-S03 | PASS | Explain rendering includes policy reason and low-confidence differentiation from engine `ExplainView` facts. |
| E09-S04 | PASS_WITH_RESIDUALS | SI-016 holds structurally: no safety/platform dependency, no executor call, and confirmation is terminal review state only. See `E09-S04/SAFETY_VERDICT.md`. |
| E09-S05 | PASS | Current Cargo resolver/MSRV configuration and successful recent `rust.yml` CI evidence preserve the contract; local 1.94 compiler cannot exercise 1.88. |

## Acceptance criteria

| Story | AC | Reproduction / evidence | Result |
| --- | --- | --- | --- |
| E09-S01 | AC1 | `Cargo.toml` and `rg` find no provider/filesystem/mutation access; mutation-boundary gate passes. | PASS |
| E09-S01 | AC2 | Empty UI action state is explicit; real data enters through policy query models. | PASS |
| E09-S01 | AC3 | `navigation` plus capability/render tests cover keyboard, ASCII/no-colour, and undersized terminal fallback. | PASS |
| E09-S02 | AC1 | `atlas_screen_shows_logical_and_reclaimable_as_visually_distinct_labeled_values`. | PASS |
| E09-S02 | AC2 | `atlas_screen_surfaces_an_incomplete_scan_prominently_without_hiding_the_totals`. | PASS |
| E09-S03 | AC1 | `explain_screen_surfaces_the_real_destructive_reason_for_a_recommended_action`. | PASS |
| E09-S03 | AC2 | `explain_screen_visibly_differentiates_low_confidence_data`. | PASS |
| E09-S04 | AC1 | No mutation route exists; confirmed state renders CLI handoff only. | PASS |
| E09-S04 | AC2 | One press for quarantinable, two plain `c` presses for irreversible, with disarm/reset tests. | PASS |
| E09-S05 | AC1 | Workspace uses resolver 3 and current stable Rust quality checks pass. | PASS |
| E09-S05 | AC2 | Current lock resolution is accepted by `cargo deny`; recent `rust.yml` MSRV legs are successful. | PASS |
| E09-S05 | AC3 | Resolver configuration makes an incompatible update fail at resolution instead of silently selecting it. | PASS |
| E09-S05 | AC4 | Full Rust quality suite passed locally except MSRV itself, which has current successful CI evidence. | PASS_WITH_RESIDUALS |

## Counterexamples exercised

- Policy conflict: irreversible `ObservationOnly` and `NotEvaluated` views cannot confirm.
- Selection and event concurrency: changing selection, leaving Plan, an arbitrary key, Ctrl+C,
  repeat `c`, empty data, and modulo-wrapped selection all preserve non-execution semantics.
- Boundary values: tiny terminal, ASCII-only/no-colour terminal, and all keyboard screen paths.
- Mutation/path/permissions/symlink/mount/crash/provider-drift/large-data axes are inapplicable
  to this TUI-only crate: it has no filesystem traversal, provider operation, persistent state,
  or execution capability. The future live-data boundary is a residual, not assumed safe.

## Ratatui upgrade assessment

E17-S08 changed ratatui API signatures and upgraded to 0.30.2. The current dependency graph and
focused render/navigation suite show no behavioural change to E09's contract: all TUI inputs
remain engine query data and all confirmation remains a local handoff state.

## Gates

- `cargo fmt --check`, workspace clippy/check/test, `cargo deny check`, and Windows/Linux target
  clippy: PASS (deny has existing duplicate/unmatched-license warnings only).
- Python suite: PASS (500 tests); repository gates and pre-commit are rerun after the final
  review records and release preparation.
- MSRV 1.88: not installed locally by owner policy; current `main` CI `rust.yml` MSRV legs are
  successful and are the compatibility evidence.

## Documents opened

`AGENTS.md`, `docs/INDEX.md`, `docs/CONSTITUTION.md`, `docs/development/ENGINEERING_SYSTEM.md`,
`docs/development/AGENT_PROTOCOL.md`, `docs/development/WORK_ITEM_MODEL.md`,
`docs/development/RELEASE_GATES.md`, `docs/architecture/TARGET.md`,
`docs/architecture/POLICY_MODEL.md`, `docs/security/SAFETY_INVARIANTS.md`,
`docs/security/THREAT_MODEL.md`, `docs/adrs/0015-rust-workspace-toolchain-and-repository-layout.md`,
`project/epics/E09.json`, and `project/evidence/E09-SELF-REVIEW.md`.

## Limitations

No live scan is wired into the shipped TUI, so a refresh race cannot be executed today. Miri does
not apply to the safe TUI code. The original macOS-only manual terminal check cannot establish
Linux/Windows terminal behaviour; cross-target compilation and CI remain the available evidence.

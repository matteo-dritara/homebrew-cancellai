# Independent Verifier Review — E25-S16

Verifier: Codex
Brief-Checksum: 4f9584598ad195248f14756741ce995bf37197f868d50a205e1f2c918dc6de81
Date: 2026-09-15

## Verdict

REPAIRED

The original repair moved a fixed anchor from E11-S01 to E19-S02. That did not cure the defect
class: E19-S02 is mutable backlog state, and its nine unmet dependencies meant the forged `done`
status was refused for dependencies before `project_os` reached the missing-evidence obligation.
The sensitivity report therefore credited `project-os` without showing the claim in the row.

The repair selects a planned story in the throwaway export at run time, clears its dependencies,
and forges `done`. It has no fixed backlog identifier to go stale; if no planned story remains it
raises `SensitivityError` rather than reporting coverage. The independent regression invokes the
actual checker on the mutated export and requires `requires committed evidence` in its diagnostic.

## Reproductions and verification

- The pre-repair E19-S02 mutant changed a story with unmet dependencies, so the dependency-status
  check ran before the evidence check.
- `python3 -m pytest tests/test_governance_extras.py tests/test_blocked_reasons.py tests/test_work_item_references.py -q` — 61 passed, 182 subtests passed after the repair.
- `python3 scripts/gate_sensitivity.py check` — 11 mutants killed; clean-tree control passed.

## Method defects

- **What happened**: E25-S16's original AC required a fixed E19-S02 anchor and its verification
  asserted only anchor presence/uniqueness. It did not require the mutant to reach the evidence
  obligation it claimed to test, so a dependency failure could satisfy the intended patch while
  leaving the silent-loss class intact. **Prevented by**: an acceptance criterion and adversarial
  test that assert the expected rejecting diagnostic. **Disposition**: proposed 2026-09-15.

## Residual risks

- The dynamic mutation intentionally depends on at least one planned story existing. Its absence is
  a loud harness failure, not evidence that the claim remains covered; a completed backlog will
  need a deliberate fixture or a revised mutant.

<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E29-S05
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: 07575fd4d09140c2d03085360a1d0825052e709a2b3b0b46d499e58969b0dbd8

<!-- end handoff header -->
# Verifier Brief - E29-S05 - A user-scope component's cost is measured where it is installed

Status: ready_for_review | Change Risk: CR2
Outcome: E28-S04 measures a component's always-on cost and ten of twelve components return `unmeasured`, because they live at user scope and CI cannot see them. The state is honest and it leaves most of the context budget resting on hand-entered numbers - the same budget that decided against carrying Task Observer. A measurement taken on a machine where the component is installed can be recorded the way `project/coverage_baseline.json` records the toolchain that produced it: dated, bound to a version, and refused when the provenance no longer matches.
Dependencies: none

## Acceptance Criteria
- A locally taken measurement shall be recordable, bound to the component version and the date it was taken.
- If a recorded measurement's component version no longer matches the manifest, then the gate shall report it as stale rather than treat it as current.
- A component with no recorded measurement shall still report as unmeasured, because the absence of a measurement is not a measurement of zero.
- The gate shall not require a local measurement to pass, so CI remains able to run without one.

## Verification Contract
- A stale measurement is shown reported as stale, and a current one as current.
- CI is shown to pass with no local measurements recorded at all.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_TOOLCHAIN.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E06-S08
Rendered-by: Claude
Rendered-on: 2026-09-23
Brief-Checksum: cf517c1d13964dbcdef87178e6188718da266910088bb8de8a965bf193971b53

<!-- end handoff header -->
# Verifier Brief - E06-S08 - The Rust CLI stays within a performance self-budget measured against the reference

Status: ready_for_review | Change Risk: CR1
Outcome: G4 names a performance self-budget for the CLI's own command paths as unaddressed. E21-S05 retargeted the benchmark onto the shipped resolution path and E21-S06 bounded rollout reads, but nothing compares the command a user runs against the engine it replaces. The owner chose the criterion: on a large synthetic corpus, the Rust CLI is no slower than the frozen Python reference for status, inspect, plan and a dry clean, and never exceeds a fixed resident-memory ceiling. Measured in CI on Linux; reported on macOS and Windows.
Dependencies: none

## Acceptance Criteria
- The system shall measure wall time and peak resident memory of status, inspect, plan and a dry clean for both the Rust CLI and the Python reference on the same generated synthetic corpus.
- If the Rust CLI is slower than the reference on any measured command beyond a recorded noise tolerance, or exceeds the resident-memory ceiling, then the gate shall fail on Linux CI.
- If a measurement runs against a corpus that resolved no artifacts, then the gate shall fail rather than report a fast empty run.
- The system shall report the same measurements on macOS and Windows without gating on them.

## Verification Contract
- The gate is shown failing against a deliberately slowed build before it is shown passing.
- The corpus generator is synthetic and writes nothing outside a temporary directory.

## Safety Obligations
- none

## Documentation Impact
- docs/development/RELEASE_GATES.md
- docs/development/VERIFICATION_STRATEGY.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

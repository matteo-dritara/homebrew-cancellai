<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E06-S09
Rendered-by: Claude
Rendered-on: 2026-09-23
Brief-Checksum: 23b452f21a1d947034bbecf63ba33556975541f80ee529e1f9dd0a033619ecf3

<!-- end handoff header -->
# Verifier Brief - E06-S09 - clean survives being killed at every mutation point

Status: ready_for_review | Change Risk: CR3
Outcome: G4's other open item is crash and recovery: nothing beyond unit tests shows what a real process kill in the middle of clean leaves behind. The owner chose a real-kill harness: clean is killed (SIGKILL, TerminateProcess) at each point on the mutation path, and what remains is checked for unsafe partial state, a consistent ledger, and an idempotent rerun. It runs on macOS, Linux and Windows CI.
Dependencies: none

## Acceptance Criteria
- The system shall provide a harness that kills a running clean at each enumerated point on its mutation path on macOS, Linux and Windows.
- If clean is killed at any enumerated point, then no artifact outside the plan shall be changed, every planned artifact shall be either intact or fully removed, and no partially written state file shall be read as valid.
- When clean is rerun after a kill, the system shall complete the remaining plan without error and without acting twice on an already removed artifact.
- If the harness cannot reach an enumerated kill point, then the run shall fail rather than count it as survived.

## Verification Contract
- Each kill point is shown reached by a marker the harness observes before killing.
- A planted defect that leaves a half-written state file is shown caught by the harness.

## Safety Obligations

### SI-019 One mutation boundary, evidence-gated

All filesystem/vendor mutations route through the safety executor. CR4 changes to this boundary require independent verification and owner-visible Safety Verdict.

Implemented at `rust/crates/cancellai-safety/src/mutation_executor.rs::execute` (E03-S05),
the sole production caller of `cancellai-platform::mutation::MutationExecutor`.
`scripts/check_mutation_boundary.py` statically enforces that the raw OS primitive and the
capability wrapping it are referenced only from those two files - E03 verifier review round 1
found the capability itself was `pub`, re-exported at `cancellai_platform`'s crate root, and
directly callable (with an unconstrained raw path) by any crate that imported it; repaired by
removing the re-export and extending the static check (`docs/architecture/TARGET.md`,
`docs/architecture/PLATFORM_MODEL.md`).

## Documentation Impact
- docs/development/RELEASE_GATES.md
- docs/development/VERIFICATION_STRATEGY.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

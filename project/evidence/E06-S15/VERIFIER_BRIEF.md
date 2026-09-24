<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E06-S15
Rendered-by: Claude
Rendered-on: 2026-09-24
Brief-Checksum: 13575c2d3f9fdea6c85190d0e7daae57f1299d780b4857a7236d69da5e915ab3

<!-- end handoff header -->
# Verifier Brief - E06-S15 - The cutover is adopted on the owner's explicit authorization, bound to the Safety Verdict it accepts

Status: ready_for_review | Change Risk: CR4
Outcome: finalize --adopt-cutover read E06-S04's done status as the owner's acceptance of the migration, so a status edit stood in for a decision (ADR-0040). It now requires project/evidence/E06-S04/CUTOVER_AUTHORIZATION.md naming the version and the SHA-256 of the SAFETY_VERDICT.md the owner accepted, whose final round must pass.
Dependencies: none

## Acceptance Criteria
- When finalize adopts the cutover, the system shall require an owner authorization that names the version being adopted and the SHA-256 of the migration Safety Verdict the owner accepted.
- If the Safety Verdict has changed since it was authorized, or its final round does not pass, or the authorization names another version, then the system shall refuse the adoption and leave the live formula unchanged.
- The system shall not treat any story status as the owner's authorization.

## Verification Contract
- Tests: a valid authorization adopts; a missing one, one for another version, one whose Safety Verdict was edited afterwards, and one whose final round fails each refuse with the formula unchanged.

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

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

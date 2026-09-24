<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E06-S04
Rendered-by: Claude
Rendered-on: 2026-09-24
Brief-Checksum: 11ce7ef3ab9b4803f9c1b4872960d5c466faef7cbfed0e13e3969de8020280ea

<!-- end handoff header -->
# Verifier Brief - E06-S04 - Canonical engine switch

Status: ready_for_review | Change Risk: CR4
Outcome: Promote Rust to stable only after functional, safety, compatibility, and operability gates pass. ADR-0040 (2026-09-24) moved the release mechanics out of this story: E06-S14 makes the formula a function of the release manifest rendered where the bytes are built, and E06-S15 adopts the cutover on the owner's explicit authorization. What stays here is the owner's: accepting the migration Safety Verdict, the transition window and the release notes.
Dependencies: E06-S03, E21, E22-S01, E06-S06, E06-S07, E06-S08, E06-S09, E06-S10, E06-S11, E06-S12, E06-S13, E06-S14, E06-S15

## Acceptance Criteria
- The independent migration Safety Verdict passes; the owner's acceptance of that verdict is then recorded as project/evidence/E06-S04/CUTOVER_AUTHORIZATION.md, bound to its SHA-256 (E06-S15), before the story is done and before the cutover is adopted.
- Python remains tagged/archiveable as reference for at least one transition window.
- Release notes state any intentional contract change.
- The blockers recorded in docs/development/RELEASE_GATES.md's cutover checklist are closed or explicitly accepted by the owner: E21 (target-engine scan-completeness authority), E22-S01 (a release workflow that actually verifies the Rust engine), and the platform/packaging prerequisites E20-S01 and E17 - or a scoped cutover perimeter decided by ADR.

## Verification Contract
- Full release evidence packet and independent verifier sign-off.
- A native reproduction on each platform inside the decided cutover perimeter, using the E21-S02 partial-scan fixtures, showing the engine withholds where the frozen reference withholds.

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
- CHANGELOG.md
- docs/development/RELEASE_GATES.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

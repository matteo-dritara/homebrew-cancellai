<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E06-S16
Rendered-by: Claude
Rendered-on: 2026-09-25
Brief-Checksum: 9bdf85e0a19b367c0724580601f62c343c5f77c2c60be30e739e501dcc18b482

<!-- end handoff header -->
# Verifier Brief - E06-S16 - A rehearsal release runs the real pipeline and is verified on its published assets

Status: ready_for_review | Change Risk: CR3
Outcome: The release mechanism E06-S14 built had only ever been verified on simulations; every review of E06-S04 recorded that as a residual, and the real release could only follow the story it was meant to prove (owner decision 2026-09-25). A non-cutover fix release, v1.21.1, runs release.yml end to end, and `release.py verify-release` checks its published assets exactly as finalize would - without changing what Homebrew installs.
Dependencies: E06-S14

## Acceptance Criteria
- The system shall provide `release.py verify-release --version <v>`, which applies every check finalize applies to a published release - closed manifest, local and remote tag, all four archives' bytes and provenance, the release run, and cancellai.rb against the rendered formula - and writes nothing.
- When v1.21.1 is published by release.yml, the system shall verify it with verify-release on its real assets, and the result shall be recorded in its release evidence.
- If verify-release finds any published asset that differs from what finalize would accept, then it shall refuse and name the asset, and the cutover shall not proceed until the difference is repaired.

## Verification Contract
- Unit tests for verify-release on an engine and a pre-cutover release; the real v1.21.1 verify-release output recorded in project/evidence/RELEASE-v1.21.1.md.

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
- docs/RELEASING.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

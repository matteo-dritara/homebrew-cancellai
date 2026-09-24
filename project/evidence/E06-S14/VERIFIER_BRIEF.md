<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E06-S14
Rendered-by: Claude
Rendered-on: 2026-09-24
Brief-Checksum: fa4a751b449e2ed74da03c05db7ca490cf6c54f8bf5e6cdf05089a7c0eabcbda

<!-- end handoff header -->
# Verifier Brief - E06-S14 - The release formula is a function of the release manifest, rendered where the bytes are built

Status: ready_for_review | Change Risk: CR4
Outcome: Nine review rounds found holes in release.py's post-hoc reconstruction of what the formula installs (ADR-0040). The release workflow now renders the Homebrew formula from the verified release manifest and the tag archive's digest and publishes it as the release asset cancellai.rb; finalize adopts that asset only if it is exactly what render_formula produces from the published manifest, after downloading every engine archive it names, hashing it to the manifest's digest and verifying its build provenance.
Dependencies: E22-S01

## Acceptance Criteria
- When a release is published, the system shall render the formula from the release manifest after the manifest's checksums have been verified against the built archives, and publish it as the release asset cancellai.rb.
- The system shall adopt a published formula only if it is byte-identical to the formula rendered from the published release manifest and the tag archive's digest.
- If any engine archive the formula names does not hash to the manifest's digest, fails build-provenance verification, or cannot be downloaded, or if the manifest names another version or a target more or less than once, then finalize shall refuse and leave the live formula unchanged.

## Verification Contract
- Adversarial tests with a simulated release: altered archive, altered manifest, formula asset differing by one byte, manifest for another version, duplicated or missing target, provenance failure, each missing asset; each refuses and leaves the formula byte-identical.

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
- docs/security/SUPPLY_CHAIN.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

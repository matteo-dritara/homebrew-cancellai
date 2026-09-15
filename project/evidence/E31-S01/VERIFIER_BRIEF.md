<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E31-S01
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: de65386747804f8bee70013ff00471bb814887858f90e5753a2f1aa465266999

<!-- end handoff header -->
# Verifier Brief - E31-S01 - A work-item id in prose names something that exists

Status: done | Change Risk: CR1
Outcome: A document may cite any story or epic and nothing checks that the citation resolves. The gate that validates this repository's documentation already refuses a broken local link and an unknown `SI-0xx`, and work-item ids - the references these documents lean on hardest - were the one class it did not read. Measuring first found 424 distinct references and exactly one that does not resolve: `E07-S06` in PLATFORM_MODEL.md, which is a deliberate historical citation - `E20-S04 (formerly E07-S06)`, from when E07 and E20 were split. That single case is the whole design constraint: a gate that refuses it would be wrong, and a gate that cannot tell it apart from a typo is worthless. A retired identifier is therefore recorded once, with what it became and when, rather than being guessed at from the surrounding prose.
Dependencies: none

## Acceptance Criteria
- Every work-item id in non-generated documentation shall resolve to an epic or story in the control plane, and the gate shall refuse one that does not, naming the document and the id.
- An identifier that existed and was renamed or removed shall be recordable once, with what it became and the date it was recorded, and shall then pass wherever it is cited.
- If a recorded retirement names a replacement that does not itself resolve, then the gate shall refuse, so the record cannot rot into the same defect it exists to prevent.
- Generated documentation shall be excluded, because it is produced from the control plane and cannot disagree with it.
- A citation of a cancelled work item shall pass, because cancelling a story does not unwrite the documents that explain why.

## Verification Contract
- The gate is shown refusing an invented id planted in a real document before it is shown passing on the committed corpus.
- The one real historical reference is shown failing without its record and passing with it.
- A retirement pointing at a replacement that does not resolve is shown to be refused.
- The seven documents citing the cancelled E07-S07 are shown to pass unchanged.

## Safety Obligations
- none

## Documentation Impact
- docs/development/ENGINEERING_SYSTEM.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

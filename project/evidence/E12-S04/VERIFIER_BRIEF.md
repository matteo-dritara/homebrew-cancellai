<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E12-S04
Rendered-by: Claude (orchestrator, E12/E13/E14 review coordination)
Rendered-on: 2026-09-20
Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12

<!-- end handoff header -->
# Verifier Brief - E12-S04 - Purge and tombstones

Status: ready_for_review | Change Risk: CR4
Outcome: Permanently purge only within authority ceilings and preserve a minimal contentless tombstone.
Dependencies: E12-S01, E13-S02

## Acceptance Criteria
- Tombstones contain no prompts/source/file contents.
- Irreversible purge is distinguishable from vendor-native conditionally reversible operations.

## Verification Contract
- Privacy field allowlist test and purge evidence tests.

## Safety Obligations

### SI-020 Irreversible actions are explicit and stronger-gated

Purge/permanent vendor delete is represented separately from reversible/conditionally reversible actions and cannot be disguised as cleanup metadata.

`mutation_executor::execute` (E03-S05) enforces this directly: `authority.rs`'s
`minimum_authority_for(ActionClass::Delete)` requires `AuthorityLevel::Govern` (strictly above
`Quarantine`'s requirement), and `reversibility_allowed` refuses a `Delete` action class
unless the plan's own recorded `Reversibility` is `Irreversible` - a plan claiming
`Reversibility::Quarantinable` while carrying `ActionClass::Delete` is refused outright, not
executed as a disguised irreversible deletion (E03 verifier review round 1 found `execute`
originally checked neither authority nor reversibility at all).

## Provider and knowledge trust

## Documentation Impact
- docs/architecture/PERSISTENCE_MODEL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

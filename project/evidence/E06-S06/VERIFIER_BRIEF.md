<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E06-S06
Rendered-by: Claude
Rendered-on: 2026-09-23
Brief-Checksum: 817de404f11c12559e5d15fc1f1c11e17aa4731db38847bbb757505ef9c96b65

<!-- end handoff header -->
# Verifier Brief - E06-S06 - An independent verifier confirms the E21 authority repairs

Status: planned | Change Risk: CR4
Outcome: G2 of the cutover checklist is open for one reason: E21's round-1 independent review found the scan-completeness authority defect, the executor repaired every finding and pinned each with a regression written against the verifier's own reproduction, and the owner then closed E21 without spending a second round on those repairs (project/evidence/E21-CLOSURE.md). Repaired-by-executor is not independently-confirmed-repaired, and that act has had no work item to carry it, which is why it lived only in E06-S04's blocker prose. This story carries it: an independent adversarial pass over E21-S03 (scan completeness makes the scope incomplete, never absent) and E21-S07 (handle-relative unlink) as they stand in the code the cutover would ship, not as they stood at E21's closure.
Dependencies: none

## Acceptance Criteria
- The system shall have an independent verifier's adversarial pass over the current E21-S03 and E21-S07 code, recorded as a Safety Verdict round against a committed verifier brief.
- If any directory, companion payload or project the scan cannot read exists under a provider root, then the Rust CLI shall withhold every destructive action in that scope and exit as the frozen reference does, on every platform in the cutover perimeter.
- If a path component is swapped between validation and unlink, then the unlink shall not reach the substituted target.
- If the pass finds a defect, then E06-S04 shall stay blocked on it and the finding shall become a backlog item carrying its own story id.

## Verification Contract
- A Codex verifier round produces E21-S03/E21-S07 Safety Verdict sections answering a brief rendered by scripts/verifier_handoff.py.
- The E21-S02 partial-scan fixtures (codex-partial-tree, claude-partial-project) are rerun through the differential gate in both root-origin scenarios on the reviewed commit.

## Safety Obligations

### SI-008 Partial scan is non-destructive

A `PARTIAL` inventory scope cannot produce irreversible actions for artifacts whose safety depends on the missing information.

Implemented for the artifact-integrity axis at `rust/crates/cancellai-safety/src/authority.rs`
(E03-S04): `lifecycle_authority` collapses to `Recommend` (non-destructive) whenever
`IntegrityState` is `Partial`, `Corrupted`, or `Unknown`, independent of every other input.

E04-S03 (`rust/crates/cancellai-inventory/src/completeness.rs`) implements the inventory-scope
half of this invariant: `derive_completeness` classifies a scope `Partial` whenever any
directory-listing failure or per-file degraded observation exists beneath an otherwise
readable root, and `planning_view` is the *only* way to obtain planning-facing candidates -
it always returns them bundled with that `ScopeCompleteness` in one `PlanningView` struct,
so a caller cannot reach candidates without also seeing whether they were produced under a
`Partial` scope. Wiring `ScopeCompleteness::Partial`/`Unknown` into `KnowledgeConfidence` so
`authority.rs`'s existing `IntegrityState`-based collapse actually fires for these scopes is
E05/E06 scope (no classification stage exists yet to make that connection) - see this
story's evidence packet for the residual.

### SI-009 Unknown scan state is non-destructive

Missing evidence is not interpreted as absence of active/protected data.

Implemented at `rust/crates/cancellai-safety/src/authority.rs` (E03-S04): `lifecycle_authority`
also collapses to `Recommend` for `ActivityState::Unknown` and `IntegrityState::Unknown`
specifically - an unknown fact is never read as "safe to act on."

E04-S03 implements the inventory-scope evidence this depends on: `derive_completeness`
returns `ScopeCompleteness::Unknown` when a scope's own root could not be observed at all
(absent or unreadable) - the strongest form of missing evidence this model expresses,
reserved specifically for "we know essentially nothing about this scope" rather than
conflated with the more common `Partial` case (a readable root with some unreadable
descendant).

### SI-010 Scan errors are visible

Permission/I/O/disappearance failures are represented as explicit evidence/diagnostics and are not collapsed silently into zero size or empty state when that could affect safety.

## Concurrency and identity

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

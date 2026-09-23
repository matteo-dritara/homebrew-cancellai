<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E06-S12
Rendered-by: Claude
Rendered-on: 2026-09-23
Brief-Checksum: 74c57318540c3cddc9de9c795d078cdc82f0d051c2935ab0169c875153ce2006

<!-- end handoff header -->
# Verifier Brief - E06-S12 - Codex lineage I/O errors and Claude companion error retention fail closed

Status: ready_for_review | Change Risk: CR4
Outcome: E06-S06's independent pass (round 1, FAIL) found that an unreadable Codex rollout can be planned for deletion while its scope reports complete, because lineage open/read errors become None without a completeness reason. Claude companion traversal also accumulates unbounded failure reasons before applying ReasonLog retention. Repair both E21-S03 authority gaps and independently verify that failures remain visible, bounded and non-destructive.
Dependencies: none

## Acceptance Criteria
- If Codex lineage content cannot be opened or read after its entry metadata was observed, then the Codex scope shall record the path and cause as incomplete and emit no destructive action, while successfully read content with no recognized parent remains a valid no-parent result.
- If Claude companion traversal encounters more than MAX_RETAINED_REASONS failures, then retained reasons shall remain bounded, the total error count shall remain exact, the scope shall be Partial and every destructive action in the scope shall be withheld.
- The unreadable Codex rollout and high-error-count Claude companion cases shall match the frozen Python reference in default and custom root-origin scenarios.
- The E21-S07 fstatat/unlinkat leaf-entry race shall have an owner-visible disposition before E06-S04 can close.

## Verification Contract
- Native CLI regressions exercise unreadable Codex lineage content after metadata observation and assert incomplete scan, zero delete actions and reference parity.
- Adapter-level Claude regression injects more than 64 failures and asserts bounded retained reasons, exact total and complete withholding.
- An independent CR4 verifier reviews the repair and the owner records a disposition for the documented unlink residual.

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

A failure to read an artifact's *content* counts as much as a failure to list its directory. A
Codex rollout whose lineage cannot be read is not a rollout with no parent: it changes which
sessions are independent safety units, so the read error is a completeness reason and the scope
is `Partial` (E06-S12, after E06-S06's independent pass found the Rust engine reporting that scope
complete). Recording is bounded as well as visible: every adapter writes each failure straight
into the scope's `ReasonLog`, which retains at most `MAX_RETAINED_REASONS` and counts the rest
exactly, rather than buffering them first (E06-S12).

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
- docs/security/SAFETY_INVARIANTS.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

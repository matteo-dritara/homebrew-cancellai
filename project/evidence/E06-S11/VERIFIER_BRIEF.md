<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E06-S11
Rendered-by: Claude
Rendered-on: 2026-09-23
Brief-Checksum: bde99837a9229b3695eddc8640ca40dd99cf399dcd69e0bee0be60a791309271

<!-- end handoff header -->
# Verifier Brief - E06-S11 - clean --json always prints a document

Status: ready_for_review | Change Risk: CR2
Outcome: E06-S09's kill harness found that `cancellai-cli clean --json` prints a plain sentence - "Nothing to clean..." or "Nothing was cleaned: safety withheld..." - when no deletion is planned, where the reference prints a JSON document carrying the exit code and a `result` of `nothing-to-do` or `safety-withheld`. Automation that parses the output of the canonical engine would break on exactly the run that did nothing, including the one where safety withheld work. The exit code is already right; the document is missing.
Dependencies: none

## Acceptance Criteria
- When clean is given --json and no deletion is planned, the system shall print a result document in which every action is safely skipped, with a reason code that says whether there was nothing to do or safety withheld the work.
- If safety withheld the requested work, then the document's reason code shall say so and the exit code shall stay the safety-block code.
- When clean is given --json and --dry-run, the system shall print the plan document plan --json prints.
- The system shall keep the human-readable sentences for runs without --json.

## Verification Contract
- CLI tests parse the output of clean --json on an empty tree and on a tree whose scan is incomplete, and of clean --dry-run --json on a tree with work to do.

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

## Documentation Impact
- docs/CLI_RUST.md
- CHANGELOG.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

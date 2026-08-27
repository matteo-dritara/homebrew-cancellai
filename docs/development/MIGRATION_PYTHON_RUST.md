# Spec-First Python -> Rust Migration

## Why not rewrite immediately

The Python implementation contains valuable behavior and hard-won safety cases. A clean-slate rewrite without an oracle risks losing them. Conversely, continuing to place new product capability into the monolith would harden the wrong architecture.

The migration therefore treats Python as a temporary executable reference.

## Sequence

### M0 - P0 safety repair

Implement E00 only. No new product capability.

Exit: owner-visible Safety Verdict for the P0 trust floor.

### M1 - Extract contract

Implement E01:

- canonical vocabulary;
- synthetic provider/adversarial fixtures;
- versioned plan/result contracts;
- Python characterization;
- differential comparison rules.

### M2 - Freeze Python

Python becomes maintenance-only. Add a visible marker in `AGENTS.md`/project state. New features target Rust.

### M3 - Bootstrap Rust

Create workspace, quality gates, typed model, filesystem seams, safety kernel, inventory, and provider contract.

### M4 - Reference-provider parity

Claude and Codex adapters must satisfy normative fixtures and compatibility evidence.

### M5 - CLI parity

Rust status/inspect/plan/clean semantics meet versioned CLI/JSON contracts. No TUI is required for cutover.

### M6 - Differential gate

Every normative fixture runs Python and Rust. An unexplained semantic divergence blocks cutover. Intentional differences require accepted architecture/spec documentation.

### M7 - Beta side-by-side

Release candidate identifies engine/version clearly and preserves rollback. Local state migrations are reversible/rebuildable.

### M8 - Canonical switch

Rust becomes stable only after G1 Functional, G2 Safety, G3 Compatibility, and G4 Operability gates are green and owner accepts the migration Safety Verdict.

## What not to preserve

The migration does not preserve accidental implementation constraints:

- one-file architecture;
- macOS-only assumptions;
- repeated filesystem traversals;
- path-only identity;
- implicit scanner-based protection;
- legacy ambiguous CLI normalization;
- any known defect reproduced by the P0 audit.

## Rollback

During transition, release artifacts/tags make the last Python release available. Because cancellAI current-state DB is not provider truth, a failed Rust beta can be removed/reset without migrating provider data backward.

## Completion

After a defined transition window, Python may remain in repository history or a `reference/` tag/branch rather than the active source tree. Do not maintain two production engines indefinitely.

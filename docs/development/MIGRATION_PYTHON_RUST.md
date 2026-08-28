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

- canonical vocabulary ([`DOMAIN_MODEL.md`](../architecture/DOMAIN_MODEL.md));
- synthetic provider/adversarial fixtures ([`tests/fixtures/`](../../tests/fixtures/));
- versioned plan/result contracts ([`JSON_CONTRACTS.md`](../architecture/JSON_CONTRACTS.md));
- Python characterization - `scripts/characterize.py` records what `cancellai.py` actually does on every fixture in the corpus and classifies it `NORMATIVE` / `INTENTIONAL_DIVERGENCE` / `LEGACY_ONLY` / `KNOWN_DEFECT` (see [Python reference contract](VERIFICATION_STRATEGY.md#python-reference-contract)); committed records live under [`tests/fixtures/characterization/`](../../tests/fixtures/characterization/) and `scripts/characterize.py check` proves they are still reproducible on a clean checkout;
- differential comparison rules.

Only fixtures classified `NORMATIVE` are binding on the Rust candidate at M6. A
`KNOWN_DEFECT` record exists precisely so that behavior is never copied forward as a
requirement merely because Python happens to do it; `INTENTIONAL_DIVERGENCE` and
`LEGACY_ONLY` need their own accepted spec/ADR before Rust may differ or drop the behavior,
per [Story changes during implementation](WORK_ITEM_MODEL.md#story-changes-during-implementation).

### M2 - Freeze Python

Python becomes maintenance-only (E01-S06). `AGENTS.md`'s "Python reference freeze" section is the visible marker; `scripts/check_process.py check` fails if it goes missing. Only parity fixes (matching the committed characterization), safety/security fixes, and migration-support tooling are accepted from here forward - not merely until this epic closes. New features target Rust.

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

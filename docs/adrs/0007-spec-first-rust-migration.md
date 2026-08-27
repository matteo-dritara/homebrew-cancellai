# ADR-0007: Spec-first clean migration from Python to Rust

- Status: Accepted
- Date: 2026-08-27
- Related: PD-010

## Context

The Python v1 is valuable as observed behavior but is close to the limit of a single-file macOS architecture. A rewrite without an oracle risks safety regressions.

## Decision

Fix P0 safety defects, extract fixtures/golden behavioral contracts, freeze Python as reference, then implement the target Rust core. Differential testing is required before cutover.

## Consequences

New features do not expand the Python monolith. Rust need not preserve accidental Python constraints or known defects. Cutover is evidence-gated and rollbackable.

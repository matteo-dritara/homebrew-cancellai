# ADR-0008: Reconstructible state, event ledger, and bounded analytical memory

- Status: accepted
- Date: 2026-08-27
- Related: PD-016

## Context

Guardian/policy/audit need history, but storing detailed agent state indefinitely would violate the product's purpose and privacy goals.

## Decision

Use three persistence layers: reconstructible current state, append-only operational events, and compacted analytical rollups. Apply a hard self-budget and remain contentless by default.

## Consequences

The local database can be reset/rebuilt. Analytics degrade/compact before self-budget is exceeded. Quarantine payload storage is tracked separately from metadata state.

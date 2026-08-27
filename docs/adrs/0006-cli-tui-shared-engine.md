# ADR-0006: CLI and TUI as first-class clients of one engine

- Status: Accepted
- Date: 2026-08-27
- Related: PD-009

## Context

Power users need scriptability and a rich exploratory interface. Duplicated UI logic would create safety drift.

## Decision

CLI and TUI are first-class clients of a shared domain/safety engine and contain no provider mutation logic. Guardian and Desktop later use the same contracts.

## Consequences

Machine-readable CLI schemas are stable product interfaces. Plans produced through different UIs must be semantically equivalent.

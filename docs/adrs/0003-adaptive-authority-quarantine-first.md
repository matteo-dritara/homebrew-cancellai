# ADR-0003: Adaptive authority and quarantine-first lifecycle

- Status: accepted
- Date: 2026-08-27
- Related: PD-004, PD-011

## Context

Manual-only cleanup limits Guardian value; fully autonomous deletion creates unacceptable risk. Different artifact classes have different reversibility and confidence.

## Decision

Authority is an ordered lattice from Observe through Autopilot, bounded by artifact/provider/confidence/reversibility/policy ceilings. Quarantine is preferred when technically safe and reversible. Irreversible actions are explicit and stronger-gated.

## Consequences

Autopilot is not a global switch. A user can grant broad intent without elevating an artifact beyond its constitutional ceiling.

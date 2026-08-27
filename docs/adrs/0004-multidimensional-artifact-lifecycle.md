# ADR-0004: Multidimensional artifact and lifecycle model

- Status: accepted
- Date: 2026-08-27
- Related: PD-005, PD-012

## Context

Provider-centric hierarchies reproduce fragmentation, and a single status such as `stale` cannot express simultaneous orphan/protected/quarantined/integrity states.

## Decision

Internally cancellAI is artifact-centric across machine/project/provider/artifact/session. Lifecycle uses independent Activity, Residency, Protection, and Integrity axes plus append-only lifecycle events and contentless purge tombstones.

## Consequences

UX can pivot by machine/project/provider without duplicating data. Orphan status never means delete by itself. Project attribution may remain Unattributed.

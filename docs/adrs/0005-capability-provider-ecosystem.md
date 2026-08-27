# ADR-0005: Capability-based provider ecosystem

- Status: Accepted
- Date: 2026-08-27
- Related: PD-006, PD-013

## Context

Providers differ in what can be safely detected, mapped, deleted, restored, or configured. An all-or-nothing support flag is dishonest and makes new integrations expensive.

## Decision

Providers expose granular capabilities with evidence/confidence. Simple integrations may be declarative manifests; complex ones use native adapters. Trust levels directly bound authority.

## Consequences

New providers can start with inventory-only support. A manifest cannot self-promote into destructive trust. Claude/Codex are reference adapters; other providers use the same conformance model.

# ADR-0001: Agent State Control Plane as the product category

- Status: Accepted
- Date: 2026-08-27
- Related: PD-001, PD-003

## Context

A narrow Claude/Codex cleanup utility is vulnerable to vendor-native retention/delete features becoming adequate. The durable cross-vendor problem is visibility and lifecycle governance of local agent state.

## Decision

We will build cancellAI as a provider-agnostic Agent State Control Plane. Cleanup remains an entry feature; inventory, explanation, prevention, policy, and lifecycle governance define the long-term product.

## Consequences

The product core models artifacts/capabilities rather than vendor cleanup folders. Generic system cleaning remains out of scope. Provider proliferation increases product value rather than multiplying unrelated special cases.

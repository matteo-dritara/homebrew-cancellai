# ADR-0002: Local authority with optional federated intelligence

- Status: accepted
- Date: 2026-08-27
- Related: PD-007

## Context

Provider layouts evolve quickly and shared compatibility knowledge is valuable, but users must be able to trust a destructive tool offline and without uploading agent content.

## Decision

The complete local core works offline. Optional signed/versioned network knowledge may inform compatibility and recommendations, but local policy and the safety kernel retain final authority. Network inputs cannot embed executable cleanup commands.

## Consequences

No account is required for local use. Knowledge updates are separate release artifacts. Future fleet control sends intent/policy; nodes still authorize and execute locally.

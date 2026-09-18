# RFCs

RFCs are used for significant proposals that need exploration before a durable architecture decision is accepted.

Create an RFC when there are meaningful competing approaches to a new public schema/protocol, dependency, mutation model, persistence strategy, trust/network boundary, or cross-platform architecture.

Use `project/templates/RFC.md`. RFC IDs are monotonic (`RFC-0001`, ...). Accepted architecture decisions should produce or reference an ADR. Rejected RFCs remain in history with their status.

## RFCs

- [RFC-0001: Remote execution boundary shape](0001-remote-execution-boundary.md) - accepted
  (Option A, [ADR-0031](../adrs/0031-remote-execution-requests-are-signed-intents-never-plans.md))
  - what a remote controller's request may contain and how it may influence
  `cancellai_safety::authority::effective_authority` (E18-S02, SI-031).

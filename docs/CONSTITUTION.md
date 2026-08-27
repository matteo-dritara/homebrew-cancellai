# Product Constitution

This document defines the non-negotiable laws of cancellAI. Product features, provider integrations, UI requests, policies, remote control, and optimization work are subordinate to these rules. Changing a constitutional rule requires an accepted ADR/RFC, explicit owner approval, and CR4-level review if the change affects destructive authority.

## C-01 Local authority

The local core is the authority over local state. Network services may provide knowledge, policy intent, or coordination but may not bypass local safety validation or directly gain filesystem authority.

## C-02 Unknown is protected

Unknown identity, classification, provider layout, integrity, activity, or scan completeness never becomes destructive permission. Uncertainty reduces authority.

## C-03 Ambiguity never escalates privilege

Ambiguous CLI syntax, policy conflicts, missing capabilities, parse errors, and compatibility drift resolve toward observation/refusal, never toward a stronger action.

## C-04 Quarantine before purge

When a safe reversible transition is technically available and policy permits it, quarantine is preferred to irreversible deletion. Irreversibility requires explicit representation and stronger verification.

## C-05 Authority is bounded and monotonic

Effective Authority is the minimum allowed by user authority, artifact ceiling, knowledge confidence, reversibility, activity/integrity state, provider capability, provider trust, and constitutional invariants. No lower-trust layer can elevate a higher-trust ceiling.

## C-06 Evidence before action

Every recommendation or mutation must be explainable from concrete evidence. Destructive plans must include preconditions that are revalidated at execution time.

## C-07 One safety kernel

All clients and providers route mutation through one safety boundary. CLI, TUI, Guardian, Desktop, plugins, manifests, and remote controllers may not implement alternate deletion paths.

## C-08 Provider neutrality

The core models artifacts and capabilities, not special cases for vendor brands. Provider-specific knowledge belongs in adapters/manifests at the edge.

## C-09 Contentless by default

cancellAI stores metadata required for identity, lifecycle, audit, policy, and analytical rollups. It does not copy transcript content, prompts, source code, secrets, or file contents into its database unless a future explicit feature and threat review requires it.

## C-10 Reconstructible local state

cancellAI's current-state database is not the source of truth for provider state. It must be disposable and rebuildable. Resetting cancellAI state can never delete provider data.

## C-11 Self-budget

cancellAI must impose storage and retention budgets on its own logs, database, event history, and analytical samples. A storage-governance tool may not become an unbounded storage producer.

## C-12 Cross-platform truthfulness

A platform is supported only when its path, identity, link/reparse, process, atomicity, and filesystem behavior is tested at the authority level claimed. WSL is a distinct environment, not an alias for generic Linux.

## C-13 Compatibility is capability-scoped

"Provider supported" is not a Boolean. Each provider/version/layout exposes a truthful capability state and confidence. Unknown versions automatically lose unsafe capabilities.

## C-14 Stable and experimental authority differ

Beta/nightly builds and unverified knowledge bundles do not inherit stable-release destructive authority by default.

## C-15 Open local control

The local single-machine safety and governance capabilities remain open source and inspectable. Commercial services may coordinate many nodes but do not hold back local safety primitives.

## C-16 Evidence-gated delivery

A feature is not complete merely because it works. It must satisfy both Definition of Done and Definition of Safe. CR3/CR4 changes require independent adversarial verification; CR4 release eligibility requires an owner-visible Safety Verdict.

## C-17 Small, reversible engineering changes

Development favors small, self-contained changes that keep main releasable. Refactors and behavior changes are separated when practical. Every change carries the tests and documentation needed to understand it.

## C-18 The project is not a black box

Major product decisions are recorded. Work items expose outcome, dependencies, acceptance criteria, safety obligations, and verification. Generated project status must be reproducible from version-controlled source data.

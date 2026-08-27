# ADR-0009: Verifiable canonical releases and channel-aware distribution

- Status: Accepted
- Date: 2026-08-27
- Related: PD-017

## Context

A filesystem-authoritative binary needs stronger distribution trust than a convenience script, and future Rust releases must serve macOS/Linux/Windows without four independent release processes.

## Decision

Canonical releases are automated from tagged source and carry checksums, SBOM, provenance/attestation and verification material. Multiple package channels consume the same canonical artifacts. Knowledge updates are separate. Release channel bounds default authority.

## Consequences

Release tooling becomes part of the security boundary. Installation source is tracked. A later dedicated canonical `cancellai` repo plus generated `homebrew-cancellai` tap is preferred after migration planning.

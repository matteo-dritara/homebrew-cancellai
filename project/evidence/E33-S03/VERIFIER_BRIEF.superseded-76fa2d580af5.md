<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E33-S03
Rendered-by: Claude
Rendered-on: 2026-09-24
Brief-Checksum: 76fa2d580af56d961223b3b67561cb2315a0f68b875cc04b82fdab2cb79ab098

<!-- end handoff header -->
# Verifier Brief - E33-S03 - The containment ledger is a SQLite table decided and appended in one transaction

Status: in_progress | Change Risk: CR4
Outcome: The JSONL history decided from one read and appended in a separate write; rounds 8 and 9 found concurrent refreshes writing duplicate or losing lines, and a crash inside the write could leave a torn line (ADR-0040). The ledger is now a SQLite table: install, refresh and lift read, replay through cancellai-safety, decide and insert at most one row inside one BEGIN IMMEDIATE transaction.
Dependencies: E06-S07

## Acceptance Criteria
- When containment is installed, refreshed or lifted, the system shall read the history, decide and insert at most one event row inside one database transaction.
- If a decision refuses, finds the notice already current, or loses to a concurrent change, then the system shall insert no row.
- If the process is interrupted before the transaction commits, then the history shall contain no part of the interrupted event.
- If the ledger database exists but cannot be opened, read or replayed, then authority shall be capped as for an unreadable ledger and nothing shall be written.

## Verification Contract
- Store tests for commit/rollback and concurrent transactions; the 16-process refresh and 8-process install CLI tests assert one row per accepted event; a kill-point test kills the process inside the transaction and asserts the history replays with no partial event; a corrupt database caps authority.

## Safety Obligations

### SI-022 Knowledge is data, not executable authority

Remote/local knowledge bundles cannot inject arbitrary commands/code or raise local destructive authority. Invalid signatures/provenance are rejected.

Partially implemented at `rust/crates/cancellai-safety/src/trust_promotion.rs` (E05-S02):
`TrustPromotionEvidence` carries only inert strings (a verifier name, fixture reference
identifiers) with no command/code field for anything to execute, and raising authority through
it requires passing through `TrustedTier::promote`'s fail-closed checks - the only public path
from which a `TrustedTier` above `Untrusted` can be obtained (see SI-021 above for the round-1
repair that made this actually true, not merely intended). E16-S02 implemented bundle signature/provenance verification in
`rust/crates/cancellai-safety/src/knowledge_bundle.rs`. E17-S08 independent review found that
its call used permissive verification despite ADR-0024 requiring `verify_strict`; the repair
uses strict Ed25519 verification and rejects weak local-policy keys/signatures before a bundle
can replace the current store. A regression proves refusal preserves the full current value.

### SI-029 Knowledge rollback/tamper fails closed

Invalid, expired, replayed, or unauthorized knowledge updates are rejected or downgraded to non-destructive use; a bad update cannot brick basic offline inspection.

Implemented for bundles at `rust/crates/cancellai-safety/src/knowledge_bundle.rs` (E16-S02) and
for incident containment at `rust/crates/cancellai-safety/src/incident.rs` (E17-S07): a refused
containment update - unreachable service, malformed text, bad signature, unknown publisher,
expiry, replay - leaves the containment ledger exactly as it was, and no refusal, rollback or
expiry can remove a containment already recorded. Only a local lift does.

Since E06-S07 the ledger persists as a history of raw signed bundles and local lifts that the
kernel re-verifies on every load, skipping only the expiry check (expiry never lifts). The
guarantee covers every input from outside the owner's account. It does not cover the owner's own
state: a process running as the same user can delete, truncate or append to the history, and
each of those lifts containment - an accepted limit (owner decision, 2026-09-23;
`docs/security/INCIDENT_RESPONSE.md`), not a gap in the verification.

## Documentation Impact
- docs/adrs/0039-the-cutover-perimeter-binds-cli-authority-to-a-local-containment-ledger.md
- docs/security/INCIDENT_RESPONSE.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

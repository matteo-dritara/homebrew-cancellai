# ADR-0033: Purge tombstone content-safety is a disclosed residual, not a closed guarantee

- Status: Accepted
- Date: 2026-09-20
- Owners: owner (Matteo Pugliese)
- Related: E12-S04, SI-020, C-09, ADR-0025, `docs/architecture/PERSISTENCE_MODEL.md`

## Context

E12-S04's original acceptance criterion read "Tombstones contain no prompts/source/file
contents" (AC1), read as an unqualified guarantee. Two independent review rounds falsified every
attempt to make a caller-facing primitive meet that reading:

- **Round 1**: `Tombstone`'s fields (`artifact_id`, `provider_id`, `category`, `reason_code`,
  `policy_id`, `plan_id`, `evidence_ids`) were plain `String`/`EvidenceId` values, and
  `record_purge_tombstone` passed them to `EventLedger` unchanged. A column allowlist restricts
  which fields exist, not what bytes a caller puts in them - every field round-tripped an
  arbitrary prompt, source excerpt, or path.
- **Round 2**: a syntactic content-safety validator (ASCII letters/digits joined by up to four
  hyphens, bounded length) was added. It still could not distinguish a real identifier from an
  attacker's message re-encoded with hyphens standing in for spaces: `do-not-purge-this` passes
  every bound and is still meaningful content. The reviewer's own conclusion: "a lexical
  length/character/segment predicate is insufficient because valid-looking short natural
  language is content."

The executor then removed `provider_id`/`category`/`reason_code`/`policy_id` outright (purely
descriptive annotations, not needed to identify what was purged) and documented the three
remaining fields (`artifact_id`, `plan_id`, `evidence_ids`) as a disclosed residual rather than a
closed guarantee, since a tombstone naming neither what was purged nor which plan/evidence
produced it is not a tombstone at all. Round 3's review found this field reduction genuinely
narrower and confirmed a public bypass around it (`EventLedger::append` accepted the same
content directly, past `record_purge_tombstone`'s validation entirely) - closed in the same
change this ADR accompanies, by enforcing the identical shape check inside `append` itself for
`EventKind::Purged`, so the check is no longer optional for any caller.

Round 3 also stated, correctly, that a verifier cannot accept a residual in place of an
unqualified acceptance criterion on its own authority: "cannot convert an unsatisfied acceptance
criterion into a passing residual without a control-plane/owner decision." It offered two paths:
narrow AC1 via an owner decision to an explicitly stated, testable, primitive-only property, or
block the story on a new orchestrator/non-forgeable-reference story that sources
`artifact_id`/`plan_id`/`evidence_ids` from the system's own already-validated records instead of
an arbitrary public caller.

**No local, orchestrator-less primitive can close AC1's unqualified reading.** Any primitive that
accepts a caller-supplied string for a required identifier field can be handed a short, ordinary
phrase in place of a real identifier, and no finite character/shape predicate can distinguish the
two - this is the same structural argument E13-S06 already made about content-based ownership
checks (a check against content the caller supplies can never prove what the check is checking
for), applied here to content-safety instead of ownership.

## Decision

E12-S04's AC1 is narrowed, by this owner decision, from an unqualified guarantee to the property
the current primitive can actually make and test:

> Tombstones carry no descriptive annotation fields (`provider_id`/`category`/`reason_code`/
> `policy_id` are absent, not merely validated). The `artifact_id`/`plan_id`/`evidence_ids`
> linkage fields are each a short, identifier-shaped value - this rejects an obvious prompt,
> source excerpt, or file path, but does not and cannot prove the absence of a short, ordinary
> phrase in their place. This is a disclosed residual, not a closed guarantee, and full closure
> requires a future orchestrator that derives these three values from the system's own
> already-validated purge/plan/evidence records rather than accepting them from an arbitrary
> public caller.

This is the primitive-only reading Round 3 named as the first available path. `project/epics/
E12.json`'s AC1 text, `docs/architecture/PERSISTENCE_MODEL.md`'s "Tombstones" section, and
`rust/crates/cancellai-store/src/tombstone.rs`'s own module doc are updated to state it, so a
future reader (including a future reviewer) sees the residual as intended, not as an oversight.

Choosing this path over blocking E12-S04 on a new orchestrator story: no orchestrator story is
currently scheduled, this primitive has no caller yet (`mutation_executor::execute`'s
`ActionClass::Delete` branch does not call it), and the field reduction plus the now-closed
`EventLedger::append` bypass materially reduce exposure (four fewer free-text fields, and the
shape check is now enforced at the one function every write path must call, not an optional
wrapper). The residual is bounded to three fields whose ordinary values are short opaque IDs, not
prose, and is tracked here for whoever schedules the orchestrator work to close.

## Consequences

- E12-S04 can close against its narrowed AC1, and E12 with it, once independent review confirms
  the current diff against that reading.
- C-09 ("does not copy transcript content, prompts, source code, secrets, or file contents...
  unless a future explicit feature and threat review requires it") is satisfied for the four
  removed annotation fields unconditionally, and for the three remaining linkage fields only up
  to the disclosed residual this ADR names - this ADR is that explicit review for the residual
  that remains, recorded rather than silently accepted.
- A future orchestrator story that wires a real purge to `record_purge_tombstone` must also close
  this residual: it should source `artifact_id`/`plan_id`/`evidence_ids` from objects the system
  already trusts (the plan/evidence records the purge itself produced), not accept them as
  arbitrary caller-typed strings, and should consider whether `EventLedger::append`'s public
  visibility should narrow once a controlled factory exists. Until that story exists, this
  residual is accepted, not resolved.
- Any other story that persists a caller-supplied "required identifier" field under a
  contentlessness claim should read this ADR first: the same structural argument applies whenever
  a finite predicate is asked to prove the absence of meaning in an unbounded string.

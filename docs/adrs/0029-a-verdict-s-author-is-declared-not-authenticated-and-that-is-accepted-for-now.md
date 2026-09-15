# ADR-0029: A verdict's author is declared, not authenticated, and that is accepted for now

- Status: Accepted
- Date: 2026-09-15
- Owners: project owner / cEOS
- Related: [ADR-0014](0014-epic-closure-is-a-release-and-review-is-bounded.md), [ADR-0025](0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md), E28-S02, E29-S03

## Context

E28-S02 gave the executor/verifier handoff an artifact. A rendered brief carries a checksum over
its own body, a verdict repeats that checksum, and `scripts/verifier_handoff.py check` refuses a
verdict that answers a document which is not the committed brief, a brief edited after rendering,
and a verdict with no author. It also refuses the case the whole mechanism exists for: a verdict
whose `Verifier:` is the party named in the brief's `Rendered-by:`.

That last refusal compares two strings, and both are written by whoever writes the file. The
round-2 review of E28 said so plainly: the mechanism prevents an accidental collapse of the two
roles and makes a declared separation auditable; it cannot authenticate the actor.

The executor's first proposal for closing this was to bind a verdict's authorship to the GPG
signature on the commit that introduced it. **The verifier refused it, and was right.** This
repository signs with a single owner key. That key authenticates the owner, which is not in
dispute, and says nothing whatever about which agent produced a verdict — the executor and the
verifier commit under the same signature. Adopting it would have replaced an honest declared claim
with a cryptographic one that attests to something nobody asked about, which is worse than the
declaration because it looks like proof.

## Decision

**Per-role signing identities are not adopted.** The residual stands, recorded and bounded:

> A verdict's author is a claim its writer makes. The mechanism prevents an accidental role
> collapse and makes the claim auditable against a specific brief. It does not authenticate the
> actor, and a party willing to write a different name on the `Verifier:` line defeats it.

Three things follow, and they are the substance of this decision rather than its consequences.

**What the mechanism does defend is worth stating, because it is not nothing.** The threat it
removes is the one that actually happened repeatedly in this repository's history: a role boundary
crossed by accident, under time pressure, by an agent that would have obeyed the rule if the rule
had been in front of it. Every collapse this project has recorded was of that kind. The threat it
does not remove — an agent deliberately forging a role it was told not to occupy — has never
occurred here and would, if it did, be a failure of the agent's alignment rather than of the
repository's plumbing. Cryptography does not fix that: an agent willing to forge a name is an agent
willing to misuse a key it holds.

**The cost is real and falls in the wrong place.** Authenticated per-role identities require
non-exportable keys held separately by each agent, a reviewed key-to-role mapping, a gate that
verifies the signature on the commit introducing each verdict, and an answer for every way history
is legitimately rewritten. This repository squash-merges — that is the only merge method its own
settings permit — so the commit that introduces a verdict on `main` is routinely not the commit
that was signed. Closing that means either abandoning squash merges or re-signing on merge, and
both are large changes to how the project ships in exchange for a threat it has never faced.

**A shared owner key is not a cheaper version of this.** It is recorded here explicitly so the
shortcut is not re-proposed by a future session that sees `commit.gpgsign` and reasons the way the
executor did.

## Consequences

### Positive

- The claim the evidence ledger makes about who verified what is now bounded in writing, rather
  than being stronger in the reader's mind than in fact.
- The mechanism that does work — brief-to-verdict binding and the role-collapse refusal — is not
  weakened by being asked to carry a guarantee it cannot provide.
- A future session that proposes the GPG shortcut will find the argument against it already made.

### Negative

- Attribution in the evidence ledger remains a declaration. A reader who wants proof that Codex,
  and not Claude, produced a given verdict does not have it and will not get it from this design.
- The process-metrics split between independent review and self-review rests on the same
  declaration, so a forged `Verifier:` would corrupt a measurement as well as a record.

### Neutral / follow-up

- **Revisit if any of three things change**: the project admits a third agent whose role boundary
  is not enforced by who holds which session; a verdict is found that misattributes its author,
  accidentally or not; or the repository stops squash-merging, which removes the largest single
  obstacle to signature-based attribution.
- The implementation story this ADR would have carried is not created. If a later decision reverses
  this one, that story's Change Risk Level is CR4 — it is an authority boundary — and it needs its
  own Safety Verdict.

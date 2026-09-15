<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E29-S03
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: 03b8df300a8bb340628be11d1223efb2f2d48bdb3cb5ad13b5ed97105feb7e29

<!-- end handoff header -->
# Verifier Brief - E29-S03 - Decide, by ADR, how a verdict's author is authenticated rather than declared

Status: ready_for_review | Change Risk: CR2
Outcome: E28-S02 made the executor/verifier handoff auditable and stopped short of authenticating it: `Verifier:` and `Rendered-by:` are strings their own author writes. The round-2 review refused the obvious shortcut and was right to - this repository's commits are signed with a single owner key, which authenticates the owner and says nothing about which agent produced a verdict. Closing this needs distinct per-role signing identities and a mapping from key to role that the gate can check, which is an authority boundary and therefore an ADR before it is an implementation. This story is the decision, not the mechanism: it produces the ADR that settles whether per-role signing identities are worth their operational cost, what the key-to-role mapping is, and what happens to a verdict whose signature history was rewritten. The implementation that follows is a separate story, and the ADR sets its Change Risk Level - almost certainly CR4, which is precisely why it is not asserted here before anyone has decided what is being built.
Dependencies: none

## Acceptance Criteria
- The ADR shall state whether authenticated per-role identities are adopted, and if they are not, what residual is accepted in their place.
- If adopted, the ADR shall name the key-to-role mapping, where it lives, and what makes adding a key an explicit owner act.
- The ADR shall say what a rewritten or squashed verdict commit means for an attribution that was valid before the rewrite.
- The ADR shall record why a single shared owner signing key does not answer this, so the shortcut is not re-proposed.

## Verification Contract
- The ADR is reviewed against the round-2 finding it answers, and the finding is shown to be discharged or explicitly accepted as residual.
- The follow-on implementation story is created with the Change Risk Level the ADR sets, rather than one assumed in advance.

## Safety Obligations
- none

## Documentation Impact
- docs/adrs/
- docs/development/AGENT_PROTOCOL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.

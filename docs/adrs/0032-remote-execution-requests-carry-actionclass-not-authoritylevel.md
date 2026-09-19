# ADR-0032: Remote execution requests carry `ActionClass`, not `AuthorityLevel`

- Status: proposed
- Date: 2026-09-19
- Owners: owner (Matteo Pugliese)
- Related: E18-S02, E18-S03, RFC-0001, ADR-0031, SI-031, TM-17, `docs/architecture/DOMAIN_MODEL.md`

## Context

RFC-0001 ("Remote execution boundary") and ADR-0031 ("Remote execution requests are signed
intents, never plans") both specify the same payload shape for a `RemoteExecutionRequest`: "a
target artifact/machine reference and one requested `ActionClass`... No `AuthorityLevel`, no
plan, no execution parameters." RFC-0001 explicitly considered and rejected the alternative -
"Option B - request carries a pre-built plan or authority level" - naming it "the literal shape
TM-17 and SI-031 name as the threat (`a command that bypasses local review/safety`)", and citing
the E05 round-1 review finding that had to close the identical bug for `ProviderTrust`.

The implementation Codex's round-1 independent review examined
(`rust/crates/cancellai-safety/src/remote_execution.rs`, committed in E18-S02) does not match
either document: `RemoteExecutionRequest::requested_authority` is `AuthorityLevel`, signed and
transmitted directly over the wire, and `TrustedRemoteController::authority_ceiling` gates it as
a raw `AuthorityLevel` ceiling. This is Option B, not Option A - the design the RFC rejected, not
the one it accepted. The review's exact words: "a material protocol and authority-boundary
change... this documentation inconsistency violates C-18's evidence-gated delivery even apart
from the runtime failures" (`project/evidence/E18-VERIFIER-REVIEW.md`).

Per `AGENTS.md`'s constitutional rule, a conflict between an accepted design record and the code
needs escalation through an ADR/RFC/owner decision, not a same-session unilateral pick by the
executor - which is why this ADR exists rather than a silent code change or a silent edit to
RFC-0001/ADR-0031's own text.

**What makes this more than a naming/shape difference.** `cancellai_model::vocabulary::
ActionClass` already exists (`Observe | Quarantine | Archive | Delete | Restore`), and
`cancellai_safety::authority::minimum_authority_for(ActionClass) -> AuthorityLevel` already
exists and is already reviewed/tested (E03-S05's own authority boundary). Its range is exactly
`{Observe, Quarantine, Govern}` - it can **never** produce `AuthorityLevel::Recommend` or
`AuthorityLevel::Autopilot` for any `ActionClass` value. Under the current (Option B)
implementation, a remote controller whose ceiling is high enough can request
`AuthorityLevel::Autopilot` directly - full standing authority - over the wire. Under Option A, a
remote controller can never express that: the wire vocabulary itself has no way to say
"autopilot," only a semantic action ("please evaluate quarantining/archiving/deleting/restoring
X"), and the *local* side is what decides the authority level that action requires, through the
same function every local plan already goes through. This is a structural containment Option B
does not have, not merely a documentation/implementation mismatch.

## Decision

We will correct `rust/crates/cancellai-safety/src/remote_execution.rs` to match RFC-0001/
ADR-0031's accepted Option A shape:

- `RemoteExecutionRequest::requested_authority: AuthorityLevel` becomes
  `requested_action: cancellai_model::ActionClass` (requires adding `Deserialize` to
  `ActionClass`'s existing derives - it currently derives `Serialize` only).
- `TrustedRemoteController::authority_ceiling` stays an `AuthorityLevel` (an operator still
  reasons about a ceiling in the same vocabulary `LocalTrustPolicy`/`ProviderTrust` already use),
  but the ceiling check goes through `minimum_authority_for(request.requested_action) <=
  controller.authority_ceiling`, never a direct `AuthorityLevel`-to-`AuthorityLevel` comparison
  against a wire-supplied value.
- `VerifiedRemoteIntent::requested_authority` becomes `requested_action: ActionClass`. The caller
  that eventually feeds `AuthorityInputs::user_requested` computes
  `minimum_authority_for(verified.requested_action)` itself - the same function, called from the
  outer-ring caller rather than duplicated inside `cancellai-safety`.
- `RemoteExecutionRequest::payload_bytes`/`signing_bytes` sign the `ActionClass` (its `serde_json`
  encoding, matching the existing pattern) in place of the `AuthorityLevel` they sign today.

This is a breaking change to `RemoteExecutionRequest`'s wire shape. Acceptable because: this
crate has shipped in no release yet (E06-S04's canonical-engine-switch gate is still open, so
Rust is not the canonical engine), and "no transport wires it to anything yet" is this story's
own recorded residual - there is no deployed producer of this format to migrate.

## Alternatives considered

### Retroactively accept `AuthorityLevel` over the wire (amend RFC-0001/ADR-0031)

Rejected. The RFC did not overlook this shape - it named it, gave the concrete threat it enables,
and cited a precedent bug it already had to fix once. Nothing about E18-S02's actual repair
(replay-bypass fix, target-binding fix) addresses that original objection; both fixes are
orthogonal to what payload shape is signed. Retroactively accepting the rejected option requires
a new, independent safety argument for why the original objection no longer holds, and none has
been made. The `minimum_authority_for` range finding above is additional, not weaker, evidence
against this alternative: Option B measurably grants a remote party reach (`Autopilot`,
`Recommend`) Option A structurally cannot.

### Leave the divergence recorded as an accepted residual, revisit later

Rejected as the standing state, though it is what shipped pending this ADR. The pieces needed to
close it already exist and are already reviewed (`ActionClass`, `minimum_authority_for`) - the
scope is a `RemoteExecutionRequest`/`RemoteExecutionLog` field-shape correction inside one
already-`ready_for_review` module, not a from-scratch design effort. Leaving a CR4 authority
boundary shipping the design it explicitly rejected, for longer than it takes to make the already-
available correction, is not proportionate to the cost of making it.

## Consequences

### Positive

- The remote-execution wire format cannot express an `AuthorityLevel` value a compromised or
  malicious controller could pick directly - it can name a semantic action, never a standing
  grant. `AuthorityLevel::Recommend`/`Autopilot` become unreachable from any remote request by
  construction, not by convention.
- `RemoteExecutionRequest` and a local `SealedPlan`'s `Action` now share the same
  `ActionClass`/`minimum_authority_for` vocabulary end to end, closing the "two different shapes
  say the same thing" drift risk the RFC's own module doc warned about.
- RFC-0001/ADR-0031 and the code agree again, restoring C-18's evidence-gated-delivery property
  for this story.

### Negative / cost

- `RemoteExecutionRequest`'s wire shape changes (breaking, but no deployed producer exists yet -
  see Decision).
- `TrustedRemoteController::authority_ceiling`'s semantics shift subtly: it now bounds "the
  minimum authority the requested action class needs," not "the exact authority level the request
  names." An operator reading `authority_ceiling: AuthorityLevel::Quarantine` must understand it
  as "this controller may request Observe/Quarantine/Archive/Restore-class actions," not "this
  controller may be granted exactly Quarantine" - worth a doc-comment update at the same time.
- Every existing `remote_execution.rs` test that constructs a `RemoteExecutionRequest` with
  `requested_authority: AuthorityLevel::X` needs updating to `requested_action: ActionClass::Y`
  with an equivalent `minimum_authority_for` mapping, and the `RequestedAuthorityExceedsCeiling`
  error variant's check needs the indirection above.

### Neutral / follow-up

- This ADR does not itself change any code. A follow-up work item (continuing E18-S02, or a new
  story if the epic's story boundaries make that cleaner) carries the actual implementation,
  under whichever Change Risk Level its own diff earns (CR4, matching E18-S02's own floor).
- `docs/security/THREAT_MODEL.md`'s TM-17 entry and `docs/architecture/TARGET.md`'s remote-
  execution section should be checked for any prose that still describes the `AuthorityLevel`-
  over-the-wire shape once the correction lands.

## Safety and compatibility impact

- Change Risk implication: CR4 (touches `cancellai-safety`, the safety kernel; a per-project
  risk floor already applies to any change in this crate regardless of the declaring story's own
  level - see `project/risk_floors.json`).
- Safety Invariants affected: SI-031 (this is the invariant's own boundary), TM-17 (the threat
  case this correction closes for the class of bug, not only the two reproductions round 1
  found).
- Migration/rollback: no live data/wire format to migrate (no deployed producer). Rollback is a
  plain revert of the follow-up commit; this ADR's own record stays as the decision history
  either way.

## Supersession

None. If a future story finds `ActionClass`'s five variants insufficient for a real coordination
need, that is a `docs/architecture/DOMAIN_MODEL.md` vocabulary change (already flagged there as
deliberately open-ended - "leaving room for a future, more specific class"), reviewed on its own
terms, not a reason to reopen the `AuthorityLevel`-over-the-wire alternative this ADR rejects.

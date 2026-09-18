# ADR-0031: Remote execution requests are signed intents, never plans or authority grants

- Status: Accepted
- Date: 2026-09-18
- Owners: project owner
- Related: [RFC-0001](../rfcs/0001-remote-execution-boundary.md), E18-S01, E18-S02, SI-031, TM-17,
  ADR-0024

## Context

E18-S02's outcome ("keep mutation execution on the target node under its local safety kernel
even when a remote controller requests policy evaluation") had no existing design: nothing in
this repository defined what a remote controller's request contains or how it may reach
`cancellai_safety::authority::effective_authority` without becoming the exact threat SI-031/TM-17
name - a remote party dictating a mutation outcome rather than asking the target to evaluate one.
[RFC-0001](../rfcs/0001-remote-execution-boundary.md) worked the competing options; this ADR
records the accepted one before E18-S02's implementation begins, per this repository's own rule
that a trust/network boundary decision needs an RFC/ADR, not an inline call inside a CR4 diff.

## Decision

We adopt RFC-0001's Option A. A remote controller's request is a `RemoteExecutionRequest`
envelope, structurally and cryptographically the same shape `cancellai_safety::knowledge_bundle`
already uses for a different signed document (`controller_id`, a strictly-increasing per-
controller `sequence`, `expires_at`, a SHA-256 content digest, an ed25519 signature -
ADR-0024's primitive, no new dependency). Its payload names exactly one target/artifact
reference and one requested `ActionClass` - nothing else. A new, local, operator-owned
`TrustedRemoteControllers` policy (mirroring `LocalTrustPolicy`, distinct from it and from
`RemoteTargetTrust` because it gates a different trust question) authenticates the controller
and bounds which `ActionClass`es its tier may even request, both checked before the request
reaches authority computation at all.

On success, the request contributes **only** the `user_requested` field of one
`AuthorityInputs` call, for the one target/action named. Every other input
(`artifact_ceiling`, `confidence`, `activity`, `protection`, `integrity`, `provider_trust`)
stays locally derived exactly as it is for a local caller today - `effective_authority`'s
signature and behavior do not change. `cancellai-safety` (kernel ring) does not depend on
`cancellai-store` (outer ring, owns `EventLedger`) and this decision does not add that edge
(ADR-0019: the kernel stays bare of `rusqlite`) - verification returns a structured outcome
(accepted/rejected, always carrying controller, sequence, requested action, and, on rejection,
the specific reason) with every field an `EventLedger` entry needs. Writing that entry is the
job of whichever outer-ring caller eventually wires a real transport to this verification
function, not of `cancellai-safety` itself.

Wire transport is explicitly out of scope of this decision (RFC-0001's Non-goals) - deferred to
E18-S03.

## Alternatives considered

### A pre-built plan or authority level arriving over the wire

Rejected: this is the literal shape TM-17/SI-031 exist to prevent, and the identical class of
bug the E05 round-1 review already found and fixed for `ProviderTrust` (a bare, externally
suppliable authority value reaching `effective_authority` with no gate). See RFC-0001's
"Option B" for the full argument.

### Observation/reporting only, no request-to-act at all

Rejected as this story's primary design, though recorded as a fallback: it would leave E18-S02's
own acceptance criteria ("remote controller requests policy evaluation") technically vacuous
rather than met. See RFC-0001's "Option C."

## Consequences

### Positive

- SI-031's "target node... retains final mutation authority" becomes a property the type system
  and the existing `effective_authority` pipeline enforce, not a promise a future author has to
  remember - a remote-originated `user_requested` value is indistinguishable, to
  `effective_authority`, from a local one, because it is the same field populated the same way.
- Reuses three already-reviewed mechanisms (`KnowledgeBundle`'s signing/replay shape,
  `TrustedTier`/`LocalTrustPolicy`'s trust-gating shape, `EventLedger`'s audit shape) rather than
  inventing a fourth kind of trust boundary.
- No new crate dependency; `ed25519-dalek`/`sha2` are already approved, kernel-ring dependencies
  (ADR-0024).

### Negative / accepted risk

- `TrustedRemoteControllers` is a new trust-policy surface an operator must configure correctly;
  a misconfigured (over-broad) entry could authorize a remote party to request an `ActionClass`
  the operator did not intend. Accepted: the same operator-configuration risk already exists for
  `LocalTrustPolicy`/provider trust, and the alternative (a scheme with no local trust table at
  all) cannot express SI-031's "authenticates, resolves policy" requirement.
- Transport-level confidentiality/availability of the request itself is not addressed here -
  RFC-0001's Non-goals defers it to whatever transport E18-S03 chooses. A network observer can
  see request metadata (controller id, target, requested action) unless the eventual transport
  adds its own confidentiality; this ADR does not claim otherwise.

### Neutral / follow-up

- The exact `EventLedger` event kind/schema and whether `TrustedRemoteControllers` reuses or
  diverges further from `LocalTrustPolicy`'s Rust type are E18-S02 implementation detail, not
  further architecture decisions (RFC-0001's Open questions).

## Safety and compatibility impact

- Change Risk implication: CR4 (E18-S02) - the safety kernel's authority-input surface gains a
  second populator of `user_requested`, gated by authentication/authorization/audit before it
  ever reaches `effective_authority`. `effective_authority` itself is not modified.
- Safety Invariants affected: SI-031 (this ADR is the concrete mechanism that discharges it);
  SI-021/SI-022 (the same self-assignment concern `TrustedTier` already closes for provider
  trust, mirrored here for controller trust); TM-17 (the threat this design directly counters).
- Migration/rollback: purely additive. Reverting E18-S02 removes the new module and trust-policy
  type entirely; no existing caller of `effective_authority` depends on the addition, and nothing
  is wired to a production entry point until a transport exists (E18-S03 or later).

## Supersession

None yet.

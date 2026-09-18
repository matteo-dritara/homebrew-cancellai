# RFC-0001: Remote execution boundary shape (E18-S02)

- Status: accepted (Option A) - 2026-09-18, recorded in
  [ADR-0031](../adrs/0031-remote-execution-requests-are-signed-intents-never-plans.md)
- Date: 2026-09-18
- Owner: Matteo Pugliese
- Related work: E18-S02 (Local-agent remote execution boundary, CR4), E18-S01 (Remote target
  abstraction, `ready_for_review`), SI-031, TM-17, ADR-0024 (ed25519-dalek for knowledge-bundle
  signatures)
- Target decision date: before E18-S02 implementation begins

## Problem

E18-S02's outcome is "keep mutation execution on the target node under its local safety kernel
even when a remote controller requests policy evaluation." Nothing in this repository yet defines
what a "remote controller" is, what a "request" from one looks like on the wire, or how such a
request is allowed to influence `cancellai_safety::authority::effective_authority` without
becoming the exact thing SI-031/TM-17 exist to prevent: a remote party dictating a mutation
outcome instead of merely asking the target to evaluate one. This RFC decides that shape before
any code is written, per this repository's own rule that competing material options on a
trust/network boundary need an RFC first (`docs/rfcs/README.md`), and because CR4 verification
depth requires the invariant to be nameable before it can be tested.

Two actors are easy to conflate and must not be: **E18-S01's `RemoteTarget`** is the *machine
being managed* (an SSH box, a dev container, a CI runner) - it has no say over what happens to it.
The **remote controller** is the *thing issuing a request about* a target - a fleet server, a CI
job, or a human's other machine. This RFC is about authenticating and bounding the controller's
request, not about `RemoteTarget` itself, which E18-S01 already modeled.

## Goals

- Define what a remote execution request contains, and what it is authenticated against.
- Define exactly how an accepted request is allowed to influence
  `cancellai_safety::authority::AuthorityInputs` - and show that nothing else it could carry
  reaches authority computation.
- Define what "audit-linked" means concretely (SI-031, AC2) using the ledger E13 already built.
- Keep the target node's own OBSERVE -> CLASSIFY -> RESOLVE -> PLAN -> REVALIDATE -> EXECUTE loop
  (`docs/architecture/TARGET.md` "Core loop") completely unchanged by this story.

## Non-goals

- The wire transport (HTTP, gRPC, SSH-tunneled, message queue) a request physically arrives over.
  E18-S03 ("OSS/commercial protocol boundary") is explicitly the story that separates an open
  local protocol from any commercial fleet-coordination service; deciding a transport here would
  duplicate that story's own job and couple this RFC to a choice E18-S03 has not made yet.
- A fleet-wide policy distribution mechanism (pushing a `PolicyDocument` scope to many targets at
  once). `cancellai-policy`'s existing scope ladder (`docs/architecture/POLICY_MODEL.md`) is local
  policy only; extending it to a network-distributed scope is a materially larger change than one
  story's CR4 budget should carry, and nothing in E18-S02's two acceptance criteria requires it.
- Guardian's own local autonomous remediation (E15). E15's "Bounded remediation planner"/"kill
  switch" is a different epic, governing local automated decisions, not a network actor.

## Constraints

- Product Constitution: C-01 (local authority; network services may provide intent/coordination,
  never bypass local safety or gain filesystem authority directly), C-05 (authority is the minimum
  of every input; no lower-trust layer elevates a higher one), C-07 (one safety kernel; no
  alternate mutation path).
- Safety Invariants: SI-031 ("remote/fleet requests are intents. The target node independently
  authenticates, resolves policy, builds/revalidates plans, and retains final mutation
  authority").
- Threat model: TM-17 ("a future server sends a command that bypasses local review/safety" -
  control: "remote is intent/policy distribution only; target node retains authority and audit").
- Compatibility: no new crate dependency needed if Option A is chosen - `ed25519-dalek`/`sha2` are
  already approved, kernel-ring dependencies (ADR-0024, ADR-0015) via the identical pattern
  `cancellai_safety::knowledge_bundle` already uses for a different signed document.

## Options

### Option A - signed, replay-protected request envelope carrying only an intent (recommended)

A `RemoteExecutionRequest` (new module, `cancellai-safety`, alongside `knowledge_bundle.rs` and
`trust_promotion.rs`) mirrors `KnowledgeBundle`'s shape and verification exactly, because it is
solving the identical problem - "did an identified, trusted party really say this, and is it
still current" - for a different payload:

- `controller_id: String` (which remote controller), `sequence: u64` (strictly increasing per
  controller, replay protection - identical to `KnowledgeBundle::sequence`), `expires_at:
  Option<u64>`, `content_digest`/`signature` over the payload (SHA-256 + ed25519, identical
  primitives, no new dependency).
- Payload is deliberately narrow: a target artifact/machine reference and one requested
  `ActionClass` (`docs/architecture/DOMAIN_MODEL.md`) - "please evaluate quarantining artifact X
  on this target." No `AuthorityLevel`, no plan, no execution parameters. There is nothing in the
  payload for a compromised or malicious controller to escalate, because the payload cannot
  express escalation - the same "capability absence is first-class, not inferred" shape
  `cancellai-provider-api::capability`'s contract already uses, applied to what a request is even
  allowed to say.
- A new `TrustedRemoteControllers` policy (mirrors `LocalTrustPolicy` exactly: a local,
  operator-owned list of `(controller_id, TrustedTier)` pairs - never self-declared by the
  controller) gates verification. A request from an unlisted or under-trusted controller is
  rejected before its payload is even inspected.
- On successful verification, the request contributes **exactly one value**: the `user_requested`
  field of one `AuthorityInputs` call, for one target/artifact, for the one request that arrived -
  never a batch, never a default applied to unnamed artifacts. Every other `AuthorityInputs` field
  (`artifact_ceiling`, `confidence`, `activity`, `protection`, `integrity`, `provider_trust`) is
  still produced by the target's own local observation and policy resolution, completely
  untouched by this story - `effective_authority` itself does not change at all. This is the
  concrete mechanism behind SI-031's "resolves policy... retains final mutation authority" and
  discharges AC1 ("remote control cannot bypass target safety invariants") by construction: a
  remote party can lower what it asks for, never raise what the target independently grants.
- **Audit-linked** (AC2): every request - accepted or rejected, and rejected ones especially -
  writes one `cancellai_store::EventLedger` entry (E13) naming the controller id, sequence, the
  requested action, and the outcome (accepted/rejected-and-why). A controller cannot make a
  request that leaves no trace, including a request that fails.
- **Authenticated + authorized** (AC2): authenticated by signature verification against a known
  controller id (identical mechanism to `KnowledgeBundle`); authorized by `TrustedRemoteControllers`
  gating which `ActionClass`es a given tier may even request (mirrors `ProviderTrust`'s ceiling
  role) - both checks happen before the request reaches `effective_authority` at all, so an
  unauthenticated or unauthorized request never becomes an `AuthorityInputs` value in the first
  place.

**Cost**: a new module, a new small trust-policy type, and a new `EventLedger` event kind. All
three reuse existing, already-reviewed primitives (ADR-0024's signing scheme, E13's ledger, the
`TrustedTier`/`LocalTrustPolicy` shape from E05/E16). No new crate dependency.

**Migration**: purely additive; nothing existing changes shape. `effective_authority`'s signature
and behavior are unchanged - a remote-originated `user_requested` value looks, to that function,
identical to a local CLI flag's.

### Option B - request carries a pre-built plan or authority level

The remote controller sends something closer to "here is the plan, execute it" or "grant
Autopilot for this run" - a completed decision rather than a request to decide. Rejected outright:
this is the literal shape TM-17 and SI-031 name as the threat ("a command that bypasses local
review/safety"), and it would require either accepting a `SealedPlan` from off-node (violating
`SealedPlan`'s own construction guarantee that only the local planner produces one -
`docs/architecture/TARGET.md`'s "Sealed Plan Builder") or accepting an `AuthorityLevel` directly
from the wire (the identical shape of bug the E05 round-1 review already found and fixed for
`ProviderTrust`, and the one E18-S01's own adversarial-cases pass explicitly declined to repeat
for `RemoteTargetTrust`). No further design work is warranted; this option exists in this RFC only
to record why it was considered and rejected.

### Option C - observation/reporting only, no mutation trigger yet

The remote channel this story adds only lets a controller *ask questions* (current inventory
summary, pressure state) with zero ability to trigger even a policy evaluation; any resulting
action stays entirely a local, human-or-Guardian decision. This trivially satisfies AC2
("authorized" is always "read-only") and is the most conservative possible CR4 surface, but it
does not deliver E18-S02's actual outcome text ("even when a remote controller requests policy
evaluation") - it would leave the story's own acceptance criteria technically vacuous rather than
met. Worth naming as the fallback if Option A's scope turns out to be too large for one story once
implementation starts, in which case AC1/AC2 would need rewording to match, not silently
reinterpreted.

## Recommendation

Option A. It is the only option that lets a remote controller do something real (trigger a policy
evaluation) while making SI-031's "target node... retains final mutation authority" a property of
the type system and the existing `effective_authority` pipeline, not a promise a future author has
to remember to keep. It reuses three already-reviewed mechanisms (`KnowledgeBundle`'s signing/
replay shape, `TrustedTier`/`LocalTrustPolicy`'s trust-gating shape, and `EventLedger`'s audit
shape) rather than inventing a fourth kind of trust boundary for this codebase to maintain.

## Verification / prototype plan

Per the CR4 safety-proof style this story's own brief names:

1. **Invariant at risk**: SI-031 / C-01 / C-05 / C-07 - a remote party raising its own effective
   authority, or reaching mutation without going through `effective_authority` at all.
2. **Counterexamples to prove failure** (adversarial-cases pass before implementation, per
   `docs/development/AGENT_PROTOCOL.md`): replayed request (same controller, same or lower
   sequence); expired request; tampered payload with a digest that still matches (mirrors
   `knowledge_bundle`'s own tamper tests); a request naming an `ActionClass` its `TrustedTier`
   does not reach; an unlisted controller id; a request racing a local user's own concurrent
   action on the same artifact; a request for a target the local node does not recognize as a
   `RemoteTarget` (E18-S01) at all.
3. **Automated tests**: unit tests mirroring `knowledge_bundle::tests`' own tamper/replay/expiry
   matrix, plus an integration test proving `effective_authority`'s output for a remote-originated
   `user_requested` is identical to the same value supplied locally - i.e., no hidden second
   authority path exists for the remote case.
4. **Residual risk**: transport-level confidentiality/availability (a network observer sees
   request metadata; a network partition delays or drops a request) is explicitly out of scope
   (Non-goals) - Option A only says what happens to a request that *arrives*, not how reliably or
   privately it arrives. That is E18-S03's and any future transport story's residual to own.
5. **Rollback**: purely additive with no existing behavior changed if this story is reverted -
   deleting the new module and trust-policy type removes the entire remote-request code path, and
   `effective_authority` and every existing caller are unaffected because they never depended on
   it.

## Rollout and rollback

Ships as library-level primitives only (matching E18-S01's and E13's own precedent of landing a
mechanism with no CLI/TUI/Guardian wiring in the same story) - nothing calls this path in
production until a later story adds a real transport (E18-S03) and wires it to an entry point.
Rollback is deleting the addition; no migration or data format is left behind because nothing
persists beyond one `EventLedger` event kind, which is additive and ignored by every existing
reader.

## Open questions

- Does `TrustedRemoteControllers` reuse `cancellai_safety::knowledge_bundle::LocalTrustPolicy`'s
  exact type, or does controller trust warrant its own type distinct from publisher trust (they
  gate different `ActionClass`/capability shapes)? Recommend a distinct type for the same reason
  `RemoteTargetTrust` (E18-S01) is distinct from `ProviderTrust`: same pattern, different domain,
  and conflating them would let a knowledge-publisher promotion accidentally also grant remote-
  execution trust.
- Exact `EventLedger` event kind/schema for a remote request (accepted and rejected cases) -
  concrete field list is implementation detail for E18-S02 itself, not a decision this RFC needs
  to make.
- Whether a rejected request's audit record needs its own retention/rollup treatment under E13's
  self-budget (SI-026) or reuses the existing `EventLedger` budget untouched - likely the latter,
  confirmed during implementation.

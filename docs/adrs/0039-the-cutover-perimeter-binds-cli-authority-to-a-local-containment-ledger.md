# ADR-0039: The cutover perimeter binds CLI authority to a local containment ledger

- Status: Accepted
- Date: 2026-09-23
- Owners: project owner (decided 2026-09-23: local-ledger perimeter, project key plus
  owner-configured trust, explicit install command)
- Related: E06-S04, E06-S07, E17-S05, E17-S07, E33, ADR-0018, SI-022, SI-029, SI-030

## Context

E06-S04 cannot close while `docs/development/RELEASE_GATES.md`'s G4 operability leg names two
open items: signed capability containment (E17-S07) has no caller on the live mutation path, and
it has no distribution channel. E06-S04's own acceptance criteria allow "a scoped cutover
perimeter decided by ADR" in place of the full checklist. This is that ADR.

Three facts shaped it, all read from the code on 2026-09-23:

- The CLI does not compute effective authority at all. `clean` seals every plan at a constant
  `AuthorityLevel::Govern`, so neither the release-channel ceiling (E17-S05,
  `effective_authority_for_channel`) nor containment (`effective_authority_under_containment`)
  can reach a mutation. Both functions exist and neither has a caller.
- `LocalTrustPolicy` is never constructed outside tests. There is no source of trusted
  publishers in any runtime, so no notice could be verified even if something tried to ingest one.
- `ContainmentLedger` is in-memory. A ledger that is not persisted is lifted by restarting the
  process, which is precisely the rollback SI-029 forbids.

E17-S07's round-5 independent review (`project/evidence/E17-VERIFIER-REVIEW-ROUND5.md`) accepted
the ledger with two residuals: no byte cap on a bundle before parsing, and no live integration.
This ADR assigns both.

## Decision

The cutover perimeter includes containment on the live mutation path, fed by **locally installed**
signed notices. A network distribution channel is outside the perimeter and moves to E33.

1. **Authority is computed, not asserted.** Every action `plan` and `clean` produce gets its
   authority from `effective_authority_under_containment`, with the running build's compiled
   `BuildChannel` and the persisted ledger. The sealed plan carries that authority instead of a
   constant. An action whose effective authority is below
   `minimum_authority_for(action_class)` is downgraded to `Observe` with an explanation naming
   the constraint that capped it (`release_channel_authority` or
   `incident_containment_authority` and the incident id), in the same way ADR-0013's root gate
   withholds a custom root. `plan` and `clean` share the computation, so a preview never differs
   from a run.
2. **Trust is the project key plus the owner's choices.** The binary compiles in one Ed25519
   public key, the cancellAI incident-response key. The owner generates the pair and holds the
   private half offline; it never enters the repository or CI. An owner-private trust file in
   cancellAI's state directory may add publishers. Nothing read from a notice changes trust.
3. **A notice enters by an explicit command.** `cancellai-cli containment install <file>` reads
   at most a fixed byte cap *before* parsing (the round-5 residual), verifies against the trust
   policy, ingests into the ledger, and persists it atomically. `containment list` shows active
   evidence. `containment lift <incident-id> --confirm` is the only lift, a local act recorded in
   the ledger. Nothing is read implicitly from a directory.
4. **The ledger persists, and unknown is not empty.** The ledger (active evidence, per-publisher
   sequences, local lift records) lives in an owner-private file. A missing file means no notice
   was ever installed and the ledger is empty; that is the offline-equals-installed-kernel
   behaviour E17-S07 specifies. A file that exists but cannot be read, parsed or validated is
   **not** empty: every action class above `Recommend` is capped at `Recommend` until the owner
   repairs or removes it (C-01, unknown state is non-destructive). Expiry never lifts a persisted
   containment (SI-029).
5. **Implementation notes (E06-S07, 2026-09-23).** The persisted form is an append-only history
   of the raw installed bundles and local lifts, replayed and re-verified by the kernel on every
   load; evidence is never deserialized. (ADR-0040, 2026-09-24: the history is a SQLite table,
   read, decided and appended in one transaction, replacing the JSONL file.) The history resists every input from outside the owner's
   account but not the owner's own account: a same-user process can delete, truncate or append
   to it. The owner accepted that limit after Codex's design review raised it. Because the release
   channel is now live, every build without `CANCELLAI_CHANNEL=stable` compiled in - every local
   and test build - withholds every deletion; the parity gate, the performance budget and the
   stable-channel CI jobs build the engine as it ships.

## Consequences

- The kill-switch works without configuration: a notice signed with the project key contains
  every installation that installs it.
- Containment on this perimeter is only as fast as the owner installing a notice. That is the
  cost of keeping the network out of the cutover, and it is E33's to remove.
- The release-channel ceiling becomes live at the same time. A beta or nightly build loses
  whatever authority E17-S05's matrix withholds from it, which is a user-visible change the
  release notes must state.
- Key custody is a new operational duty for the owner. Losing the private key means rotating the
  compiled key in a release; leaking it lets an attacker *reduce* authority (a denial of
  service) and never raise it, because the ceiling vocabulary has no member above `Recommend`.

## Alternatives considered

- **Full perimeter, with a network channel before cutover.** Rejected by the owner: it needs a
  publication infrastructure and a fetch client that the cutover does not otherwise need, and
  it would add a network-facing input to the first canonical release.
- **No containment wiring at cutover.** Rejected: the canonical engine would ship with a
  kill-switch that exists only as a library, and both authority functions would stay callerless.
- **An inbox directory read on every run.** Rejected: a file left there would change authority
  implicitly, and lifting would still need a command.
- **Owner-configured trust only.** Rejected: containment would stay inert on every installation
  that never configured it, which is nearly all of them.

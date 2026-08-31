# ADR-0016: Rust artifact risk classification and the `clean` authority ceiling

- Status: Accepted
- Date: 2026-08-31
- Owners: project owner
- Related: PD-021, ADR-0007, E03-S04, E05-S03, E05-S04, E06-S01, C-04, C-05, SI-001, SI-007,
  SI-008, SI-009, SI-020

## Context

`docs/architecture/DOMAIN_MODEL.md`'s `AgentArtifact` and `RiskClass` sections, and
`cancellai-safety::authority`'s own module docs (E03-S04), are explicit that deriving an
artifact's `RiskClass` and the `AuthorityLevel` ceiling it grants is "a classification decision
this story does not invent" - deliberately left to whichever story first has the
provider/policy knowledge to make it. E06-S01 (Rust CLI contract parity) is that story: it is
the first to connect a discovered Claude/Codex session to a real `plan`/`clean` command, and so
is the first that must decide, concretely, what risk class an ordinary discovered session gets
and what authority ceiling that implies.

Two facts about the current implementation bound the decision:

- `cancellai-safety::mutation_executor::execute` implements exactly one destructive operation
  today, `ActionClass::Delete` on a plain file, requiring `AuthorityLevel::Govern`
  (`minimum_authority_for`). `ActionClass::Quarantine` has no OS-primitive wiring yet -
  `SealedPlan` does not even carry a quarantine destination - and `execute` explicitly refuses
  it (`mutation_executor.rs`'s own docs: "Quarantine needs a destination `SealedPlan` does not
  carry yet").
- `cancellai.py` (the frozen reference) itself performs real, permanent deletion for `clean`,
  not a reversible move-to-quarantine step.

## Decision

### Ordinary discovered sessions: `R3_RESUMABLE`, ceiling `Govern`, `Reversibility::Irreversible`

A discovered Claude/Codex session that is not a protected-name match gets `RiskClass::
R3Resumable` ("removal can destroy session/history/resume value," DOMAIN_MODEL.md's own
definition - matches the risk class already used in `tests/fixtures/schemas/golden/
inventory.golden.json`) and an `authority_ceiling` of `AuthorityLevel::Govern` - the exact
minimum `minimum_authority_for(ActionClass::Delete)` requires. `Reversibility::Irreversible`
follows the same real-capability constraint: this build cannot claim `Quarantinable` for an
action it cannot actually reverse.

**Ceiling `Quarantine` was considered and rejected for this story specifically because it would
make `clean` permanently unable to do anything.** `AuthorityLevel::Quarantine <
AuthorityLevel::Govern`, and `effective_authority`'s monotonic minimum means a `Quarantine`
ceiling caps every other constraint at `Quarantine` regardless of how permissive they are -
`minimum_authority_for(ActionClass::Delete)` (`Govern`) could then never be reached, by
construction. That is not a more conservative choice than `Govern`; it is a differently-broken
one that silently defeats the story's own acceptance criteria ("no flag implies clean" implies
clean must be *capable* of doing something when explicitly requested).

Constitution C-04 ("quarantine before purge... when a safe reversible transition is
*technically available*") does not mandate `Quarantine` here: quarantine is not technically
available in this build (no destination field, no OS-primitive wiring), so `Delete` is the only
destructive operation this build can honestly offer - exactly matching what
`cancellai.py`'s own `clean` already does. A future story that adds real quarantine support to
`cancellai-safety` (a `SealedPlan` quarantine-destination field, `MutationOperation::Quarantine`
wired into `mutation_executor::execute`) changes this mapping's second half - once quarantine is
technically available, C-04 then does require preferring it, and this ADR's `Govern`/`Delete`
choice for ordinary sessions becomes the *fallback* behavior, not the default. That is
future work, not a defect in this ADR's scope today.

### Protected-name matches: `R5_PROTECTED`, ceiling `Observe`, `Reversibility::Unknown`

Unchanged from the existing protected-name barrier (`cancellai-provider-{claude,codex}::
protected_names`, SI-006): a match is never destructive under any authority, by construction -
ceiling `Observe` means no `ActionClass` above `Observe` can ever pass
`minimum_authority_for`'s check for it, independent of every other constraint.

### Everything else stays generic, not a new special case

Degraded evidence (an unreadable/missing mtime, a companion payload that could not be fully
listed), a currently-running provider process, and low provider-root confidence are expressed
through the *existing* `KnowledgeConfidence`/`ActivityState`/`IntegrityState` fields feeding
`cancellai-safety::effective_authority`'s already-wired constraints (`confidence_ceiling`,
`lifecycle_ceiling`) - not through additional `RiskClass`/ceiling special cases in
`cancellai-policy`. The monotonic minimum these constraints already implement (E03-S04) collapses
authority toward `Recommend` for any of them independently; duplicating that logic per-condition
in the classifier would be redundant and a second place for the two to drift apart.

## Alternatives considered

### Ceiling `Quarantine` for ordinary sessions ("safer-looking" default)

Rejected: see Decision above - it makes `clean` permanently inert given today's
`mutation_executor`, which is not actually safer, just non-functional. A ceiling that makes the
one authorized destructive operation unreachable is indistinguishable, from a user's
perspective, from a bug.

### Ceiling `Autopilot` for ordinary sessions (skip the extra headroom)

Rejected: `Autopilot` is reserved for a future release-channel/fully-verified-trust
configuration this story does not build (`ReleaseChannelAuthority` is not wired into
`effective_authority` yet, per that function's own module docs). Capping at exactly `Govern` -
the real minimum `Delete` needs, no more - keeps this decision auditable against a concrete
requirement rather than an arbitrary "maximum" choice.

### Deferring `clean`'s real deletion capability entirely (ship `plan`-only in E06-S01, add `clean` execution in a later story)

Considered, since it would sidestep this decision for now. Rejected: E06-S01's acceptance
criteria and verification contract ("read-only default is explicit and no flag implies clean")
presuppose `clean` exists as the one mutating command being contrasted with the read-only
default - a CLI with no working mutating command at all does not satisfy that contract, it
avoids it.

## Consequences

### Positive

- `clean` performs real, working deletions today, matching `cancellai.py`'s behavior and this
  story's acceptance criteria, without inventing any new mutation model in
  `cancellai-safety` (SI-019's single mutation boundary is unchanged - see
  `cancellai_safety::execute_with_system_capabilities`, added alongside this ADR purely as a
  boundary-compliant production entry point, not a new authority path).
- The mapping is a two-bucket decision (protected vs. ordinary), auditable at a glance, with
  every other safety-relevant fact routed through already-reviewed, already-tested constraints
  instead of new ad hoc branches.

### Negative / cost

- No quarantine/undo exists for anything `clean` deletes in this build - deletion is genuinely
  permanent, same as `cancellai.py` today. This is a real, disclosed limitation, not a hidden
  regression from the Python reference.
- The `Govern`-ceiling choice is coupled to `mutation_executor`'s current single-operation
  scope; a future story adding `Quarantine`/`Archive` support must revisit this ADR's mapping
  (flagged explicitly above), not silently leave stale prose behind.

### Neutral / follow-up

- Real quarantine support (a `SealedPlan` destination field, `mutation_executor` wiring) is
  future work this ADR anticipates but does not implement.
- `ProviderCapabilityAuthority`/`ReleaseChannelAuthority` remain unwired in
  `effective_authority` (as before this story); nothing here changes that.

## Safety and compatibility impact

- Change Risk: E06-S01 is CR3 (reversible/conditional mutation, per `WORK_ITEM_MODEL.md`'s
  taxonomy - `clean`'s deletion is in fact irreversible in this build, but the *story* itself
  adds an execution path gated by explicit confirmation/`--yes`/`--dry-run`, consistent with
  CR3's "conditional mutation" framing already assigned in `project/epics/E06.json`).
- Safety Invariants affected: SI-001 (constitutional floor - unaffected, still enforced via
  `constitutional_safety_floor`), SI-007 (ambiguous CLI input never implies `clean` - enforced
  at the CLI argument-parsing layer, not by this mapping), SI-008/SI-009 (incomplete evidence
  never raises authority - unaffected, confidence/integrity constraints are unchanged),
  SI-020 (irreversible actions are stronger-gated - `Govern` ceiling plus
  `reversibility_allowed(Delete, Irreversible)` is exactly this gate, already enforced in
  `cancellai-safety::authority`, untouched by this ADR).
- Migration/rollback: none - this is a classification policy inside `cancellai-policy`
  (E06-S01, new crate content), not a change to committed state or a schema. Reverting to a
  different mapping is a future ADR, not a rollback of running data.

## Supersession

If replaced later (for example once real quarantine support lands), keep this ADR and mark it
superseded by the ADR that replaces it.

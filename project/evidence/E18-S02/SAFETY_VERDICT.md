# Safety Verdict - E18-S02

- Change: Local-agent remote execution boundary (`cancellai_safety::remote_execution`) - signed,
  replay-checked, target-bound `RemoteExecutionRequest` verification producing at most a
  `VerifiedRemoteIntent`
- Risk: CR4
- Review target: `2fe937d..4dcab69` (round 3 scope; cumulative with rounds 1-2 back to `96f645e`)
- Independent verifier: Codex
- Verifier: Codex
- Date: 2026-09-19

Brief-Checksum: ababc394cafa0c872500d5cbf99e99abcb7a2786c8e2a89a351740cc55021ed3
Verifier: Codex

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Authentication/authorization of a remote-originated semantic intent, signed target binding,
replay admission, and (as of this round) immutable checked provenance for the verified result
type itself. Three independent review rounds total: round 1 found and this repaired a replay
bypass and a target-confusion gap; round 2 found and this repaired `VerifiedRemoteIntent` being
publicly forgeable/mutable (F1); round 3 independently confirmed F1 is closed and found no
further bypass.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-031 / C-01 / TM-17 | Target authenticates intent; local node retains final authority | Signed-field/target/ceiling probes from an external scratch crate; opaque, immutable result type; local safety-floor tests; no production transport exists | PASS within library scope; integration residuals below |
| C-03 / C-05 | Ambiguity never escalates privilege | Malformed wire input refused; all 25 action/ceiling combinations agree with an independent exhaustive match; wire vocabulary cannot express `AuthorityLevel` directly; checked action is immutable post-verification | PASS |
| C-07 / SI-019 | No second mutation path | No mutation primitive in this module; `check_mutation_boundary.py` passes | PASS |
| C-18 | Claims match evidence | Round 3's own runtime/compile-fail reproductions independently established the current fix; residual prose-accuracy gaps in evidence packets are recorded, not hidden | PASS_WITH_RESIDUALS |

## Adversarial cases

- External struct-literal construction of `VerifiedRemoteIntent`: fails to compile (E0451, private
  fields).
- Four separate external field writes after genuine verification (`controller_id`, `sequence`,
  `target`, `requested_action`): each fails to compile (E0616).
- Private stateless verifier import from outside the crate: fails (E0603).
- `Deserialize`/`Default` construction attempts: fail (E0277 / E0599) - no such derive/impl exists.
- `Clone` was found to produce a second instance without a new log call; independently confirmed
  this always requires a prior genuinely-verified source and preserves (never escalates) its
  provenance - not a fabrication route.
- Replay: duplicate sequence, out-of-order sequence, `u64::MAX`/wraparound boundaries, key
  rotation under the same controller id, and 12-thread concurrent admission through one
  `Mutex`-protected log (exactly one success, eleven `Stale`) all behave correctly.
- Target confusion: a genuinely signed node-A request checked against node-B returns
  `TargetMismatch` without consuming the sequence; retrying for node-A then succeeds.
- Signature binding: tampering `requested_action` (or any envelope field) without re-signing
  fails; recomputing the digest without a valid signature still fails.
- Local safety independence: an empty/unconfigured remote policy refuses cleanly while a
  synthetic local Govern decision is unaffected; protected-state and unknown-integrity local
  inputs still lower a verified `Delete` request below destructive authority.

## Differential / compatibility evidence

`cargo fmt --check`, `clippy -D warnings`, `check --workspace`, `test --workspace` (including the
new `compile_fail` doctest), `cargo deny check` all pass locally and the exact reviewed commit
(`4dcab69`) is green across the full CI matrix (coverage, Ubuntu/macOS/Windows quality, and all
six stable/MSRV platform-check legs) - independently confirmed via `gh run view` at round 3, after
round 2's separately-reported Windows CI failures (unrelated to this module, in `cancellai-store`)
were fixed. `ActionClass` wire correction (ADR-0032) is intentionally breaking with no deployed
producer to migrate.

## Known residual risks

1. **Durable replay-state persistence is not provided.** `RemoteExecutionLog` is pure in-memory by
   this crate's own kernel-ring/no-I/O architecture (ADR-0019); a process restart, a fresh log, or
   a log cloned before admission each reset/fork the replay floor. This is mandatory to close
   before any real remote execution ships, and needs a future outer-ring caller (most likely
   `cancellai-store`) this workspace does not have yet.
2. **No integration exists.** No transport, persisted audit sink, execution consumer, operator
   configuration surface, or remote plan builder is wired to this primitive. Trusted clock input,
   current local policy/target identity at execution time, plan revalidation, and ingress resource
   limits remain future caller obligations.
3. **Documentation/test-maintenance gaps**, all non-blocking per round 3's own disposition:
   `E18-S02/EVIDENCE.md`'s AC2 row and Safety table still say "requested authority"/cite an
   `Autopilot`-vs-`Quarantine` example instead of the current `Delete`-vs-`Quarantine` one; its
   "Outcome" line and "no independent verifier yet" header text are stale; the ADR-0032 code
   example and module doc still show `minimum_authority_for(verified.requested_action)` rather
   than the accessor-method form; `every_action_class()`'s exhaustive match and its parallel
   `[ActionClass; 5]` array both need updating together if a variant is ever added (coverage
   limitation, not a reachable bypass).

## Rollback / recovery

The remote-execution module stays unwired to any production path; disabling or removing it
touches no local authority caller. This repair makes previously-public result fields private (an
intentional Rust API break closing F1) without changing the wire format again since round 2's
ADR-0032 correction - do not restore public/forgeable fields as a rollback strategy for anything.
No durable remote state exists to migrate; production crash-recovery is not claimed tested.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: Accepted 2026-09-19 as an unwired library primitive with no production transport.
The residuals above (durable replay-state persistence and full transport/audit integration) are
accepted as tracked, open risk - not resolved, and must be closed before this boundary can ever
authorize real remote mutation.

# Safety Verdict - E14-S05

Change: Live provider-layout capability gate at the mutation boundary
Risk: CR4
Verifier: Codex

This is an append-only round history (`docs/development/AGENT_PROTOCOL.md`): each round's record
stays when a later round repairs and passes it. The operative verdict is the most recent one by
position in this file (Round 2), not whether an earlier round ever failed - see
`scripts/project_os.py`'s `safety_verdict_passes` for the exact rule this file is read against.
Each round's full reproduction, adversarial cases, and gate results are recorded in
`project/evidence/E14-S05-VERIFIER-REVIEW.md` (both rounds, append-only in that one file); this
file is the consolidated verdict history the governance gate reads to decide whether E14-S05 may
close.

## Round 1 (E14-S05-VERIFIER-REVIEW.md)

`FAIL`. Three findings, all committed as reproductions in the review file:

- F1 (SI-004, AC2): `mutation_executor::execute`/`execute_all` were `pub`, so an external Rust
  crate depending on `cancellai-safety` as a library could call `execute` directly with a
  fabricated `SyntheticProviderLayoutObserver` alongside the real `SystemMutationExecutor`,
  reaching a genuine, unconfirmed deletion `scripts/check_mutation_boundary.py` cannot see.
- F2 (SI-004, SI-013, AC1): the fresh layout observation was not held through the actual
  `executor.mutate` call, leaving a TOCTOU window between the read and the real mutation.
- F3 (SI-004, SI-013, AC1/AC3): `seal_restore` recorded its layout snapshot against the
  quarantine store instead of the real destination provider root, leaving Restore plans with no
  real SI-004 protection at all.

## Round 2 (E14-S05-VERIFIER-REVIEW.md, appended) - operative verdict

Independent repair review of round 1's F1/F2/F3, re-diffing `c7f12bf..414a9fc` rather than
trusting the executor's own account. Verbatim from the review file's own CR4 Safety Verdict
section:

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

The public mutation entry point now requires a fresh, system-backed provider-layout observation
for the root the action actually mutates against. Injectable internal execution seams are no
longer callable by an external Rust consumer.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-004 | A fabricated, unknown, or drifted provider layout cannot preserve destructive capability for a real mutation. | Expected E0603 external direct-executor failure; system-wrapper fabricated-seal block; native Restore-destination drift block; fail-closed absent/error paths. | PASS_WITH_RESIDUALS |
| SI-013 | Relevant target and provider-root identity/preconditions are freshly checked immediately before mutation. | Actual last-precondition placement; root/marker comparisons; native Restore drift reproduction. Layout observation is not retained through the syscall. | PASS_WITH_RESIDUALS |

## Adversarial cases

- External fabricated-observer consumer cannot call `execute` because it is crate-private
  (reproduced independently: `cargo check --offline` on a standalone external crate fails with
  `error[E0603]: function 'execute' is private`).
- External synthetic seal-time contradiction cannot evade the wrapper's fresh system observation
  (a fabricated seal-time signature, executed via the real `execute_with_system_capabilities`,
  is safely blocked and the artifact retained).
- Restore into a provider root drifted after sealing is refused without moving the quarantine
  source (native reproduction: quarantine root + distinct provider destination root, marker
  added to the destination after sealing, `execute_with_system_capabilities` returns
  `SafelyBlocked`, quarantined artifact remains, no destination artifact written).
- Source-root action constructors (`seal`, `seal_with_process_guard`, `seal_quarantine`,
  `seal_archive`) retain their own source-root observation mapping, confirmed unaffected by the
  `seal_restore`-specific repair.

## Differential / compatibility evidence

No Python differential applies. Windows/non-Unix provider-layout observation remains
unsupported and therefore refuses destructive execution fail-closed; Windows runtime testing was
not available in the reviewer's environment, while the requested Windows cross-target lint
passed.

## Known residual risks

- **F2**: a concurrent provider-layout change between the final layout observation and the OS
  mutation syscall is not caught. Full closure needs a handle-retained directory-layout
  capability through the platform mutation operation (mirroring E21-S07's single-file identity
  confirmation) - scoped as its own future CR4 story, not attempted in this round. The reviewer
  judged this an acceptable disclosed residual for `PASS_WITH_RESIDUALS`, on the same precedent
  this repository already accepted for single-file identity before E21-S07's closure.
- **Windows/non-Unix**: destructive execution remains refused until a verified handle-bound
  provider-layout observation is implemented (unchanged from ADR-0036's own disclosed residual).
- **ADR-0036 residual (1)**: there is still no trusted known-layout-signature source; this story
  verifies live-vs-seal freshness, not known-good classification.
- CI status was unknown to the reviewer's environment (GitHub API connectivity unavailable);
  not treated as green.

## Rollback / recovery

Reverting `414a9fc` restores the rejected round-1 implementation and is not a safety-preserving
rollback. The safe recovery for the remaining F2 residual is to withhold the action when a
provider root cannot be reliably observed, which the current implementation already does; a
future handle-retained design should receive its own CR4 story and review.

## Owner decision

Independent verifier recommendation: `ACCEPT_WITH_RECORDED_RESIDUALS` (Round 2, Codex) - "owner
acceptance remains required. This verifier does not change the story status or make that owner
decision." Owner acceptance obtained explicitly in-session (2026-09-22): presented with the
three named residuals above - F2 (TOCTOU window, narrowed not closed), the Windows/non-Unix
destructive-mutation narrowing this story introduces, and ADR-0036's pre-existing residual (1) -
the owner (Matteo Pugliese) confirmed acceptance and directed closure of E14-S05 and epic E14 on
this verdict. The story moves to `done` on this verdict.

# Safety Verdict - E00-S05 (owner acceptance)

- Change: scan-completeness authority gate
- Risk: CR4
- Decided by: **project owner**, not an independent verifier
- Date: 2026-08-28
- Independent review history: round 1 rejected three unrecorded observation paths; round 2 rejected `Path.exists()` guards that collapsed access failures before anything could be recorded. Both are repaired.

## What this file is, and is not

This is not an independent verification. The owner directed closure of E00 without a further
review round, and this records that decision under the authority the Constitution reserves to
the owner for risk acceptance. The independent verdicts that preceded it are committed beside
this file and are not altered.

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Whether an incomplete observation can authorize deletion.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 | Partial inventory cannot authorize deletion | An unreadable path withholds destructive authority for that provider | PASS |
| SI-009 | Missing evidence is non-destructive | `observe()` separates absent from unreadable at every discovery guard | PASS |
| SI-010 | Scan errors are visible | `status` lists unreadable paths and prints partial totals as lower bounds | PASS |

## Adversarial cases

- a directory at mode 000 inside a scanned scope;
- an unreadable provider root, so the guard itself is what fails;
- an unreadable session transcript, which changes lineage evidence;
- an unreadable `history.jsonl`;
- a plan hand-built with an incomplete scan, refused at execution;
- a path that vanished mid-scan, recorded as a race rather than as blindness;
- 75 recorded errors against a bound of 50.

## Differential / compatibility evidence

The baseline returned `0` and `[]` from every failed observation, so an unreadable tree was
indistinguishable from an empty one. A full audit of every `except OSError` and
`contextlib.suppress(OSError)` in the module accompanied the repair; the remaining sites
carry a comment stating why they are fail-closed.

## Known residual risks

- Completeness is tracked per tool, not per scope: one unreadable path anywhere under a
  provider root withholds all cleanup for that provider in that run. Coarse, and deferred to
  the inventory engine in E04. Measured on a real machine this produced zero errors.
- The remaining error-swallowing sites are fail-closed by construction rather than by test:
  marker validators can only reduce confidence, and `prune_empty_dirs` runs after mutation.
- No independent verifier examined the final state of this story. Closure rests on the
  executor's evidence and the owner's acceptance.

## Rollback / recovery

Revert the merge commit that carried this story. The Python reference keeps no persistent
state, so there is nothing to migrate back.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: the owner directed closure without a fourth review round, having read the
residual risks above.

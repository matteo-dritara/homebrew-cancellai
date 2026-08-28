# Safety Verdict - E00-S09 (owner acceptance)

- Change: provider-activity observation
- Risk: CR4
- Decided by: **project owner**, not an independent verifier
- Date: 2026-08-28
- Independent review history: opened during round-2 remediation after an executor audit found the defect; round 2 rejected treating any parseable output as a complete observation. Repaired.

## What this file is, and is not

This is not an independent verification. The owner directed closure of E00 without a further
review round, and this records that decision under the authority the Constitution reserves to
the owner for risk acceptance. The independent verdicts that preceded it are committed beside
this file and are not altered.

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Whether unknown provider activity can be read as absence of activity.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-009 | Missing evidence is non-destructive | An incomplete observation blocks every target tool | PASS |
| SI-011 | Shared provider metadata is not rewritten under unsafe concurrency | The history trim is skipped when activity is unknown | PASS |
| SI-014 | Safety-blocked is not success | The run reports exit 4 and a populated `deferred` list | PASS |

## Adversarial cases

- `ps` missing, failing, timing out, or exiting non-zero;
- empty output;
- unparsable output;
- output listing only unrelated processes;
- output naming a target provider but not this process.

## Differential / compatibility evidence

The baseline returned an empty mapping on failure, which is byte-identical to "no provider is
running" - a fail-open safety signal inside a trust-floor epic. Completeness now requires the
listing to contain this process, because a full process listing necessarily does.

## Known residual risks

- Detection remains best-effort on success: exact-name matching cannot prove that no writer
  exists. Failing closed on observation failure is a different property from proving absence.
- `--allow-running` overrides an unknown observation. That is deliberate - the operator
  accepting a stated risk - but it is the widest remaining hole in this gate.
- No independent verifier examined the final state of this story. Closure rests on the
  executor's evidence and the owner's acceptance.

## Rollback / recovery

Revert the merge commit that carried this story. The Python reference keeps no persistent
state, so there is nothing to migrate back.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: the owner directed closure without a fourth review round, having read the
residual risks above.

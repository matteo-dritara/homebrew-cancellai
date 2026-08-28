# Safety Verdict - E00-S01 (owner acceptance)

- Change: protected-name barrier at planning and execution
- Risk: CR4
- Decided by: **project owner**, not an independent verifier
- Date: 2026-08-28
- Independent review history: round 1 rejected a symlink bypass; round 2 rejected case-sensitive matching; round 3 rejected Unicode-naive folding. Each finding is repaired.

## What this file is, and is not

This is not an independent verification. The owner directed closure of E00 without a further
review round, and this records that decision under the authority the Constitution reserves to
the owner for risk acceptance. The independent verdicts that preceded it are committed beside
this file and are not altered.

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Filesystem deletion protection for named provider state.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-001 | Protected state is non-destructive | Injected protected actions are refused at execution | PASS |
| SI-003 | Mutation stays bounded | A protected symlink pointing outside the root keeps its protection | PASS |
| SI-006 | Barrier holds at planning and execution | Enforced in `build_plan` and again in `safe_remove`, on the lexical and the resolved view | PASS |

## Adversarial cases

- protected entry that is itself a symlink pointing outside the root;
- `..` traversal reaching a protected component;
- case variants `Plugins`, `PLUGINS`, `pLuGiNs`, both providers;
- composed and decomposed Unicode spellings in both directions, combined with case variance;
- a scanner patched to emit a protected root as a candidate;
- `--aggressive` attempting to reach protected names.

## Differential / compatibility evidence

The baseline had no executable barrier at all: the constants were documentation, and
protection depended on scanners never emitting those paths. Every case above is now refused
inside `safe_remove`, not merely filtered during planning. The four Unicode regressions fail
against `review/e00-round2-rejected` and pass on `main`.

## Known residual risks

- Matching is over-inclusive on case-sensitive volumes: a distinct directory differing from a
  protected name only by case or Unicode form cannot be cleaned. Non-destructive direction.
- The `codex-cli` strategy deletes by session id and is exempt from the path barrier by
  design; its safety rests on Codex's own deletion semantics.
- No independent verifier examined the final state of this story. Closure rests on the
  executor's evidence and the owner's acceptance.

## Rollback / recovery

Revert the merge commit that carried this story. The Python reference keeps no persistent
state, so there is nothing to migrate back.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: the owner directed closure without a fourth review round, having read the
residual risks above.

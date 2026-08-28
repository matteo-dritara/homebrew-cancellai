# Safety Verdict - E00-S02 (owner acceptance)

- Change: provider-root authority for destructive work
- Risk: CR4
- Decided by: **project owner**, not an independent verifier
- Date: 2026-08-28
- Independent review history: round 1 rejected filename markers; round 2 rejected content-validated structure plus an operator flag. ADR-0013 replaced both schemes.

## What this file is, and is not

This is not an independent verification. The owner directed closure of E00 without a further
review round, and this records that decision under the authority the Constitution reserves to
the owner for risk acceptance. The independent verdicts that preceded it are committed beside
this file and are not altered.

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Which directories may be mutated at all.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-002 | Root is positively bounded | Only the provider's own default directory is mutable | PASS |
| SI-004 | Layout drift reduces capability | The structural fingerprint is reported and explicitly non-authoritative | PASS |
| SI-009 | Missing provider identity is non-destructive | Every custom-root shape - empty, weak, structurally perfect - is refused | PASS |

## Adversarial cases

- an ordinary project directory containing `tmp/` and `log/`;
- a root carrying a single supporting marker;
- a lookalike with valid-JSON `auth.json`, a real `config.toml` and a genuine
  `rollout-<uuid>.jsonl` - the counterexample that rejected the previous scheme;
- `configure`, which writes provider configuration, pointed at a custom root;
- a plan hand-built to bypass the planning gate, refused again at execution.

## Differential / compatibility evidence

The baseline accepted any path with three or more components that was not `/` or `$HOME`.
Two intermediate schemes were built and rejected by independent review before the current
one. ADR-0012 is retained rather than deleted and links forward to ADR-0013, so the reasoning
that produced the rejected designs stays readable.

## Known residual risks

- This is a capability regression: an operator who relocated a provider root has no supported
  cleanup path until the Rust core ships provider-native identity (E05).
- Authority derives from a path comparison against the default home. An attacker who can move
  or replace the default directory defeats it; that is outside the threat model of a local
  tool run by the owner of the directory it cleans.
- No independent verifier examined the final state of this story. Closure rests on the
  executor's evidence and the owner's acceptance.

## Rollback / recovery

Revert the merge commit that carried this story. The Python reference keeps no persistent
state, so there is nothing to migrate back.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: the owner directed closure without a fourth review round, having read the
residual risks above, including the loss of custom-root cleanup.

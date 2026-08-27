# Safety Verdict - E00-S02

- Change: provider-root fingerprint authority gate
- Risk: CR4
- Commit/PR: working tree against `4b2df0130e62d83e3a10caaae73daa456211f92d`
- Independent verifier: Codex
- Date: 2026-08-27

## Verdict

`FAIL`

## Safety surface changed

Custom provider roots may now receive destructive authority.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-002 | Root is positively bounded | An ordinary directory with `auth.json` and a `sessions/` tree gets `high` confidence. | FAIL |
| SI-004 | Layout drift reduces capability | Generic names are treated as provider identity rather than unknown. | FAIL |
| SI-009 | Missing provider identity is non-destructive | The generic fixture emits a destructive filesystem action. | FAIL |

## Adversarial cases

- `tests/test_cancellai.py::IndependentVerifierAdversarialTests::test_generic_custom_root_can_falsely_earn_high_authority` fails. The synthetic ordinary project is marked destructive-allowed and its old rollout enters the plan.

## Differential / compatibility evidence

- Default-root and low-marker tests pass, but they do not establish that the two accepted marker names are provider-specific.

## Known residual risks

- `CODEX_HOME` can authorize deletion from an unrelated project whose files happen to use these common names.

## Rollback / recovery

- Do not rely on fingerprinting for custom-root deletion until identifying evidence is strengthened or custom roots are restricted to inspection.

## Owner decision

`REJECT`

Owner note: implementation/authority-model defect must be resolved before CR4 closure.

# Safety Verdict - E00-S01

- Change: protected-name planning and execution barriers
- Risk: CR4
- Commit/PR: working tree against `4b2df0130e62d83e3a10caaae73daa456211f92d`
- Independent verifier: Codex
- Date: 2026-08-27

## Verdict

`FAIL`

## Safety surface changed

Filesystem deletion protection for named provider state.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-001 | Protected state is non-destructive | Protected `plugins` symlink can be unlinked. | FAIL |
| SI-003 | Mutation stays bounded | The link target is not followed, but the protected root entry itself is still removed. | FAIL |
| SI-006 | Barrier holds at planning and execution | `protected_component()` resolves the link before inspecting components, then `safe_remove()` unlinks it. | FAIL |

## Adversarial cases

- `tests/test_cancellai.py::IndependentVerifierAdversarialTests::test_protected_symlink_name_cannot_be_unlinked` fails: `safe_remove(root / "plugins", root, CODEX_PROTECTED_NAMES)` deletes the link.

## Differential / compatibility evidence

- The baseline lacked any executable barrier. The new code improves ordinary paths but fails the story-required symlink case.

## Known residual risks

- A scanner or direct caller emitting a protected symlink can delete it, including under future category expansion.

## Rollback / recovery

- Do not release this barrier as complete; restore the symlink manually from its target or provider installation if deleted.

## Owner decision

`REJECT`

Owner note: implementation defect must be fixed and independently re-reviewed.

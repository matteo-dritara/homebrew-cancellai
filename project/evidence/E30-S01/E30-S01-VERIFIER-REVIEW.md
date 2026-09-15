# Independent Verifier Review — E30-S01

Verifier: Codex
Brief-Checksum: c9972e04c6e0e006af579793120e585bb12b0cf0b48b449a77aa063f52c6ad0f
Date: 2026-09-15

## Verdict

REPAIRED

The rule requiring `blocked_by` only when declared dependencies do not explain a block is a sound
avoidance of duplicate authority: an open dependency is already machine-readable truth. It leaves
a declared residual — a distinct blocker can hide behind that dependency — but the packet states it
plainly and that trade-off is proportionate to CR2 control-plane metadata.

The implementation did not meet AC2. Both schema variants accepted a `blocked_by` without
`argument`, and `blocked_by_errors()` accepted a missing or unreadable path. It also accepted an
unknown `waiting_on` identifier and treated a cancelled story as open. Those records are not
interrogable explanations. The repair makes `argument` mandatory, requires it to resolve to a
repository file, rejects unknown wait targets, and treats cancelled targets as closed.

Checking `blocked_by` after dependency validation is also correct: a malformed dependency graph is
the more fundamental invalid state. The repair's direct tests call the narrower blocker validator,
so its own failures remain observable without weakening that ordering in the full gate.

## Reproductions and verification

- Before repair, `blocked_by_errors()` returned `[]` for a blocked item with only `summary` and
  `recorded`; the schema also lacked `argument` in `required`.
- New adversarial tests reproduce missing and unreadable arguments, unknown `waiting_on`, and a
  cancelled wait target.
- `python3 -m pytest tests/test_blocked_reasons.py tests/test_work_item_references.py -q` — 24 passed.
- `python3 scripts/project_os.py check` and `python3 scripts/check_schemas.py check` pass.

## Method defects

- none.

## Residual risks

- An item blocked for a reason unrelated to an open declared dependency may still omit
  `blocked_by`; this is the story's disclosed trade-off, not removed by this review.
- This is retrospective review of a closed `done_no_release` epic. It does not reopen E30; the
  owner should reopen it if the repaired finding changes that decision.

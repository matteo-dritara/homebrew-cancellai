# Safety Verdict - E06-S15

## Verdict

The latest round below is authoritative.

## Round 10 — 2026-09-24

Verifier: Codex
Brief-Checksum: 13575c2d3f9fdea6c85190d0e7daae57f1299d780b4857a7236d69da5e915ab3
Change: explicit cutover authorization bound to the migration Safety Verdict
Risk: CR4
Commit/PR: `8ea967e..0afe3ad`

### Safety surface changed

The one-time Rust formula cutover requires a versioned authorization and the
digest of the final passing migration Safety Verdict before formula mutation.

### Invariants and adversarial cases

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 / E06-S15 AC1–AC3 | Status alone grants no cutover; missing, stale, wrong-version or failing-verdict authorization refuses before the formula write. | Inspected `cutover_authorization_problems` and `finalize` call order. `tests/test_release.py` passed 68 tests, including the real verdict parser and field checks. | PASS |

The owner name is an identity claim from `.github/CODEOWNERS`, not cryptographic
authentication. ADR-0040 explicitly accepts the same-user limit. No actual
owner authorization file exists for the pending migration, so this verdict
does not authorize cutover. Main CI was unavailable here.

### Recovery

On refusal, `finalize` has not written the live formula. The owner can add a
version-bound authorization only after accepting a passing E06-S04 verdict.

### Owner decision

Pending for the migration itself.

PASS

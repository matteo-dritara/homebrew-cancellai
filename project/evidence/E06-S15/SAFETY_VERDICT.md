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

## Round 11 — 2026-09-24

Verifier: Codex
Brief-Checksum: 13575c2d3f9fdea6c85190d0e7daae57f1299d780b4857a7236d69da5e915ab3
Review target: `0afe3ad..2414975a40538cbda0d4a60f59945906d2eba062`
Risk: CR4

### Safety surface and evidence

`finalize --adopt-cutover` checks the owner identity claim, version, exact
Safety Verdict SHA-256, and final passing round before writing the live
formula. In an independent temporary-file probe, missing authorization,
changed verdict, a failing final round followed by an owner-note `PASS`,
stale digest, and wrong version all refused. A valid authorization passed;
`load_epic` was patched to raise if story status was consulted and was not
called. The parser's E35 repairs and `finalize` call order were inspected.

| Invariant / obligation | Required property | Result |
| --- | --- | --- |
| E06-S15 AC1–AC3, SI-019 | Only the owner's version- and verdict-bound authorization can permit the one-time cutover. | PASS |

The identity field is an owner-accepted claim, not authentication
(ADR-0040). No actual cutover authorization file exists; this passing story
verdict does not authorize E06-S04 or a release. Main CI was unavailable to
this reviewer. Full evidence:
`project/evidence/E06-VERIFIER-REVIEW-ROUND11.md`.

### Owner decision

Pending for the migration cutover.

PASS

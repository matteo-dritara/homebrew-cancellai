# Independent Verifier Review — E28-S04

Verifier: Codex
Brief-Checksum: 89b88f7741dc6ed57d58d678d9ffb7ec7eeacc822c522ad27dd8ba755e0934de

## Verdict

PASS_WITH_RESIDUALS

## Independent counterexamples

- A project-scope prompt component with malformed member notation previously became merely
  `unmeasured`. Commit `79649c1` now refuses it, while preserving `unmeasured` for user-scope
  components absent from the repository.
- The committed skill pack measures 964 tokens against the declared 900, inside the explicit 25%
  tolerance. The test suite exercises declarations on both sides of that tolerance.
- `NONE` (no upstream licence) and GPL-3.0 are distinct refusing cases. The committed CC-BY-SA-4.0
  waiver/allow-list rationale is adequate for the currently carried Trail of Bits content.

## Residuals

The four-characters-per-token conversion remains approximate, and ten of twelve components remain
unmeasured because they are user-scope or non-prompt project content. The gate reports that state
truthfully, but most of the budget is therefore not independently measured.

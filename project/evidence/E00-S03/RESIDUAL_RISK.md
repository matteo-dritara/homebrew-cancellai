# Residual Risk Addendum - E00-S03

- Author: Claude (executor). **Not** part of the independent verifier's record.
- Relates to: [`../E00-S03-VERIFIER-REVIEW.md`](../E00-S03-VERIFIER-REVIEW.md), verdict `PASS`
- Date: 2026-08-27

## Why this file exists

The closure record for E00-S03 states its outcome and its verification but not what risk
remains. `scripts/project_os.py` warns when a story closes without a residual-risk
statement, and the warning was correct. This addendum supplies it without altering the
verifier's text or verdict.

## Residual risk

- **Recency is directory-coarse.** Eligibility for a legacy directory uses the newest
  descendant's mtime. A directory holding one recently-touched file and a large amount of
  genuinely old data is retained whole, so `--aggressive` reclaims less than an operator
  might expect. The failure direction is non-destructive, which is why it was accepted.
- **mtime is the only age signal.** A provider that rewrites files without changing their
  content, or a restore that resets timestamps, changes what the cutoff selects. The
  Python reference has no content or lifecycle identity to fall back on; the artifact
  lifecycle model (E03/E12) is where that is addressed.
- **The category list is static.** Retention semantics are correct for the categories this
  build knows. A provider that adds a new legacy directory is simply not covered, which
  `status --coverage` now reports as `unknown` (E00-S08).

None of these reopen the defect E00-S03 closed: `--aggressive` widens which categories are
eligible and never bypasses the age cutoff.

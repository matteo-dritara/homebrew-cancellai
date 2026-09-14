# Evidence Packet - E27-S07

- Source finding: E27-S06 independent verifier, 2026-09-14
- Status: planned
- Change Risk: CR1

## Finding

python3 scripts/check_coverage.py check failed twice on the same macOS checkout:

    cancellai-platform: 93.24% measured, 95.83% recorded

The recorded baseline is commit 2fda2cf. There is no cancellai-platform source diff from that
commit to the E27-S06 review target or verifier repairs. This packet exists to prevent a silent
baseline reduction in an unrelated CR4 verification.

## Required investigation

- Compare cargo llvm-cov and compiler/tool provenance with the baseline measurement.
- Determine whether test selection, coverage report attribution, or tool version explains the
  result.
- Add a regression check before changing the ratchet or baseline.

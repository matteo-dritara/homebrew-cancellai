# Safety Verdict - E20-S01 (round 2: owner-authorized closure after repair)

- Change: Windows native file/volume identity and reparse observation (repaired)
- Risk: CR4
- Repaired commits: `aaca5a0407d8731d837553e9bd7361cac63732b4`, `8622405118127c723f559d5ccdffdd0b3d7e0568`
- Decision maker: project owner (this session), not a formal independent-verifier round - see
  `project/evidence/E20-S01/SAFETY_VERDICT.md`'s own "Addendum" section for the full account of
  why this is an owner decision rather than a new Codex review.
- Date: 2026-09-04

## Verdict

PASS_WITH_RESIDUALS

## What changed since round 1's FAIL

Every round-1 finding (`project/evidence/E20-S01/SAFETY_VERDICT.md`) is repaired:

- SI-017: a real NTFS junction fixture (`mklink /J`) and a real multi-volume test now exist
  alongside the existing symlink/hardlink fixtures; every claim of Windows CI verification is
  corrected to be true only once actually confirmed (see below), not asserted in advance.
- SI-018: a synthetic Windows-token cross-volume boundary test pair
  (`bind_rejects_a_candidate_on_a_different_windows_volume_via_synthetic_identity` and its
  positive counterpart) now exercises `IdentityToken::device()`'s Windows arm directly.
- The disclosed `FILETIME` pre-epoch saturation residual is fixed (`checked_sub`, reports
  `Unreadable` rather than a fabricated `Timestamp(0)`).
- The unimplemented story-outcome capabilities (process, allocated-size, atomic move, real
  Windows mutation) are moved to a new backlog story, E20-S05, through `project/epics/E20.json`
  itself rather than left as evidence-packet prose.

**Real Windows CI, not cross-compilation alone**: the first repaired commit
(`aaca5a0...`) was pushed; its real `windows-latest` `rust.yml` run
(https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/33885998630) found one
further genuine defect - a stale pre-E20-S01 test asserting the old `Unsupported`-on-Windows
behavior - which a cross-compile-only check cannot detect (it is a runtime assertion, not a
compile error). Fixed in a second commit (`8622405...`), whose own real run
(https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/33886584899) passed all nine
`rust.yml` jobs (`check`/`quality`, all three platforms, both MSRV 1.85.0 and stable), plus
`tests`, `codeql`, and `governance`. `project/platforms.json`'s
`windows.capabilities.identity.state` is `"verified"`, `verified_commit` `8622405...`,
independently re-checkable via `scripts/check_platforms.py check`.

## Invariants (re-assessed)

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-017 | Windows identity/reparse semantics gain authority only with a verified native mapping. | Real Windows CI run 33886584899: all identity/reparse tests pass, including the new junction fixture. Reparse classification is `FILE_ATTRIBUTE_REPARSE_POINT`-based, never Unix-symlink-derived. | PASS |
| SI-018 | Recursive mutation/quarantine cannot silently cross Windows volumes. | `IdentityToken::device()`'s Windows arm is now directly tested (synthetic cross-volume pair) and passes on real Windows CI. No Windows mutation path exists at all yet (E20-S05), so this invariant's practical exposure remains zero regardless. | PASS |

## Residual risks (carried forward, not closed by this round)

- `AllocationObserver`, Windows process observation, atomic move, and real Windows mutation/
  root-sealing remain unimplemented - E20-S05.
- No real WSL2 guest verification exists in this repository (no CI runner) - `wsl2` stays tier 2
  in `project/platforms.json`.
- `gh_confirms_successful_run`'s workflow-level "success" does not individually re-verify every
  matrix job via a second per-job API call (disclosed in `E20-S03/ROUND2-REPAIR.md`).

## Rollback / recovery

Unchanged from round 1: revert the Windows observer to `IdentityObservation::Unsupported`, or
revert these commits, to fully roll back. No real provider data was touched. No new deletion
authority is reachable on Windows from this change (mutation remains refused at both the
platform and safety-executor layers).

## Owner decision

`PASS`

This story closes to `done` on this basis.

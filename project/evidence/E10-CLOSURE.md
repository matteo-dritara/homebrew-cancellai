# E10 Closure - owner-directed, self-review accepted in lieu of independent review

- Epic: E10 - Storage Accounting and Performance
- Decided by: **project owner** (Matteo Pugliese)
- Date: 2026-09-12

## What this file is, and is not

This is not an independent verification, for the same reason recorded in
`project/evidence/E09-CLOSURE.md`: no Codex CLI is available in this environment, so
`project/evidence/E10-SELF-REVIEW.md` is Claude reviewing Claude's own work through a fresh,
context-isolated session, not the independent review `docs/development/AGENT_PROTOCOL.md`'s
standing assignment calls for. The owner reviewed that self-review's findings and explicitly
decided to accept it as sufficient to close this epic.

## What the self-review found, and what closes it

The self-review's initial verdict for this epic was **FAIL** for E10-S01, not a residual: a
concrete, twice-reproduced regression that breaks a mandatory, unconditional CI gate on a
required tier-1 platform (`cargo clippy --workspace --all-targets --all-features -- -D warnings`
on `windows-latest`), caused by `filesystem_kind.rs`'s `classify_filesystem_name` and its two
backing constants being dead code on Windows (nothing in a Windows production build called
them). This is not the kind of finding a closure normally accepts as residual risk - it is a
build break - so it was repaired before this closure, not carried forward:

- Gated `KNOWN_NOT_SHARING`, `KNOWN_SHARING`, and `classify_filesystem_name` to
  `cfg(any(test, target_os = "macos", target_os = "linux"))`, the exact precedent
  `crate::wsl::classify_fstype`/`longest_matching_mount_fstype` already set in the same crate
  for the identical shape of gap.
- Re-verified on all three targets this session actually has installed
  (`x86_64-apple-darwin` native, `x86_64-unknown-linux-gnu` cross, `x86_64-pc-windows-gnu`
  cross) - not just the one target (Linux) the story's original evidence packet had exercised.
  See `project/evidence/E10-S01/EVIDENCE.md`'s addendum and
  `project/evidence/E10-SELF-REVIEW.md`'s own addendum for the full command list and results.

With that repair applied and re-verified, the self-review's own recommendation ("no other
blocking defect was found in either story's own acceptance criteria") stands: E10-S01's estimator
logic (AC1/AC2) and E10-S02's memory gate (both ACs) were independently traced against
counterexamples beyond the committed tests and held.

## Residual risk the owner accepts

1. **No independent confirmation of either story**, for the reason above - same
   model-family limitation as E09's closure.
2. **Real Linux execution of the memory gate has never happened in any environment available
   to either the executor or the self-reviewer** - both cross-compiled and `clippy`-checked
   `x86_64-unknown-linux-gnu` but neither had a Linux runtime to actually run
   `performance_memory.rs`'s test or `performance_scheduled_shipped.rs`'s heavy datasets. First
   real execution is the next `rust.yml` CI run on Linux tier-1.
3. **macOS and Windows have no real peak-memory measurement at all** (E10-S02's own disclosed
   scope boundary) - a regression there would not be caught by this epic's gate.
4. **Filesystem clone-detection has no real Windows implementation** (E10-S01's own disclosed
   scope boundary) - `CloneSemantics::Unsupported` on Windows is honest, not a defect, but it
   means `estimate_reclaim` can never report `Verified` for a Windows-observed scope today.

## Consequence for E11

`E11` (Deterministic Policy Engine) does not depend on E10 directly - only on E09. This closure
does not itself unblock anything E11 needs, but it keeps both P2-epic closures (E09, E10)
consistent with each other rather than closing one under owner-accepted self-review and leaving
the other in limbo.

## Release

Both E09 and E10 close in the same release, `project/evidence/RELEASE-v1.13.0.md` - both were
verified and owner-accepted in the same decision at the same time, so one release names both
rather than forcing a second, artificial version bump for work already landing in the same tag
(the historical one-epic-per-version pattern in `project/evidence/RELEASE-v*.md` reflects epics
having closed sequentially in time, not a rule that a release may name only one).

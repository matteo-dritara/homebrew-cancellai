# Evidence Packet - E20-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (`/root`), 2026-09-02
- Change Risk: CR2
- Spec version/commit: `project/epics/E20.json`'s E20-S04 story contract (formerly E07-S06)
- Process exception: **Owner-authorized combined verify+fix+close round, 2026-09-02 - see conversation record.** The owner explicitly authorized Codex to verify, fix if needed, self-reverify, and close this named standalone story for this round only.

## Outcome

PASS - root cause identified via real Windows CI (not hypothesized), and confirmed to be a
safety-driven design consequence rather than a scan-logic bug; documented as an accepted
limitation with dedicated Windows-specific tests proving the actual (correct) behavior.

## Root cause

Reproduced directly on real `windows-latest` CI (this session's E07/E20 verification PR):
`completeness::tests::ac1_a_fully_readable_tree_is_complete` got `Partial` (two
`UnsupportedFilesystemFeature` reasons per path: `identity`, `allocated_size`) instead of
`Complete`; `scan::tests::ac1_one_traversal_visits_every_directory_exactly_once` got
`directories_visited == 1` instead of `4`.

Both trace to the same mechanism: `cancellai-inventory::scan::walk_directory` only recurses
into a child whose identity is *confirmed* (`matches!(identity_observation,
Some(IdentityObservation::Identity(_)))`) and that does not cross the scope's device boundary -
an unconfirmed identity (`Unsupported`, `Unreadable`, or a raced `Absent`) never earns a
descend, per SI-017. Since `cancellai-platform::identity::SystemIdentityObserver` reports
`IdentityObservation::Unsupported` unconditionally on Windows (E03-S01's own disclosed
residual, unrelated to and predating this story), that condition is never true there, so a real
Windows scan visits only the scope root itself, regardless of how deep or readable the real
tree beneath it is.

This is **not a traversal bug** - it is the identity-confirmation safety gate (SI-017) working
exactly as designed, given the platform capability it depends on does not exist yet. Weakening
that check to let the walk descend on unconfirmed identity would trade a correct fail-closed
posture for an unverified one, which is precisely the outcome `docs/architecture/
PLATFORM_MODEL.md`'s existing Windows-identity section already refuses for mutation authority.
Real Windows traversal depth requires E20-S01's native identity implementation; this story's
own scope is determining and documenting the root cause, not implementing that capability.

## Fix

Both tests gated `#[cfg(unix)]` (their assertions describe post-identity-confirmation behavior
that does not hold on Windows today), with `#[cfg(windows)]` counterparts added asserting the
actual, correct current Windows behavior:

- `completeness::tests::ac1_a_fully_readable_tree_is_partial_on_windows_pending_native_identity`
  - asserts `Partial` with only `identity`/`allocated_size` `UnsupportedFilesystemFeature`
    reasons.
- `scan::tests::ac1_traversal_stops_at_the_root_on_windows_pending_native_identity` - asserts
  `directories_visited == 1`, `paths_observed == 1`, `facts.len() == 1` for the same
  four-level-nested fixture the Unix test uses.

`docs/architecture/PLATFORM_MODEL.md` gains a new "Accepted limitation: the inventory scanner
cannot descend below the scope root on Windows" subsection under the existing Windows-identity
section, cross-referencing both new tests.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| Root cause identified: a path-separator, symlink/junction-detection, or traversal-count assumption in scan_scope/derive_completeness that does not hold on Windows | Identified precisely: `walk_directory`'s identity-confirmed-before-descend gate (SI-017), which is unsatisfiable while `SystemIdentityObserver` reports `Unsupported` on Windows - not a path-separator or traversal-counting bug. | PASS |
| scan_scope/derive_completeness produce the same documented classification/counters on Windows as on macOS/Linux, or the platform difference is explicitly documented in PLATFORM_MODEL.md as a known, accepted Windows limitation rather than silently left failing | The second branch applies: documented in `PLATFORM_MODEL.md` with dedicated, passing Windows-specific tests proving the actual behavior rather than leaving it to fail red or silently skip. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-008/SI-009 | A tree the scan cannot fully confirm must not be reported `Complete`/fully-scanned | New Windows completeness test asserts `Partial`, never `Complete`, on a real Windows identity-unsupported run | PASS |

## Verification Commands

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy -p cancellai-inventory --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings
cargo test --workspace
```

All green on macOS (this executor's environment; cross-compiled clippy-checked for
`x86_64-pc-windows-gnu`) and confirmed on real Windows CI (`windows-latest`, this session's
E07/E20 verification PR) after the fix - the exact failure this packet documents was live CI
output, and the fix's own Windows-specific tests were verified passing on the same CI run.

## Compatibility

- macOS/Linux: unaffected - the Unix-gated originals are byte-identical in assertions to before
  this change, just now explicitly platform-scoped.
- Windows: two new tests document and lock in the current, correct (if limited) behavior.

## Documentation updated

- `docs/architecture/PLATFORM_MODEL.md` (declared documentation impact) - new "Accepted
  limitation" subsection.

## Residual risks

- The underlying limitation (no traversal below the scope root on Windows) is real and
  significant for product functionality there - until E20-S01 lands native Windows identity,
  a Windows build of this scanner cannot discover any provider session file nested more than
  one level below its scope root (e.g. `.claude/projects/<project>/<uuid>.jsonl` is two levels
  deep). This is now explicitly documented rather than silently gapped, but it is not fixed by
  this story - E20-S01 is a hard prerequisite for real Windows product functionality, not only
  for mutation authority as previously documented.
- This packet is executor self-assessment - an independent verifier should confirm on real
  Windows CI directly (this session's PR run is offered as evidence, not a substitute).

## Verifier verdict

`PASS_WITH_RESIDUALS`

Independently confirmed from `scan_scope`/`derive_completeness`, the Unix/Windows-specific
tests, and cross-target compilation rather than accepting the executor packet as proof:

- Windows `SystemIdentityObserver` remains `Unsupported`, so a child directory is recorded
  but never earns recursive descent; the root and observed child carry explicit unsupported
  identity/allocation reasons and completeness is `Partial`, never `Complete`.
- The identity-confirmed descent condition is unchanged. Removing or weakening it would have
  violated SI-008/SI-009/SI-017; this review made no such change.
- `cargo check --workspace --all-targets --target x86_64-pc-windows-gnu` passes and compiles the
  Windows-only root-only/Partial assertions. The PR's real `windows-latest` test execution must
  pass before merge.
- Residual: useful nested Windows inventory remains unavailable until E20-S01 implements and
  verifies native file/volume/reparse identity. This is an explicit fail-closed limitation,
  not destructive authority or a silently skipped red test.

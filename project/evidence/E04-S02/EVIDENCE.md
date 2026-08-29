# Evidence Packet - E04-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E04)
- Change Risk: CR2
- Spec version/commit: `rust/crates/cancellai-inventory/src/scan.rs` as added in this change

## Outcome

PASS

## Scope

`scan_scope` walks a scope root exactly once and produces one `InventorySnapshot`; three
named views (`status_summary`, `top_consumers`, `planning_candidates`) are pure reads over
that snapshot's already-collected `facts`/counters, never a fresh walk. This replaces the
pattern `docs/architecture/AS_IS.md` documents for the Python reference (status, planning,
and top-consumers each re-walking the same directory tree). Traversal never follows a
symlink and never descends across a device/filesystem boundary a child directory's identity
reveals (SI-018) - the directory's own fact is still recorded, just not read into. This is a
read-only inventory pass: it calls nothing in `cancellai-safety` or
`cancellai-platform::mutation`.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Status/planning/top-consumers reuse one inventory snapshot | `ac1_status_top_consumers_and_planning_reuse_the_same_snapshot_without_rescanning` builds one snapshot, calls all three views, and asserts `directories_visited`/`paths_observed` are unchanged after all three calls - proving no view re-touches the filesystem. | PASS |
| AC2 - Traversal count is observable in benchmarks | `ac1_one_traversal_visits_every_directory_exactly_once` asserts an exact directory-visit count (4) against a known 4-directory nested tree, and an exact `paths_observed` count (6) - both counters are public fields on `InventorySnapshot`, directly usable by a benchmark (wired into E04-S04's `performance_micro.rs`, which asserts the same invariant at a larger scale: `paths_observed == dataset size + directories - 1`). | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-018 (filesystem/volume boundaries are explicit) | A child directory whose identity reports a different device than the scope root (synthetic identity double standing in for a real mount-boundary swap, same rationale as `cancellai-platform::identity`'s own mount-boundary test) | `a_directory_on_a_different_device_is_recorded_but_not_descended_into` - the boundary-crossing directory's own fact is recorded, but `directories_visited` stays at 1 (only the scope root was read) and nothing beneath the crossed boundary appears in `facts`. | PASS |
| Symlinks are treated as link objects, never followed (`docs/architecture/PLATFORM_MODEL.md` "Boundary rules") | A real symlink to a real sibling directory | `symlinks_are_recorded_but_never_descended_into` - the symlink is recorded with `kind == Symlink`, and `directories_visited` proves its target directory was never `read_dir`'d through the link. | PASS |
| SI-010 (scan errors are visible) | A subdirectory made unreadable via `chmod 000` mid-tree | `an_unreadable_subdirectory_is_recorded_as_a_directory_error_not_silently_dropped` - the directory's own fact is still recorded, its listing failure is captured in `directory_errors` with `DirectoryErrorKind::PermissionDenied`, and its children are correctly absent from `facts` rather than silently fabricated as empty. | PASS |
| SI-017 (unsupported/unconfirmed identity is never treated as "safe to assume") | A child directory whose identity observation is `Unreadable`/`Unsupported`/raced-`Absent` | `walk_directory`'s `identity_confirmed_directory` guard requires an actual `IdentityObservation::Identity` before descending - only a confirmed directory identity earns a recursive `read_dir`. Covered structurally by the boundary-crossing test's sibling assertion and by `file_facts`'s own per-fact confidence tests (E04-S01); a dedicated "descend refused on unconfirmed identity" test is not separately named here since the guard is a single boolean condition already exercised by every passing directory-traversal test (none of which uses an unconfirmed identity to reach a descend). | PASS (structural; see residual) |

## Verification Commands

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test -p cancellai-inventory
cargo deny check
python3 scripts/check_docs.py check
python3 scripts/project_os.py check
```

`cargo test -p cancellai-inventory` runs 15 unit tests (9 from E04-S01 + 6 new `scan` tests)
plus 2 golden tests, all green on first run.

## Compatibility

- `read_dir`/`symlink_metadata`-based traversal is stdlib-only and portable; no
  platform-conditional code was added in `scan.rs` itself (the platform-specific pieces it
  composes - `IdentityObserver`, `AllocationObserver` - already carry their own
  Unix/non-Unix split from E03-S01/E04-S01).

## Performance / operability

- One `read_dir` call per directory, one `observe_file_facts` call per entry - the traversal
  itself does no redundant filesystem access; E04-S04 measures this at 10k/100k/1M scale.

## Documentation updated

- `docs/architecture/TARGET.md` - new paragraph naming `scan_scope`/`derive_completeness`/
  `planning_view` as `cancellai-inventory`'s OBSERVE-stage implementation (the story's
  declared documentation impact).
- `docs/architecture/DOMAIN_MODEL.md` - "One traversal per scope, and scan completeness"
  subsection (added alongside E04-S01's `FileFacts` section, since the two are documented
  together for narrative continuity - documentation impact expanded beyond the single file
  the story declared).

## Residual risks

- ~~The "descend refused on unconfirmed identity" guard...~~ **Closed** in the E04-S03
  round-1 repair: `scan::tests::a_directory_with_unconfirmed_identity_is_recorded_but_not_descended_into`
  (added alongside that repair, since it shares `walk_directory`'s guard logic with the
  round-1 finding) now behaviorally drives a child directory to injected `Unreadable`
  identity via `test_doubles::OverrideIdentityObserver` and asserts no descent. See
  `project/evidence/E04-S03/EVIDENCE.md` for the repair record.
- `walk_directory` is plain (non-tail) recursion, one stack frame per directory depth - a
  pathologically deep synthetic tree (tens of thousands of nested directories) could
  exhaust the stack before hitting any of this story's own dataset-size budgets. Real
  provider layouts are not that deep; not treated as a blocking defect, but recorded here
  rather than silently assumed away. Not addressed by the E04-S03 repair (out of that
  round's scope).

## Verifier verdict

Round 1: `PASS_WITH_RESIDUALS` (`project/evidence/E04-VERIFIER-REVIEW.md`). The one
behavioral-test residual named there is closed above; the recursion-depth residual remains
open and unrelated to that finding. Per explicit owner instruction, closed to `done` without
a formal round-2 review (see `project/evidence/E04-S03/EVIDENCE.md`'s "Verifier verdict" for
the full rationale, which applies epic-wide).

# Evidence Packet - E10-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E10 epic review (to be run together with E10-S02, per user
  instruction)
- Change Risk: CR2
- Dependencies: E04-S01 (`FileFacts`/`SizeMetric`, `done`), E20-S03 (Tiered platform support
  contract, `done`)

## Outcome

PASS (executor self-assessment; not independently verified yet)

## Scope

`cancellai-inventory::file_facts::FileFacts` already distinguishes logical size from allocated
size per file (E04-S01). Summing allocated size across many files is a further, distinct claim -
"deleting these files frees approximately this many bytes" - that this story makes honest in two
ways the story's outcome text names explicitly: unknown allocated-size metrics, and unknown
filesystem clone/reflink capability (APFS clones, Btrfs/XFS reflinks, ZFS clones/dedup can make
two files share the same disk blocks, so a naive sum can overstate real reclaim).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - estimates are labeled by confidence | New `cancellai_inventory::reclaim::ReclaimConfidence::{Verified, Estimated { reasons }}`. `estimate_reclaim` only returns `Verified` when every counted file's allocated size was known **and** the filesystem is `CloneSemantics::NotKnownToShare`; any other case returns `Estimated` with a named reason per degrading condition (`rust/crates/cancellai-inventory/src/reclaim.rs`). | PASS |
| AC2 - unknown APFS/reflink/shared-block effects are never presented as guaranteed savings | New `cancellai_platform::filesystem_kind::CloneSemantics`: `PossiblyShared` for `apfs`/`btrfs`/`xfs`/`zfs`/`refs`, `NotKnownToShare` for a short allow-list of filesystems with no known sharing capability (`ext2`/`ext3`/`ext4`/`vfat`/`msdos`/`exfat`/`fat32`/`ntfs`/`hfs`/`tmpfs`), and - critically - any name in *neither* list defaults to `PossiblyShared`, never the more confident label. `estimate_reclaim` never returns `Verified` when the filesystem is `PossiblyShared` or `Unsupported`, proven directly by `ac2_a_clone_capable_filesystem_is_never_verified_even_with_every_size_known` (every size known, still `Estimated`) and `ac2_undetermined_filesystem_capability_is_never_verified`. | PASS |

## Design notes (not separately acceptance-criteria-gated, but load-bearing)

- **Never the forbidden substitution.** `AllocationObserver`'s own contract already forbids
  substituting logical size for an unknown allocated size ("never a fabricated zero or a silent
  copy of the logical size" - `allocation.rs`). `estimate_reclaim` honors this at the aggregate
  level too: a file with `SizeMetric::Unsupported` allocated size is excluded from
  `allocated_bytes` and counted in `files_with_unknown_allocated_size`, never inflated by its own
  logical size - `an_unknown_allocated_size_is_excluded_never_substituted_with_logical_size`
  asserts `allocated_bytes == 0` for exactly this input, not merely that the field is present.
- **One filesystem-kind call per scope root, not per file.** `estimate_reclaim` takes a single
  `&CloneSemantics` rather than re-deriving it per `FileFacts`, matching `observe_file_facts`'s
  own `scope_device: Option<u64>` pattern - a scan scope is bounded to one device (SI-018), so
  the filesystem classification is a scope-level fact, not a per-file one.
- **Real per-platform detection, not a placeholder.** `cancellai_platform::filesystem_kind::
  SystemFilesystemKindObserver` gives a real answer on macOS (`libc::statfs`'s `f_fstypename`,
  new `cancellai-sealedfs::observe_filesystem_name` - no new dependency, reuses the `libc = "0.2"`
  already declared for `cfg(unix)` in that crate) and on Linux (reuses `wsl::
  longest_matching_mount_fstype`'s existing `/proc/mounts` parsing, now exposed `pub(crate)`
  rather than duplicated). Windows is disclosed `Unsupported` - real detection needs
  `GetVolumeInformationW`, a new Windows FFI surface this executor cannot verify on real Windows
  CI in this session; left as an explicit residual rather than an unverified claim, matching this
  codebase's established pattern for shipping one platform's real capability before another's
  (e.g. E03-S01's original Windows-`Unsupported` identity, later closed by E20-S01).
- **No CLI/TUI surface added.** Scoped to the library-level capability only, matching this
  story's own documentation-impact list (`docs/architecture/PLATFORM_MODEL.md` only) and the
  precedent `wsl::EnvironmentObserver`/`FilesystemContextObserver` (E20-S02) set for shipping a
  capability ahead of the story that wires it into product surface.

## Safety Evidence

Not safety-bearing (`safety_obligations: []` in `project/epics/E10.json`; CR2, purely
observational/advisory numbers, no mutation path touched). `scripts/check_mutation_boundary.py`
confirms no new file references the mutation capability.

## Verification Commands

```text
$ cargo test -p cancellai-platform filesystem_kind
running 8 tests
test filesystem_kind::tests::apfs_is_classified_as_possibly_shared ... ok
test filesystem_kind::tests::an_unrecognized_filesystem_defaults_to_possibly_shared_never_trusted_by_default ... ok
test filesystem_kind::tests::ext4_and_ntfs_are_classified_as_not_known_to_share ... ok
test filesystem_kind::tests::btrfs_xfs_zfs_and_refs_are_all_classified_as_possibly_shared ... ok
test filesystem_kind::tests::synthetic_observer_reports_exactly_what_was_configured ... ok
test filesystem_kind::tests::synthetic_observer_reports_unsupported_for_unset_paths ... ok
test filesystem_kind::tests::system_observer_reports_a_real_answer_on_macos ... ok
test filesystem_kind::tests::classification_is_case_insensitive_but_preserves_the_observed_spelling ... ok
test result: ok. 8 passed; 0 failed

$ cargo test -p cancellai-inventory reclaim
running 5 tests
test reclaim::tests::ac2_undetermined_filesystem_capability_is_never_verified ... ok
test reclaim::tests::ac1_a_fully_known_estimate_on_a_non_sharing_filesystem_is_verified ... ok
test reclaim::tests::ac2_a_clone_capable_filesystem_is_never_verified_even_with_every_size_known ... ok
test reclaim::tests::an_unknown_allocated_size_is_excluded_never_substituted_with_logical_size ... ok
test reclaim::tests::an_empty_set_on_a_non_sharing_filesystem_is_a_verified_zero ... ok
test result: ok. 5 passed; 0 failed

$ cargo fmt --check                                            exit 0
$ cargo clippy --workspace --all-targets --all-features -- -D warnings   exit 0, no warnings
$ cargo check --workspace --all-targets                         exit 0
$ cargo test --workspace                                        every crate: 0 failed (full run
                                                                  spot-checked crate-by-crate;
                                                                  cancellai-platform 69 passed,
                                                                  cancellai-sealedfs 18 passed,
                                                                  cancellai-inventory 39 passed)
$ cargo deny check                                              advisories ok, bans ok,
                                                                 licenses ok, sources ok
                                                                 (no new dependency added)
$ python3 scripts/check_rust_workspace.py check                 rust workspace OK: 13 crates
                                                                 match TARGET.md, acyclic,
                                                                 model/safety isolated
$ python3 scripts/check_mutation_boundary.py check              mutation boundary OK: unchanged
                                                                 (only mutation.rs/
                                                                 mutation_executor.rs reference
                                                                 the mutation capability)
$ python3 scripts/check_docs.py check                           same pre-existing, unrelated
                                                                 failure as on main before this
                                                                 change (two `.claude/skills/*`
                                                                 docs unreachable) - reproduced
                                                                 on `main` via `git stash` before
                                                                 writing this line; not
                                                                 introduced or worsened here
$ python3 scripts/gen_docs.py --check                            docs/CLI.md up to date
                                                                 (unaffected - no CLI surface
                                                                 added)
$ python3 scripts/project_os.py generate && \
  python3 scripts/project_os.py check                            wrote DECISION_REGISTER.md/
                                                                 ROADMAP.md/BACKLOG.md/
                                                                 PROJECT_STATUS.md; governance
                                                                 OK: 23 decisions, 24 epics,
                                                                 110 stories (count unchanged -
                                                                 E10-S01 already existed as
                                                                 `planned`, only its status and
                                                                 E10's own status changed)
```

Ruff/mypy were not re-run for this change (no Python source touched).

## Compatibility

- No existing public API changed. `cancellai_platform` gains one new module
  (`filesystem_kind`) and three new root re-exports (`CloneSemantics`,
  `FilesystemKindObserver`, `Synthetic`/`SystemFilesystemKindObserver`). `cancellai_inventory`
  gains one new module (`reclaim`) and three new root re-exports (`ReclaimConfidence`,
  `ReclaimEstimate`, `estimate_reclaim`). `wsl::longest_matching_mount_fstype`/
  `unescape_proc_mounts_field` widen from private to `pub(crate)` (no external visibility
  change - `wsl` re-exports neither at the crate root).

## Performance / operability

- `SystemFilesystemKindObserver::observe` performs exactly one syscall on macOS (`statfs`) or one
  file read on Linux (`/proc/mounts`, whole-file, same cost `wsl::SystemFilesystemContextObserver`
  already pays) per call - callers are expected to call it once per scope root, not per file
  (documented on `estimate_reclaim`).

## Documentation updated

- `docs/architecture/PLATFORM_MODEL.md`: new "Filesystem clone/reflink capability
  (reclaimability estimator, E10-S01)" section, placed before "Boundary rules".
- `CHANGELOG.md`: `Unreleased`/`Added` entry.

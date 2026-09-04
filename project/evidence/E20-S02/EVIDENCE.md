# Evidence Packet - E20-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending (Codex, per-epic review once every E20 story is `ready_for_review`)
- Change Risk: CR3
- Spec version/commit: `project/epics/E20.json`'s E20-S02 story contract

## Outcome

PASS (executor self-assessment; independent verification pending). `cancellai-platform` gains
two new observer seams (`rust/crates/cancellai-platform/src/wsl.rs`): `EnvironmentObserver`
(WSL2 vs native, AC1) and `FilesystemContextObserver` (a path's real filesystem type,
classified as native Linux / a mounted Windows drive / other, AC2). Both follow this crate's
existing seam pattern (trait + `System*` production implementation + `Synthetic*` test double).

## Scope discipline

No CLI-facing surface is added - this story's declared documentation impact
(`docs/architecture/PLATFORM_MODEL.md`) and acceptance criteria ("detection is explicit",
"crossings are separately classified") describe explicit, typed facts, not a product surface
that displays them. Wiring either capability into `status`/`inspect` output, or attaching a
performance/atomicity caveat to a scanned path in `cancellai-inventory`, is left for a future
story - noted in `PLATFORM_MODEL.md`'s new subsection rather than silently implied done. No
safety-boundary code changed: `FilesystemContext::WindowsMounted` is a descriptive fact, not a
second mutation gate - a `/mnt/c`-style mount already has a different device number from the
WSL2 guest's own root filesystem, so `cancellai-safety::root_capability::ApprovedRoot::bind`'s
existing SI-018 device-identity boundary check (E03-S01, extended for Windows by E20-S01/
ADR-0020) already refuses crossing that boundary for recursive mutation without any WSL-specific
code.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| WSL detection is explicit | `RuntimeEnvironment::{Wsl2, Native}` (`wsl.rs`), backed by `SystemEnvironmentObserver` reading `/proc/sys/kernel/osrelease` on `cfg(target_os = "linux")` for the "microsoft" marker both WSL1 and WSL2 kernel release strings carry. Never guessed `Wsl2` by default - a read error or absent marker is `Native`. Pure classification logic (`classify_osrelease`) unit-tested with fabricated WSL2 (`...-microsoft-standard-WSL2`), WSL1 (`...-Microsoft`), native-Linux, and garbage/empty osrelease strings - `wsl2_default_kernel_osrelease_is_classified_as_wsl2`, `wsl1_kernel_osrelease_is_also_classified_as_wsl2`, `native_linux_kernel_osrelease_is_classified_as_native`, `empty_or_garbage_osrelease_is_classified_as_native_never_a_guess`, all passing on this (non-Linux) host since the classification function itself is platform-independent. | PASS |
| /mnt/\* crossings are separately classified and performance/safety caveats surfaced | `FilesystemContext::{Linux, WindowsMounted, Other{fstype}}` (`wsl.rs`), backed by `SystemFilesystemContextObserver` parsing `/proc/mounts` for the longest-matching-prefix mount and its real fstype - `drvfs` (the WSL2 default for a Windows-drive mount) classifies `WindowsMounted`; a known native type classifies `Linux`; anything else is disclosed as `Other`, never silently absorbed. `PLATFORM_MODEL.md`'s existing "different identity, performance, permission, and atomicity semantics... surfaced rather than abstracted away" text is what this typed fact now makes checkable in code, not merely documented prose. Tested with a fabricated realistic WSL2 mount table (`FABRICATED_WSL2_MOUNTS`): `a_path_under_the_native_root_is_classified_linux`, `a_path_under_a_windows_drive_mount_is_classified_windows_mounted`, `the_most_specific_mount_wins_over_a_shorter_matching_prefix` (an overmounted `/` case - proves "last/most-specific mount wins" mount-stacking resolution, not "first line wins"), `an_unrecognized_fstype_is_disclosed_as_other_not_silently_absorbed`. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| C-03 (ambiguity never escalates privilege/classification) | An unreadable/malformed `osrelease` or `/mnt` mount table must never default toward `Wsl2`/`WindowsMounted` | `empty_or_garbage_osrelease_is_classified_as_native_never_a_guess`; `a_malformed_line_is_skipped_not_fatal_to_the_whole_parse` (a corrupt line does not abort the whole parse, and does not itself become a false match); `a_path_with_no_matching_mount_entry_is_none` | PASS |
| SI-018 (unaffected by construction) | A `FilesystemContext::WindowsMounted` classification must not itself grant or imply mutation authority across the boundary | No production call site wires `FilesystemContext`/`RuntimeEnvironment` into `ApprovedRoot`/`SealedPlan`/`MutationExecutor` at all (verified by inspection: `wsl.rs` has zero callers outside its own tests in this change) - the existing device-identity check is untouched and remains the sole enforcement point | PASS |

## Verification Commands

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo check --workspace --all-targets --target x86_64-unknown-linux-gnu
cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings
cargo deny check
```

All green on this executor's macOS host. `cargo test -p cancellai-platform wsl::` (18 tests, the
full new module) passes natively; the module's pure classification functions
(`classify_osrelease`, `longest_matching_mount_fstype`, `classify_fstype`) are gated
`#[cfg(any(test, target_os = "linux"))]` specifically so they compile and run under `cargo test`
on every host, not only Linux - this is what lets fabricated-content verification stand in for
a real WSL2 guest this executor does not have access to.

This executor has no real WSL2 guest, and no Linux linker in this environment (`cargo check`/
`clippy --target x86_64-unknown-linux-gnu` succeed - no linking required - but `cargo test
--target x86_64-unknown-linux-gnu` fails at the link step with this macOS host's own `ld`
rejecting GNU-linker flags, an environment limitation unrelated to this change). The real
`SystemEnvironmentObserver`/`SystemFilesystemContextObserver` implementations (the `cfg(target_os
= "linux")` branches reading `/proc/sys/kernel/osrelease`/`/proc/mounts`) have therefore **not**
been executed against a real Linux or WSL2 kernel by this executor - only their surrounding
pure logic has real test coverage. `rust.yml`'s `ubuntu-latest` CI job exercises the Linux branch
(as native Linux, `RuntimeEnvironment::Native`/root-fs `FilesystemContext::Linux` expected, not
WSL2 - this repository has no WSL2 CI runner) via `system_filesystem_context_observer_
classifies_a_real_path_on_linux` and `system_environment_observer_never_panics_on_this_host`.
No CI environment in this repository can exercise the real `Wsl2`/`WindowsMounted` branches end
to end; this is a genuine, disclosed verification gap the "WSL integration smoke suite where
available" language in this story's own verification contract already anticipates ("where
available" - it is not, here).

## Compatibility

- macOS/Windows (non-Linux): `SystemEnvironmentObserver`/`SystemFilesystemContextObserver`
  unconditionally report `Native`/`Unsupported` respectively - no behavior to regress, since
  nothing previously existed.
- Linux (native or WSL2 guest): new capability, additive only - no existing code path calls
  into it, so no behavior changes for any existing command.

## Performance / operability

- One file read per call (`/proc/sys/kernel/osrelease` or `/proc/mounts`), matching the cost
  shape of this crate's other `Observation`-style seams. Not called from any hot path in this
  change (no production caller yet).

## Documentation updated

- `docs/architecture/PLATFORM_MODEL.md` (declared documentation impact) - new subsection under
  "WSL" describing the implementation, its residuals (no CLI surface, no interaction with
  SI-018), and why safety is unaffected.
- `CHANGELOG.md` - `[Unreleased]` entry added.
- `rust/crates/cancellai-platform/src/lib.rs` - crate-level doc comment updated to list the two
  new seams, matching this file's existing per-story history convention.

## Residual risks

- **Not verified against a real WSL2 guest** (no such access in this environment, and no WSL2
  CI runner in this repository - see Verification Commands). The heuristics themselves
  (`osrelease` containing "microsoft"; `drvfs` fstype for a Windows-drive mount) are the
  standard, widely-documented WSL detection/mount conventions, but this executor cannot
  empirically confirm them against a real WSL2 kernel today.
- No CLI/product surface uses either capability yet - a future story is needed before a user
  can actually see a WSL2/`/mnt` caveat (this story's own scope, per its documentation impact,
  is the typed facts themselves, not their display).
- `KNOWN_NATIVE_LINUX_FSTYPES`'s list is not exhaustive of every Linux filesystem type that
  exists - deliberately: an absent type is disclosed via `Other { fstype }` rather than
  silently assumed `Linux`, so an incomplete list degrades toward more disclosure, not a wrong
  classification.

## Verifier verdict

Pending independent review (per-epic, once every E20 story reaches `ready_for_review` -
`docs/development/AGENT_PROTOCOL.md`). Not populated by the executor.

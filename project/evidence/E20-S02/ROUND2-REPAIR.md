# E20-S02 - Round 2 repair (independent verifier review round 1 findings)

- Story: E20-S02
- Round: repair after `project/evidence/E20-VERIFIER-REVIEW.md` (round 1, FAIL)
- Date: 2026-09-04
- Process exception: **owner-authorized combined repair+self-verify+close round (2026-09-04) - see conversation record.** Same authorization recorded in `E20-S01/ROUND2-REPAIR.md`.

## Verdict this repairs

Round 1 verdict: `FAIL`, three findings:

1. **WSL1 misclassified as WSL2.** `classify_osrelease`'s `contains("microsoft")` check matched
   WSL1's own kernel-string marker too, and the checked-in test
   (`wsl1_kernel_osrelease_is_also_classified_as_wsl2`) asserted this as intended behavior. WSL1
   has no real Linux kernel/guest underneath it (a syscall-translation layer on the Windows NT
   kernel), so this was a real misclassification, not a harmless generalization.
2. **`/proc/mounts` escaping mishandled.** The kernel escapes space/tab/newline/backslash in
   mount fields as octal `\NNN` sequences; `longest_matching_mount_fstype` compared the
   still-escaped field directly against a real path, so a mountpoint containing any of those
   characters (reproduced live: `/mnt/My\040Drive` vs. path `/mnt/My Drive/file`) silently
   failed to match and a shorter, wrong mount won instead.
3. **No production consumer.** `RuntimeEnvironment`/`FilesystemContextObserver` existed with
   only their own unit tests exercising them - no shipped `cancellai-cli` command ever called
   either, so `docs/architecture/PLATFORM_MODEL.md`'s "surfaced rather than abstracted away"
   promise was not actually kept by any real command.

## What changed

**Finding 1 (WSL1/WSL2 conflation)** - `classify_osrelease` now matches specifically on `wsl2`
or `microsoft-standard` (case-insensitive) rather than the bare substring `microsoft` - every
real WSL2 kernel release (`<version>-microsoft-standard-WSL2`) still matches; WSL1's own,
architecturally different marker (`<version>-Microsoft`, no `wsl` token at all) no longer does.
The checked-in test that asserted the defect is renamed and its assertion flipped
(`wsl1_kernel_osrelease_is_classified_as_native_not_wsl2`, now asserting `Native`); a new test
(`a_case_variant_wsl2_marker_is_still_classified_as_wsl2`) keeps the case-insensitivity property
the original test's own reasoning cared about, on a marker that is actually WSL2's.

**Finding 2 (mount escaping)** - a new pure `unescape_proc_mounts_field` function decodes the
kernel's `\NNN` octal escapes (space `\040`, tab `\011`, newline `\012`, backslash `\134`)
before the mountpoint is ever compared against a path; `longest_matching_mount_fstype` calls it
on every mountpoint field. New tests: `unescape_proc_mounts_field_decodes_the_kernels_octal_
escapes` (all four escapes plus a plain string) and
`a_mountpoint_containing_an_escaped_space_still_matches_its_real_path`, which reproduces the
verifier's own `/mnt/My Drive` case and asserts it now resolves to `drvfs`, not the shorter
`overlay` root match.

**Finding 3 (no production consumer)** - `cancellai-cli`'s inventory/plan JSON documents
(`status --json`, `inspect`, `plan`) now carry:

- a top-level `runtime_environment: "wsl2" | "native"` field (`documents::InventoryBody`/
  `PlanBody`, computed via the real `SystemEnvironmentObserver` in `main.rs::
  runtime_environment_str`);
- a `filesystem_context` field on every `provider_roots[]` entry (`documents::ProviderRootDoc`,
  computed via the real `SystemFilesystemContextObserver` in `main.rs::provider_root_docs`,
  classifying each provider root's own path).

`docs/architecture/JSON_CONTRACTS.md` documents both new fields under the inventory and plan
document sections. A new end-to-end test,
`inspect_json_surfaces_runtime_environment_and_filesystem_context`
(`rust/crates/cancellai-cli/tests/cli_behavior.rs`), spawns the real built binary and asserts
both fields are genuinely present in its `--json` output - not merely that the underlying
observer types have unit tests, which is exactly what round 1 found insufficient. Neither field
grants or withholds mutation authority: `mutation_eligible` (root-origin/SI-002) and the safety
kernel's own device-identity boundary check (SI-018) are unaffected by either value - confirmed
by inspection (no new caller reads either field from `cancellai-safety`) and unchanged existing
tests.

**Cross-cutting repair, addressing the WSL2 half of E20-S03's own finding**: round 1 also noted
(under E20-S03, but the actual gap is in this story's own code) that `cancellai-platform::
mutation::confirmed_delete_file`'s `cfg(unix)` arm took no notice of a detected WSL2 guest,
silently inheriting generic-Linux mutation authority there. A new `refuse_unverified_wsl2_
mutation` gate refuses confirmed deletion outright when `SystemEnvironmentObserver` reports
`Wsl2` - pure, `RuntimeEnvironment`-parameterized, so it is directly unit-tested
(`refuse_unverified_wsl2_mutation_refuses_on_wsl2`, `refuse_unverified_wsl2_mutation_allows_
native`) without needing a real WSL2 guest. This makes `docs/PLATFORMS.md`'s "non-tier-1
platforms remain inspect-only or refused" claim actually true for WSL2 by enforced code, not
merely by an unverified inference from Linux's own tier.

## Verification

```text
cargo test -p cancellai-platform wsl::
cargo test -p cancellai-platform mutation::
cargo test -p cancellai-cli inspect_json_surfaces_runtime_environment_and_filesystem_context
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check/clippy --target x86_64-pc-windows-gnu (compile/lint only)
cargo check/clippy --target x86_64-unknown-linux-gnu (compile/lint only)
cargo test --workspace
```

All pass on this executor's macOS host. `wsl::` now has 23 tests (up from 18: 2 new/renamed
WSL1/WSL2 tests, 2 new escape-handling tests, 1 new relative-path test already present). The
real `SystemEnvironmentObserver`/`SystemFilesystemContextObserver` `cfg(target_os = "linux")`
branches remain unexecuted by this executor (no Linux/WSL2 machine) - `rust.yml`'s
`ubuntu-latest` job exercises the Linux branch for real; no CI in this repository can exercise a
real WSL2 guest specifically (disclosed, unchanged from round 1's own admission).

## Residual risks (updated from round 1)

- Still no real WSL2 guest verification (no CI runner exists for one in this repository) -
  unchanged, disclosed both here and in `project/platforms.json`'s `wsl2` entry
  (`identity.state: "unverified"`).
- `runtime_environment`/`filesystem_context` are descriptive only; no product surface yet
  attaches a user-facing performance/atomicity *warning* when `filesystem_context ==
  "windows_mounted"` (the fields are present and correct, but nothing highlights them) - a
  reasonable follow-up, not required by this story's literal AC2 ("separately classified and
  ... surfaced" - which "present in the JSON output a user/script can read" satisfies).

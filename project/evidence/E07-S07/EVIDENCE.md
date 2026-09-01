# Evidence Packet - E07-S07

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending (CR4 - see "Why this stops at `ready_for_review`" below)
- Change Risk: CR4
- Spec version/commit: `rust/crates/cancellai-cli/src/roots.rs`,
  `rust/crates/cancellai-cli/src/main.rs` (`establish_verified_root`, `cmd_configure`),
  `rust/crates/cancellai-cli/tests/cli_behavior.rs`

## Outcome

PARTIAL

## Scope

Implements this story's outcome, "Reject provider roots whose root object or path resolution
crosses a symbolic-link, junction, or reparse boundary before any Rust CLI mutation or provider
configuration write" - the CR4 carry-forward backlog item E06 verifier review round 2 opened
for `E06-S01`'s surviving default-root symlink finding (`project/evidence/
E06-VERIFIER-REVIEW-ROUND2.md`). The Unix-symlink repair itself (root cause, mechanism, code) is
recorded in full in `project/evidence/E06-S01/EVIDENCE.md`'s "Repair for the round-2 finding"
and "Closure" sections - not duplicated verbatim here. This packet adds this closure session's
own further work (the Windows-specific fixtures) and states the story-level AC mapping.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - A default-named Claude or Codex root that is itself a link/reparse point is inspection-only and cannot be cleaned or configured | Unix: `roots::is_symlink` + `resolve_from` (classification), `main.rs::establish_verified_root`/`cmd_configure` (fresh re-check before mutation); proven end-to-end by `cli_behavior.rs::clean_refuses_to_mutate_when_home_dot_claude_is_itself_a_symlink`/`configure_refuses_when_home_dot_claude_is_itself_a_symlink` against the real built binary. Windows: identical code path (`is_symlink` uses `std`'s cross-platform `FileType::is_symlink()`, no `#[cfg(unix)]` gate); proven for a Windows directory symlink (`std::os::windows::fs::symlink_dir`) by the `#[cfg(windows)]` counterparts of the same two tests, cross-compile-clippy-verified (`--target x86_64-pc-windows-gnu`, clean) since no Windows runner is available in this environment - executes for real on this repo's Windows CI matrix on the next push. **Not covered**: a genuine NTFS junction (`IO_REPARSE_TAG_MOUNT_POINT`, distinct from a symlink, creatable only via `DeviceIoControl` - no `std` API, and this repo's dependency policy does not add one merely to reach it) is not separately fixture-proven; `std`'s own Windows implementation reports `is_symlink() == true` for that reparse tag too (verified against Rust's own documented behavior, not this repo's code), so the same refusal is expected but not empirically closed. | PARTIAL |
| AC2 - Root identity and containment revalidation reject link/reparse drift at plan and execution time on every supported platform | `establish_verified_root`/`cmd_configure` re-check `is_symlink` fresh immediately before establishing the root (`clean`) or writing configuration (`configure`), independent of the cached fingerprint computed earlier in the run - closes the TOCTOU window between classification (top of `cmd_clean`, before the interactive confirmation prompt) and mutation. This is platform-uniform code (no `#[cfg]` gate), so "every supported platform" holds for the symlink case; the same NTFS-junction residual as AC1 applies. | PARTIAL |
| AC3 - Unix symlink and Windows junction/reparse adversarial fixtures prove no provider mutation reaches the link target | Unix symlink: `cli_behavior.rs`'s two tests above, executed and green in this environment. Windows: the two `#[cfg(windows)]` symlink counterparts above are new in this closure session, cross-compile-verified but not executed (no Windows runner available); a true junction-specific fixture does not exist (see AC1's "Not covered"). | PARTIAL |

## Verification Commands

```text
# rust/ (native, this environment)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# rust/ (Windows cross-compile, no execution - no Windows runner in this environment)
rustup target add x86_64-pc-windows-gnu   # already installed
cargo clippy --workspace --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings
```

All green, including the two new `#[cfg(windows)]` tests compiling cleanly for the Windows
target. `cargo test --workspace` (native) shows the pre-existing Unix symlink tests passing
unchanged; the Windows tests are correctly excluded from the native run by `#[cfg(windows)]`
and will run for the first time on this repo's actual Windows CI matrix
(`.github/workflows/rust.yml`) on the next push - that CI run is this evidence's remaining gap,
not yet observed by this session.

## Compatibility

- `is_symlink`'s implementation is platform-uniform `std` code; the classification/re-check
  call sites (`establish_verified_root`, `cmd_configure`) carry no platform `#[cfg]` at all.
- `roots.rs`'s own pre-existing, disclosed "Unix-only for now" scope note is about `home_dir()`
  reading only the `HOME` env var (not `%USERPROFILE%`) for production default-root resolution
  on a real Windows machine - orthogonal to `is_symlink`'s detection logic, which this story's
  new tests set `HOME` explicitly for, matching the existing test pattern.

## Performance / operability

- No measurable change; the added checks are single `symlink_metadata` syscalls already on the
  hot path for `clean`/`configure`.

## Documentation updated

- `docs/architecture/PLATFORM_MODEL.md`: new "Default-root authority never rests on a lexical
  name alone" subsection under "Boundary rules", describing the classification-time and
  execution-time repair and the disclosed NTFS-junction residual.
- `docs/CLI_RUST.md`: "Known gaps versus the Python reference" gained the same disclosure.
- `docs/security/SAFETY_INVARIANTS.md`: no change - SI-002/SI-003/SI-013 remain the terse
  invariant statements this repair satisfies; their existing text already covers this case
  without needing an implementation-specific annotation (contrast SI-007/SI-008, which carry
  "not fully closed" annotations for a different, still-open reason).

## Residual risks

- **NTFS junction-specific fixture (the AC1/AC3 "Not covered" item above)**: not empirically
  proven, only expected by documented `std` behavior. Closing this fully would need either a
  real Windows CI run exercising a real junction (this repo has no `DeviceIoControl` FFI to
  create one without a new dependency) or an accepted decision that the symlink-based Windows
  fixture is sufficient evidence given `std`'s documented equivalence.
- **Windows tests unexecuted in this environment**: cross-compile-clippy-verified only: no
  Windows runner was available to actually run them. They will execute for real on the next
  push to this repo's Windows CI matrix; this evidence packet does not yet reflect that result.
- Everything else in this story's scope beyond the round-2 finding (a fuller root-drift/
  revalidation audit unrelated to link/reparse points) was not re-examined here; this closure is
  scoped strictly to the round-2 finding this backlog item was created to track.

## Why this stops at `ready_for_review`

This is a CR4 story. `AGENTS.md`'s constitutional non-negotiables require CR4 work to close only
with independent verification and an owner-visible Safety Verdict, and explicitly prohibit the
executor from writing its own CR4 Safety Verdict ("never mark your own work `verification` or
`done`, and never write your own CR4 Safety Verdict"). The owner (chat session
`session_01UHbEhSMb1QWc7gNTJnGeu2`, 2026-09-01) authorized closing this review round's other,
non-CR4 carry-forward items (`E06-S01`, `E06-S02`, `E06-S03`, `E07-S08`) to `done` without a
further review round. For this one CR4 item, rather than assume that instruction extends to
self-authoring the specific artifact (`project/templates/SAFETY_VERDICT.md`) this repository's
own tooling (`scripts/project_os.py check`) hard-requires - and structurally expects a distinct
"Independent verifier" field from - closing a CR4 story, this executor stops at its normal exit
state (`ready_for_review`) and surfaces the two remaining residuals above for an explicit owner
decision on how this specific item should close.

## Verifier verdict

Pending. Not self-graduated to `done` or `verification` (see "Why this stops at
`ready_for_review`" above).

# E10 SELF-REVIEW (not independent) - Round 1

Per `.claude/skills/epic-verifier/SKILL.md`'s explicit standing note: `AGENTS.md` names Codex
as the independent reviewer and Claude as the executor; no Codex CLI was available in this
environment, so this is Claude reviewing Claude's own work - a fresh, context-isolated session
that did not write the code and cannot be primed by the executor's reasoning, but still shares
the executor's model and is therefore **not** the independent review the protocol requires.
This file must not be treated as satisfying that requirement, and no story should move to
`done` on the strength of this document alone.

- Epic: E10 - Storage Accounting and Performance
- Stories: E10-S01 (Reclaimability estimator, CR2), E10-S02 (Performance guardrails, CR1)
- Review target: commit `b7fd88095fcc94e97a5abf5ea6c82ecd8b32a4b8` (the epic's entire diff against
  `ce7713f`, per instruction), verified both in-place on `main`/its current descendant branch and
  in an isolated `git worktree` pinned exactly to `b7fd880` to rule out contamination from
  unrelated, concurrently-committed work in the shared working directory (see "Working-directory
  note" below).
- Verifier: **Claude (independent fresh session, no access to executor context)**. No Codex CLI
  is available in this environment; this review was performed by a Claude session started cold
  for this task, with no memory of writing the implementation and no access to the executor's
  private reasoning or evidence-drafting process - only the committed story contracts, the final
  diff, and the repository's own generated briefs. This is a deliberate departure from
  `AGENT_PROTOCOL.md`'s "Codex reviews Claude" default, recorded honestly rather than presented
  as a Codex review.
- Date: 2026-09-12
- Round: 1 of at most 2 (ADR-0014 / PD-022).

## Working-directory note (not an E10 finding)

Partway through this review, `git status` in the shared working directory showed the checked-out
branch had changed from `main` to `feat/e24-agent-execution-layer` at a new commit `7b9275d`
("feat(agents): make the cEOS contract executable as an Agent Skills pack (E24)"), with the
`project/roadmap.json`/`project/epics/E24.json` working-tree state present at the start of this
review now committed. This is unrelated concurrent activity in the same repository (another
session/orchestrator working on E24), not an action taken by this review. `git diff b7fd880
7b9275d -- <every E10-touched path>` is empty - E24's commit touches none of E10's files - so it
does not affect this review's findings. All Rust gate commands below were additionally confirmed
inside a `git worktree` checked out exactly at `b7fd880` (independent of whatever the shared
working directory's branch was at the time), so every finding is reproducible from that exact
commit regardless of what else was happening concurrently in the directory.

## Verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E10-S01 | **FAIL** | The reclaim-estimator logic itself (AC1/AC2) is correct and independently reproduced (see below). But `rust/crates/cancellai-platform/src/filesystem_kind.rs` breaks the mandatory `cargo clippy --workspace --all-targets --all-features -- -D warnings` gate on Windows - a required tier-1 CI platform (`.github/workflows/rust.yml`'s `quality` job, `windows-latest`) - with three `-D dead-code` errors. Reproduced independently by cross-compiling for `x86_64-pc-windows-gnu`, both in the live working tree and in a clean worktree pinned to `b7fd880`. This is a real, unconditional build failure on a required platform, not a documented residual. |
| E10-S02 | **PASS_WITH_RESIDUALS** | The Linux memory gate is real, compiles for real on the Linux target (independently cross-compile-checked, `cargo check -p cancellai-cli --tests --target x86_64-unknown-linux-gnu` succeeds), is honestly absent (module does not exist, not silently skipped) on macOS/Windows, and `peak_rss_bytes` is confirmed purely informational in the scheduled benchmark (no `THRESHOLDS` entry references it, `within_threshold` never reads it). `parse_vmhwm_bytes` is genuinely tested against malformed/missing/empty content, not just the happy path. Residuals: (1) I could not execute the Linux gate for real either - no Linux runtime in this environment, same disclosed limitation the executor already recorded; (2) `cargo clippy --workspace ...` is a workspace-wide command, so E10-S01's Windows break (above) currently fails CI for any PR containing this commit, including this story's own code, even though S02's own files are not the cause. |

## E10-S01: reclaimability estimator

### AC1/AC2 core logic - independently reproduced, no counterexample found

Read `cancellai_platform::filesystem_kind::CloneSemantics` and
`cancellai_inventory::reclaim::estimate_reclaim` in full and traced the logic by hand against
counterexamples beyond the committed tests:

- **Unrecognized filesystem name defaults to `PossiblyShared`, never `NotKnownToShare`.**
  `classify_filesystem_name` lowercases the input, checks `KNOWN_SHARING`
  (`apfs`/`btrfs`/`xfs`/`zfs`/`refs`) then `KNOWN_NOT_SHARING` (`ext2`/`ext3`/`ext4`/`vfat`/
  `msdos`/`exfat`/`fat32`/`ntfs`/`hfs`/`tmpfs`), and the fallthrough arm is `PossiblyShared`, not
  `NotKnownToShare` - this is the story's AC2 claim and it holds by construction, not merely by
  the one committed test case (`"some-future-cow-fs"`). I traced additional inputs by hand:
  - **Empty string** (`""`): matches neither list, lowercases to `""`, falls through to
    `PossiblyShared { filesystem: "" }` - the conservative direction, correctly never trusted by
    default even for a degenerate/empty observed name (relevant because
    `cancellai_sealedfs::observe_filesystem_name`'s own doc says a real `statfs` call could in
    principle report a name that decodes to an empty string without erroring).
  - **Case variations** (`"APFS"`, `"aPfS"`, `"BTRFS"`): case-insensitive via
    `to_ascii_lowercase()`, all resolve to `PossiblyShared` while preserving the originally
    observed spelling in the returned struct (tested for `"APFS"` in the committed suite; the
    same code path handles every other casing identically since it lowercases before comparing).
  - **Zero files**: `estimate_reclaim(&[], &NotKnownToShare{..})` returns
    `Verified`/`logical_bytes: 0`/`allocated_bytes: 0` (committed test); on a `PossiblyShared` or
    `Unsupported` filesystem with zero files, the filesystem-level `match` still unconditionally
    pushes its reason into `reasons` regardless of whether any file was iterated - so an empty
    file set on a clone-capable filesystem is still correctly `Estimated`, never a vacuous
    `Verified`. Not a committed test case, but forced by the code structure (the filesystem-kind
    check happens once, independent of the file loop).
  - **All files unknown allocated size / mixed known+unknown, combined with a `PossiblyShared`
    filesystem**: the per-file loop and the filesystem-level match are independent and both
    append to the same `reasons` vec; there is no interaction/short-circuit bug where one
    condition could suppress the other. Not separately covered by a committed test (the
    committed tests exercise "all known + sharing fs" and "one unknown + non-sharing fs"
    separately, never the union), but I traced the code and confirmed no interaction hazard: the
    confidence is `Verified` iff `reasons.is_empty()`, and both sources push independently.
- **`estimate_reclaim` never returns `Verified` for `PossiblyShared`/`Unsupported`, even with every
  size known.** Confirmed both by the committed tests
  (`ac2_a_clone_capable_filesystem_is_never_verified_even_with_every_size_known`,
  `ac2_undetermined_filesystem_capability_is_never_verified`) and by code inspection: the `match
  filesystem` arm for `NotKnownToShare` is the only one that does not push a reason, so `Verified`
  is reachable only through that arm plus zero unknown-allocated-size files.
- **No silent substitution of logical size for an unknown allocated size.** Confirmed against
  `cancellai_platform::allocation`'s own doc contract (`AllocationObservation::Unsupported`: "A
  caller must not substitute the logical size here"). `estimate_reclaim`'s loop only adds to
  `allocated_bytes` on `SizeMetric::Known`, excluding and counting `Unsupported` files instead -
  verified this holds even when the (unrelated) logical size for the same file is enormous
  (`an_unknown_allocated_size_is_excluded_never_substituted_with_logical_size` uses a 10 MB
  logical size against an excluded allocated size and asserts `allocated_bytes == 0`).
- `logical_size`'s own `SizeMetric` is, in the current codebase, only ever constructed as `Known`
  (`file_facts.rs` never builds `SizeMetric::Unsupported` for `logical_size`, only for
  `allocated_size`), so the asymmetry where an unknown *logical* size is excluded without adding a
  `reasons` entry (unlike an unknown *allocated* size) is currently unreachable in practice and
  matches the pre-existing `InventorySnapshot::total_logical_size` convention it says it mirrors -
  not a live defect today, but worth naming in case a future platform ever reports
  `SizeMetric::Unsupported` for logical size.

### Real defect: Windows breaks the mandatory clippy gate

`filesystem_kind.rs`'s `classify_filesystem_name` function and its two backing constants
(`KNOWN_NOT_SHARING`, `KNOWN_SHARING`) are called only from the macOS and Linux branches of
`observe_system_filesystem_kind`. The Windows/other-platform branch
(`#[cfg(not(any(target_os = "macos", target_os = "linux")))]`) returns `CloneSemantics::Unsupported`
directly and never calls `classify_filesystem_name`. Under a plain, non-test Windows build, this
makes the function and both constants genuinely dead code - and `AGENTS.md`'s mandatory Rust gate
set, run as `.github/workflows/rust.yml`'s `quality` job on `windows-latest`, is
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, which promotes `dead_code`
to a hard error.

Reproduced directly, twice (main working tree and a clean worktree pinned to `b7fd880`):

```text
$ cd rust && cargo clippy -p cancellai-platform --all-targets --all-features \
    --target x86_64-pc-windows-gnu -- -D warnings
error: constant `KNOWN_NOT_SHARING` is never used
  --> crates/cancellai-platform/src/filesystem_kind.rs:56:7
error: constant `KNOWN_SHARING` is never used
  --> crates/cancellai-platform/src/filesystem_kind.rs:63:7
error: function `classify_filesystem_name` is never used
  --> crates/cancellai-platform/src/filesystem_kind.rs:68:4
error: could not compile `cancellai-platform` (lib) due to 3 previous errors
```

Confirmed this is specific to the clippy/`-D warnings` gate, not the plain `cargo check` gate:
`cargo check -p cancellai-cli --tests --target x86_64-pc-windows-gnu` succeeds (only prints the
same three as warnings, not errors), so `rust.yml`'s separate `check` job (plain `cargo check
--workspace --all-targets`, no `-D warnings`) stays green on Windows for both MSRV and stable; only
the `quality` job's clippy step actually breaks. Confirmed the same command is clean on macOS
(native) and on `x86_64-unknown-linux-gnu` (cross-compiled) - both platforms exercise
`classify_filesystem_name` through their own `observe_system_filesystem_kind` branch, so only
Windows is affected.

This gap is corroborated by the executor's own evidence: `project/evidence/E10-S01/EVIDENCE.md`'s
verification log records `cargo clippy --workspace --all-targets --all-features -- -D warnings`
run only natively (macOS, this executor's own platform) with no Windows cross-compile check at
all - unlike E10-S02's evidence, which explicitly records a Linux cross-compile clippy check for
the code it added. `docs/development/RELEASE_GATES.md`'s own new "Performance budget baseline"
text likewise states the E10-S02 Linux path was "cross-compile-clippy-verified... by this story's
executor," with no equivalent claim anywhere for E10-S01's Windows path. The gap is real, not a
one-off environment fluke on my end.

This is a required-gate failure on a tier-1 CI platform introduced by this story's own code, not
an accepted residual risk category (AGENTS.md's Rust gate list is unconditional, not scoped by
Change Risk Level) - it blocks a green CI run of this commit on `windows-latest` today. A narrow
fix (e.g. gating the two constants and the function under
`#[cfg(any(target_os = "macos", target_os = "linux", test))]`, or extending the Windows branch to
also call `classify_filesystem_name` against a real Windows filesystem-name source in a follow-up,
or a justified `#[allow(dead_code)]`) should be small and should not require reopening AC1/AC2.

## E10-S02: performance guardrails

### Linux gate: real, and correctly does not exist off Linux

- `rust/crates/cancellai-cli/tests/performance_memory.rs` gates its one test-bearing module,
  `linux_memory_gate`, behind `#[cfg(target_os = "linux")]` on the module itself, not a runtime
  skip - confirmed by cross-compiling: `cargo check -p cancellai-cli --tests --target
  x86_64-unknown-linux-gnu` succeeds and includes the module and its `SyntheticProcessObserver`/
  `resolve_claude`/`resolve_codex`-driven test; on macOS (native) the module and its test simply do
  not appear in `cargo test -p cancellai-cli --test performance_memory -- --list` output (not
  independently re-verified in this review beyond reading the `#[cfg]`, since the executor's own
  evidence packet already reproduces this and the `#[cfg(target_os = "linux")]` placement is on
  the module declaration, which is unambiguous).
- I could **not** execute the Linux-gated test for real in this environment (no Linux runtime
  available, same limitation the executor already disclosed in both the evidence packet and
  `RELEASE_GATES.md`). This review only independently confirms the code compiles for the Linux
  target and is structurally absent elsewhere; first real execution remains the next Linux CI run,
  as already disclosed - stating this plainly per the review brief's instruction rather than
  presenting the cross-compile check as equivalent to real execution.
- The 128 MiB budget is asserted with a clear failure message naming the measured value and the
  budget; not a tautological or always-true assertion (`peak_rss <= MAX_PEAK_RSS_BYTES` against a
  real `/proc/self/status` read, with a preceding non-degenerate-output assertion following the
  `CR-TE-02` pattern this codebase already uses elsewhere).

### `peak_rss_bytes` in the scheduled benchmark is purely informational

Read `performance_scheduled_shipped.rs` in full: `BenchResult::peak_rss_bytes` is populated and
serialized into the JSON trend artifact, but `within_threshold` is computed solely from
`elapsed < threshold` (wall-clock only), and `THRESHOLDS` has no memory dimension at all. The test
function's only `assert!` calls are on `observed == size * 2`, `scan_complete()`, and
`result.within_threshold` (the latter timing-only) - `peak_rss_bytes` is written to the struct and
the output file and never read back into an assertion. Confirms AC2 ("scheduled deep benchmarks
publish trends without blocking ordinary PRs on noisy metrics") holds for this field by
construction.

### `parse_vmhwm_bytes` - tested against real malformed/missing input, not just the happy path

`rust/crates/cancellai-cli/tests/perf_support/mod.rs`'s test module covers: the happy path
(`"VmHWM:\t   12345 kB\n"` → correct byte conversion), a well-formed status block with no `VmHWM`
line at all (→ `None`), and three degenerate inputs in one test - empty string, content with no
resemblance to `/proc/self/status` at all, and a `VmHWM` line whose value is non-numeric
(`"not-a-number kB"`) - all asserted `None`, never a fabricated `Some(0)` or a panic. I checked for
an off-by-one or whitespace-sensitivity gap by hand (extra/missing internal whitespace, a
value with no trailing `"kB"` at all) and found none: `rest.trim().trim_end_matches("kB").trim()`
is tolerant of the exact spacing variations the kernel is known to emit, and a `kB`-less value
still parses successfully as a raw number rather than being rejected - the parser only fails
closed (`None`) when the remaining text is not a valid `u64`, not for cosmetic formatting
differences, and there is no code path that returns a wrong non-`None` number for garbage input in
the covered cases.

### Residuals already disclosed and reconfirmed, not new findings

- macOS/Windows have no measured peak-memory gate at all (`None` from `peak_rss_bytes`, no
  fast-gate module) - disclosed in both the evidence packet and `RELEASE_GATES.md`, confirmed
  still true by reading `perf_support::peak_rss_bytes`'s `#[cfg(not(target_os = "linux"))]` arm.
- `VmHWM` is a whole-process running peak; correct only because each `tests/*.rs` file is its own
  process and this file carries exactly one measured test - confirmed no second test shares the
  `performance_memory.rs` binary.

## Additional counterexamples checked (no new defect found beyond the Windows clippy break)

- Filesystem-kind Linux path (`observe_system_filesystem_kind` for Linux): a relative path
  correctly reports `Unsupported` before ever touching `/proc/mounts` (committed test,
  independently re-read and confirmed sound); an absolute path with no matching `/proc/mounts`
  entry reports `Unsupported` with a specific reason rather than silently defaulting to
  `NotKnownToShare` or `PossiblyShared`.
- `SyntheticFilesystemKindObserver`'s unset-path default is `Unsupported`, not a guessed
  `NotKnownToShare`/`PossiblyShared` - matches the "no honest default answer" convention the doc
  comment claims and mirrors `wsl::SyntheticFilesystemContextObserver`.
- `cargo deny check` (both in the live tree and the pinned worktree) reports `advisories ok, bans
  ok, licenses ok, sources ok`, exit 0; the only output is a pre-existing, unrelated duplicate
  `windows-sys` version warning (not gated, not introduced by this commit - `windows-sys` 0.59.0
  vs 0.61.2 both already present via unrelated transitive dependency chains).
- `scripts/check_docs.py`'s gitignore-exclusion fix (out of E10's own AC scope, sanity-checked
  only): confirmed `git check-ignore --stdin` round-trips the exact absolute path strings passed
  to it (spot-checked with a synthetic `.claude/...` path), and `check_docs.py check` passes
  (252 Markdown files) in the live tree. Not evaluated against any E10 acceptance criterion.
- `docs/development/RELEASE_GATES.md`'s "Rust cutover gate status" prose correction (also out of
  E10's own AC scope): read for basic coherence only: dated, internally consistent with the
  epics it cites as closed (E20/E21/E22/E17-S02). Not deep-audited against those epics' own
  contracts, per the review brief's scope note.

## Gates executed

| Command | Result |
| --- | --- |
| `python3 scripts/project_os.py brief E10-S01/E10-S02 --role verifier` | PASS - generated both story contracts (CR2/CR1, no CR4 Safety Verdict required) |
| `python3 scripts/project_os.py check` (isolated worktree at `b7fd880`) | PASS: `governance OK: 23 decisions, 24 epics, 110 stories` |
| `python3 scripts/check_docs.py check` | PASS: 242 Markdown files (worktree) / 252 (live tree with unrelated E24 docs); local links and safety IDs consistent |
| `python3 scripts/check_rust_workspace.py check` | PASS: 13 crates match TARGET.md, acyclic, model/safety isolated |
| `python3 scripts/check_mutation_boundary.py check` | PASS: only `mutation.rs`/`mutation_executor.rs` reference the mutation capability - no new file added a second path |
| `python3 -m pytest tests -q` | PASS: 301 passed, 34 subtests passed |
| `python3 -m ruff check .` | PASS: all checks passed |
| `python3 -m ruff format --check .` | PASS: 289 files already formatted |
| `python3 -m mypy scripts/check_docs.py` | PASS: no issues found |
| `cargo fmt --check` (rust/) | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` (native macOS, live tree and worktree) | PASS (no warnings) |
| `cargo clippy -p cancellai-platform --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings` | PASS |
| `cargo clippy -p cancellai-platform --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings` | **FAIL**: 3 `dead_code` errors (see E10-S01 findings) - reproduced in both the live tree and the `b7fd880` worktree |
| `cargo check --workspace --all-targets` (native macOS) | PASS (warnings only, no `-D warnings`) |
| `cargo check -p cancellai-cli --tests --target x86_64-unknown-linux-gnu` | PASS |
| `cargo check -p cancellai-cli --tests --target x86_64-pc-windows-gnu` | PASS (warnings only - the same 3 dead-code lints, non-fatal without `-D warnings`) |
| `cargo test --workspace` (native macOS, live tree and worktree) | PASS: 0 failed across all crates |
| `cargo deny check` | PASS: advisories ok, bans ok, licenses ok, sources ok (pre-existing unrelated `windows-sys` duplicate-version warning only) |

Not executed: real execution of `performance_memory.rs`'s Linux-gated test and
`performance_scheduled_shipped.rs`'s heavy datasets on real Linux hardware - no Linux runtime
available in this environment, matching the limitation the executor already disclosed. Only
cross-compilation/clippy checks against the Linux target were independently verified here.

## Overall verdict

**The epic cannot close as-is.** E10-S01 fails a mandatory, unconditional CI gate
(`cargo clippy --workspace --all-targets --all-features -- -D warnings` on `windows-latest`) with
a concrete, twice-reproduced error; this is a real build break on a required tier-1 platform, not
an accepted residual. E10-S02's own code is sound (PASS_WITH_RESIDUALS for the already-disclosed
Linux-execution and macOS/Windows-measurement gaps), but it cannot get a fully green CI run either
while E10-S01's Windows break stands, since both stories share one workspace-wide clippy
invocation.

Recommended repair for round 2: gate `KNOWN_NOT_SHARING`, `KNOWN_SHARING`, and
`classify_filesystem_name` (or otherwise make them reachable) on the Windows target in
`rust/crates/cancellai-platform/src/filesystem_kind.rs`, re-run the full Rust gate set on all
three platforms (native plus both cross-compile targets used in this review), and re-run
`cargo clippy --workspace --all-targets --all-features -- -D warnings --target
x86_64-pc-windows-gnu` specifically before re-submitting, since this exact gap was not caught by
either story's own verification commands. No other blocking defect was found in either story's
own acceptance criteria.

## Addendum - repair applied and re-verified

The executor (this repository's standing Claude session) repaired the finding above immediately
after this self-review completed, before any real independent review occurred. `rust/crates/
cancellai-platform/src/filesystem_kind.rs`'s `KNOWN_NOT_SHARING`, `KNOWN_SHARING`, and
`classify_filesystem_name` are now `#[cfg(any(test, target_os = "macos", target_os = "linux"))]`,
matching the exact precedent `crate::wsl::classify_fstype`/`longest_matching_mount_fstype`
already set in this same crate for the identical shape of gap - the pure classification logic
stays exhaustively unit-tested on every host, while a genuinely non-macOS/non-Linux production
build (Windows included) no longer defines a function/constants nothing in that build calls.

Re-verified, this time including the Windows target this self-review's own recommendation named
explicitly:

```text
$ cargo clippy -p cancellai-platform --all-targets --all-features -- -D warnings                    (native macOS)  PASS
$ cargo clippy -p cancellai-platform --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings  PASS
$ cargo clippy -p cancellai-platform --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings     PASS
$ cargo clippy --workspace --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings               PASS
$ cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings            PASS
$ cargo fmt --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace && cargo deny check   (native macOS)  all PASS, 0 failed
```

This addendum does not upgrade this document to an independent review - it records that the one
concrete, reproduced defect this self-review found is now fixed and cross-platform-verified,
including on the exact target (`x86_64-pc-windows-gnu`) whose clippy run had never actually been
exercised before this review (a real gap in the original executor evidence, now closed). E10-S01
and E10-S02 remain `ready_for_review`, not `done` - that transition still requires the genuine
independent review this document is not.

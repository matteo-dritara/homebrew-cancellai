# Release Evidence - v1.7.0

## Source

- Tag: `v1.7.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-02
- Process exception: **Owner-authorized combined verify+fix+close round, 2026-09-02 - see conversation record.** Codex was explicitly authorized to verify, repair, self-reverify, write the E07-S09 CR4 Safety Verdict, and close the named stories for this release round only.

## Included work

- Epic: E07 - Unix Cross-Platform Hardening
- Closed stories: E07-S01 (CR3), E07-S05 (CR3), E07-S09 (CR4); E07-S08 (CR2) was already done.
- Cancelled/superseded: E07-S07 (CR4); its prior `REJECT` verdict is historical evidence and not a passing release verdict. E07-S09 carries and closes its remaining scope.
- Epic verifier record: `project/evidence/E07-VERIFIER-REVIEW.md`.
- Story evidence: `project/evidence/E07-S01/EVIDENCE.md`, `project/evidence/E07-S05/EVIDENCE.md`, `project/evidence/E07-S09/EVIDENCE.md`.
- Passing CR4 Safety Verdict: `project/evidence/E07-S09/SAFETY_VERDICT.md` (`PASS_WITH_RESIDUALS`).
- Also reviewed standalone in the same batch, without closing E20: `project/evidence/E20-S04/EVIDENCE.md` (CR2, `PASS_WITH_RESIDUALS`).

## Gates

Re-run at the tag by `.github/workflows/release.yml`; run locally before tagging:

```text
Full Python and Rust command sets from `AGENTS.md`, including pytest, Ruff, mypy, all project
validators/parity checks, release/process checks, and Rust fmt/clippy/check/test/deny.
```

- G1 Functional: PASS locally; full unit/integration/characterization/parity suites green.
- G2 Safety: PASS_WITH_RESIDUALS; mutation-boundary check and adversarial root-link/race tests green; E07-S09 Safety Verdict accepted with recorded fail-closed residuals.
- G3 Compatibility: PASS locally on macOS plus Rust cross-target compilation for Linux and Windows; tag/PR CI must pass the real macOS/Linux/Windows matrix before publication.
- G4 Operability: PASS for current scope; workspace tests, install/rollback smoke, performance microbenchmark, and release/process checks green.

## Compatibility

- Platforms: macOS exercised locally. Rust cross-target compilation passed for
  `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu`; PR/tag CI supplies native
  macOS/Linux/Windows execution before release publication.
- Unix: macOS/Linux identity, allocation, process, filesystem, and sealed-root capabilities
  remain explicit. Intermediate component links are refused for configure and clean.
- Windows: native identity and reparse-safe root handling remain unsupported/fail-closed;
  inventory is explicitly `Partial` and root-only until E20-S01. No destructive Windows
  capability is claimed.
- Providers/capabilities: Codex CLI and Claude Code reference adapters. Unknown/unsupported
  facts reduce authority rather than being inferred as safe.
- State/schema migrations: none. The shipping Python reference keeps no persistent state; the
  Rust JSON identity shape includes the already-reviewed `modified_nanos` field.

## Supply chain

- Checksums: the Homebrew formula records the SHA-256 of the immutable tag archive, written and independently reviewed after `scripts/release.py finalize`.
- Rust dependency policy: `cargo deny check` passes advisories, bans, licenses, and sources; only the pre-existing unmatched allowance warnings for BSD-2-Clause/BSD-3-Clause/ISC remain.
- SBOM: not yet produced; deferred to E17. The shipping Python file remains stdlib-only.
- Provenance/attestation: deferred to E17.
- Signature verification: deferred to E17.
- Release manifest: this file.

## Install smoke tests

- Homebrew: formula audit/style/install/test run in CI; final checksum is produced only from the published v1.7.0 tag archive.
- Rust beta: `cancellai-cli` install/rollback integration tests pass; it remains side-by-side and is not the canonical shipping CLI.
- Direct PowerShell/Linux packages: not shipped at this stage.

## Performance

- `cancellai-inventory/tests/performance_micro.rs` passes its CI regression ceiling; scheduled heavy benchmark infrastructure remains separate.
- The E07 root checks add a bounded number of `openat`/identity syscalls per path component and are not a hot loop.
- No persistent runtime/self-budget format changes are introduced.

## User-visible changes

### Added

- E06-S01: `cancellai-cli` gains its first real command surface -
  `status`/`inspect`/`plan`/`clean`/`configure`/`version` against the Rust engine
  (`docs/CLI_RUST.md`). `status` is the read-only default (no subcommand or flag ever implies
  `clean`); `clean` is the only mutating command, gated by `--dry-run`/`--yes`/interactive
  confirmation and routed exclusively through `cancellai-safety`'s single mutation boundary.
  This is a beta command surface, not yet the canonical engine (`docs/development/
  MIGRATION_PYTHON_RUST.md`) - `cancellai.py` remains the shipping reference until E06 closes.
- E06-S02: a differential parity gate (`scripts/rust_python_parity.py`) runs the Python
  reference and the Rust CLI over the full `NORMATIVE` fixture corpus, comparing which
  sessions each engine would delete. Wired into pre-commit/CI. Building it surfaced and fixed
  two real E06-S01 defects: an incomplete companion-payload scan only withheld the one
  affected session instead of the whole tool (SI-008/SI-009), and a Claude home with no
  `projects/` directory was misreported as an incomplete scan instead of legitimately empty.
- E06-S03: documents and proves the beta side-by-side model for `cancellai-cli` -
  `version` identifies the engine, and `cancellai`/`cancellai-cli` share no install path or
  local state (`docs/RELEASING.md`, `docs/development/MIGRATION_PYTHON_RUST.md`), so rollback
  during beta is simply not invoking the Rust binary. Proven with new smoke tests
  (`rust/crates/cancellai-cli/tests/install_rollback.rs`): every read-only command, and even a
  real `clean`, touches nothing under `$HOME` outside the provider artifacts explicitly
  targeted.
- E06-S04: records the Rust cutover gate checklist (`docs/development/RELEASE_GATES.md` "Rust
  cutover gate status") and its current verdict - **not ready**; `cancellai.py` remains the
  sole canonical, shipping engine. No user-visible behavior changed in this entry; it exists so
  this file does not read, by omission, as though cutover had happened.
- E07-S07: `cancellai-cli clean`/`configure` refuse a default-named root
  (`$HOME/.claude`/`$HOME/.codex`, no override) that is itself a symlink/reparse point,
  independently re-checked immediately before establishing the root or writing configuration -
  not only at classification time (`docs/architecture/PLATFORM_MODEL.md` "Default-root
  authority never rests on a lexical name alone"). Closes an E06 verifier review round 2
  finding: authority previously followed the lexical `$HOME/.claude` name alone, so a symlinked
  default root was still treated as mutation-eligible.
- E07-S07 (round 2): closes an E07-S07 round-1 independent verifier review finding - `configure`'s
  own re-check above narrowed but did not close its TOCTOU: a default root swapped to a symlink
  *after* that check and before the raw path-based settings write reached outside the approved
  root. `configure` now routes every read/write through a new `cancellai-sealedfs` crate
  (`docs/adrs/0017-sealed-root-handle-for-configuration-writes.md`): the root is opened exactly
  once with `O_NOFOLLOW` and retained, with every following operation issued via
  `openat`/`renameat` against that descriptor rather than the original path, closing the race by
  construction. **Behavior change**: `configure` now refuses outright (rather than attempting an
  unprotected write) on every platform without a verified no-follow/handle-relative
  implementation - today, every non-Unix platform - matching `clean`'s existing fail-closed
  posture there.
- E07-S09: closes an E07-S07 round-2 independent verifier review finding - round-1's
  `O_NOFOLLOW` bound only `configure`'s final root component, so a *default* root reached
  through an intermediate symlink (e.g. `$HOME` itself being a link, with a real, non-symlink
  leaf directory underneath it) was still silently followed and written through
  (`docs/architecture/PLATFORM_MODEL.md` "Intermediate components need the same no-follow
  treatment as the leaf"). `cancellai-sealedfs::SealedRoot::establish` now walks every path
  component handle-relatively from the filesystem root, refusing the moment any component -
  intermediate or final - is a symlink/reparse point, and creating only the final absent
  component via `mkdirat` against an already-held parent descriptor. E07-S09's own round-1
  independent verifier review found this closure reached only `configure`: `clean` establishes
  its root through the separate `ApprovedRoot` capability, whose `canonicalize()` step still
  silently resolved through the identical intermediate link, so `clean --yes` could still purge
  a stale session reachable only through a symlinked `$HOME` (`docs/architecture/
  PLATFORM_MODEL.md` "The fix had to reach `clean`, not only `configure`"). Round 2 exports a
  read-only counterpart, `verify_no_intermediate_links`, used by `establish_verified_root`
  before `ApprovedRoot::establish` for the default root. The owner-authorized combined closure
  review found one further race in that handoff: a component could be swapped after the walk
  but before canonicalization. The walk now returns a retained final-directory handle, and
  cleanup refuses unless the subsequently established root has the same device/inode identity.
- E07-S08: `scripts/rust_python_parity.py`'s divergence allow-list is now structured
  (fixture/scenario/field-scoped, citation content-checked) rather than free-text, and its
  comparison surface grew from six to eight fields covering every discovered identity record,
  protection coverage, and root authority for every `NORMATIVE` fixture - closing an E06
  verifier review round 2 finding where any real, accepted ADR citation could suppress an
  unrelated divergence regardless of what it actually authorized
  (`docs/development/MIGRATION_PYTHON_RUST.md` M6).
- E07-S05: closes the intermittent Linux CI failure of `cancellai-platform`'s
  `identity::tests::toctou_file_deleted_and_recreated_with_identical_content_still_changes_
  identity` and `mutation::tests::confirmed_delete_rejects_a_target_already_swapped_before_open`.
  Reproduced natively in a real Linux container (not hypothesized): a zero-delay
  delete-and-recreate reuses the freed inode in ~98% of iterations and lands within the same
  ~1ms mtime clock tick, so `device`+`inode`+`kind`+whole-second-`modified` alone cannot always
  distinguish the two objects - a real `IdentityToken` gap, not only a fixture one.
  `IdentityToken::Unix` gains `modified_nanos` (the raw `st_mtime_nsec` sub-second remainder,
  not derivable from the shared whole-second `Timestamp` clock/retention type);
  `cancellai-platform::mutation`'s `confirmed_delete_file_inner` - which compared device+inode
  only, bypassing `IdentityToken` entirely - now also compares it at both its open-time and
  immediately-before-unlink checks (SI-013/SI-017). The two fixtures also had an
  over-specific/false-on-Linux assertion ("recreation must allocate a new inode") removed and
  gained a small real-world-realistic delay in place of an unrealistic zero-delay recreate,
  without weakening the byte-identical-content case either test verifies. Verified with 60
  consecutive passing runs (30 iterations of both tests) in a real Linux container - exceeding
  the story's own 20-consecutive-run bar.

### Fixed

- E20-S04 (formerly E07-S06): identified why `cancellai-inventory`'s
  `completeness::tests::ac1_a_fully_readable_tree_is_complete` and
  `scan::tests::ac1_one_traversal_visits_every_directory_exactly_once` fail on real Windows CI -
  `scan::walk_directory` only recurses into a child whose identity is *confirmed*
  (`IdentityObservation::Identity`, never `Unsupported`, per SI-017), and
  `SystemIdentityObserver` reports `Unsupported` unconditionally on Windows (E03-S01's
  pre-existing residual), so a real Windows scan currently visits only the scope root -
  correct, safety-driven behavior, not a traversal bug; weakening the identity-confirmed gate
  to make the old assertions pass would have been the wrong fix. Both tests gated `#[cfg(unix)]`
  with `#[cfg(windows)]` counterparts added asserting the actual current behavior
  (`Partial`/`directories_visited == 1`), and `docs/architecture/PLATFORM_MODEL.md` gains an
  "Accepted limitation" subsection - real Windows traversal depth requires E20-S01's native
  identity implementation.
- `cancellai-inventory/tests/performance_micro.rs`'s
  `scan_scope_completes_within_budget_for_a_small_dataset` had the identical E20-S04
  Windows-traversal assumption (an exact `paths_observed` count only reachable with confirmed
  identity) - gated that specific assertion `#[cfg(unix)]`; the time-budget and
  views-do-not-re-walk checks it also makes remain meaningful and run on every platform.
- `cancellai-safety`'s `mutation_executor`/`root_capability`/`sealed_plan` had 19 tests
  (`mutation_executor`'s entire test module, plus `root_capability::tests::bind_a_plain_child_
  succeeds`/`bind_the_root_itself_is_rejected`/`bind_a_path_outside_the_root_is_rejected`, plus
  `sealed_plan::tests::seal_derives_root_and_artifact_identity_from_real_capabilities`) that
  construct a real `ApprovedRoot` via the real `SystemIdentityObserver` and were not
  `#[cfg(unix)]`-gated - real Windows CI failed every one of them with the same
  `CandidateIdentityUnsupported` error (E03-S01's pre-existing residual, unrelated to and
  predating this session). `mutation_executor`'s entire `mod tests` is now `#[cfg(unix)]`
  (every test in it depended on the same real-root helper); the three `root_capability` tests
  and the one `sealed_plan` test are individually gated.
- `cancellai-cli/tests/cli_behavior.rs`'s
  `configure_writes_the_native_claude_retention_setting_and_preserves_other_keys` was not
  `#[cfg(unix)]`-gated, so real Windows CI ran it expecting a successful write - but
  `configure`'s write capability (`SealedRoot`) has no verified handle-relative implementation
  on non-Unix platforms and fails closed there by design (`docs/CLI_RUST.md`'s own "Known
  gaps", unrelated to this session's other changes). Gated the success-path test `#[cfg(unix)]`
  and added a `#[cfg(windows)]` counterpart asserting the disclosed refusal instead, matching
  the existing pattern for the symlinked-`$HOME` configure/clean tests.
- `cancellai-sealedfs` failed to build on Windows: `validate_child_name` and its `CString`
  import lived outside the `#[cfg(unix)]` boundary, so they became genuine dead code once
  `unix_impl` (the only caller) stopped compiling in on non-Unix targets - found on real
  Windows CI while verifying E07-S09, not caught locally since this executor's environment is
  macOS. Both are now `#[cfg(unix)]`-gated with the rest of the module they belong to.
- `cancellai-provider-codex::native_delete`'s `FakeCli`-based tests
  (`ac2_a_fake_cli_advertising_force_is_reported_supported` and three others) intermittently
  failed on Linux CI with `ProbeFailed { reason: "Text file busy (os error 26)" }` - reproduced
  directly in a real Linux container (not hypothesized): writing a fresh script then executing
  it from a highly parallel `cargo test` run can race a *different*, concurrently-forking test
  thread on the process's own shared file-descriptor table. Confirmed serial-safe (0/200
  failures at `--test-threads=1`) and only observed under real concurrency, so the fix is a
  bounded retry-on-`ProbeFailed` in the test harness itself (`codex_delete_supported_retrying`)
  rather than any change to `codex_delete_supported`'s production logic - `ProbeFailed` is
  already the correct, conservative production answer for "could not tell". Verified with 60
  consecutive passing runs at `--test-threads=8` in the same container (previously ~14% flake
  rate).
- `directory_size`/`safe_lstat_size` no longer count a symlink's own `lstat().st_size` toward
  a reported byte total. For a symlink that value is the byte length of the stored target path
  string, not real disk footprint - reporting it as "size" made coverage/size output for any
  entry containing a symlink depend on the absolute path length of wherever the symlink
  happened to live, silently differing by machine and even by which temp-directory prefix a
  test run used. Found via the `codex-symlink-escape`/`claude-symlink-protected-name`
  characterization fixtures diverging between macOS and Linux CI; a symlink already contributes
  nothing to deletion or discovery accounting elsewhere (E00-S02 / ADR-0013) and now
  consistently contributes nothing to size accounting either.

## Known residual risks

- E07-S01: notification and user-service capability seams remain deferred to the Guardian
  stories that first consume them; no current caller silently assumes they exist.
- E07-S05: metadata identity can still collide if inode reuse and recreation occur inside the
  filesystem's underlying timestamp tick. Immediate revalidation/open checks narrow but cannot
  mathematically eliminate that residual without a different persistent-handle/generation design.
- E07-S09/E20: Windows native identity/junction/reparse handling is unimplemented and fails
  closed. E20-S01 carries the implementation/real-environment verification work.
- The pre-existing Unix final recheck-to-`unlink` race remains disclosed in
  `cancellai-platform::mutation`; this release does not widen it.
- SBOM/provenance/signature/attestation remain E17 scope.

No unresolved HIGH/CRITICAL release-specific safety risk is accepted silently; destructive
authority is reduced or refused for every unsupported platform/root state above.

## Rollback

Point the Homebrew formula back to v1.6.0 and its prior checksum. The shipping Python tool has
no persistent migration to reverse; Rust beta data formats are unchanged by the root-handle
repair. Do not delete or move published tags. If the E07-S09 root boundary is implicated,
disable Rust `clean`/`configure` use and revert the code while preserving the fail-closed
posture—never restore the former check-only handoff as an accepted safety state.

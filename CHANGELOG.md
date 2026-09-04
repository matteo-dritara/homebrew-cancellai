# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `cancellai-cli` (the beta target-engine CLI) now has a real `--help`/`-h`/`--version`
  surface and per-command help (`cancellai-cli clean --help`, etc.), matching the reference
  CLI's own top-level surface (E22-S03, `CR-TE-07`). Argument parsing moved from a hand-rolled
  loop in `main.rs` to `clap` ([ADR-0019](docs/adrs/0019-dependency-rings-per-crate.md)).

### Changed

- `cancellai-cli` now refuses a flag irrelevant to the chosen command (e.g. `status --dry-run`,
  `clean --claude-retention`) with exit code 2, instead of silently accepting and ignoring it
  as every command's flags did before this release. `--help`/`-h`/`--version` are an explicit
  exception: wherever they appear, they still short-circuit remaining validation and exit
  before any command runs, matching `clap`'s own precedence and common CLI convention (`git`,
  `cargo`) - see `docs/CLI_RUST.md`'s "Argument parsing" section.

### Fixed

- **A Codex subagent tree with a stale root and a recently-touched child is no longer an
  individual delete candidate for the stale member** (E22-S04). `cancellai-policy::retention`
  gated `--keep-latest` pinning on the tree's effective (max-of-members) mtime but evaluated
  each member's own staleness independently, so a tree the reference protects in full - any
  recent member protects the whole tree, not just the pinning rail - could still surface the
  old-looking member as a `Delete` action in the target engine. `resolve_codex` now applies the
  same tree-level cutoff gate `cancellai.py::choose_codex_old_sessions` does before classifying
  any member's staleness.

### Documentation

- Recorded that `cancellai-cli clean` deletes Codex sessions at the filesystem level only,
  even when the installed `codex` CLI advertises its own `--force`-capable delete: this is now
  a stated, permanent divergence from `cancellai.py` (which prefers the vendor command) rather
  than an unstated gap (E22-S05, `CR-TE-10`). See `docs/CLI_RUST.md`'s "Known gaps" for why -
  wiring it would add a second mutation primitive to the safety kernel and is deferred to a
  dedicated future story, not a side effect of this one.

## [1.8.0] - 2026-09-03

### Fixed

- **A directory the scan could not read no longer authorizes deletion** (E21-S03, `CR-TE-01`).
  On an ordinary tree - one directory without read permission - `cancellai-cli` deleted an
  eligible artifact and exited `0` while reporting the scope complete and its knowledge
  verified; `cancellai.py` withheld every destructive action for that tool and exited `4`. Both
  provider adapters discarded every failure to observe part of the tree with a bare
  `else { continue }`. They now record each one with a cause and a path, withhold the whole
  tool, and degrade `knowledge_confidence` for every artifact in the scope (SI-008, SI-009,
  SI-010, C-02). The Claude side was broader than previously recorded: E06-S02 had repaired only
  the companion-payload branch, and an unreadable **project** directory still passed silently,
  disclosed nowhere.
- `scan_completeness[].error_count` reports the real number of unobservable paths instead of
  `u32::from(!complete)`, which was only ever `0` or `1` (`CR-TE-10`).
- **The delete path prevents the path-swap race instead of detecting it** (E21-S07, `CR-TE-05`).
  The unlink is issued through `cancellai-sealedfs`'s handle-relative `unlinkat` against a
  directory descriptor opened once with `O_NOFOLLOW` at every component, so a rename or
  symlink-swap after validation cannot redirect it. Consequence, intended and user-visible: a
  provider root reached through a symlinked path component can no longer be cleaned - the rule
  E07-S09 already set for root establishment, now holding at the moment of mutation.
  `MutationOperation`'s two unconfirmed, unreachable variants (`Quarantine`,
  `DeleteDirectoryTree`) were removed rather than left armed for E12 to inherit (`CR-TE-11`).
- Rollout metadata reading honours the 512 KiB bound it documents instead of loading the whole
  transcript (E21-S06, `CR-TE-04`). Measured on a single 287 MB rollout: peak RSS **2.9 MB**,
  against 303 MB before - and below the Python reference's own 27.7 MB.

### Added

- Two fixtures the corpus never had - `codex-partial-tree` and `claude-partial-project` - both
  `NORMATIVE` and running through the differential gate in both root-origin scenarios (E21-S02).
  They were written to **fail** against the unrepaired engine, and did; the failing run is
  committed in their evidence packet. `scripts/check_fixtures.py` now refuses an undeclared
  category asymmetry between the two reference providers, because the corpus carrying
  `partial_tree` for Claude and not for Codex is what let the gate stay green while the engine
  deleted (`CR-TE-03`).
- Scope completeness is a shared *type* obligation on every provider adapter
  ([ADR-0018](docs/adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md),
  E21-S04). `ProviderResolution` hands out planning candidates only through a value carrying the
  scope's completeness, with a `compile_fail` regression proving the bare-candidates route is
  unreachable; `scripts/check_rust_workspace.py` fails if `cancellai-cli` stops being able to
  reach `cancellai-inventory` at all, which is how `CR-TE-02` went unnoticed.
- A performance gate on the discovery path the CLI actually executes (E21-S05). Every timing
  assertion is paired with an assertion on what the resolution produced, so a benchmark
  measuring an empty tree fails instead of reporting an excellent number.

### Note on how E21 was verified

Round 1 of independent adversarial review (Codex) returned `FAIL` for five of the seven stories
and reproduced a real escape the first implementation had left open: an unreadable Claude
`projects/` root was converted into a clean empty scan, so `clean --yes` exited `0` where the
frozen reference exits `4`. The completeness was computed correctly in discovery and discarded
one layer up - the same class of defect this epic exists to close. Every finding is repaired and
pinned by a regression written against the verifier's own reproduction; repairing them surfaced
one more instance of the same pattern (`Path::exists()` collapsing "not installed" into "not
readable"), also closed. The epic was closed by owner decision without spending the second
review round, which means these repairs carry no independent confirmation -
`project/evidence/E21-CLOSURE.md` records that and the residual risk it accepts.

- An independent target-engine review is committed as
  [`docs/audits/2026-09-03-CODE_REVIEW.md`](docs/audits/2026-09-03-CODE_REVIEW.md), with
  thirteen findings (`CR-TE-01`..`CR-TE-13`) converted into story contracts under two new
  epics rather than left as prose: **E21 Target Engine Trust Remediation** and **E22
  Engineering System Hardening**. They are new epics, not additions to E06, because E06 has
  already used both independent review rounds ADR-0014 permits.
- [ADR-0018](docs/adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md):
  scope completeness becomes a shared *type* obligation on every provider adapter, while the
  adapters keep their layout-specific traversal. `cancellai-inventory`'s completeness model is
  currently unreachable from the shipped binary, which is why the defect its own reviewer
  rejected in E04-S03 reappeared in the adapters that replaced it.
- [ADR-0019](docs/adrs/0019-dependency-rings-per-crate.md): the safety kernel stays
  dependency-free except by dedicated ADR; the experience and persistence crates may use
  mature, licence-checked libraries. This is what will give `cancellai-cli` the `--help`/
  `--version` surface it currently lacks entirely.

### Fixed

- `project/roadmap.json` declared `current_phase: "P0"` while both P0 epics were `done` and P1
  stood one epic from closing, so `PROJECT_STATUS.md` generated a phase the project had already
  left. Corrected to `P1` (`CR-TE-12`).

### Note

No user-visible behavior changed in this entry. `cancellai.py` remains the sole canonical,
shipping engine; the findings concern the beta Rust engine and the gates around it. `E06-S04`
now records `E21` and `E22-S01` among its blockers, so this file does not read, by omission, as
though the cutover checklist were unchanged.

## [1.7.0] - 2026-09-02

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

## [1.6.0] - 2026-08-31

### Changed

- Epic E05 implemented the Provider API and Reference Adapters: a nine-capability
  `ProviderCapabilities` contract (`cancellai-provider-api`) where capability absence is
  explicit and never inferred from provider identity, and every response carries evidence and
  confidence by construction; Built-in Verified/Community Verified/Local Custom/Untrusted
  provider trust wired into the Effective Authority lattice as its own constraint, gated by
  `TrustedTier`, an opaque type whose only public constructors are the safe `Untrusted` default
  and a checked, evidence-requiring promotion - closing the `ProviderTrustAuthority` gap
  `docs/architecture/DOMAIN_MODEL.md` had called out since E03-S04; Claude Code and Codex CLI
  reference adapters porting `cancellai.py`'s discovery/classification/session-relationship
  logic to Rust (root fingerprinting, the Unicode-canonical-caseless protected-name barrier,
  session/subagent-graph discovery, native-delete capability detection), each checked against
  the committed Python characterization corpus by reproducing its fixtures directly; and a
  generated, per-capability reference-provider compatibility matrix in `docs/PROVIDERS.md`. An
  independent review round found and this epic's own repair cycle closed a CR4 defect before
  close: the first version of provider trust typed its authority-lattice input as a bare,
  publicly-constructible enum, so an external caller could self-assign the highest trust tier
  with no promotion evidence at all - the exact self-assignment SI-021 prohibits. `cancellai.py`'s
  own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface
  yet.

## [1.5.0] - 2026-08-29

### Changed

- Epic E04 implemented the Single-Pass Inventory Engine: `FileFacts`, a per-path evidence record (`rust/crates/cancellai-inventory`) composed from independently-observed logical size, allocated/physical size (a new `AllocationObserver` platform seam distinguishing sparse/cloned/compressed allocation from logical length), identity, and filesystem-boundary facts, with every unsupported metric an explicit typed value rather than a fabricated zero or borrowed metric; `scan_scope`, a single recursive walk per scope whose status/top-consumers/planning report views are pure reads over one snapshot, never a re-walk, and which never follows a symlink or descends across a device/filesystem boundary (SI-018); scope-level completeness classification (`Complete`/`Partial`/`Unknown` with named permission/I/O/disappearance/unsupported-feature reasons, SI-008/SI-009) that a planning-facing view cannot be obtained without, enforced by construction and by a `compile_fail` regression proving the bare-candidates accessor is unreachable outside the crate; and a performance baseline (a CI microbenchmark plus scheduled 10k/100k/1M-entry benchmarks with a machine-readable trend artifact). An independent review round found and this epic's own repair cycle closed a CR3 defect before close: a `read_dir`-listed entry's unreadable/vanished observation was silently dropped instead of degrading scope completeness, and the bare planning-candidates accessor was reachable without the completeness it should always carry. `cancellai.py`'s own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface yet.

## [1.4.0] - 2026-08-29

### Changed

- Epic E03 implemented the Formal Safety Kernel: cross-platform artifact identity tokens with fail-closed Windows refusal (`rust/crates/cancellai-platform`, SI-013/SI-017); an immutable `SealedPlan` sealed only from a verified root/target capability pair, with fail-closed identity/root revalidation (SI-013/SI-016); typed `ApprovedRoot`/`BoundedPath` root and filesystem-boundary capabilities rejecting root-self deletion, escapes, symlink-escape tricks, and cross-device mounts (SI-002/SI-003/SI-018); a monotonic-minimum Effective Authority lattice with a deterministic explanation trace, collapsing unknown/active/protected/partial state to non-destructive authority (SI-001/SI-007/SI-008/SI-009); and the mutation executor itself - the sole, statically-enforced path to real filesystem deletion, checking root binding, authority, and reversibility before mutating, and confirming a plain file's identity via an open file descriptor immediately around the delete syscall (SI-013/SI-019/SI-020). An independent review round found and this epic's own repair cycle closed three CR4 defects before close: a raw mutation capability bypassing every one of the above checks, a plan executable against a target from a different root, and an executor that never consulted its own recorded authority/reversibility. `cancellai.py`'s own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface yet.

## [1.3.0] - 2026-08-29

### Changed

- Epic E02 bootstrapped the target Rust workspace ahead of the spec-first migration: twelve crates (`docs/architecture/TARGET.md`) with an acyclic dependency graph and no provider-specific code in `cancellai-model`/`cancellai-safety`; a quality baseline enforcing `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, and `cargo deny` (license allow-list, unknown-registry/git denial, MSRV 1.85.0) across macOS/Linux/Windows CI (`rust/deny.toml`, ADR-0015); a typed diagnostic model separating invalid-input/safety-block/incomplete-inventory/compatibility/mutation-failure/internal-fault with stable human/JSON error codes; and deterministic `Clock`/`FsObserver` seams (`rust/crates/cancellai-platform`) that keep the Python reference's absent-vs-unreadable filesystem distinction (SI-008/SI-009/SI-010) as a typed contract, including for a modification time the OS cannot report or represent - never silently substituted with a credible-looking epoch timestamp. `cancellai.py`'s own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface yet.

## [1.2.0] - 2026-08-28

### Changed

- Epic E01 turned the Python v1 CLI into a characterized, versioned executable reference ahead of the Rust migration: canonical domain vocabulary (`docs/architecture/DOMAIN_MODEL.md`), a synthetic Claude/Codex provider-layout fixture corpus (`tests/fixtures/`), versioned inventory/plan/explanation/result JSON contracts with an explicit compatibility policy (`docs/architecture/JSON_CONTRACTS.md`), a committed characterization of Python's actual behavior on that corpus classified normative/intentional-divergence/legacy-only/known-defect (`scripts/characterize.py`), and a differential comparison contract and self-testing harness for the eventual Python-vs-Rust migration gate (`scripts/diff_harness.py`, `docs/development/VERIFICATION_STRATEGY.md`). `cancellai.py`'s own runtime behavior is unchanged.
- `cancellai.py` is now maintenance-only (the Python reference freeze, `AGENTS.md`): only parity fixes against the committed characterization, safety/security fixes, and migration-support tooling are accepted going forward, not merely until this epic closed. New product capability targets the Rust implementation.

## [1.1.0] - 2026-08-28

### Security

- Protected names (`CLAUDE_PROTECTED_NAMES` / `CODEX_PROTECTED_NAMES`) are now an executable barrier instead of documentation. They are enforced when the plan is built and again inside `safe_remove`, immediately before any deletion, so a future discovery change cannot silently invalidate them. Comparison uses the Unicode canonical caseless form (NFD, casefold, NFD): APFS is case-insensitive and stores decomposed filenames, so neither raw string equality nor case folding alone is filename comparison (E00-S01).
- `--aggressive` no longer bypasses the age cutoff for Claude legacy directories and rebuildable cache files. It widens which categories are eligible; retention is applied independently (E00-S03).
- Only the provider's own default directory is mutated. A root relocated with `CODEX_HOME` or `CLAUDE_CONFIG_DIR` is fully inspectable but is never deleted from or written to: nothing observable in a filesystem proves a directory belongs to a provider, so this release refuses to act on structural resemblance. Two weaker schemes were tried and rejected by independent review before this one (E00-S02, ADR-0013 superseding ADR-0012).
- The protected-name barrier is applied to the path as written as well as after resolution, and matches case-insensitively. Previously a protected entry that was itself a symlink lost its protection entirely, and a candidate spelled `Plugins` bypassed the barrier on case-insensitive APFS (E00-S01).
- An unusable process observation is no longer read as "no provider is running". `ps` output that does not contain this process is not a full listing, so a missing, failing, filtered or stubbed enumeration refuses cleanup unless `--allow-running` is given (E00-S09).
- `history.jsonl` is never rewritten through a symlink. `os.replace` would have swapped the link for a regular file and silently detached whatever it pointed at (E00-S06).
- Filesystem observation errors are no longer silently flattened into zero. Every discovery guard goes through an `lstat` that separates "not there" from "could not look" - `Path.exists()` answers False for both, so using it as a guard turned an unreadable directory into an empty one. An unreadable path now withholds destructive authority for that provider, and `status` lists the unreadable paths and prints partial totals as lower bounds (E00-S05).
- Claude `history.jsonl` trimming now streams bytes instead of loading and re-encoding the file, so retained lines - including CRLF endings and a missing trailing newline - are preserved verbatim. It re-identifies the source immediately before the atomic replace and abandons the rewrite if a provider wrote concurrently. Trimming is skipped entirely while a Claude process is running, even under `--allow-running`, and a failed trim is reported instead of looking like "nothing to do" (E00-S06).

### Changed

- **Breaking:** flags without a subcommand no longer normalize to `clean`. `cancellai --days 14` now runs the read-only `status` view; deletion requires typing `clean`. An unrecognized verb is a usage error (E00-S04).
- **Breaking:** a relocated `$CODEX_HOME` / `$CLAUDE_CONFIG_DIR` can no longer be cleaned or configured, only inspected. This is a capability regression, taken deliberately: see ADR-0013. Default roots are unaffected.
- **Breaking:** `clean` exits `3` on mutation failure (previously `2`) and `4` when safety blocked or deferred the requested work. No failure path escapes the taxonomy: an unexpected bug also reports `3` rather than Python's exit code `1`, which automation cannot distinguish from a declined prompt. Exit `2` is now reserved for invalid usage and refused configuration roots. `--json` output carries `exit_code`, `blocked_tools` and `deferred` (E00-S04).
- `status` reads each provider root in a single pass instead of traversing it for the total and again for the largest entries.
- `status --json` and `clean --json` now report per-root `origin`, `confidence`, provider `markers` and `destructive_allowed`, plus a `scan` object and `withheld_tools`.

### Added

- `status --coverage` classifies every top-level provider entry as `selective`, `selective-aggressive`, `aggressive-only`, `trimmed`, `protected`, `reported` or `unknown`, with a legend. There is deliberately no state meaning "deleted as it stands", because no top-level entry is treated that way: `projects/` and `sessions/` are containers whose *contents* are selected by age and policy, and `history.jsonl` is trimmed rather than deleted. Unknown entries are reported so provider layout drift stays visible and are never cleanup candidates. The same classification is exposed in `status --json` (E00-S08).

### Changed

- Added the cancellAI Engineering Operating System (cEOS): product constitution, decision register, target architecture, threat model, safety invariants, evidence-gated development model, Claude/Codex executor-verifier protocol, and machine-readable roadmap/backlog control plane.
- Reframed the long-term product from a macOS Claude/Codex cleanup script to a local-first, cross-platform, provider-agnostic Agent State Control Plane while clearly separating that target from the currently released Python v1 feature set.
- Documented the spec-first Python-to-Rust migration and the P0 trust-floor work that must land before the reference implementation is frozen.
- Required status-check names in branch protection are now verified against the contexts the workflows can actually report. A required check named `test` was blocking every pull request permanently while a matrix produced `test (3.10)` and `test (3.14)`; a name that matches no job never reports and is indistinguishable from a slow check.
- Added governance/document integrity automation, story-specific executor/verifier briefs, CodeQL scanning, CODEOWNERS, incident response, synthetic-fixture policy, and supply-chain-aware CI foundations.
- Bumped the pinned `pytest` development dependency to 9.0.3, closing a Dependabot advisory about vulnerable tmpdir handling. Development tooling only; the shipped tool has no runtime dependencies.
- Replaced automatic Dependabot merge behavior with review-gated dependency updates and pinned first-party GitHub Actions to immutable revisions in active workflows.


## [1.0.2] - 2026-08-27

### Fixed

- `CODEX_PROTECTED_NAMES` now includes `plugins`, matching
  `CLAUDE_PROTECTED_NAMES`. Found by dogfooding against a real `~/.codex`:
  `plugins/` holds genuine installed-plugin state (`plugins/cache`,
  `plugins/.plugin-appserver`), not disposable cache. No code path sweeps
  it today, so this is a defense-in-depth fix, not a behavior change.

## [1.0.1] - 2026-08-27

### Added

- `AGENTS.md` / `CLAUDE.md`: repo-specific instructions for AI coding agents.
- `.github/CONTRIBUTING.md`, `.github/SECURITY.md`, `.github/CODE_OF_CONDUCT.md`,
  issue and pull request templates, and an issue template chooser that
  disables blank issues.
- `docs/ARCHITECTURE.md` and `docs/RELEASING.md`.
- `docs/CLI.md`: a command reference generated directly from the argparse
  definitions by the new `scripts/gen_docs.py`, checked for drift in CI.
- `pyproject.toml` dev-tooling config (`ruff`, `mypy` in strict mode) and a
  matching `.pre-commit-config.yaml`.
- `.editorconfig` and `.github/dependabot.yml` (GitHub Actions ecosystem).
- `.github/workflows/dependabot-auto-merge.yml`: auto-merges Dependabot PRs
  once the required `test`/`lint`/`homebrew` checks pass.
- CI now also runs `ruff check`, `ruff format --check`, `mypy --strict`, and
  the docs-drift check, in addition to the existing test suite.
- Repository hardening: branch protection on `main` (required status
  checks, no force-push/deletion), squash-only merges, Dependabot
  vulnerability alerts + security updates + automated fixes, private
  vulnerability reporting, and repo topics/description for discoverability.

### Changed

- Reorganized repository layout: `test_cancellai.py` moved to
  `tests/test_cancellai.py`; `CONTRIBUTING.md`, `SECURITY.md`, and
  `CODE_OF_CONDUCT.md` moved to `.github/` (a location GitHub recognizes
  natively for these files), decluttering the repo root.
- Modernized type hints to PEP 604 syntax (`X | None` instead of
  `Optional[X]`) and moved `Iterator`/`Sequence` imports to
  `collections.abc`.
- `active_processes()` now resolves `ps` to an absolute path via
  `shutil.which` instead of relying on `$PATH` resolution at call time.
- Replaced an internal `assert` in `delete_codex_via_cli` with an explicit
  `ValueError` guard (assertions can be optimized away with `python -O`;
  this is a real invariant, not a debug check).
- Simplified several `try`/`except ...: pass` blocks to
  `contextlib.suppress(...)`.
- `cancellai.py` is now tracked as executable in git (it has a shebang).

### Fixed

- The `tests` CI job never installed `pytest`, so it failed on every run
  since it was added; every CI job now also invokes tools via
  `python3 -m <tool>` so the installer and the invocation always share the
  same interpreter.
- `.gitignore` now excludes the local `.claude/` session directory so it
  can never end up tracked by accident.

## [1.0.0] - 2026-08-27

Initial public release.

### Added

- Safe cleanup CLI for old Codex CLI and Claude Code session data:
  `status` (read-only, default), `clean` (with dry-run, confirmation
  prompt, age cutoff, and keep-latest safety rail), and `configure` (sets
  Claude Code's own `cleanupPeriodDays`).
- Conservative-by-default safety model: protected name lists for
  auth/config/plugins/skills/memory, symlink-safe deletion, config-root
  validation, running-process detection, and preference for the official
  `codex delete --force` backend over raw filesystem deletion.
- MIT license, README, and a Homebrew formula (`Formula/cancellai.rb`) so
  the tool installs via `brew tap matteo-dritara/cancellai && brew install
  cancellai`.

[Unreleased]: https://github.com/matteo-dritara/homebrew-cancellai/compare/v1.0.2...HEAD
[1.0.2]: https://github.com/matteo-dritara/homebrew-cancellai/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/matteo-dritara/homebrew-cancellai/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/matteo-dritara/homebrew-cancellai/releases/tag/v1.0.0

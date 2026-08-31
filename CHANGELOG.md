# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

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

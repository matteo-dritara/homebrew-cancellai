# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Security

- Protected names (`CLAUDE_PROTECTED_NAMES` / `CODEX_PROTECTED_NAMES`) are now an executable barrier instead of documentation. They are enforced when the plan is built and again inside `safe_remove`, immediately before any deletion, so a future discovery change cannot silently invalidate them (E00-S01).
- `--aggressive` no longer bypasses the age cutoff for Claude legacy directories and rebuildable cache files. It widens which categories are eligible; retention is applied independently (E00-S03).
- Destructive work on a relocated provider root now requires two independent conditions: content-validated provider structure *and* the new `--allow-custom-root` flag. A matching filename is not an identity, so a directory that merely contains `auth.json` or `sessions/` is inspectable with `status` and refused by `clean`. Default roots keep working unchanged, including on a fresh machine. `configure` routes through the same boundary (E00-S02, ADR-0012).
- The protected-name barrier is now applied to the path as written as well as after resolution. Previously a protected entry that was itself a symlink out of the root lost its protection and could be unlinked (E00-S01).
- An unusable process observation is no longer read as "no provider is running". If `ps` is missing, fails, times out or returns nothing parsable, cleanup is refused unless `--allow-running` is given (E00-S09).
- Filesystem observation errors are no longer silently flattened into zero. Every discovery scope carries a completeness channel, including session lineage reads, history counting and root sizing; an unreadable path withholds destructive authority for that provider instead of making its data look absent. `status` lists the unreadable paths and prints partial totals as lower bounds (E00-S05).
- Claude `history.jsonl` trimming now streams bytes instead of loading and re-encoding the file, so retained lines - including CRLF endings and a missing trailing newline - are preserved verbatim. It re-identifies the source immediately before the atomic replace and abandons the rewrite if a provider wrote concurrently. Trimming is skipped entirely while a Claude process is running, even under `--allow-running`, and a failed trim is reported instead of looking like "nothing to do" (E00-S06).

### Changed

- **Breaking:** flags without a subcommand no longer normalize to `clean`. `cancellai --days 14` now runs the read-only `status` view; deletion requires typing `clean`. An unrecognized verb is a usage error (E00-S04).
- **Breaking:** a relocated `$CODEX_HOME` / `$CLAUDE_CONFIG_DIR` now needs `--allow-custom-root` for `clean` and `configure`. Default roots are unaffected.
- **Breaking:** `clean` exits `3` on mutation failure (previously `2`) and `4` when safety blocked or deferred the requested work. Exit `2` is now reserved for invalid usage and refused configuration roots. `--json` output carries `exit_code`, `blocked_tools` and `deferred` (E00-S04).
- `status` reads each provider root in a single pass instead of traversing it for the total and again for the largest entries.
- `status --json` and `clean --json` now report per-root `origin`, `confidence`, provider `markers` and `destructive_allowed`, plus a `scan` object and `withheld_tools`.

### Added

- `status --coverage` classifies every top-level provider entry as `cleanable`, `aggressive-only`, `trimmed`, `protected`, `reported` or `unknown`, with a legend. The vocabulary is deliberately narrow so the report cannot overclaim what cleanup actually reaches: `history.jsonl` is `trimmed`, not `cleanable`, because no rule ever deletes it. Unknown entries are reported so provider layout drift stays visible and are never cleanup candidates. The same classification is exposed in `status --json` (E00-S08).

### Changed

- Added the cancellAI Engineering Operating System (cEOS): product constitution, decision register, target architecture, threat model, safety invariants, evidence-gated development model, Claude/Codex executor-verifier protocol, and machine-readable roadmap/backlog control plane.
- Reframed the long-term product from a macOS Claude/Codex cleanup script to a local-first, cross-platform, provider-agnostic Agent State Control Plane while clearly separating that target from the currently released Python v1 feature set.
- Documented the spec-first Python-to-Rust migration and the P0 trust-floor work that must land before the reference implementation is frozen.
- Added governance/document integrity automation, story-specific executor/verifier briefs, CodeQL scanning, CODEOWNERS, incident response, synthetic-fixture policy, and supply-chain-aware CI foundations.
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

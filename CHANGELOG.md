# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/matteo-dritara/homebrew-cancellai/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/matteo-dritara/homebrew-cancellai/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/matteo-dritara/homebrew-cancellai/releases/tag/v1.0.0

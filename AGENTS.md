# AGENTS.md

Instructions for AI coding agents (Claude Code, Codex, Cursor, or any other
tool that reads this file) working in this repository. Human contributors
follow the same rules — see [CONTRIBUTING.md](CONTRIBUTING.md) for the
human-facing version.

## What this project is

cancellai is a single-file, stdlib-only Python CLI that deletes old Codex
CLI / Claude Code session data to reclaim disk space, distributed as a
Homebrew formula. Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before
touching `cancellai.py` — it explains the discover → plan → execute pipeline
and, more importantly, which three things are the actual security boundary.

## Non-negotiables

These are deliberate design decisions from real back-and-forth, not
oversights. Do not "fix" them without discussing it first:

- **Zero runtime dependencies.** `cancellai.py` must stay standard-library
  only. It ships as a single file installed straight into Homebrew's `bin/`
  — no venv, no `pip install`, no lockfile. Dev-only tooling (`ruff`,
  `mypy`, `pytest`) belongs in `pyproject.toml`'s dev config, never in
  anything the formula or the script itself needs at runtime.
- **`status` is the default command, `clean` is not.** Running `cancellai`
  with zero arguments must stay read-only. This was a deliberate change from
  an earlier version where the bare command deleted things behind a
  confirmation prompt — don't revert it.
- **The `install` subcommand does not exist, on purpose.** An earlier
  version self-copied to `~/.local/bin` and edited the user's shell rc file
  to add an alias. That's redundant with, and conflicts with, Homebrew
  managing installation and PATH. Don't re-add a self-installer.
- **macOS only, for now.** `active_processes()` shells out to `ps`, the
  default shell handling assumes zsh/bash conventions, and none of it has
  been tested on Linux. If you add Linux support, it needs real testing on
  Linux and an update to the README platform section — don't just remove
  the caveat.
- **The protected-name lists are load-bearing.** `CLAUDE_PROTECTED_NAMES`,
  `CODEX_PROTECTED_NAMES`, and the auto-memory exclusion in
  `discover_claude_sessions` exist because deleting them would destroy
  things a user did not ask to lose (auth, settings, plugins, memory).
  Widening what gets deleted (e.g. under `--aggressive`) is fine when asked
  for; silently shrinking this list is not.

## Before you're done with any change

1. **Run the tests**: `python3 -m pytest test_cancellai.py -v`. Add tests
   for new behavior — anything touching `safe_remove`, `validate_config_root`,
   `choose_old_sessions`/`choose_codex_old_sessions`, or a protected-name set
   needs matching test coverage in the same change, not a follow-up.
2. **Lint, format, type-check**:

   ```sh
   ruff check .
   ruff format --check .
   mypy cancellai.py scripts/gen_docs.py
   ```

   These three plus pytest are exactly what CI runs
   (`.github/workflows/tests.yml`) — if they pass locally, CI passes.
3. **Regenerate the CLI docs if you touched `build_parser()`**:
   `python3 scripts/gen_docs.py`, then commit the diff to `docs/CLI.md`.
   `python3 scripts/gen_docs.py --check` is what CI runs to catch drift.
4. **Update `CHANGELOG.md`** — see below.

## Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Every
change that affects behavior, flags, or the public interface gets one line
under `## [Unreleased]` in the change's own commit/PR — not batched later by
someone else. Pure internal refactors with no observable effect don't need
an entry. See [docs/RELEASING.md](docs/RELEASING.md) for how `[Unreleased]`
turns into a tagged version.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org/) prefixes
(`feat:`, `fix:`, `docs:`, `chore:`, `test:`, `refactor:`, `style:`) — this
repo's history is small enough that consistent prefixes make it possible to
audit "what actually changed behavior" from `git log` alone, which matters
more here than in most projects given what this tool is allowed to delete.

## Style

Match what's already there: full type hints, `dataclass` for structured
data, no comments except to explain a non-obvious *why* (a safety
invariant, a subtle ordering requirement, a workaround). Don't add
abstractions, config options, or defensive error handling for scenarios
that can't occur — this file's existing conservatism (age cutoff +
keep-latest + protected names + process detection, layered rather than any
single one being load-bearing alone) is the level of caution to match, not
exceed reflexively.

## Repository settings

`main` is protected: the `test`, `lint`, and `homebrew` checks from
`tests.yml` must pass before anything can be merged into it via a pull
request, force-pushes and branch deletion are blocked, and merges are
squash-only. Direct `git push` to `main` (not through a PR) is still
allowed for the maintainer — that's a deliberate choice, not an oversight,
since this is a single-maintainer repo and required status checks don't
gate direct pushes anyway (only the PR merge button). Dependabot PRs
(`.github/dependabot.yml`, GitHub Actions ecosystem only) are auto-merged
by `.github/workflows/dependabot-auto-merge.yml` once CI is green — no
manual rebase/merge dance needed for routine Action version bumps.

## Releasing

Full runbook: [docs/RELEASING.md](docs/RELEASING.md). Short version: bump
`VERSION` in `cancellai.py` and `pyproject.toml`, move the changelog entry,
tag, compute the tarball sha256, update `Formula/cancellai.rb`, then
actually run `brew install` + `brew test` against it before calling a
release done — a formula that only "looks right" is worse than no formula.

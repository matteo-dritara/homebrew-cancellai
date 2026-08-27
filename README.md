# cancellAI

[![tests](https://github.com/matteo-dritara/homebrew-cancellai/actions/workflows/tests.yml/badge.svg)](https://github.com/matteo-dritara/homebrew-cancellai/actions/workflows/tests.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Safely reclaim disk space from old [Codex CLI](https://github.com/openai/codex) and [Claude Code](https://claude.com/claude-code) session data.

Codex and Claude Code both accumulate transcripts, caches and auxiliary files under `~/.codex` and `~/.claude` that are never cleaned up automatically. cancellAI finds what is safe to delete and removes it, conservatively, on request.

## Platform support

macOS only for this release. The tool has not been tested on Linux.

## Install

```sh
brew tap matteo-dritara/cancellai
brew install cancellai
```

## Safety model

- **Read-only by default.** Running `cancellai` with no arguments shows a `status` report; nothing is ever deleted unless you explicitly run `clean`.
- **Dry-run first.** `cancellai clean --dry-run` shows exactly what would be removed, with no side effects.
- **Explicit confirmation.** A real `clean` run asks `[y/N]` before deleting anything, unless you pass `--yes`.
- **Age + keep-latest.** Only data older than `--days` (default 7) is a candidate, and the `--keep-latest` newest sessions per tool (default 2) are always protected regardless of age.
- **Never touched:** Claude Code's auto-memory, `settings.json`, `keybindings.json`, plugins/skills/agents/commands; Codex's `auth.json`, `config.toml`, skills/rules/memories. These are excluded by name, unconditionally.
- **Official deletion path preferred.** For Codex, `cancellai` uses `codex delete --force` when the installed CLI supports it (so subagent/thread bookkeping stays consistent), falling back to raw file removal only when explicitly requested via `--codex-backend filesystem`.
- **Symlinks are never followed** into or out of the target directories.

Read the source before trusting it with your data — it is a single, readable, stdlib-only Python file.

## Usage

```sh
cancellai                       # status report (default, read-only)
cancellai status --paths        # status report with largest candidate paths
cancellai clean --dry-run       # preview what would be deleted
cancellai clean                 # delete with a confirmation prompt
cancellai clean -y --days 14    # delete data older than 14 days, no prompt
cancellai configure --claude-retention 7   # set Claude Code's own cleanupPeriodDays
```

Run `cancellai --help` or `cancellai <command> --help` for the full option list.

## A note on how this works

Codex CLI and Claude Code do not publish a stable spec for their local storage layout. cancellai targets directories and file-naming patterns observed in current released versions of both tools. If a future version changes that layout, the fail-safe behavior is to find nothing to clean, not to guess — but layouts can and do change, so review `clean --dry-run` output before trusting it on a new tool version.

## Uninstall

```sh
brew uninstall cancellai
brew untap matteo-dritara/cancellai
```

cancellai never modifies your shell configuration or installs itself outside of what Homebrew manages, so uninstalling is a clean removal.

## Documentation

- [docs/CLI.md](docs/CLI.md) — full command reference, generated from the CLI itself
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — how the discover/plan/execute pipeline and the safety model work
- [CHANGELOG.md](CHANGELOG.md) — what changed, per release
- [CONTRIBUTING.md](.github/CONTRIBUTING.md) / [AGENTS.md](AGENTS.md) — how to work on this repo, for humans and AI coding agents alike
- [SECURITY.md](.github/SECURITY.md) — how to report a way this tool deletes something it shouldn't

## License

MIT — see [LICENSE](LICENSE).

# cancellAI

[![tests](https://github.com/matteo-dritara/homebrew-cancellai/actions/workflows/tests.yml/badge.svg)](https://github.com/matteo-dritara/homebrew-cancellai/actions/workflows/tests.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Safely reclaim disk space from old [Codex CLI](https://github.com/openai/codex) and [Claude Code](https://claude.com/claude-code) session data.

Codex and Claude Code accumulate transcripts, caches and auxiliary files under `~/.codex` and `~/.claude`. cancellAI finds what is safe to delete, tells you what it cannot classify, and removes only what you confirm.

> Your AI agents create. cancellAI keeps their footprint under control.

## Platform support

macOS only for this release. The tool has not been tested on Linux. Cross-platform support belongs to the target architecture and is not claimed by the Python v1 release.

## Install

```sh
brew tap matteo-dritara/cancellai
brew install cancellai
```

## Safety model

- **Read-only by default.** `cancellai` with no arguments, and `cancellai status`, only report. Flags without a subcommand also resolve to `status`: destructive intent must be typed as `clean`.
- **Dry-run first.** `cancellai clean --dry-run` shows exactly what would be removed, with no side effects.
- **Explicit confirmation.** A real `clean` asks `[y/N]` before deleting anything, unless you pass `--yes`.
- **Age + keep-latest.** Only data older than `--days` (default 7) is a candidate, and the `--keep-latest` newest sessions per tool (default 2) are always protected regardless of age. `--aggressive` widens which *categories* are eligible; it never bypasses the age cutoff.
- **Never touched:** Claude Code's auto-memory, `settings.json`, `keybindings.json`, plugins/skills/agents/commands; Codex's `auth.json`, `config.toml`, plugins/skills/rules/memories. These names are enforced twice - when the plan is built and again immediately before any deletion - applied to the path as written as well as after resolution, and matched case-insensitively, because macOS mounts APFS case-insensitively and a barrier must not depend on how a name happens to be spelled.
- **Unknown is never deleted.** `cancellai status --coverage` lists every top-level provider entry this build does *not* classify. Those entries are reported, never cleaned.
- **Only the provider's own directory is ever mutated.** `~/.codex` and `~/.claude` are authoritative by definition. If you have relocated a root with `CODEX_HOME` or `CLAUDE_CONFIG_DIR`, cancellAI will inspect it in full but will not delete from it or write to it: nothing the filesystem can show proves a directory belongs to a provider, and this release will not act on a guess. See [ADR-0013](docs/adrs/0013-custom-provider-roots-are-inspection-only-in-python-v1.md).
- **An incomplete scan is not an empty one.** If a path cannot be read, cancellAI says so, reports sizes as lower bounds, and withholds deletion for that tool rather than treating unreadable as absent.
- **Unknown activity is not absence of activity.** If cancellAI cannot tell whether Codex or Claude is running, it refuses rather than assuming they are not.
- **Official deletion path preferred.** For Codex, cancellAI uses `codex delete --force` when the installed CLI supports it, so subagent/thread bookkeeping stays consistent, falling back to raw file removal only when explicitly requested via `--codex-backend filesystem`.
- **Symlinks are never followed** into or out of the target directories.

Read the source before trusting it with your data - it is a single, readable, stdlib-only Python file.

## Usage

```sh
cancellai                       # status report (default, read-only)
cancellai status --paths        # status report with largest candidate paths
cancellai status --coverage     # what this build classifies, and what it does not
cancellai clean --dry-run       # preview what would be deleted
cancellai clean                 # delete with a confirmation prompt
cancellai clean -y --days 14    # delete data older than 14 days, no prompt
cancellai configure --claude-retention 7   # set Claude Code's own cleanupPeriodDays
```

Run `cancellai --help` or `cancellai <command> --help` for the full option list. See [docs/CLI.md](docs/CLI.md) for the generated command reference.

### Exit codes

Automation can distinguish "cleaned" from "deliberately did not clean":

| Code | Meaning |
| --- | --- |
| 0 | requested work completed |
| 1 | you declined the confirmation prompt |
| 2 | invalid usage or a refused configuration root |
| 3 | at least one deletion failed |
| 4 | safety blocked, withheld or deferred the work; nothing may be assumed cleaned |

Exit code 4 is what you get when a Codex or Claude process is running or cannot be observed, when the configured root is not the provider's default directory, or when part of the scan was unreadable. cancellAI skips that tool rather than guessing. Every failure path lands somewhere in this table: a safety boundary that fires mid-run reports exit 4, an I/O failure reports 3, and even an unexpected bug reports 3 rather than leaving Python's exit code 1, which automation cannot tell apart from you declining the prompt.

## A note on how this works

Codex CLI and Claude Code do not publish a stable spec for their local storage layout. cancellAI targets directories and file-naming patterns observed in current released versions of both tools, and reports the rest as unclassified. If a future version changes the layout, the fail-safe behavior is to find nothing to clean, not to guess - so review `clean --dry-run` output before trusting it on a new tool version.

## Development safety notice

The architecture review of 2026-08-27 identified seven P0 trust-floor defects in the Python implementation, scheduled in roadmap epic `E00`. They have been through two remediation rounds and two independent reviews. Round 1 rejected six of seven stories, round 2 rejected all seven, and every defect both reviews found is now repaired. One story is closed; the rest await a third review, and nothing is marked done until a reviewer issues a verdict. See [the baseline code review](docs/audits/2026-08-27-CODE_REVIEW.md), the [round-1](project/evidence/E00-VERIFIER-REVIEW.md) and [round-2](project/evidence/E00-VERIFIER-REVIEW-ROUND2.md) reviews, and [the as-is architecture](docs/architecture/AS_IS.md).

## Uninstall

```sh
brew uninstall cancellai
brew untap matteo-dritara/cancellai
```

cancellAI never modifies your shell configuration or installs itself outside of what Homebrew manages, so uninstalling is a clean removal.

## Where this is going

The released v1 is a conservative macOS CLI. The target product is a local-first, cross-platform control plane for the state that AI coding agents create: cross-provider inventory, explanation, policy, quarantine, storage budgets, and predictive Guardian behavior across macOS, Linux, Windows and WSL.

Claude Code and other agents increasingly ship their own retention and delete commands. cancellAI is not trying to beat those one by one; its long-term value is neutral visibility and governance **across** providers.

```text
SEE + RECLAIM  ->  UNDERSTAND  ->  PREVENT  ->  GOVERN  ->  FULL AGENT-STATE LIFECYCLE
```

The Python v1 is not being expanded into that product. The transition is: P0 safety fixes -> executable Python behavioral contract -> freeze Python as reference -> provider-neutral Rust core -> differential parity -> evidence-gated cutover. See [Spec-First Python -> Rust Migration](docs/development/MIGRATION_PYTHON_RUST.md).

Start with [docs/INDEX.md](docs/INDEX.md). Key views: [Product](docs/PRODUCT.md), [Constitution](docs/CONSTITUTION.md), [Roadmap](docs/ROADMAP.md), [Backlog](docs/BACKLOG.md), [Target Architecture](docs/architecture/TARGET.md), [Safety Invariants](docs/security/SAFETY_INVARIANTS.md), [Threat Model](docs/security/THREAT_MODEL.md), [Engineering Operating System](docs/development/ENGINEERING_SYSTEM.md), [Agent Protocol](docs/development/AGENT_PROTOCOL.md).

```sh
python3 scripts/project_os.py check
python3 scripts/project_os.py status
python3 scripts/project_os.py next
```

This repository started as a Homebrew tap and currently doubles as the source repository. The eventual split into a canonical `cancellai` source repository plus a generated tap is deferred until after the reference-contract phase.

## Contributing and security

Read [AGENTS.md](AGENTS.md) before changing the project, whether you are a human or a coding agent - [CLAUDE.md](CLAUDE.md) points Claude at the same contract. See [CONTRIBUTING.md](.github/CONTRIBUTING.md), [SECURITY.md](.github/SECURITY.md) and the [Code of Conduct](.github/CODE_OF_CONDUCT.md).

Per-release changes are in [CHANGELOG.md](CHANGELOG.md); the release runbook is [docs/RELEASING.md](docs/RELEASING.md).

## License

MIT - see [LICENSE](LICENSE).

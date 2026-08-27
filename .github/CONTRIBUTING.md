# Contributing

Thanks for considering a contribution to cancellai. The short version: this
repo's technical conventions are documented once, in
[AGENTS.md](../AGENTS.md), and apply equally to human contributors and to AI
coding agents — read that first for testing, linting, changelog, and style
rules.

## Setup

No build step, no dependencies to install for the tool itself — it's
stdlib-only. For development:

```sh
python3 -m venv .venv && source .venv/bin/activate
pip install ruff mypy pytest
```

## Before opening a pull request

```sh
python3 -m pytest tests/test_cancellai.py -v
ruff check .
ruff format --check .
mypy cancellai.py scripts/gen_docs.py
python3 scripts/gen_docs.py --check   # only relevant if you touched build_parser()
```

All four are exactly what CI checks. Also add a line under
`## [Unreleased]` in [CHANGELOG.md](../CHANGELOG.md) if your change is
user-visible.

## What kind of changes need extra care

This tool deletes files in the user's home directory. A change to
`safe_remove`, `validate_config_root`, `choose_old_sessions` /
`choose_codex_old_sessions`, or either protected-name set
(`CLAUDE_PROTECTED_NAMES`, `CODEX_PROTECTED_NAMES`) needs new or updated
tests in the same PR — see
[docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md#the-safety-critical-core) for
why those specific functions are the ones that matter most.

## Reporting a bug

Open a GitHub issue with the output of `cancellai status --json` (redact
anything you don't want to share — it includes real paths from your home
directory) and, if the bug is about `clean` deleting the wrong thing, the
output of `cancellai clean --dry-run --verbose` beforehand if you still
have it.

If what you found is a way for `clean` to delete something it explicitly
promises not to (protected files, symlink escapes, running-session data),
please see [SECURITY.md](SECURITY.md) instead of opening a public issue.

## Platform scope

macOS only for now (see [AGENTS.md](../AGENTS.md#non-negotiables) for why).
Linux support is welcome as a contribution, but needs to be actually tested
on Linux, not just assumed to work.

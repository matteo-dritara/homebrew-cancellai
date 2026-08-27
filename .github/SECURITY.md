# Security Policy

cancellai deletes files in your home directory. Its entire value proposition
is "does that safely" — so a way to make it delete something it explicitly
promises not to (see [docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md#the-safety-critical-core)
for what that list actually is) is treated as a security issue, not a
regular bug.

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting for this repository
(the **Security** tab → **Report a vulnerability**) rather than a public
issue, so it can be assessed and fixed before the details are public.

In scope, for example:

- A `clean` run (with or without unusual flags) deleting a file or
  directory in `CLAUDE_PROTECTED_NAMES` / `CODEX_PROTECTED_NAMES`, Claude's
  auto-memory, or anything outside the resolved `$CODEX_HOME` /
  `$CLAUDE_CONFIG_DIR` root.
- A symlink causing deletion of a file outside the approved root
  (see `safe_remove` in `cancellai.py`).
- `validate_config_root` accepting a root that is `/`, the user's home
  directory, or another catastrophically broad path.
- Command injection via `--tool`, `--codex-backend`, or any other flag
  reaching a subprocess call.

Not in scope: the tool doing exactly what `--dry-run`/`--verbose` output
said it would do, or platform issues on operating systems this project
doesn't yet support (see the Platform support section of
[README.md](../README.md)).

## Supported versions

Only the latest tagged release is supported. There is no long-term-support
branch for a single-maintainer CLI tool at this stage.

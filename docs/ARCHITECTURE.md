# Architecture

cancellai is intentionally a single file: `cancellai.py`. There is no package,
no plugin system, and no runtime dependency beyond the Python standard
library. That is a design choice, not a starting point to grow out of — see
[AGENTS.md](../AGENTS.md) for why.

For the generated command reference, see [CLI.md](CLI.md). For how to cut a
release, see [RELEASING.md](RELEASING.md).

## Pipeline

Every command follows the same three-stage pipeline:

```
discover_*()  -->  build_plan()  -->  execute_plan()
(scan disk)       (decide what is  (actually delete,
                   safe to delete,  only when called
                   pure/no I/O      with dry_run=False)
                   writes)
```

- **`discover_codex_sessions` / `discover_claude_sessions` / `discover_*_aux`**
  walk `~/.codex` and `~/.claude` (or `$CODEX_HOME` / `$CLAUDE_CONFIG_DIR`) and
  return `Action` objects: candidate paths with their size, mtime, and
  (when known) session id. Discovery never deletes anything.
- **`build_plan`** takes the discovered actions, applies the age cutoff
  (`--days`) and the keep-latest safety rail (`--keep-latest`), and resolves
  Codex's subagent/thread graph (`choose_codex_old_sessions`) so a whole
  session tree is treated as one unit instead of deleting a parent transcript
  out from under its still-recent subagents. The result is a `Plan`: a pure
  data structure, still nothing has been touched on disk.
- **`execute_plan`** is the only function that deletes or moves anything. It
  re-checks for running Codex/Claude processes right before acting, deletes
  through `safe_remove` (which re-validates the symlink/root-containment
  invariants at the moment of deletion, not at plan time — see
  "Why re-check at execute time" below), and only then trims
  `history.jsonl` lines tied to sessions that were *actually* deleted
  (not merely planned).

## The safety-critical core

Three things are the actual security boundary of this tool. Any change to
them needs matching test coverage before it merges, not after:

1. **`validate_config_root`** — refuses to operate if `$CODEX_HOME` /
   `$CLAUDE_CONFIG_DIR` resolves to `/`, the user's home directory, or
   anything shallower than a few path segments. This is what stands between
   a misconfigured environment variable and `rm -rf`-ing something enormous.
2. **`safe_remove`** — the only function allowed to call `unlink`/`rmtree`.
   It never follows a symlink outside the approved root, and it re-resolves
   the path immediately before deleting (not using a path resolved back in
   the discovery phase), which closes most of the TOCTOU window between
   planning and acting.
3. **`CLAUDE_PROTECTED_NAMES` / `CODEX_PROTECTED_NAMES`** — settings,
   plugins, skills, auth, and Claude's auto-memory are excluded by name,
   unconditionally, regardless of age or flags. `--aggressive` widens what
   counts as a cleanup candidate; it never touches this list.

### Why re-check at execute time, not plan time

A `Plan` can be built and then handed to `execute_plan` later (or never, in
`--dry-run`). Re-validating symlinks and root containment inside
`safe_remove` — right before the actual `unlink`/`rmtree` call — means a
change to the filesystem between planning and execution (someone swaps a
directory for a symlink, say) is caught at the last possible moment instead
of trusted from stale information.

## Data model

- **`Action`** — one candidate deletion: tool (`codex`/`claude`), category
  (`session`, `old-log`, `file-history`, ...), path, size, mtime, and for
  Codex, the session/parent-session id used to resolve the subagent graph.
- **`Plan`** — the immutable output of `build_plan`: the list of `Action`s
  selected for deletion, plus bookkeeping (cutoff timestamp, notes about
  skipped tools, which Claude `history.jsonl` lines are linked to selected
  sessions).
- **`CleanResult`** — the output of `execute_plan`: what actually happened
  (succeeded/failed/skipped counts, bytes freed, error messages, which
  Claude session ids were actually deleted — used to drive the
  `history.jsonl` trim).

## Codex's subagent graph

Codex threads can have subagents whose rollout files reference a
`parent_thread_id`. `choose_codex_old_sessions` walks that graph
(`root_id_for`) so that:

- `--keep-latest` counts root session trees, not every subagent file
  individually — an old root with a very recent subagent is protected as a
  whole.
- When the official `codex delete --force` backend is available
  (`codex-cli` strategy), one delete action is emitted per root, since Codex
  itself cascades the deletion to subagents.
- When falling back to raw filesystem deletion (`--codex-backend
  filesystem`, an explicit opt-in), every rollout file belonging to a
  selected tree is deleted individually, since raw unlinking does not
  cascade.

## Where things live

| Path | What |
| --- | --- |
| `cancellai.py` | The entire implementation. |
| `test_cancellai.py` | The test suite (`unittest`, stdlib only). |
| `scripts/gen_docs.py` | Regenerates `docs/CLI.md` from the real argparse definitions. |
| `Formula/cancellai.rb` | The Homebrew formula (this repo doubles as its own tap). |
| `docs/` | This architecture doc, the release runbook, and the generated CLI reference. |
| `.github/workflows/tests.yml` | CI: tests, lint, format check, type check, docs-drift check. |

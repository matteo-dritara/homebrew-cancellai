# Rust CLI reference (target engine, beta)

This documents `cancellai-cli` (`rust/crates/cancellai-cli`), the target-engine command
surface built starting at E06-S01. It is a separate document from [`CLI.md`](CLI.md), which
stays generated from the frozen Python reference's `argparse` definitions per `AGENTS.md`
("`docs/CLI.md` remains generated from the current Python CLI until the Rust CLI generator
replaces it through an explicit story/ADR") - no such story/ADR exists yet, so this file is
hand-maintained until one does.

This is not yet the canonical CLI (see `docs/development/MIGRATION_PYTHON_RUST.md` - cutover
happens at E06-S04, gated on the full migration Safety Verdict). During the beta/side-by-side
period (E06-S03), `cancellai-cli` and `cancellai.py` coexist; `cancellai-cli version` identifies
which engine and version a given binary is.

## Commands

### `cancellai-cli` (no subcommand)

Equivalent to `status` - the read-only default. No flag on any command implies `clean` (SI-007).

### `status`

Read-only. Prints a per-provider summary, or (`--json`) a
[`docs/architecture/JSON_CONTRACTS.md`](architecture/JSON_CONTRACTS.md) inventory document.

### `inspect`

Read-only. Always prints the full inventory document (equivalent to `status --json`) - a
detail-oriented view with no Python CLI equivalent, added for the Rust target's CLASSIFY-stage
transparency.

### `plan`

Read-only. Resolves the current retention policy against discovered sessions and prints the
proposed actions - a human summary, or (`--json`) a `JSON_CONTRACTS.md` plan document. Never
mutates anything, regardless of any other flag.

### `clean`

The only mutating command. `--dry-run` previews without touching anything; otherwise an
interactive confirmation is required unless `--yes`/`-y` is given. `--json` combined with a
real (non-`--dry-run`) run additionally requires `--yes` - a machine-readable destructive
invocation must state its intent explicitly, mirroring `cancellai.py`'s own automation-safety
gate. Prints (`--json`) a `JSON_CONTRACTS.md` result document.

Every deletion routes through `cancellai-safety`'s single mutation boundary (SI-019); see
[ADR-0016](adrs/0016-rust-artifact-risk-classification.md) for what `clean` can and cannot do
in this build (real, permanent deletion of stale, unprotected session files - no
quarantine/undo yet).

### `configure`

`--claude-retention DAYS` sets Claude Code's own `cleanupPeriodDays` setting (a vendor
configuration value, not a cancellAI-tracked artifact - this does not go through the mutation
boundary, matching `cancellai.py`'s own `configure_claude_retention`). Every read/write is
issued through `cancellai-sealedfs::SealedRoot` (ADR-0017, E07-S07 round-1 repair): the root is
opened exactly once with `O_NOFOLLOW` and retained, so a symlink-swap of the root's path after
that point cannot redirect the write - a re-check of the path alone, which is what this command
did before, cannot make that same guarantee. On a platform with no verified reparse-safe
handle-relative implementation (every non-Unix platform today), `configure` refuses outright
rather than falling back to an unprotected path-based write - see "Known gaps" below.

### `version`

Prints the engine name and version.

## Shared flags

| Flag | Applies to | Meaning |
| --- | --- | --- |
| `--days N` | status, inspect, plan, clean | Retention cutoff in days (default 7) |
| `--keep-latest N` | status, inspect, plan, clean | Always protect the N most-recently-modified sessions per tool (default 2), independent of age |
| `--tool {all,codex,claude}` | status, inspect, plan, clean | Restrict to one provider (default all) |
| `--json` | every command | Machine-readable output |
| `--allow-running` | status, inspect, plan, clean | Proceed even though a Codex/Claude process appears to be running, or process liveness could not be determined |
| `--dry-run` | clean | Preview only; never mutates |
| `--yes` / `-y` | clean | Skip interactive confirmation |

## Exit codes

Matches the taxonomy `docs/architecture/DOMAIN_MODEL.md`'s Diagnostics section defines for the
Rust target (`cancellai-model::ErrorCategory`), not the Python reference's coarser 0-4 mapping
one-for-one:

| Code | Category | Meaning |
| --- | --- | --- |
| 0 | - | Success |
| 1 | - | Declined confirmation (`clean` without `--yes`) |
| 2 | `INVALID_INPUT` | Invalid/ambiguous usage - never resolved toward mutation (SI-007) |
| 3 | `MUTATION_FAILURE` | At least one `clean` deletion failed |
| 4 | `SAFETY_BLOCK` / `INCOMPLETE_INVENTORY` / `COMPATIBILITY_FAILURE` | Requested work was withheld or deferred for safety; nothing may be assumed cleaned |

## Known gaps versus the Python reference (tracked, not silent)

- `--aggressive` (legacy/cache category widening) is not implemented - `cancellai-policy`
  finds a subset of what `cancellai.py --aggressive` would, never a superset (fail-closed, not
  a safety gap).
- `status --paths`/`--coverage`/`--top` and `clean --keep-claude-history`/`--verbose` have no
  Rust equivalent yet.
- Windows process-liveness and identity are not implemented (`cancellai-platform`'s
  `SystemProcessObserver`/`SystemIdentityObserver` report an honest "unsupported"/"incomplete"
  result on non-Unix platforms today, per `docs/architecture/PLATFORM_MODEL.md`'s own
  escape hatch - never a false "not running"/"unchanged").
- A default-named root (`$HOME/.claude`/`$HOME/.codex`, no override) that is itself a link is
  refused as non-default on every platform (E07-S07, `rust/crates/cancellai-cli/src/roots.rs`'s
  `is_symlink`) - proven with real fixtures for a Unix symlink and, since `std` exposes no
  junction-creation API without a new dependency, a Windows directory symlink
  (`std::os::windows::fs::symlink_dir`; cross-compile-clippy-verified for
  `x86_64-pc-windows-gnu`, executes for real on this repo's Windows CI). A genuine NTFS
  junction (the distinct `IO_REPARSE_TAG_MOUNT_POINT` reparse tag, created only via
  `DeviceIoControl`) is not separately fixture-proven; Rust's own cross-platform
  `FileType::is_symlink()` reports `true` for that reparse tag too (it is the same check this
  gate calls), so the same refusal is expected to apply, but this is a disclosed residual, not
  an empirically closed case.
- `configure`'s underlying write capability (`cancellai-sealedfs::SealedRoot`, ADR-0017) has no
  verified no-follow/handle-relative implementation on non-Unix platforms yet - `configure`
  refuses outright there (`SealError::Unsupported`), not only when the root happens to be a
  link, until a future story (the natural home is E20-S01, "Windows native backend", split from
  E07 into a dedicated Windows/WSL epic pending real environment access) implements a genuine
  reparse-safe handle. This is a real, disclosed capability reduction versus the
  previous behavior of attempting the (unprotected) raw path write whenever `$HOME` happened to
  resolve on such a platform, not an oversight.
- On Unix, `SealedRoot::establish` (E07-S09) refuses a root reached through an intermediate
  symlink component (e.g. `$HOME` itself being a link), not only a symlinked leaf - closing the
  gap E07-S07 round-2 independent verifier review found in round-1's leaf-only `O_NOFOLLOW`
  check. `clean`'s own root establishment (`ApprovedRoot`, a different capability than
  `configure`'s `SealedRoot`) needed the identical fix separately - E07-S09 round-1 independent
  verifier review found the round-1 patch only reached `configure` - via
  `cancellai_sealedfs::verify_no_intermediate_links`, called immediately before
  `ApprovedRoot::establish` for the default root. Windows/reparse-point intermediate-component
  handling remains E20-S01 scope for both callers.

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
| `--json` | status, inspect, plan, clean | Machine-readable output (`configure`/`version` have no `--json`, and now refuse it - see "Argument parsing" above) |
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

## Argument parsing

E22-S03 replaced the hand-rolled parser `main.rs` used through E06 with `clap`
([ADR-0019](adrs/0019-dependency-rings-per-crate.md)'s outer-ring dependency), closing
`CR-TE-07`. `cancellai-cli --help`/`-h`/`--version` now work, matching the reference CLI's own
top-level surface, and every subcommand has its own `--help` (`cancellai-cli clean --help`,
etc.) generated from the same argument definitions that parse it - it cannot drift out of sync
with what the command actually accepts the way a hand-written usage string could. A flag
irrelevant to the chosen command (`--dry-run` on `status`, `--claude-retention` on `clean`) is
now refused with exit `2`, not silently accepted-and-ignored as it was before this story - each
subcommand has its own argument struct rather than one shared, permissive flag set.

The one hand-written piece that remains, deliberately, is which token selects which
subcommand at all (`cli::normalize_args`): no argument, or a leading flag with no subcommand,
always normalizes to `status` - the read-only default - and the *only* token that can select
`clean` is the literal string `"clean"` appearing first. This is SI-007's own property
("ambiguous CLI/configuration is non-destructive"), and it is a property of this workspace's
command dispatch regardless of which crate parses the tokens (ADR-0019): `clap` decides what
a valid `status`/`plan`/`clean`/... invocation's flags mean, but never which subcommand an
ambiguous or empty invocation resolves to.

`--help`/`-h`/`--version`, wherever either appears in the argument list, make `clap` print
help/version text and exit `0` immediately - the same precedence `git`, `cargo`, and most
`clap`-based CLIs already follow (`git commit --help --bogus-flag` shows help too), and the
reason `cancellai-cli status --help --dry-run` exits `0` rather than refusing `--dry-run`
(E22-S03 verifier review round 1). This is deliberately *not* treated as a violation of "a
flag irrelevant to the chosen command is refused": that AC describes ordinary argument
validation, and this precedence is safe by construction rather than merely by convention -
`cli::parse` only returns an `Invocation` when `clap` neither printed help/version nor
errored, so no code path from `--help`/`-h`/`--version` can reach `main.rs`'s dispatch, in
particular never `Invocation::Clean` (SI-007). An irrelevant flag that appears *before*
`--help` in the same invocation is still refused with exit `2`, because `clap` parses left to
right and only short-circuits once it actually reaches the help/version action -
`cancellai-cli status --dry-run --help` is a usage error, not help text
(`rust/crates/cancellai-cli/tests/cli_behavior.rs`'s `help_short_circuits_remaining_
argument_validation_by_design` and `an_irrelevant_flag_before_help_is_still_refused` pin both
orderings).

## Known gaps versus the Python reference (tracked, not silent)

- **The Codex native delete backend is detected but deliberately not wired to `clean` -
  a permanent, disclosed divergence, not a missing flag (`CR-TE-10`, E22-S05).** "Permanent"
  means this build does not close the gap and will not close it as a side effect of an
  unrelated story - not that closing it is out of scope forever. Wiring it remains real,
  wanted future work; it stays open only behind its own dedicated, reviewed CR4 story (see
  the mutation-boundary reasoning below), the same distinction
  `project/evidence/E22-S05/EVIDENCE.md`'s "Residual risks" section draws.
  `cancellai-provider-codex` implements `codex_delete_supported`/`NativeDeleteSupport` with
  four distinct outcomes, and `CodexProvider::capability(NativeDeleteCapability)` already
  reports them accurately (`docs/PROVIDERS.md`'s generated matrix) - detection is real and
  correct. Nothing calls it from `clean`, though: this CLI always deletes at the filesystem
  level, while `cancellai.py` prefers `codex delete --force` when the installed CLI supports
  it, so Codex's own subagent/thread bookkeeping stays consistent.

  E22-S05 evaluated wiring it and chose not to, because in the Python reference the native
  path is not a bookkeeping step alongside filesystem deletion - `perform_delete` calls
  `delete_codex_via_cli` **instead of** `safe_remove` when `codex-cli` is the chosen strategy;
  the vendor command *is* the mutation. Reproducing that in the Rust engine while keeping
  SI-019/C-07 ("all filesystem/vendor mutations route through the one safety executor," the
  authority the raw `unlink` already goes through) intact means the kernel's mutation boundary
  itself (`cancellai-safety::mutation_executor`, `cancellai-platform::mutation`) needs a second
  primitive - authorizing and then invoking an external, PATH-resolved binary under the same
  root/process/authority checks the filesystem path uses today, not a call `cancellai-cli`
  can make on the kernel's behalf. `scripts/check_mutation_boundary.py`'s existing guarantee
  (only `cancellai-platform::mutation.rs` deletes anything, only it and the mutation executor
  reference that capability) would need to grow to admit a second production mutation
  mechanism - the exact class of change ADR-0017 (the `libc`/`unsafe` kernel exception) and
  E21-S07 (removing the two unconfirmed `MutationOperation` variants rather than leaving them
  armed) both treat as requiring its own dedicated, reviewed story, not a CR3 side effect of
  fixing a documentation gap. TM-09 ("native vendor delete semantics change - a provider
  command starts deleting broader data or changes cascade behavior") is exactly the risk that
  review would need to close. There is no `--codex-backend` selector for the same reason: it
  would advertise a choice this build cannot yet honor safely.

  Consequence, disclosed rather than silent: a Codex CLI installed alongside cancellAI keeps
  its own internal bookkeeping (if any) unaware of deletions this engine performs. Every
  deletion still succeeds - the artifact is removed either way - and every safety property
  (root/process/authority checks, protected-name barrier, SI-008/SI-009 completeness gating)
  applies identically regardless of backend, because there is only the one backend today.
- **`--aggressive` remains unimplemented** - see the entry below; the two completeness gaps
  that used to be listed here (a scope reported complete despite an unreadable directory, and
  an `error_count` that was never a count) were repaired by `E21-S03` and are now covered by
  the fixture corpus in both root-origin scenarios.
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
  `cancellai_sealedfs::verify_no_intermediate_links`. Its returned directory handle is retained
  through `ApprovedRoot::establish`, and the two native identities must match; this also refuses
  a component swapped in the interval between the no-follow walk and canonicalization (found
  during the owner-authorized combined verifier/executor closure review). Windows/reparse-point
  intermediate-component handling remains E20-S01 scope for both callers.

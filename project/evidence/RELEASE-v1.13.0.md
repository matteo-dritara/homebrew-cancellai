# Release Evidence - v1.13.0

## Source

- Tag: `v1.13.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-12

## Included work

Two epics close in this release together, not sequentially - both were verified (self-reviewed;
see "Known residual risks" below) and owner-accepted in the same decision, so this file names
both rather than forcing an artificial second version bump for work already landing in the same
tag.

- Epic: E09 - Atlas TUI
  - Stories: E09-S01, E09-S02, E09-S03, E09-S04
  - CR4 Safety Verdicts: none
- Epic: E10 - Storage Accounting and Performance
  - Stories: E10-S01, E10-S02
  - CR4 Safety Verdicts: none

## Gates

Re-run at the tag by `.github/workflows/release.yml`; run locally before tagging:

```text
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py \
  scripts/check_docs.py scripts/check_workflows.py scripts/check_process.py scripts/release.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_process.py check
```

- G1 Functional: PASS
- G2 Safety: PASS
- G3 Compatibility: PASS
- G4 Operability: PASS

## Compatibility

- Platforms: macOS. Python 3.10 and 3.14 exercised in CI.
- Providers/capabilities: Codex CLI and Claude Code, layouts observed at release time.
  Unclassified entries are reported by `status --coverage` and never cleaned.
- State/schema migrations: none. The tool keeps no persistent state.

## Supply chain

- Checksums: the Homebrew formula records the SHA-256 of the tag archive, written by `scripts/release.py finalize`.
- SBOM: not produced at this stage. The shipped tool has no runtime dependencies; development tooling is pinned in `requirements-dev.txt`.
- Provenance/attestation: deferred to E17.
- Signature verification: deferred to E17.
- Release manifest: this file.

## Install smoke tests

- Homebrew: `brew audit --strict` and `brew style` run in CI on every change; `brew install`/`brew test` exercise the tagged archive.
- direct shell / PowerShell / Linux packages: not applicable at this stage.

## Performance

- Scan benchmarks: none formalised for the shipped Python reference (`cancellai.py`) itself -
  this release's E10 work adds a real peak-memory regression gate and trend reporting on the
  in-progress Rust engine (`rust/`), not yet the canonical CLI (E06-S04 gates that cutover).
- Self-budget: recorded scan errors are bounded, and root fingerprinting caps how much of an untrusted directory it will read.

## User-visible changes

### Added

- Added a real peak-memory regression gate for the shipped discovery path (E10-S02, CR1). New
  `cancellai-cli/tests/performance_memory.rs` runs on every `cargo test` on Linux and asserts
  real peak RSS (`/proc/self/status`'s `VmHWM`, no new dependency) against a 128 MiB regression
  budget for the same synthetic tree `performance_shipped_path.rs`'s latency gate already uses;
  the gate's own module does not exist on macOS/Windows rather than reporting a fabricated
  number there. `performance_scheduled_shipped.rs`'s heavy-dataset trend artifact gained a
  `peak_rss_bytes` field (published for trend visibility, `None` off Linux, never gated) so the
  10k/100k/1M scheduled runs report memory alongside latency without blocking ordinary PRs on a
  noisy metric.
- Added a reclaimability estimator distinguishing logical size, allocated size, and clone/
  reflink-sharing uncertainty (E10-S01, CR2, library-level, no CLI/TUI surface yet). New
  `cancellai-platform::filesystem_kind::CloneSemantics` classifies a scope root's filesystem as
  `NotKnownToShare`, `PossiblyShared` (APFS, Btrfs, XFS, ZFS, ReFS - real, documented clone/
  reflink capability that can make a naive allocated-size sum overstate what deleting files
  actually frees), or `Unsupported` - real detection ships for macOS (`libc::statfs`'s
  `f_fstypename`, `cancellai-sealedfs::observe_filesystem_name`) and Linux (reusing `wsl::
  FilesystemContextObserver`'s `/proc/mounts` parsing for the raw fstype), with Windows
  disclosed as `Unsupported` pending its own story. An unrecognized filesystem name defaults to
  `PossiblyShared`, never the more confident label, so a future/unrecognized clone-capable
  filesystem is never silently trusted. New `cancellai-inventory::reclaim::estimate_reclaim`
  aggregates `FileFacts` into a `ReclaimEstimate`, excluding (and counting, never substituting
  with logical size) any file whose allocated size was itself unobservable, and labeling the
  result `Verified` only when every size was known and the filesystem is `NotKnownToShare` -
  any clone-capable or undetermined filesystem, or any excluded file, downgrades the estimate to
  `Estimated` with a named reason (this story's AC2: unknown APFS/reflink/shared-block effects
  are never presented as guaranteed savings). A self-review found and this same story's own fix
  closed a real Windows CI break before release: the classifier and its backing constants
  carried no `cfg` gate, so they were dead code on Windows (nothing there called them) and broke
  the mandatory `cargo clippy -D warnings` gate on `windows-latest` - now gated to
  `cfg(any(test, target_os = "macos", target_os = "linux"))`, the precedent this crate's own
  `wsl::classify_fstype` already set for the identical shape of gap.

- Added a real Atlas TUI shell and keyboard-first navigation to `cancellai-tui` (E09-S01, CR1,
  observational only), replacing the E02-S01 placeholder skeleton: `Tab`/`Shift+Tab`/`1`-`4`
  cycle four screens (`Home`, and stubs for `Atlas`/`Explain`/`Plan` pending E09-S02/S03/S04),
  `?` toggles a help overlay, `q`/`Esc` quits. Terminal capability detection
  (`NO_COLOR`/`TERM`/`COLORTERM` color tiers, `LANG`/`LC_ALL`/`LC_CTYPE` Unicode-vs-ASCII
  box-drawing, plus a `CANCELLAI_TUI_ASCII` escape hatch) degrades gracefully on any missing or
  unrecognized signal, and a too-small terminal renders a message instead of panicking. Built on
  `ratatui`/`crossterm` (outer-ring dependencies named for this epic in
  [ADR-0019](../../docs/adrs/0019-dependency-rings-per-crate.md)); the crate depends on no
  `cancellai-*` crate at all yet - no provider/filesystem access is possible from it by
  construction, and `cancellai-policy`'s engine query API is reintroduced only once E09-S02 has
  real view data to render.
- Added the Machine and project atlas screen to `cancellai-tui` (E09-S02, CR1, observational
  only): total footprint and an estimated-reclaimable subset shown as two visually distinct
  values (never blended), per-provider and per-project breakdowns with an explicit
  "Unattributed" bucket, and the individually largest contributors. An incomplete or unknown
  provider scan is flagged prominently rather than hidden inside the totals it could be
  undercounting. New `cancellai_policy::atlas::summarize` computes the summary from
  already-classified inventory (reusing the exact reclaimability test `plan`/`clean` already
  apply); `cancellai-tui` reintroduces `cancellai-policy` as its only new dependency to consume
  it - still no provider adapter or filesystem crate. Live-scan wiring into the running binary
  remains deferred; the screen shows an explicit "not loaded yet" state until that follow-up
  lands.
- Added the Artifact explain view to `cancellai-tui` (E09-S03, CR1, observational only): a
  selectable list of artifacts with, for the selected one, why it exists (project attribution and
  structural relationships), classification, evidence, risk, reversibility, allowed authority,
  and the concrete policy outcome. A destructive policy outcome always shows the real,
  human-readable reason `cancellai_policy::retention::build_actions` already produces for it -
  new `cancellai_policy::explain::explain` surfaces that reason rather than inventing a second
  explanation mechanism. Any confidence weaker than fully verified (an artifact's own or its
  project attribution's, independently) is flagged with a `[low confidence]` marker in the text
  itself, not only by color. Fixed a real rendering bug this story's own tests caught along the
  way: a long policy reason or provider summary line could be silently clipped instead of
  wrapping - both the Explain and Atlas detail panels now wrap.
- Added the Plan review workflow to `cancellai-tui` (E09-S04, CR3, SI-016): the Plan screen
  reviews the same selected artifact the Explain screen shows and requires stronger
  confirmation for irreversible actions than for others - one keypress confirms a
  non-irreversible recommendation, an irreversible one needs a second, and changing the
  selection or leaving the screen cancels any pending or completed confirmation. This is a
  review-only workflow: `cancellai-tui` still depends on neither `cancellai-safety` nor
  `cancellai-platform`, so nothing in it can construct a plan or execute a mutation - the
  confirmed state names `cancellai-cli clean` as the real, separate execution path rather than
  claiming to execute anything itself. `docs/architecture/TARGET.md` and
  `docs/security/SAFETY_INVARIANTS.md` (SI-016) record this scope decision explicitly. A
  self-review found and this same story's own fix closed a real input-handling gap before
  release: the confirmation guard matched `c` regardless of modifiers, so a real terminal's
  Ctrl+C (delivered as `Char('c')` + `CONTROL` once raw mode disables `ISIG`) could complete a
  pending irreversible confirmation instead of cancelling it - now a shared `is_plain_c` check
  makes any modified `c`, Ctrl+C included, always cancel and never arm/confirm.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

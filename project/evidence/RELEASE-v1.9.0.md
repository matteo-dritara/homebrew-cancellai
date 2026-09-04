# Release Evidence - v1.9.0

## Source

- Tag: `v1.9.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-04

## Included work

- Epic: E22 - Engineering System Hardening
- Stories: E22-S01, E22-S02, E22-S03, E22-S04, E22-S05, E22-S06
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

- Scan benchmarks: none formalised; deferred to E10.
- Self-budget: recorded scan errors are bounded, and root fingerprinting caps how much of an untrusted directory it will read.

## User-visible changes

### Added

- `cancellai-cli` (the beta target-engine CLI) now has a real `--help`/`-h`/`--version`
  surface and per-command help (`cancellai-cli clean --help`, etc.), matching the reference
  CLI's own top-level surface (E22-S03, `CR-TE-07`). Argument parsing moved from a hand-rolled
  loop in `main.rs` to `clap` ([ADR-0019](../../docs/adrs/0019-dependency-rings-per-crate.md)).

### Changed

- `cancellai-cli` now refuses a flag irrelevant to the chosen command (e.g. `status --dry-run`,
  `clean --claude-retention`) with exit code 2, instead of silently accepting and ignoring it
  as every command's flags did before this release. `--help`/`-h`/`--version` are an explicit
  exception: wherever they appear, they still short-circuit remaining validation and exit
  before any command runs, matching `clap`'s own precedence and common CLI convention (`git`,
  `cargo`) - see `docs/CLI_RUST.md`'s "Argument parsing" section.

### Fixed

- **A Codex subagent tree with a stale root and a recently-touched child is no longer an
  individual delete candidate for the stale member** (E22-S04). `cancellai-policy::retention`
  gated `--keep-latest` pinning on the tree's effective (max-of-members) mtime but evaluated
  each member's own staleness independently, so a tree the reference protects in full - any
  recent member protects the whole tree, not just the pinning rail - could still surface the
  old-looking member as a `Delete` action in the target engine. `resolve_codex` now applies the
  same tree-level cutoff gate `cancellai.py::choose_codex_old_sessions` does before classifying
  any member's staleness.

### Documentation

- Recorded that `cancellai-cli clean` deletes Codex sessions at the filesystem level only,
  even when the installed `codex` CLI advertises its own `--force`-capable delete: this is now
  a stated, permanent divergence from `cancellai.py` (which prefers the vendor command) rather
  than an unstated gap (E22-S05, `CR-TE-10`). See `docs/CLI_RUST.md`'s "Known gaps" for why -
  wiring it would add a second mutation primitive to the safety kernel and is deferred to a
  dedicated future story, not a side effect of this one.

## Known residual risks

- **This epic closed after one independent verifier round, not two.** Codex's round 1
  (`project/evidence/E22-VERIFIER-REVIEW.md`) returned FAIL for five of the six stories
  (E22-S01, E22-S02, E22-S03, E22-S04, E22-S05); every finding was repaired
  (`project/evidence/E22-S0{1,2,3,4,5}/ROUND2-REPAIR.md`) with local test evidence plus, for
  E22-S01/S02, real GitHub Actions/CodeQL/Dependabot evidence gathered after pushing the repair
  commit. The epic was then closed by owner decision without spending the second independent
  review round ADR-0014 permits, the same pattern `project/evidence/RELEASE-v1.8.0.md`/
  CHANGELOG's E21 note already used - these repairs carry no independent re-confirmation beyond
  the executor's own re-run of every local and real-service gate the round-1 review specified.
- E22-S05's decision not to wire the Codex native-delete backend remains open, disclosed,
  future work (`docs/CLI_RUST.md`'s "Known gaps" section) - a dedicated CR4 story, not a defect
  in this release.
- E22-S01's own verification bullets also call for a real tag dry-run of `release.yml`
  (including a controlled Windows-clippy-failure replay). This release's own tag push is that
  dry run for the non-failure path; a deliberate failure replay was not performed live against
  this tag - `scripts/check_workflows.py`'s round-2 static checks (continue-on-error/`if:`
  guards, matrix parity, `publish`'s `needs:`) are what now make that class of regression
  detectable pre-tag, in addition to the workflow's own real per-platform execution.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

# Release Evidence - v1.16.0

## Source

- Tag: `v1.16.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-22
- Published: pending

## Included work

- Epic: E14 - Predictive Guardian Intelligence
- Stories: E14-S01, E14-S02, E14-S03, E14-S04, E14-S05
- CR4 Safety Verdicts: `project/evidence/E14-S04/SAFETY_VERDICT.md`, `project/evidence/E14-S05/SAFETY_VERDICT.md`

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

### Security

- The Rust engine's mutation boundary (`cancellai_safety::mutation_executor::execute`) now
  requires a live, freshly-observed provider-root layout immediately before every destructive
  mutation, refusing if it has drifted (or become unobservable) since the plan was sealed
  (SI-004, SI-013, ADR-0037). This closes a disclosed residual from the E14-S04/ADR-0036
  provider-layout work: a plan's own layout data was never re-checked at the one moment that
  matters. Independent review round 2 found and repaired three defects in the first version
  before it shipped: `execute`/`execute_all` were reachable from outside the crate with a
  fabricated observation (now `pub(crate)`, production-only via `execute_with_system_
  capabilities`); the fresh observation was not held through the mutation call itself (the
  check now runs immediately before it, narrowing but not fully closing that window - a
  disclosed residual); and `Restore` plans checked the quarantine store instead of the real
  destination provider root (now bound explicitly per action class). **Disclosed consequence:**
  the underlying observation capability is currently Unix-only, so every destructive mutation -
  Delete included - is now refused on non-Unix platforms, Windows included, until a verified
  Windows implementation lands.

### Fixed

- The Rust engine's `clean`/`plan`/`inspect` treated a Claude `~/.claude/projects` that exists
  but is not a directory (a regular file, a device node, etc.) as a structurally empty,
  known-clean scope - the same branch used for an absent or symlinked root - and reported a
  clean empty scan (`clean --yes` exited `0`) instead of withholding. The frozen Python
  reference records the resulting `ENOTDIR` and exits `4`. Found by independent review
  (`project/evidence/E21-S03-S07-INDEPENDENT-REVIEW-ROUND2.md`); repaired so this case is
  reported as unobservable evidence and withholds destructive work like every other unreadable
  scope root.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

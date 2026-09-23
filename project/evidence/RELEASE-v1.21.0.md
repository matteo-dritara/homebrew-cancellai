# Release Evidence - v1.21.0

## Source

- Tag: `v1.21.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-23
- Published: pending

## Included work

- Epic: E17 - Verifiable Supply Chain and Distribution
- Stories: E17-S01, E17-S02, E17-S03, E17-S04, E17-S05, E17-S06, E17-S07, E17-S08, E17-S09, E17-S10, E17-S11
- CR4 Safety Verdicts: `project/evidence/E17-S05/SAFETY_VERDICT.md`, `project/evidence/E17-S07/SAFETY_VERDICT.md`, `project/evidence/E17-S08/SAFETY_VERDICT.md`

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

- The safety kernel can contain a safety incident by signed capability downgrade (E17-S07): a
  verified containment notice caps a provider, version, action class or platform at Observe or
  Recommend and nothing more, and replay, rollback, expiry or an offline node cannot lift it -
  only a local lift can. It is a kernel capability only: no CLI command reads notices yet, which
  is cutover work (E06-S04).

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

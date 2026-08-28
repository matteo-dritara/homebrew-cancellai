# Release Evidence - v1.2.0

## Source

- Tag: `v1.2.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-08-28

## Included work

- Epic: E01 - Executable Reference Contract
- Stories: E01-S01, E01-S02, E01-S03, E01-S04, E01-S05, E01-S06
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

### Changed

- Epic E01 turned the Python v1 CLI into a characterized, versioned executable reference ahead of the Rust migration: canonical domain vocabulary (`docs/architecture/DOMAIN_MODEL.md`), a synthetic Claude/Codex provider-layout fixture corpus (`tests/fixtures/`), versioned inventory/plan/explanation/result JSON contracts with an explicit compatibility policy (`docs/architecture/JSON_CONTRACTS.md`), a committed characterization of Python's actual behavior on that corpus classified normative/intentional-divergence/legacy-only/known-defect (`scripts/characterize.py`), and a differential comparison contract and self-testing harness for the eventual Python-vs-Rust migration gate (`scripts/diff_harness.py`, `docs/development/VERIFICATION_STRATEGY.md`). `cancellai.py`'s own runtime behavior is unchanged.
- `cancellai.py` is now maintenance-only (the Python reference freeze, `AGENTS.md`): only parity fixes against the committed characterization, safety/security fixes, and migration-support tooling are accepted going forward, not merely until this epic closed. New product capability targets the Rust implementation.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

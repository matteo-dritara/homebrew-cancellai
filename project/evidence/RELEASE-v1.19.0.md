# Release Evidence - v1.19.0

## Source

- Tag: `v1.19.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-23
- Published: no - verify-rust (windows-latest) failed: an_unsupported_version_is_refused_with_the_supported_list lost the server's refusal to a connection reset (run 35850978612); repaired by graceful connection close in the desktop API server

## Included work

- Epic: E18 - Remote Targets and Fleet Boundary
- Stories: E18-S01, E18-S02, E18-S03
- CR4 Safety Verdicts: `project/evidence/E18-S02/SAFETY_VERDICT.md`, `project/evidence/E18-S03/SAFETY_VERDICT.md`

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

- `cancellai-cli desktop-api` serves the engine's read-only documents - the inventory and plan
  documents `status --json` and `plan --json` print - to a local desktop client over a
  versioned, token-authenticated loopback channel (E19-S01). The protocol has no request that
  can clean, configure or execute anything.
- `cancellai-desktop`, an optional read-only dashboard (E19-S02): it starts the engine's desktop
  API and opens a local page in the system browser showing the same per-provider summary and
  plan preview the CLI prints, with the same warnings. It has no control that cleans or
  executes, and its access token is never printed or passed on a command line: it is written
  only into a fresh directory made private to the current user before anything is created in it
  - a launcher page for the browser, or with `--no-open` a URL file whose path is printed. No
  tray or menu-bar icon (ADR-0038). Not included in release archives yet.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

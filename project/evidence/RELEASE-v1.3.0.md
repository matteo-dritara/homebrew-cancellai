# Release Evidence - v1.3.0

## Source

- Tag: `v1.3.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-08-29

## Included work

- Epic: E02 - Rust Workspace Bootstrap
- Stories: E02-S01, E02-S02, E02-S03, E02-S04
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

- Epic E02 bootstrapped the target Rust workspace ahead of the spec-first migration: twelve crates (`docs/architecture/TARGET.md`) with an acyclic dependency graph and no provider-specific code in `cancellai-model`/`cancellai-safety`; a quality baseline enforcing `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, and `cargo deny` (license allow-list, unknown-registry/git denial, MSRV 1.85.0) across macOS/Linux/Windows CI (`rust/deny.toml`, ADR-0015); a typed diagnostic model separating invalid-input/safety-block/incomplete-inventory/compatibility/mutation-failure/internal-fault with stable human/JSON error codes; and deterministic `Clock`/`FsObserver` seams (`rust/crates/cancellai-platform`) that keep the Python reference's absent-vs-unreadable filesystem distinction (SI-008/SI-009/SI-010) as a typed contract, including for a modification time the OS cannot report or represent - never silently substituted with a credible-looking epoch timestamp. `cancellai.py`'s own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface yet.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

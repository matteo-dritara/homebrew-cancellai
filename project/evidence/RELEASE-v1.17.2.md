# Release Evidence - v1.17.2

## Source

- Tag: `v1.17.2`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-22
- Published: pending

## Included work

This release closes no epic. It exists because an already tagged version could not
carry the fix below, and a published tag is immutable history:

v1.17.1's release workflow failed cargo test --workspace on windows-latest and ubuntu-latest (three real, distinct platform defects: KillSwitch fail-safe on a non-directory path component, schtasks XML UTF-16 decoding, and a systemd unit-cache propagation gap), while clippy itself passed on all three platforms confirming the v1.17.0 fix worked; all three repaired, the tag stands as immutable history

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

### Fixed

- The Rust engine's `cancellai-guardian` crate failed real cross-platform CI on the v1.17.1 tag
  in three independent ways, none reproducible locally: `KillSwitch::is_engaged` (Windows)
  trusted a bare `NotFound` read error as a confirmed-absent marker, but a path through a
  non-directory component also reports `NotFound` there, misreading a failed `engage()` as
  disengaged - it now also requires the marker's own parent to be confirmed a real directory
  before trusting `NotFound` at all. `schtasks /Query ... /XML` output (Windows) does not decode
  as UTF-8 - its own XML declares `encoding="UTF-16"` and `schtasks.exe` writes genuine UTF-16LE
  bytes with a byte-order mark, which `String::from_utf8_lossy` silently mangled into unmatched
  replacement characters - the shared command-output decoder now detects a UTF-16 BOM and
  decodes accordingly. `systemctl --user enable --now` (Linux) can fail with "Unit file ... does
  not exist" immediately after `install()` writes it, even with a reachable session bus and an
  explicit `daemon-reload` - `enable()` now retries up to three times, reloading fresh before
  each attempt, before reporting the failure honestly. v1.17.1's tag stands as immutable history
  and is recorded as unpublished; this fix ships as the next version.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

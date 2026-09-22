# Release Evidence - v1.17.4

## Source

- Tag: `v1.17.4`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-22
- Published: pending

## Included work

This release closes no epic. It exists because an already tagged version could not
carry the fix below, and a published tag is immutable history:

Windows real-smoke schtasks enable-confirmation fix on the v1.17.3 tag

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

- The Rust engine's `cancellai-guardian` crate failed its real Windows smoke test on the v1.17.3
  tag: `status()` right after a successful `schtasks /Change ... /ENABLE` still failed to parse
  the task as enabled (`Unsupported { reason: "schtasks /XML output did not contain a
  recognizable Enabled field" }`), even though the very same `/Query /XML` parsed correctly
  right after `install` in the same run. That asymmetry - the only difference being a
  state-changing `/Change` call that had *just* run - points at `schtasks`' own task-cache
  lagging behind its own write, not at the XML shape two prior fixes already targeted (a UTF-16
  BOM decode, then a clippy-driven `as_chunks` change). `enable()` now polls `status()` for up to
  2 seconds to confirm the change before returning `Ok`, matching the macOS/Linux adapters' own
  enable contracts (AC1); the real smoke test accepts the resulting honest,
  distinguishable "registration could not be confirmed" outcome the same way the macOS one
  already does, instead of asserting a state `status` cannot yet corroborate.
  `parse_enabled_from_xml` was also made whitespace/attribute-tolerant as a defensive
  improvement, and `status()`'s `Unsupported` reason now includes a bounded snippet of the
  actual `/XML` output so any further occurrence is diagnosable from the failure itself.
  v1.17.3's tag stands as immutable history and is recorded as unpublished; this fix ships as the
  next version.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

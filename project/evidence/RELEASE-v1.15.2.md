# Release Evidence - v1.15.2

## Source

- Tag: `v1.15.2`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-21
- Published: pending

## Included work

This release closes no epic. It exists because an already tagged version could not
carry the fix below, and a published tag is immutable history:

v1.15.1's tagged release workflow failed too: test_the_formula_never_lags_by_more_than_the_in_flight_window hardcoded the in-flight formula target as the two most recent changelog entries, which does not hold once an unpublished release sits between two valid ones.

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

- v1.15.1's tagged release workflow failed too, once the Windows tempdir-cleanup fix cleared
  `verify-rust`: `tests/test_release.py::ReleaseConsistencyTests::
  test_the_formula_never_lags_by_more_than_the_in_flight_window` hardcoded the legitimate
  in-flight formula target as `{cut[0], cut[1]}` - the two most recent changelog entries - which
  does not hold once a failed, unpublished release (v1.15.0) sits between two valid ones.
  `release.py`'s own `formula_should_point_at` already correctly skips past an unpublished
  version; the test now delegates to it directly instead of re-deriving a narrower version of the
  same rule by hand.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

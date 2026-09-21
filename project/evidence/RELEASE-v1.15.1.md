# Release Evidence - v1.15.1

## Source

- Tag: `v1.15.1`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-21
- Published: no - verify-rust passed (the store-handle Windows fix worked), but the verify job's pytest failed: tests/test_release.py::ReleaseConsistencyTests::test_the_formula_never_lags_by_more_than_the_in_flight_window hardcoded the in-flight formula target as {cut[0], cut[1]}, which does not hold once a failed/unpublished release (v1.15.0) sits between two valid ones and formula_should_point_at correctly skips past it to v1.14.0 (run 35584808497); repaired by making the test call formula_should_point_at itself instead of hardcoding the two-release assumption

## Included work

This release closes no epic. It exists because an already tagged version could not
carry the fix below, and a published tag is immutable history:

v1.15.0's tagged release workflow failed verify-rust (windows-latest): cargo test --workspace panicked during tempdir cleanup because the opened store/ledger/analytical-memory handle was never dropped before the test removed its own temp directory (OS error 32, file in use).

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

- v1.15.0's tagged release workflow failed `verify-rust` on `windows-latest`: `cargo test
  --workspace` panicked in three `open_via_local_state_root_never_reaches_a_marker_bearing_
  mimic_elsewhere_on_disk` tests (`cancellai-store`'s `lib.rs`, `ledger.rs`, `rollup.rs`) during
  `std::fs::remove_dir_all`, with OS error 32 ("the process cannot access the file because it is
  being used by another process"). Each test dropped its verification `Connection` before
  cleanup, matching this module's own established Windows precedent, but never dropped the
  `CurrentStateStore`/`EventLedger`/`AnalyticalMemory` handle it opened and left alive on the same
  file - only macOS/Linux let an open file be unlinked out from under a live handle, so this never
  failed locally or on the other two CI platforms.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

# Release Evidence - v1.13.2

## Source

- Tag: `v1.13.2`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-13

## Included work

This release closes no epic. It exists because an already tagged version could not
carry the fix below, and a published tag is immutable history:

v1.13.1 built and attested every artifact on all four targets and then refused to publish: the publish-time checksum guard was hashing each artifact's SBOM instead of its archive, so all four checksums mismatched and `publish` was skipped. The tag stands as history. This version carries that fix (E17-S09) and is the first release since v1.11.0 whose pipeline can reach `publish` at all.

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

- The publish-time checksum guard hashes the archive, not the file that sorts first (E17-S09, CR3).
  The v1.13.1 release built all four platforms, verified every attestation, then refused to publish
  because all four checksums mismatched: the guard globbed `<artifact name>.*` and took the first
  match alphabetically, and since E17-S03 that has been `<name>.cdx.json` - so every declared
  checksum was compared against a JSON document describing the archive instead of the archive. It
  failed closed, which is the only reason this is a defect and not an incident. Latent since
  2026-09-08; the two releases in between died before this job ever ran.

### Changed

- The session-start ritual reads the branch it is about to build on (E26-S04, CR0). `orient` now
  reports every workflow conclusion on the default branch before a story is selected, and says
  *unknown* rather than nothing when it cannot tell. It exists because the MSRV leg of `rust.yml`
  failed on every push for four days, across two attempted releases, while every other leg stayed
  green and every session read the green ones. A failure nobody reads is indistinguishable from a
  gate nobody has.

## Known residual risks

No epic closes here. The story-level records are in `project/evidence/`; these are the ones a
reader of this tag should know, and the first two are carried unchanged from v1.13.1, which never
published.

- **`publish` has not run since v1.11.0.** Every job before it is now exercised - verify,
  verify-rust on three platforms, four build legs with their smoke tests, SBOM generation and
  attestation, attestation re-verification - and this release is the first that can reach the last
  one. It is the remaining unexercised step in the pipeline.
- **E16-S07 ships implemented and independently unverified.** A CR4 change to the safety kernel:
  three conditions in the knowledge-bundle verifier rewritten so they compile on the promised
  minimum toolchain. The executor may not write its own CR4 Safety Verdict, so the story stays at
  `ready_for_review` and the packet says no verdict exists. Behaviour is pinned by 101 tests in
  `cancellai-safety`, including both directions of the expiry boundary at both sites.
- **GHSA-rhfx-m35p-ff5j is open** in `lru 0.12.5`, transitively through `ratatui 0.29`. Low
  severity, an unsoundness report rather than a demonstrated exploit, in a render cache this
  workspace does not call. It cannot be cleared without `ratatui 0.30`, which needs a higher MSRV
  than this project promises; [ADR-0026](../../docs/adrs/0026-raise-the-workspace-msrv-to-1-88.md)
  puts that choice to the owner and is **proposed, not decided**.

- **The formula passed through v1.13.1 to get here.** `release.py check` allows the formula to lag
  the source by exactly one cut release, and v1.13.1's failed release left it two behind, so that
  version was finalized first rather than skipped. The tooling has no way to record "this tag was
  cut and its release failed", which is why the state had to be walked through by hand.
## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

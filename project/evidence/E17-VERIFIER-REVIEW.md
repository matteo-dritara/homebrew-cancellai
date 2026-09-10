# E17 Independent Verifier Review - Round 1

- Review target: `29cfea6fb18247c9c3cdc736a63efa121edc9aea..4d19ac62b864a12f6599b9875053bd6ccf7c6796` on `main`
- Verifier: Codex
- Date: 2026-09-11

## Verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E17-S01 | PASS | Generated an independent one-artifact release manifest from a temporary file and passed `verify-checksums`; invalid/missing artifact inputs refuse generation. The schema and script reject unknown fields and duplicate canonical names. |
| E17-S02 | PASS_WITH_RESIDUALS | The release graph has tier-1 macOS/Linux/Windows build legs, archive smoke tests, checksum sidecars, and publish depends on `verify`, `verify-rust`, `build-artifacts`, and manifest generation. All action pins are full 40-character commits matching their public tag references. A real GitHub Actions execution remains unavailable locally. |
| E17-S03 | PASS_WITH_RESIDUALS | `attestation-verify` independently checks every downloaded archive’s provenance and CycloneDX predicate, and `publish` structurally requires it. The pinned `actions/attest` source maps CycloneDX SBOM input to `https://cyclonedx.org/bom`. A real GitHub Actions execution remains unavailable locally. |
| E17-S04 | PASS | Real CLI integration coverage exercised `version --source`, `update --check`, bare `update`, and invalid update/version input. `update --check` is read-only and returns the detected source’s own guidance. |
| E17-S05 | PASS_WITH_RESIDUALS | The opaque channel type’s external compile-fail doctest ran, nightly/beta/stable authority constraints passed, and a whole-workspace caller scan found no production caller. See [Safety Verdict](E17-S05/SAFETY_VERDICT.md). |
| E17-S06 | PASS | In separate scratch copies, changing the Formula homepage, `release.py`’s `REPO`, and the release runbook’s current-remote value each made `check_repository_topology.py check` fail. No repository was created or moved. |

## Additional checks

- ADR-0021’s 2026-09-08 correction explicitly withdraws its earlier no-network premise; the
  build-matrix decision rests on inspection/audit of the hand-written workflow, not that premise.
  No reviewed evidence claim depends on the withdrawn assertion.
- The E16-S06 provider-trust check was added after this epic’s original changes. It is ordered
  after provider compatibility and before platform/parity/process gates in `verify`; the release
  job graph and publish dependencies remain intact.

## Gate status

- PASS: the full Python gate list in `AGENTS.md`, including 268 tests, Ruff, formatting, MyPy,
  project/docs/workflow/process/release/release-manifest/topology checks, and all migration
  compatibility checks.
- PASS: the full Rust gate list in `AGENTS.md`: format, warnings-as-errors Clippy, workspace
  check, workspace test, and cargo-deny. cargo-deny emitted only existing configuration/duplicate
  warnings and ended `advisories ok, bans ok, licenses ok, sources ok`.
- PASS: independent topology scratch mutations, release-manifest checksum round trip, action tag
  reference checks, pinned action-source predicate inspection, channel caller scan, and both
  CR4 compile-fail doctests.

## Overall verdict

`PASS_WITH_RESIDUALS`

E17-S01 through E17-S06 reached `done`. E17 remains open because E17-S07 is blocked and was not
implemented or reviewed in this round. E16-S05 is now done, so E17-S07 has satisfied that story
dependency; changing its status and implementing it belong to a future work/review cycle and are
deliberately outside this record.

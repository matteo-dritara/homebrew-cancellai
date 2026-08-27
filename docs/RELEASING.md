# Releasing

This document has two release modes because cancellAI is transitioning from the current Python/Homebrew v1 to a future cross-platform Rust release factory.

## Current Python v1 release process

Until the Rust cutover, the release artifact remains the tagged source used by the Homebrew formula.

1. All required story/release gates are green.
2. `python3 scripts/project_os.py check` passes.
3. Python tests/lint/type/docs/Homebrew checks pass.
4. Update `VERSION` in `cancellai.py` and `version` in `pyproject.toml` together.
5. Move changelog entries from Unreleased to `X.Y.Z` with date and create a new Unreleased section.
6. Commit the release metadata.
7. Tag `vX.Y.Z` and push.
8. Download the GitHub tag tarball and calculate SHA-256.
9. Update `Formula/cancellai.rb` URL/SHA in a separate commit.
10. Run Homebrew audit/style/install/test end to end.
11. If the release contains CR4 work, ensure its owner-visible Safety Verdict is part of durable release evidence before declaring the release complete.

Do not add future product features to Python merely because the Python release path is simpler.

## Target Rust release factory

Epic E17 replaces the manual build/package steps with canonical cross-platform release automation. The target release includes:

- macOS/Linux/Windows canonical binaries;
- checksums and release manifest;
- SBOM;
- build provenance/attestation and chosen signature material;
- channel identity (stable/beta/nightly);
- compatibility/knowledge version;
- installer/package outputs from the same canonical build lineage;
- automated install smoke tests;
- G1 Functional, G2 Safety, G3 Compatibility, G4 Operability evidence.

The exact release tool (for example `dist`/cargo-dist) is selected by ADR during E17 rather than being a permanent decision in this document.

## Versioning

Semantic Versioning remains the public version scheme.

Safety/authority behavior is part of the public contract. A change that materially expands destructive authority or breaks machine-facing schema compatibility may require a MINOR/MAJOR change even if the command spelling remains unchanged.

Provider knowledge bundles have their own version/content identity and are not forced to share the binary SemVer.

## Release channels

- stable: highest verified default authority;
- beta: migration/compatibility validation with reduced autonomous defaults;
- nightly: experimental, Observe/Recommend oriented by default.

See `docs/security/SUPPLY_CHAIN.md` and SI-030.

## Repository topology transition

The current remote is `matteo-dritara/homebrew-cancellai`. It remains canonical while Python v1/Homebrew is the shipping product. Do not rename/split the repository during P0/P1 merely for aesthetics.

When the Rust core and cross-platform release factory are proven, E17-S06 evaluates a controlled split:

```text
matteo-dritara/cancellai          canonical product source/releases
matteo-dritara/homebrew-cancellai  Homebrew tap/distribution compatibility
```

The migration must preserve existing Homebrew upgrade paths and make release provenance point unambiguously at the canonical source repository. Repository movement is a distribution migration with compatibility evidence, not a housekeeping rename.

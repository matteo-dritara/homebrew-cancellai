# Releasing

This document has two release modes because cancellAI is transitioning from the current Python/Homebrew v1 to a future cross-platform Rust release factory.

## When a release happens

**Closing an epic cuts a release.** An epic reaching `done` produces a version tag and
everything that follows it; this is not an optional follow-up (ADR-0014, PD-021).
`scripts/release.py check` fails when a closed epic has no release evidence naming it, and
that check runs in `pre-commit` and in the governance workflow.

A closed epic is at least a **minor** release. Safety and authority behaviour is part of the
public contract, so an epic that changes what the tool is willing to do is never a patch
release even when the command spelling is unchanged.

## Current Python v1 release process

Until the Rust cutover the release artifact is the tagged source the Homebrew formula points
at. The sequence is automated because it is four files that must agree plus a checksum that
cannot exist before the tag does - exactly the shape of task that gets done wrong by hand.

```sh
# 1. everything green, epic closed
pre-commit run --all-files && python3 -m pytest tests -v

# 2. bump versions, cut the changelog, write the release evidence packet
python3 scripts/release.py prepare --version X.Y.Z --epic EXX

# 3. review the diff, then commit and tag
git commit -am "chore(release): X.Y.Z"
git tag -a vX.Y.Z -m "cancellAI X.Y.Z" && git push --follow-tags

# 4. write the archive checksum into the formula
python3 scripts/release.py finalize --version X.Y.Z
git commit -am "chore(release): point formula at the vX.Y.Z tarball" && git push
```

Step 2 writes `project/evidence/RELEASE-vX.Y.Z.md` from the epic's contract: stories, CR4
Safety Verdict links, gate results, compatibility, residual risks and rollback.

Pushing the tag triggers `.github/workflows/release.yml`, which re-runs every gate **at the
tagged commit** - the full Python checker set AGENTS.md lists (`verify` job) and the Rust
quality set (`fmt --check`, `clippy -D warnings`, `cargo test`, `cargo deny check`, on all
three tier-1 platforms, `verify-rust` job) - verifies that the tag matches `VERSION` and that
the evidence packet exists, then publishes the GitHub release from that packet with the
archive checksum appended. A release verified against whatever `main` looked like afterwards
is not evidence about the artifact users install.

`scripts/check_workflows.py` keeps this from drifting: it derives the required Python gate set
from `.pre-commit-config.yaml`'s local hooks and the required Rust gate set from `rust.yml`'s
`quality` job, and fails if `release.yml` stops re-running one of them (E22-S01, closing
`CR-TE-06` - at v1.8.0 `release.yml` ran no Rust check at all and reported success while `rust
/ quality (windows-latest)` failed on the same tagged commit; see
`project/evidence/RELEASE-v1.8.0.md`).

`finalize` refuses to leave the repository inconsistent: it re-runs `release.py check` and
fails if the source, the packaging metadata and the formula disagree.

If the release contains CR4 work, its owner-visible Safety Verdict is part of the durable
release evidence before the release is complete. `scripts/project_os.py` already refuses to
close a CR4 story without one that records a pass.

Do not add future product features to Python merely because the Python release path is
simpler.

## Beta side-by-side (E06)

Between E06-S01 (first real Rust CLI command surface) and E06-S04 (canonical engine switch),
`cancellai-cli` is a beta artifact built from source, not a packaged release - it ships through
none of the mechanisms above. It is safe to build and run alongside the installed Python
`cancellai` command because the two share no install path and no local state
(`docs/development/MIGRATION_PYTHON_RUST.md`'s M7 section, E06-S03): different binary names
(`cancellai` vs `cancellai-cli`), and no cancellAI-owned local state file exists in either
engine to migrate or corrupt. `cancellai-cli version` identifies the engine/version a beta user
is running; "rolling back" is simply not invoking `cancellai-cli` again, never an uninstall or
data-migration step.

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

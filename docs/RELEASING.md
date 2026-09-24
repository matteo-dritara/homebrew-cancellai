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

**A release can also carry a fix that closes no epic** (E22-S07). Substitute
`--fix "<what the tagged version could not carry>"` for `--epic EXX` in step 2 below. Such a
release takes the next **patch** number and its packet declares no epic, so it cannot satisfy
PD-021 for one - the two shapes are mutually exclusive at the command line and in the packet.
This exists because a published tag is immutable history: when the v1.12.0 and v1.13.0 workflows
failed, the repair had nowhere to ship.

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

### The cutover release (E06-S04)

The first release after the owner accepts the migration Safety Verdict is `2.0.0`. `prepare` moves the Rust workspace version with the source version (the engine used to
report `0.1.0`), and the release workflow refuses a tag that disagrees with either. Finalize the
cutover release with the switch stated explicitly:

```sh
python3 scripts/release.py finalize --version 2.0.0 --adopt-cutover
```

`finalize` never edits the formula: `render_formula` generates it whole for the tag - Python-only
before 2.0.0, the engine formula from 2.0.0 - writes it atomically, and restores the original if
the post-write check fails. From 2.0.0 the formula it writes is the release asset `cancellai.rb`
that `release.yml` rendered from the manifest (E06-S14, ADR-0040), adopted only when:

- the published `release-manifest.json` is byte-identical to what the generator writes from the
  tag's commit (the same locally and on GitHub), the one run the attestations of all four
  archives name, and the SHA-256 of each archive `finalize` downloads - no `.sha256` sidecar is
  read, and `--sha256` cannot stand in for any of it;
- every archive's provenance verifies with `gh attestation verify`, bound to `release.yml`, the tag
  ref, the tag commit and GitHub-hosted runners, and that run is the tag's successful release run;
- `cancellai.rb` is byte-identical to the formula rendered from that manifest.

`finalize` therefore needs `gh`, authenticated, and the network; anything unavailable refuses.
`--adopt-cutover` is required for the one release that first carries the engine and is refused
without the owner's `project/evidence/E06-S04/CUTOVER_AUTHORIZATION.md` (E06-S15:
`Authorized-by: @matteo-dritara`, `Version: 2.0.0`, `Safety-Verdict-SHA256: <sha256 of
SAFETY_VERDICT.md>`, whose final round must pass). `release.py check` requires the live formula to
be byte for byte what `render_formula` produces for its version, and `release.py verify-formula`
repeats the published-release verification; `tests.yml` installs the engine formula built from
each commit and runs its `brew test`.

### Verifying a published release, and the v1.21.1 rehearsal (E06-S16)

`python3 scripts/release.py verify-release --version X.Y.Z` applies every check `finalize` applies
to a published release - the closed manifest, the local and GitHub tag, all four archives' bytes and
provenance, the release run, and `cancellai.rb` against the formula rendered from them - and writes
nothing. It works for any version whose release published a manifest, before or after the cutover.
v1.21.1 is the rehearsal: a fix release that changes nothing Homebrew installs and exists to run
the new pipeline for real before 2.0.0 depends on it.

### Rolling back the cutover

Documented, not rehearsed (owner decision 2026-09-25). Three levers, fastest first:

1. **A safety defect in the engine** - sign and publish a containment notice with the incident
   key (`docs/security/INCIDENT_RESPONSE.md`). Every installation that installs it, or picks it up
   with `cancellai-cli containment refresh`, caps the affected provider at `Observe`/`Recommend`
   and stops deleting. No release is needed, and nothing but a local `containment lift` undoes it.
2. **A user who needs the previous behaviour** - `cancellai-legacy`, the frozen Python reference,
   is installed beside the engine through 2.1.0 and behaves exactly as `cancellai` did before 2.0.0.
3. **The formula itself** - revert the `finalize` commit on `main`, so the tap serves the v1.21.1
   Python formula again. `release.py check` then reports the version drift on purpose, because the
   tap no longer matches the source; it clears when a repaired 2.0.x is finalized.

### When a release fails

A tag is immutable history here, so a failed release is not undone - it is recorded, and the fix
ships as the next version. Four have failed so far (v1.10.0, v1.12.0, v1.13.0, v1.13.1), each in a
job that only runs on a tag.

```sh
python3 scripts/release.py outcome --version X.Y.Z --state no --reason "which job failed, and the run id"
```

`prepare` writes `Published: pending` into the packet; this sets it. `release.py check` then names
every unpublished version with its cause, and the formula-lag rule skips one - the formula cannot
point at a version whose artifacts do not exist. Without this, v1.13.1 had to be finalized by hand
before v1.13.2 could be prepared, and the only record of why was whoever remembered.

Record the successful ones too (`--state yes`): an older version with no recorded outcome is
reported as a question, which is how v1.10.0's silent failure was found a month late.

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
from both `.pre-commit-config.yaml`'s local hooks and AGENTS.md's own "Current Python checks"
list (pytest and the remote ruff/mypy hooks have no pre-commit `entry:` of their own, so the
pre-commit source alone cannot see them removed) and the required Rust gate set from
`rust.yml`'s `quality` job, and fails if `release.yml` stops re-running one of them, drops a
platform from either Rust matrix, or defeats a gate structurally (`continue-on-error: true`,
an `if:` guard, or detaching a job from `publish`'s `needs:`) without changing its command
text (E22-S01, closing `CR-TE-06` - at v1.8.0 `release.yml` ran no Rust check at all and
reported success while `rust / quality (windows-latest)` failed on the same tagged commit; see
`project/evidence/RELEASE-v1.8.0.md`).

The `verify` job's checkout uses `fetch-depth: 0` (full history), not GitHub Actions' default
shallow checkout, because `check_platforms.py check`'s ancestor check
(`git merge-base --is-ancestor <verified_commit> HEAD`) needs the `verified_commit` object to
exist locally at all - a shallow checkout does not merely make an older `verified_commit`
unreachable, it makes the commit object absent, failing with `fatal: Not a valid commit name`
instead of validating provenance. This broke the tagged `verify` job at v1.10.0 (release run
34252829459); `scripts/check_workflows.py`'s `release_history_gate_errors()` now fails if the
checkout step in the job running that gate reverts to anything but `fetch-depth: 0` (E23-S01).

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

The release manifest's contract is fixed first (E17-S01):
`project/schemas/release_manifest.schema.json`, validated by `python3 scripts/release_manifest.py
check` against `tests/fixtures/release_manifest/golden/` (`pre-commit` and CI). It is a
machine-verifiable, versioned document naming every distributed binary exactly once
(`name`/`target_triple`/`sha256`) alongside `channel`, `source_sha`, `build_identity`, and
`knowledge_compatibility`. See `docs/security/SUPPLY_CHAIN.md`'s "Canonical release evidence".

E17-S02 ([ADR-0021](adrs/0021-hand-rolled-release-build-matrix.md)) implements the tier-1 build
matrix as hand-written jobs in `.github/workflows/release.yml`, run on every `v*` tag alongside
`verify`/`verify-rust`:

- `build-artifacts` - native `cargo build --release --target <triple> -p cancellai-cli` on each
  tier-1 target's own runner (`docs/PLATFORMS.md`: `aarch64-apple-darwin` and
  `x86_64-apple-darwin` on `macos-latest`, `x86_64-unknown-linux-gnu` on `ubuntu-latest`,
  `x86_64-pc-windows-msvc` on `windows-latest`), packages an archive plus a SHA-256 checksum
  sidecar, smoke-tests the packaged binary by unpacking it and running `cancellai-cli version`
  on the platform that just built it, generates a per-target CycloneDX SBOM
  ([ADR-0022](adrs/0022-cyclonedx-sbom-via-cargo-cyclonedx.md)), and signs both a build-provenance
  and an SBOM attestation for the archive (E17-S03) before anything is uploaded;
- `release-manifest-generate` - assembles `release-manifest.json` from those real artifacts via
  `python3 scripts/release_manifest.py generate` (real checksums of real files, not typed by
  hand), then immediately round-trips it with `release_manifest.py verify-checksums`;
- `attestation-verify` - independently re-fetches and cryptographically verifies every
  provenance and SBOM attestation `build-artifacts` created (`gh attestation verify`), separate
  from the run that created them (E17-S03's "Automated attestation verification job");
- `publish` (now gated on all three of the above, in addition to `verify`/`verify-rust`) re-runs
  the checksum round-trip against the downloaded artifacts before creating the GitHub Release,
  and attaches the archives, their checksums, their SBOMs, and `release-manifest.json` as
  release assets. A missing or invalid attestation, or a checksum mismatch, fails the release
  rather than shipping silently.

Cross-platform binaries produced this way are not yet a shipping product - `cancellai-cli`
remains a beta, source-built artifact until E06-S04's cutover gate opens (see "Beta
side-by-side" above); this pipeline exists so the automation is proven ahead of that cutover,
not so users install these archives today. Native installer formats (`.pkg`/`.msi`/`.deb`) are
deliberately out of this story's scope - ADR-0021 records that as the main reason a future
cargo-dist adoption remains open (see that ADR's 2026-09-08 correction for why "no network
access" is no longer part of that reasoning). No beta/nightly tag scheme exists yet, so
`build-artifacts` always builds with `CANCELLAI_CHANNEL=stable` set - E17-S05
([ADR-0023](adrs/0023-release-channel-authority-as-opt-in-function.md)) implements the SI-030
authority binding this value feeds (`cancellai-safety::BuildChannel::from_compiled_env`, read at
compile time and baked into the binary, never a runtime override), but no caller in
`cancellai-cli` enforces it yet; see `docs/security/SUPPLY_CHAIN.md`'s "Release channels"
section.

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

The current remote is `matteo-dritara/homebrew-cancellai`. It remains canonical while Python v1/Homebrew is the shipping product. Do not rename/split the repository during P0/P1 merely for aesthetics ([ADR-0011](adrs/0011-defer-canonical-repository-split.md)).

`scripts/check_repository_topology.py check` (E17-S06) enforces AC2 ("the canonical source
repository is unambiguously identified") as a real, running check rather than only a documented
claim: it cross-checks that `Formula/cancellai.rb`'s `homepage`, `scripts/release.py`'s `REPO`
constant, and this section's own "current remote" both name the exact same repository, and fails
if any of the three drifts from the others. It runs in `pre-commit` and CI, alongside every
other topology-relevant check (`scripts/check_workflows.py`, `scripts/release.py check`).

### Target topology

```text
matteo-dritara/cancellai          canonical product source/releases
matteo-dritara/homebrew-cancellai  Homebrew tap/distribution compatibility
```

ADR-0011 already decided *that* this split happens, deferred until "the Rust core and
cross-platform release factory are proven" (E06-S04's cutover, plus E17-S02/S03 giving the new
repository something real to release from day one). This section is the "exact timing and GitHub
migration mechanics" ADR-0011 says that decision still needs before execution - a runbook to
follow when the trigger condition is met, not an instruction this story executes now. No
external repository state is created or moved by this story; the plan below is written so
whoever executes it later does not have to re-derive these decisions under time pressure.

### Trigger condition

Do not begin this migration until **all** of the following hold:

- E06-S04's cutover gate has opened (`cancellai-cli` is the canonical engine, not the beta
  side-by-side artifact `docs/CLI_RUST.md` currently describes);
- at least one real `v*` release has shipped canonical tier-1 binaries through E17-S02's
  `build-artifacts`/E17-S03's attestation pipeline, so the new repository's first release is a
  genuine, evidenced artifact rather than an empty shell;
- the project owner has explicitly authorized the migration - creating a repository, transferring
  history, and repointing the tap are all real, hard-to-reverse actions on shared GitHub state,
  never something an agent executes as a side effect of a story reaching this point in the
  backlog.

### Migration steps

1. **Create `matteo-dritara/cancellai`** and push the full history there with `git push --mirror`
   (every branch and every tag, byte-identical commit SHAs - not a squashed or partial copy), so
   `v1.0.0` through the last pre-migration tag resolve to the same commits in both repositories
   during the transition window. Alternatively, GitHub's own repository-transfer feature (Settings
   → Transfer ownership, or the REST API's `repos/{owner}/{repo}/transfer` endpoint) moves a
   repository in place, preserving stars/watchers/issues/PRs automatically - but transfers the
   *existing* `homebrew-cancellai` repository itself, which is the wrong direction here (the tap
   must keep its current name/location for `brew tap`/`brew install cancellai` to keep working
   unmodified - see "Homebrew continuity" below); a mirror push into a newly created `cancellai`
   repository, leaving `homebrew-cancellai` in place, is the mechanics that actually preserves
   both properties at once.
2. **Issues**: `gh issue transfer <number> matteo-dritara/cancellai` for issues that concern the
   product going forward (real bugs/feature requests); issues that are specifically about the
   Python v1/Homebrew-only era stay in `homebrew-cancellai` as historical record. This is a
   documented, deliberate split, not an automatic bulk migration - GitHub has no "transfer every
   issue" primitive, and a bulk transfer would silently lose the "which era does this issue
   belong to" distinction that makes the split meaningful.
3. **Releases/tags**: existing tags and their GitHub Releases are immutable history (this
   document's own release process already treats published tags this way - "Published tags are
   immutable history and are never deleted") and stay in `homebrew-cancellai` permanently, exactly
   where users who `brew install`ed an old version can still find them. New releases, from the
   first tag cut after this migration, are tagged and published from `matteo-cancellai/cancellai`.
   `release-manifest.json`'s `build_identity.repository` field (E17-S01/E02-S02) then reads
   `matteo-dritara/cancellai` automatically (it is `${GITHUB_REPOSITORY}`, not a hand-maintained
   string) - this is what makes AC2 true going forward without a separate manual step.
4. **Homebrew continuity (AC "does not break existing Homebrew users")**: `homebrew-cancellai`
   is not touched as a *location* - Homebrew's own tap-naming convention (`brew tap OWNER/NAME`
   resolves to `github.com/OWNER/homebrew-NAME`) is exactly why the repository must keep this
   name. Only `Formula/cancellai.rb`'s `homepage`/`url` fields change, to point at
   `matteo-dritara/cancellai`'s release archives instead of `homebrew-cancellai`'s own. An
   existing user's `brew update && brew upgrade cancellai` continues to work with zero action on
   their part: the tap they already have resolves to the same repository it always did, and that
   repository's formula now happens to fetch from a different upstream. `scripts/release.py`'s
   `REPO` constant and `check_repository_topology.py`'s cross-check (above) are what keeps this
   formula from drifting silently out of sync with wherever canonical source actually moved to.
5. **`homebrew-cancellai` is never silently retired.** Per ADR-0011's target topology, it remains
   the Homebrew tap/distribution compatibility surface indefinitely - the same `brew audit
   --strict`/`brew style`/`brew install`/`brew test` CI gates `docs/security/SUPPLY_CHAIN.md`
   already describes keep running against it after the migration, unchanged. Retiring it (not
   merely repointing it) would itself need its own explicit compatibility plan and release
   evidence, per this same AC - nothing in this migration does that, and nothing should, absent a
   separate, later decision with its own ADR.

### Dry-run and smoke test (this story's verification contract)

- **Dry-run**: the steps above, reviewed and left ready to execute - this story does not create,
  transfer, or push anything to a new repository (see "Trigger condition"). `git push --mirror`
  and `gh issue transfer` are both real, standard commands verified against a real `gh` CLI
  installation during this story (`gh issue transfer --help`); neither was executed against this
  repository or any other.
- **Existing Homebrew upgrade-path smoke test**: already continuously exercised today, not a new
  addition this story invents - `brew audit --strict`/`brew style` run in CI on every change, and
  `brew install`/`brew test` exercise the tagged archive at release time
  (`docs/security/SUPPLY_CHAIN.md`). `check_repository_topology.py check` adds the piece that
  did not exist before: an automated guarantee that the Formula, `scripts/release.py`, and this
  document cannot silently disagree about which repository is canonical while that smoke test
  keeps passing.

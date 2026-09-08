# Evidence Packet - E17-S03

- Commit/PR: this working-tree change (executor session, 2026-09-08)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR3
- Spec version/commit: `project/epics/E17.json` (E17-S03), as of this change

## Outcome

PASS

`.github/workflows/release.yml`'s `build-artifacts` job (E17-S02) gained, per tier-1 target:

- a CycloneDX 1.5 JSON SBOM (`cargo-cyclonedx@0.5.9 --locked`, pinned, real `--target` so the
  SBOM reflects that leg's own conditionally-compiled dependency graph - e.g. `windows-sys`
  only appears in the Windows leg's SBOM, not approximated from whichever OS built it);
- a signed build-provenance attestation (`actions/attest-build-provenance`) against the archive;
- a signed SBOM attestation (`actions/attest` with `sbom-path` - the dedicated
  `actions/attest-sbom` action is deprecated by its own publisher, confirmed by fetching its
  current `action.yml` directly rather than assumed from a version number, and was not used).

A new `attestation-verify` job independently re-fetches and cryptographically verifies both
attestations for every archive (`gh attestation verify`, once for the default build-provenance
predicate type and once with `--predicate-type https://cyclonedx.org/bom` for the SBOM
attestation - that exact predicate-type string was confirmed by grepping
`actions/attest`'s bundled `dist/index.js`, not guessed). `publish` now depends on
`attestation-verify` in addition to `verify`/`verify-rust`/`build-artifacts`/
`release-manifest-generate`, and attaches the SBOM files to the GitHub Release alongside the
archives/checksums/manifest.

[ADR-0022](../../../docs/adrs/0022-cyclonedx-sbom-via-cargo-cyclonedx.md) records the tool/format
decision (CycloneDX via `cargo-cyclonedx`, attestation via `actions/attest-build-provenance` +
`actions/attest`).

**Process note:** while researching this story, `cargo install cargo-dist --locked` was
attempted directly and succeeded, disproving ADR-0021's (E17-S02) claim that this environment
has no network access to `crates.io`. That claim had been inferred only from `curl
https://crates.io` returning 403, never actually tested against `cargo install`. Corrected in
ADR-0021 itself and in E17-S02's evidence packet (separate commit, same session, project owner
reconfirmed the hand-rolled-matrix decision on corrected grounds before this story continued).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Users can verify origin from repository/workflow/commit | `actions/attest-build-provenance`'s signed attestation binds the archive's digest to the GitHub Actions OIDC identity (repository/workflow/commit) by construction - this is what the action does, not a claim this story adds on top; `attestation-verify` re-fetches and verifies it independently (`gh attestation verify <archive> --repo $GITHUB_REPOSITORY`) rather than trusting the generation step's own exit code | PASS (structural: workflow parses, job graph correct, action inputs confirmed against each action's real `action.yml`/bundled source via `gh api` - not assumed; full attestation-issuance-and-verification round trip is real GitHub infrastructure this session cannot execute locally, so it is proven on the next real tag push, consistent with how this repository has always disclosed CI-only behavior, e.g. `docs/PLATFORMS.md`'s Windows section) |
| AC2 - SBOM is attached/attested for each canonical build family | Each tier-1 target's `build-artifacts` leg generates its own CycloneDX SBOM and attests it (`actions/attest` + `sbom-path`) against that leg's archive; `publish` attaches every `dist/*.cdx.json` to the release; local dry-run of the generator itself (`cargo cyclonedx --manifest-path crates/cancellai-cli/Cargo.toml --describe binaries --format json --spec-version 1.5`) confirms real output: CycloneDX 1.5, 38 components for `cancellai-cli`'s dependency graph | PASS (generation confirmed locally; attestation confirmed on next real tag push, same disclosed-residual pattern as AC1) |
| AC3 - Release fails if required evidence is missing | `publish`'s `needs:` includes `attestation-verify`; `attestation-verify`'s two verification loops (`set -euo pipefail`, no `continue-on-error`) exit non-zero on the first `gh attestation verify` failure, which fails the job, which blocks `publish` via `needs:` - GitHub Actions' own dependency semantics, not a custom gate this story reimplements | PASS |

## Safety Evidence

Not applicable in the CR4 sense (`safety_obligations: []`); CR3 per the story's own
classification (reversible/conditional - reverting is deleting the new jobs, no state to
migrate, no channel currently distributes these artifacts). The safety-relevant property this
story adds is refusal-on-missing-evidence (`publish` cannot run without a passing
`attestation-verify`), addressed under AC3 above.

## Verification Commands

```text
python3 -c "import yaml; d = yaml.safe_load(open('.github/workflows/release.yml')); print(list(d['jobs']))"
python3 scripts/check_workflows.py check
python3 scripts/check_docs.py check
python3 scripts/check_process.py check
python3 -m pytest tests -v
# real, one-off local confirmations (not part of the committed test suite - no cargo-cyclonedx
# dependency is added to rust/Cargo.lock; it is installed transiently in CI exactly as
# cargo-deny already is in rust.yml's quality job):
cd rust && cargo install cargo-cyclonedx@0.5.9 --locked && \
  cargo cyclonedx --manifest-path crates/cancellai-cli/Cargo.toml --describe binaries \
    --format json --spec-version 1.5
gh api repos/actions/attest-sbom/contents/action.yml --jq '.content' | base64 -d   # confirms deprecation notice
gh api repos/actions/attest/contents/action.yml --jq '.content' | base64 -d       # confirms sbom-path input
gh api repos/actions/attest/contents/dist/index.js -H "Accept: application/vnd.github.raw" | grep -o 'https://cyclonedx[^"]*'
```

All PASS. `tests/test_workflows.py`'s `test_dropping_verify_rust_from_publish_needs_is_caught`
updated to match `publish`'s new five-job `needs` list (adds `attestation-verify`); full
242-test suite still green.

## Compatibility

- Attestation/SBOM generation runs on all three tier-1 OS runners identically (the actions and
  `cargo-cyclonedx` are all cross-platform); no platform-specific branching was needed beyond
  what `build-artifacts` already had for archive format (`.tar.gz` vs `.zip`).

## Performance / operability

- `cargo install cargo-cyclonedx@0.5.9 --locked` recompiles per matrix leg (~40s measured
  locally) - no shared cache exists yet; disclosed as a cost in ADR-0022, not hidden.
- `attestation-verify` runs once (not per-target), fanning in after all four `build-artifacts`
  legs and looping over their downloaded archives - bounded by the number of archives, not a
  separate job per target.

## Documentation updated

- `docs/adrs/0022-cyclonedx-sbom-via-cargo-cyclonedx.md` (new) - SBOM tool/format and
  attestation-mechanism decision.
- `docs/security/SUPPLY_CHAIN.md` - "Canonical release evidence" section now describes the real
  SBOM/provenance/attestation pipeline and what "achieved level" means for this repository.
- `docs/RELEASING.md` - "Target Rust release factory" section describes `attestation-verify`
  and the SBOM/attestation additions to `build-artifacts`/`publish`.
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- Real attestation issuance and verification (Sigstore/Fulcio OIDC signing, GitHub's
  attestation API) cannot be executed in this local session - proven on the next real `v*` tag
  push, same disclosed-residual pattern this repository has always used for CI-only behavior.
- `cargo-cyclonedx`'s dependency-graph accuracy is only as good as `Cargo.lock` and the crate's
  own `Cargo.toml` metadata; it does not independently verify that a declared dependency's
  published source matches what was actually compiled (that is `cargo-deny`'s and the Rust
  toolchain's own supply-chain surface, already covered by ADR-0015's `cargo deny check`, not a
  gap this story introduces).
- No native installer formats (`.pkg`/`.msi`/`.deb`) exist to attest beyond the archives
  themselves - unchanged residual from E17-S02/ADR-0021.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)

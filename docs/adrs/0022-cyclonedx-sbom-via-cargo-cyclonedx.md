# ADR-0022: CycloneDX SBOMs via cargo-cyclonedx, attested with GitHub's generic `attest` action

- Status: Accepted
- Date: 2026-09-08
- Owners: project owner / cEOS
- Related: PD-017, ADR-0009, ADR-0021, E17-S03

## Context

`docs/security/SUPPLY_CHAIN.md`'s "Canonical release evidence" section requires an SBOM ("SPDX
or CycloneDX, selected by release ADR/tooling") for each canonical build, and names GitHub
artifact attestations as the mechanism for attaching/signing it. E17-S03 needs both a concrete
SBOM tool and a concrete way to attest it.

`cargo install cargo-cyclonedx@0.5.9 --locked` was attempted for real in this session (network
access confirmed working per ADR-0021's 2026-09-08 correction) and produces a valid CycloneDX
1.5 JSON SBOM (38 components for `cancellai-cli`'s dependency graph) with `cargo cyclonedx
--manifest-path crates/cancellai-cli/Cargo.toml --describe binaries --format json
--spec-version 1.5 --target <triple>`.

For attestation, `actions/attest-sbom`'s own `action.yml` (fetched directly via `gh api
repos/actions/attest-sbom/contents/action.yml`, not assumed) emits `::warning::actions/
attest-sbom has been deprecated, please use actions/attest instead` and internally forwards to
the generic `actions/attest` action anyway. `actions/attest`'s `action.yml` confirms it accepts
`sbom-path` directly ("When provided, creates an SBOM attestation"), and its bundled JS
(`dist/index.js`, also fetched via `gh api` rather than assumed) contains the literal string
`https://cyclonedx.org/bom` as the CycloneDX predicate type it emits - the exact value
`gh attestation verify --predicate-type` needs to check that attestation specifically.

## Decision

- SBOM format: **CycloneDX 1.5 JSON**, generated per tier-1 target by `cargo-cyclonedx@0.5.9`
  (pinned, `--locked`) in `build-artifacts` (E17-S02's job), named `<archive-base-name>.cdx.json`
  and uploaded alongside that target's archive.
- Provenance attestation: `actions/attest-build-provenance@4d101475d8b20a2381f78447822ac1eab6504dd8`
  (`v4.2.2`) against the archive - not deprecated, purpose-built, unchanged from a plain
  provenance-only use case.
- SBOM attestation: the generic `actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6`
  (`v4.2.2`) with `subject-path` (the archive) and `sbom-path` (the CycloneDX file) - not
  `actions/attest-sbom`, which is deprecated.
- Verification: a new `attestation-verify` job (`gh attestation verify <archive> --repo
  $GITHUB_REPOSITORY` for provenance, and the same command with `--predicate-type
  https://cyclonedx.org/bom` for the SBOM attestation) runs against every downloaded archive
  before `publish` - `publish` depends on it, so a missing or invalid attestation fails the
  release (E17-S03 AC3) rather than shipping silently.

## Alternatives considered

### SPDX instead of CycloneDX

Rejected for this story: no equivalent `cargo install`-able SPDX generator was identified with
the same one-command simplicity as `cargo-cyclonedx` (the closest, `cargo-sbom`, emits a
custom/simplified schema rather than full SPDX). CycloneDX is the format GitHub's own tooling
documents first-class support for via `actions/attest`'s `sbom-path`, and
`docs/security/SUPPLY_CHAIN.md` explicitly leaves the choice open between the two - nothing
elsewhere in this repository commits to SPDX.

### `actions/attest-sbom` (the dedicated action)

Rejected: deprecated by its own publisher, confirmed from its current `action.yml` rather than
assumed from a version number. Using a deprecated action in a supply-chain-security-relevant
workflow the moment it is introduced, when the maintainer-recommended replacement is one field
away, is not a corner to cut.

### One workspace-wide SBOM instead of one per target

Rejected: `cargo cyclonedx --describe binaries` without `--target` reflects only the host
toolchain's conditional-compilation graph, which would silently omit Windows-only dependencies
(`windows-sys`, used by `cancellai-sealedfs`) from every SBOM generated on a non-Windows runner.
Generating one per `--target` inside `build-artifacts` (E17-S02's own per-target job) costs one
extra `cargo install` and one `cargo cyclonedx` invocation per matrix leg and reflects each
target's actual dependency graph.

## Consequences

### Positive

- SBOM accuracy is per-target, not approximated from the host toolchain;
- no deprecated action is introduced into a security-relevant workflow;
- verification is a real, independent re-check against GitHub's attestation API in its own job,
  not merely trusting that the generation step reported success.

### Negative / cost

- `cargo install cargo-cyclonedx@0.5.9 --locked` re-compiles the tool on every matrix leg
  (macOS aarch64/x86_64, Linux, Windows) - roughly 40 extra seconds per leg in this session's
  measurement; no shared cache exists for it yet (a future story could add one, out of scope
  here);
- CycloneDX-only; a consumer that specifically wants SPDX is not served without a future
  addition.

### Neutral / follow-up

- if a maintained SPDX generator with comparable simplicity appears, or a consumer specifically
  requires SPDX, add it alongside (not instead of) the CycloneDX SBOM via a follow-up ADR.

## Safety and compatibility impact

- Change Risk implication: CR3 (E17-S03) - this adds signed, verifiable supply-chain evidence;
  it does not touch runtime authority or mutation paths. `attestation-verify` gating `publish`
  is new refusal-on-missing-evidence behavior, matching the story's own AC3.
- Safety Invariants affected: none directly (no `safety_obligations` listed for E17-S03).
- Migration/rollback: reverting is a revert of the `build-artifacts` SBOM/attest steps and the
  `attestation-verify` job; no state migration exists because no channel currently distributes
  these artifacts (`cancellai-cli` remains beta/source-built per `docs/RELEASING.md`).

## Supersession

If a different SBOM tool or format replaces this later, keep this ADR and mark it superseded by
the ADR that makes that change.

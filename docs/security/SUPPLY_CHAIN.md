# Software Supply Chain

cancellAI's distribution is security-sensitive because users grant the binary filesystem authority. The release pipeline is therefore part of the product safety boundary.

## Standards baseline

The engineering system maps to:

- NIST Secure Software Development Framework (SSDF) v1.1 as the current finalized baseline, while tracking the v1.2 revision process;
- SLSA v1.2 concepts for source/build provenance and progressive assurance;
- OpenSSF Scorecard/source-management guidance;
- CNCF Software Supply Chain Security Best Practices v2;
- GitHub artifact attestations for build/SBOM provenance where supported;
- RustSec/cargo-deny for the future Rust dependency supply chain.

We do not claim a formal certification merely because tools are enabled. Claims such as a SLSA level must be tied to verifiable release evidence and the exact build architecture in use.

## Source controls

Target repository rules:

- protected default branch/ruleset;
- required CI and governance checks;
- no force-push on protected release history;
- least-privilege GitHub Actions permissions;
- dependency automation with review/gates appropriate to risk;
- secret scanning/private vulnerability reporting;
- CodeQL or equivalent supported static analysis;
- OpenSSF Scorecard monitoring;
- CODEOWNERS/review requirements for safety/release surfaces once maintainer topology supports it.

For a single-maintainer project, independent AI verification is useful evidence but is not cryptographic human separation of duties. The repository should not pretend otherwise.

Current CI actions are immutable-SHA pinned and checked by `scripts/check_workflows.py`; Python development/CI tools are version-pinned in `requirements-dev.txt`. Dependabot proposes updates, but workflow/development dependency updates are review-gated rather than auto-merged.

### Canonical repository identity

The canonical source repository is `matteo-dritara/homebrew-cancellai` today; a controlled
split to a product-named canonical repository plus a Homebrew-tap-only `homebrew-cancellai`
is a deferred, evidence-gated decision, not an aesthetic rename
([ADR-0011](../adrs/0011-defer-canonical-repository-split.md), `docs/RELEASING.md`'s
"Repository topology transition" - E17-S06). `scripts/check_repository_topology.py check`
enforces that `Formula/cancellai.rb`'s `homepage`, `scripts/release.py`'s `REPO` constant, and
`docs/RELEASING.md`'s documented "current remote" all name the same repository, in `pre-commit`
and CI - the identity claim this section and the release manifest's `build_identity.repository`
field (E17-S01) both rely on cannot drift silently out of sync between them.

The Rust workspace carries the same posture (E22-S02): `.github/dependabot.yml` proposes cargo-ecosystem updates for `rust/` (`serde`, `serde_json`, `unicode-normalization`, and `libc` - the last inside `cancellai-sealedfs`, the only crate in the workspace containing `unsafe`), and `.github/workflows/codeql.yml`'s `analyze-rust` job runs CodeQL over the built workspace - the authority kernel, the `sealedfs` FFI boundary, and the provider adapters - reporting to the same repository security-events channel the Python analysis already uses (`permissions.security-events: write`, shared by both jobs in that workflow).

## Dependency policy after Rust bootstrap

E02-S02 implemented this policy at `rust/deny.toml` (ADR-0015), enforced by `cargo deny
check` locally and in `.github/workflows/rust.yml`'s `quality` job on macOS/Linux/Windows:

- committed `Cargo.lock` for application builds (`rust/Cargo.lock`);
- `cargo-deny` for licenses, sources, duplicate/banned dependencies, advisories - one command
  covers all four; a separate `cargo audit` is redundant with it (both read the RustSec
  Advisory Database) and is not used;
- strict permissive license allow-list (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Unicode-3.0,
  Zlib); anything else, including weak and strong copyleft, is denied by default;
- minimize native/system dependencies where practical;
- unknown registries/git sources denied by default (`[sources] unknown-registry =
  "deny"`/`unknown-git = "deny"`); a specific registry or git source needs an explicit,
  reviewed addition to the allow-list, not an implicit pass;
- a wildcard (`*`) version requirement is denied (`[bans] wildcards = "deny"`), so a
  dependency version is always pinned to something explicit;
- MSRV is pinned at 1.85.0 (`rust/Cargo.toml`'s `rust-version`), bumped only by deliberate,
  reviewed decision - never implicitly by a dependency update (ADR-0015).

## Canonical release evidence

Each canonical binary release should produce:

- source commit/tag identity;
- target triple and build configuration;
- SHA-256 checksums;
- release manifest;
- SBOM (SPDX or CycloneDX, selected by release ADR/tooling);
- build provenance/attestation;
- signature/verification material appropriate to the chosen distribution channel;
- test/gate summary and Safety Verdict references for CR4 changes;
- knowledge bundle compatibility version.

E17-S01 defines the release manifest as a machine-verifiable, versioned document:
`project/schemas/release_manifest.schema.json` is the contract, `scripts/release_manifest.py
check` validates it against the golden corpus under
`tests/fixtures/release_manifest/golden/`, and both run in `pre-commit` and CI (E17's
`release.yml` `verify` job). A manifest records `version`, `channel`, `source_sha`,
`build_identity` (repository/workflow/run_id), `knowledge_compatibility`
(min/max provider-knowledge schema version), and an `artifacts` list where every distributed
binary appears under a unique canonical `name` with its `target_triple` and SHA-256 checksum -
two artifacts may share a target triple (for example an archive and an installer) but never a
name. Generating a manifest from a real multi-platform build, and attaching SBOM/provenance
evidence to it, is E17-S02/E17-S03 scope; this story only fixes the contract and its
validator.

GitHub artifact attestations can establish signed provenance and attach SBOM attestations. SLSA's current specification is v1.2; GitHub's documentation describes the assurance delivered by its current attestation mechanisms and reusable workflows. The project records the exact achieved level rather than copying a marketing label.

E17-S03 ([ADR-0022](../adrs/0022-cyclonedx-sbom-via-cargo-cyclonedx.md)) implements this for
real in `build-artifacts` (E17-S02's per-target job): a CycloneDX 1.5 JSON SBOM
(`cargo-cyclonedx`, generated with `--target` so it reflects that leg's actual
conditionally-compiled dependency graph, not an approximation from the host toolchain), a
signed build-provenance attestation (`actions/attest-build-provenance`) naming the repository/
workflow/commit, and a signed SBOM attestation (the generic `actions/attest` with `sbom-path` -
`actions/attest-sbom` is deprecated by its own publisher and is not used). A dedicated
`attestation-verify` job re-fetches and cryptographically verifies both attestations for every
canonical archive (`gh attestation verify`, once for the default provenance predicate type and
once for CycloneDX's `https://cyclonedx.org/bom` predicate) independently of the run that
created them; `publish` depends on it, so a missing or invalid attestation fails the release
rather than shipping silently (AC3). This is what "achieved level" means in practice today: SLSA
build-provenance-shaped evidence plus an attested SBOM, per canonical archive, both
independently re-verified before publish - not a claim to a specific numbered SLSA level.

## Release automation

The target Rust release factory should evaluate `dist`/cargo-dist (or a successor with equivalent evidence) because it can generate cross-platform archives and multiple installers including shell, PowerShell, Homebrew, and MSI. Tool adoption remains an ADR because release infrastructure is security-sensitive.

## Knowledge updates

Provider knowledge updates are separate signed artifacts from software releases. A knowledge bundle:

- has schema version, publisher identity, issue/expiry metadata, and content digest;
- is verified before use;
- can disable/downgrade unsafe capabilities quickly;
- cannot add arbitrary executable code;
- cannot elevate above local trust/authority ceilings;
- supports rollback to last known trusted bundle.

E16-S02 ([ADR-0024](../adrs/0024-ed25519-dalek-for-knowledge-bundle-signatures.md)) implements
this: `cancellai-safety::knowledge_bundle::KnowledgeBundle` carries `schema_version`,
`publisher_id`, `sequence`, `issued_at`, `expires_at`, `content_digest`, `payload`, and an
Ed25519 `signature` - `parse_bundle` rejects a bundle missing any field (including one missing
`signature` entirely - the literal "unsigned" case) before verification is ever attempted.
`verify_bundle` checks schema version, publisher identity against a caller-supplied
`LocalTrustPolicy`, the SHA-256 content digest, and the Ed25519 signature, then expiry; the
`VerifiedKnowledgeBundle` it returns carries the `TrustedTier` the local policy already assigned
that publisher, never anything read from the bundle - a knowledge update cannot elevate trust
because there is no field in the wire format that could assert one. `KnowledgeStore::apply`
refuses a bundle whose `sequence` does not exceed the currently installed bundle's from the same
publisher (replay/rollback-of-trust protection), and `KnowledgeStore::rollback` reinstates the
bundle a later one replaced, re-checking expiry against the current clock so a failed rollback
never displaces the still-current bundle. The bundle's `payload` is opaque to this module - a
provider-manifest payload (E16-S01/`cancellai-provider-api::parse_manifest`) is a natural fit,
since that type is independently, structurally incapable of expressing a capability/trust/
authority claim, but wiring a verified bundle into that parser and into an actual distribution
channel is deferred to whichever future story makes knowledge updates a real, shipped mechanism
- this story delivers the format and its verification, adversarially tested (tamper, replay,
expiry, unknown-signer, rollback), not a running update service.

## Release channels

- `stable` - highest verified default authority allowed by product policy.
- `beta` - lower default autonomous authority; used for migration/compatibility validation.
- `nightly` - Observe/Recommend oriented by default; irreversible/autonomous behavior requires explicit development override and never piggybacks stable settings silently.

E17-S05 implements the SI-030 binding this describes: `cancellai-safety::authority::
effective_authority_for_channel` adds a `ReleaseChannelAuthority` constraint to the Effective
Authority minimum, sourced from `cancellai-safety::BuildChannel` - an opaque wrapper (mirroring
`TrustedTier`'s split from `ProviderTrust`, SI-021) whose only production constructor,
`BuildChannel::from_compiled_env`, reads a `CANCELLAI_CHANNEL` value baked in at *compile* time
(`option_env!`), never a runtime environment variable a user running the binary could set to
claim a higher channel than the build actually is. `stable` carries no additional cap (channel
alone never lowers a stable build below what its other constraints already allow); `beta` caps
at `Govern` (a confirmed, non-unattended `Delete` is still reachable; `Autopilot` is not);
`nightly` (and any unset/unrecognized value - a malformed override is never treated as more
permissive than an absent one) caps at `Recommend`, strictly below what even `Quarantine`
requires, let alone `Delete`. `.github/workflows/release.yml`'s `build-artifacts` job
(E17-S02) sets `CANCELLAI_CHANNEL=stable` for every canonical tier-1 build.

Wiring this into `cancellai-cli`'s own classification pipeline
(`cancellai-policy::retention::RetentionPolicy`/`classify`) is deferred to whichever story
makes `cancellai-cli` the canonical, packaged-release engine (E06-S04) - it remains a beta,
source-built artifact today (`docs/RELEASING.md`'s "Beta side-by-side" section), and every
`cargo test`/local `cargo build` in this repository is, correctly, an unlabeled (Nightly-
equivalent) build; forcing that pipeline to declare a channel now would have meant picking an
arbitrary placeholder for callers nothing yet asks to enforce it against. The constraint itself
is fully implemented and adversarially tested in `cancellai-safety::authority`'s own test suite
today, ready for that future caller to adopt.

## Installation-source awareness

cancellAI records whether it was installed via Homebrew, direct installer, Windows package channel, Linux package, etc. `update --check` can inform, but upgrades should follow the original package manager/channel rather than silently replacing package-managed binaries.

## Verification UX

Future `cancellai version --provenance` or equivalent should make supply-chain evidence usable by developers, not merely present in CI logs.

## Incident containment

Supply-chain and compatibility incidents use the runbook in [`INCIDENT_RESPONSE.md`](INCIDENT_RESPONSE.md). The federated knowledge path may rapidly **downgrade** a provider/version/capability to Observe/Recommend after verified compromise evidence, but it can never become a remote destructive switch. E17-S07 turns this containment model into tested release/knowledge infrastructure.

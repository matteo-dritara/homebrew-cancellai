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

GitHub artifact attestations can establish signed provenance and attach SBOM attestations. SLSA's current specification is v1.2; GitHub's documentation describes the assurance delivered by its current attestation mechanisms and reusable workflows. The project records the exact achieved level rather than copying a marketing label.

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

## Release channels

- `stable` - highest verified default authority allowed by product policy.
- `beta` - lower default autonomous authority; used for migration/compatibility validation.
- `nightly` - Observe/Recommend oriented by default; irreversible/autonomous behavior requires explicit development override and never piggybacks stable settings silently.

## Installation-source awareness

cancellAI records whether it was installed via Homebrew, direct installer, Windows package channel, Linux package, etc. `update --check` can inform, but upgrades should follow the original package manager/channel rather than silently replacing package-managed binaries.

## Verification UX

Future `cancellai version --provenance` or equivalent should make supply-chain evidence usable by developers, not merely present in CI logs.

## Incident containment

Supply-chain and compatibility incidents use the runbook in [`INCIDENT_RESPONSE.md`](INCIDENT_RESPONSE.md). The federated knowledge path may rapidly **downgrade** a provider/version/capability to Observe/Recommend after verified compromise evidence, but it can never become a remote destructive switch. E17-S07 turns this containment model into tested release/knowledge infrastructure.

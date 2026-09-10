# Release Evidence - v1.12.0

## Source

- Tag: `v1.12.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-11

## Included work

- Epic: E08 - Universal Artifact and Project Intelligence
- Stories: E08-S01, E08-S02, E08-S03, E08-S04
- CR4 Safety Verdicts: none

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

### Added

- Defined the canonical release artifact manifest contract (E17-S01):
  `project/schemas/release_manifest.schema.json` is a machine-verifiable, versioned document
  naming every distributed binary exactly once (`name`/`target_triple`/`sha256`), alongside
  `channel`, `source_sha`, `build_identity`, and `knowledge_compatibility`.
  `scripts/release_manifest.py check` validates it against a golden fixture corpus
  (`tests/fixtures/release_manifest/golden/`) and runs in `pre-commit` and CI. Generating a
  manifest from a real multi-platform build is E17-S02/E17-S03 scope.
- Automated the tier-1 cross-platform release build (E17-S02,
  [ADR-0021](../../docs/adrs/0021-hand-rolled-release-build-matrix.md)): `.github/workflows/release.yml`
  now builds `cancellai-cli` natively for `aarch64-apple-darwin`/`x86_64-apple-darwin`/
  `x86_64-unknown-linux-gnu`/`x86_64-pc-windows-msvc` on every `v*` tag, packages an archive
  plus SHA-256 checksum per target, smoke-tests each packaged binary by unpacking and running
  it on the platform that built it, and assembles/round-trips a real `release-manifest.json`
  (E17-S01) before `publish` attaches everything to the GitHub Release.
  `scripts/release_manifest.py` gained `generate` and `verify-checksums` subcommands for this.
  These binaries are not a shipping product yet - `cancellai-cli` stays a beta, source-built
  artifact until E06-S04's cutover gate opens.
- Added installation-source awareness to `cancellai-cli` (E17-S04, CR1, observational only):
  `cancellai-cli update --check` and `cancellai-cli version --source` report the installation
  source detected from the running binary's own resolved path (`homebrew`, `windows_package`,
  `linux_package`, `direct_download`, or `unknown`) and source-specific upgrade guidance that
  never names a different channel than the one detected. A bare `update` (no `--check`) is
  refused rather than silently implying a future auto-update default (SI-007); `version`'s bare
  output is unchanged - `--source` only ever appends lines.
- Added provenance, SBOM, and signed attestation to the release build (E17-S03,
  [ADR-0022](../../docs/adrs/0022-cyclonedx-sbom-via-cargo-cyclonedx.md)): `build-artifacts`
  generates a per-target CycloneDX 1.5 SBOM (`cargo-cyclonedx`, target-accurate - reflects
  each leg's own conditional dependencies rather than the host toolchain's) and signs a
  build-provenance attestation and an SBOM attestation for each canonical archive
  (`actions/attest-build-provenance`, `actions/attest` with `sbom-path` - not the deprecated
  `actions/attest-sbom`). A new `attestation-verify` job independently re-fetches and
  cryptographically verifies both attestations for every archive before `publish`, which now
  depends on it: a missing or invalid attestation fails the release rather than shipping
  silently.
- Bound release channel to maximum default authority (E17-S05, CR4, SI-030,
  [ADR-0023](../../docs/adrs/0023-release-channel-authority-as-opt-in-function.md)):
  `cancellai-safety::authority::effective_authority_for_channel` adds a `ReleaseChannelAuthority`
  constraint to the Effective Authority minimum, sourced from the new `cancellai-safety::
  BuildChannel` - an opaque wrapper (mirroring `TrustedTier`'s split from `ProviderTrust`,
  SI-021) whose only production constructor reads a `CANCELLAI_CHANNEL` value baked in at
  *compile* time, never a runtime environment variable a user could set to claim a higher
  channel than the build actually is. `stable` carries no additional cap; `beta` caps at
  `Govern` (reaches a confirmed `Delete`, never unattended `Autopilot`); `nightly` (and any
  unset/unrecognized value) caps at `Recommend`, strictly below what even `Quarantine`
  requires. `.github/workflows/release.yml`'s `build-artifacts` job (E17-S02) now sets
  `CANCELLAI_CHANNEL=stable` for every canonical tier-1 build. Wiring this into
  `cancellai-cli`'s own classification pipeline is deferred to E06-S04 (the cutover story) -
  see `docs/security/SUPPLY_CHAIN.md`'s "Release channels" section for why.
- Wrote the canonical-repository-topology migration runbook and made its identity claim
  enforced rather than only documented (E17-S06, CR2): `docs/RELEASING.md`'s "Repository
  topology transition" section now names the trigger condition, migration steps (history
  preservation, issue handling, release/tag continuity, and - critically - why
  `homebrew-cancellai` keeps its name so `brew tap`/`brew install cancellai` keeps working for
  existing users with zero action on their part), and an explicit "never silently retired" tap
  commitment, executing ADR-0011's already-accepted decision to defer the actual split rather
  than deciding it now. `scripts/check_repository_topology.py check` (new; `pre-commit` and CI)
  cross-checks that `Formula/cancellai.rb`, `scripts/release.py`'s `REPO` constant, and that
  document's own "current remote" claim all name the same repository.
- Defined the provider manifest schema v1 (E16-S01, `docs/architecture/PROVIDER_MODEL.md`
  "Manifest-only" integration level, SI-021): `rust/crates/cancellai-provider-api/src/manifest.rs`
  (`ProviderManifest`, versioned, `#[serde(deny_unknown_fields)]` on every struct) has no field
  anywhere that can express a capability, trust, or authority claim - a manifest can only ever
  describe *where* provider state lives (roots) and *what kind* of thing each matched file is
  (session/protected/cache), never claim delete capability or a trust level. `manifest_provider.rs`
  adds `ManifestProvider`, a generic `ProviderCapabilities` implementation driven entirely by a
  parsed manifest and resolved root paths - it answers `DETECT`/`FINGERPRINT_ROOT`/
  `INVENTORY_MAP` from real evidence (reusing the same root-confidence rule Claude/Codex's
  adapters use) and reports every other capability unconditionally `UNSUPPORTED`. A
  manifest-driven provider's authority is computed through the identical
  `cancellai_safety::TrustedTier` gate every hand-written adapter uses and defaults to
  `TrustedTier::untrusted()`.
- Added the OpenCode provider manifest (E16-S05, `docs/PROVIDERS.md` "Tier 2 ecosystem
  providers"): the first real "Manifest-only" integration, built from OpenCode's real,
  source-confirmed layout (`anomalyco/opencode`, formerly `sst/opencode`) - `auth.json`
  (credentials, protected) and a `storage/` tree of session/message/part/session_diff/project
  records under `$XDG_DATA_HOME/opencode`, plus declared (not yet scanned) config/cache roots.
  Ships committed and reviewed in this repository via `rust/crates/cancellai-provider-api::
  opencode_manifest`, but its provider still defaults to `TrustedTier::untrusted()` like every
  other provider - no code path here grants it anything a community-contributed manifest would
  not also have to earn through the identical trust pipeline. An unrecognized layout stays
  inspection-only.
- Added the Gemini CLI provider manifest (E16-S03, `docs/PROVIDERS.md` "Tier 2 ecosystem
  providers"): a second "Manifest-only" integration on the same E16-S01 engine, built from
  `google-gemini/gemini-cli`'s real source - `oauth_creds.json`/`google_accounts.json`/
  `settings.json`/`trustedFolders.json`/`projects.json` (protected) and `tmp/<projectIdentifier>/
  chats/*.jsonl` (session) under `GEMINI_CLI_HOME`-or-`~/.gemini`. A new `vendor_notes` manifest
  field cites the tool's documented built-in session retention policy in `EXPLAIN`'s evidence
  text without upgrading `EXPLAIN` out of `UNSUPPORTED` - it makes the honest answer more
  informative, never a claim to detect a user's actual configured value. Defaults to
  `TrustedTier::untrusted()` like every other provider.
- Added the GitHub Copilot CLI provider manifest (E16-S04, `docs/PROVIDERS.md` "Tier 2
  ecosystem providers"): a third "Manifest-only" integration, built from GitHub's published
  documentation (Copilot CLI is closed-source) - `config.json`/`settings.json` (protected) and
  `session-state/<sessionID>/events.jsonl` (session) under a `COPILOT_HOME` full-path override
  or `~/.copilot` default. Being manifest-only means there is no mutation path to begin with,
  satisfying "internal state is not mutated without documented native capability" by
  construction. The separate, platform-conditional `COPILOT_CACHE_HOME` directory is not
  modeled as a root in this schema version. Defaults to `TrustedTier::untrusted()`.
- Defined the signed knowledge bundle format (E16-S02, CR4,
  [ADR-0024](../../docs/adrs/0024-ed25519-dalek-for-knowledge-bundle-signatures.md),
  `docs/security/SUPPLY_CHAIN.md` "Knowledge updates"):
  `cancellai-safety::knowledge_bundle::KnowledgeBundle` packages provider/version/layout
  intelligence separately from the binary, verified with Ed25519 (verification-only dependency
  - no signing capability ships) against a caller-supplied `LocalTrustPolicy`. A verified
  bundle's trust tier always comes from local policy, never from the bundle itself - there is no
  field in the wire format that could assert one. `KnowledgeStore` refuses a stale/replayed
  update from the same publisher and supports rollback to the prior trusted bundle, re-checking
  expiry so a failed rollback never displaces the still-current bundle. Adversarially tested:
  tamper (payload, digest, signature), expiry, unknown-signer, and rollback-after-expiry cases.
- Automated the community provider verification workflow (E16-S06, `.github/CONTRIBUTING.md`
  "Provider contributions"): `scripts/check_provider_trust.py check` (`pre-commit` and CI) lints
  every manifest under `rust/crates/cancellai-provider-api/manifests/*.json` against that
  schema's own known field set - a smuggled `trust`/`capability`/`authority` field, anywhere in
  the manifest including nested inside a root/marker/artifact, fails here independently of the
  Rust parser's own `deny_unknown_fields` check - and validates the new
  `project/provider_trust.json` registry recording each shipped manifest's current
  `ProviderTrust` tier. A tier above `Untrusted` requires a non-empty `verified_by` and at least
  one `fixture_references` entry, mirroring `cancellai-safety::trust_promotion`'s own
  evidence-required rule; `project/provider_trust.json` is added to `CODEOWNERS` so only the
  project owner can merge a change to it. A community PR therefore cannot mark itself Built-in
  Verified through either a smuggled manifest field or a bare registry claim.
- Widened `cancellai-model::AgentArtifact` with `relationships: Vec<ArtifactRelationship>`
  (E08-S01, `docs/architecture/DOMAIN_MODEL.md` "`relationships` (E08-S01)"): each entry is a
  `{ kind: RelationshipKind, related_artifact_id: ArtifactId }` pair, with `RelationshipKind`
  carrying only `ChildOf` today. Populated from `cancellai_provider_codex::CodexSession::
  parent_session_id`, which `cancellai-policy::retention::resolve_codex` was already reading but
  discarding before this change; an unresolved `parent_thread_id` produces no relationship
  rather than a fabricated one. Claude sessions are flat and always report an empty list. New
  key, additive-only, in `--json` inventory output's `artifacts[]` entries.
- Widened `cancellai-model::AgentArtifact` with `project_attribution: Option<ProjectAttribution>`
  (E08-S02, CR2, SI-023, `docs/architecture/DOMAIN_MODEL.md` "`project_attribution` (E08-S02)"):
  `None` means `Unattributed`; a `Some` names the project (`ProjectRef`), the evidence category
  (`AttributionSource::ExplicitProviderMetadata` today - Claude's own `projects/<name>/`
  directory, taken verbatim, never decoded into a guessed real filesystem path), and a
  confidence that starts equal to the artifact's own and is downgraded alongside it by the
  existing partial-scan handling. A blank/whitespace-only project name resolves to
  `Unattributed` rather than a hollow reference. Codex sessions have no project concept this
  adapter observes and are always `Unattributed`. New key, additive-only, in `--json` inventory
  output's `artifacts[]` entries.
- Gave `cancellai-model::ActivityState::Orphaned` its first producer, and added
  `AgentArtifact::activity_signal: Option<ActivitySignal>` (E08-S03,
  `docs/architecture/DOMAIN_MODEL.md` "`activity_signal` / `ActivityState::Orphaned` (E08-S03)"):
  a Codex session declaring a `parent_session_id` this scan did not discover is now classified
  `Orphaned` (a dangling parent reference), with `activity_signal` naming the missing parent id;
  an ordinary `Stale` session's `activity_signal` names the observed mtime and cutoff. An
  unresolved parent only ever overrides what would otherwise be `Idle`/`Stale` - never `Active`
  or `Unknown` - preserving the existing authority-capping protection those two values already
  have in `cancellai-safety::authority::lifecycle_ceiling`. `activity_signal` is `None` for
  `Active`/`Idle`/`Unknown`. New key, additive-only, in `--json` inventory output's `artifacts[]`
  entries.
- Added `cancellai-policy::views` (E08-S04, CR1, `docs/architecture/TARGET.md` "Engine / Query
  API (E08-S04)"): machine/project/provider/artifact/session query views over one classified
  inventory (`by_machine`/`by_project`/`by_provider`/`by_artifact`/`by_session`), the first real
  occupant of the target architecture's "Engine / Query API" layer. Every view groups borrowed
  references rather than cloning artifact data; `by_project` carries an explicit `Unattributed`
  bucket for E08-S02's `None` attribution; `by_session` walks a full `ChildOf` chain to its
  ultimate root (E08-S01) rather than one edge, so a transitive Codex subagent tree lands in one
  bucket, with a target absent from the given slice or a cycle falling back to the artifact's own
  id instead of propagating an unbacked id (round-1 independent verifier review finding, repaired
  in the same round); Claude's flat sessions each stay their own bucket. `by_machine` is a single
  bucket today (no multi-machine support exists yet). A generic reconciliation test compares
  counted id occurrences, not a set, so a duplicate or dropped source row is observable in every
  view.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

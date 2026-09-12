# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The engineering contract is now loadable by the agent harness that is supposed to execute it
  (E24, CR0). `.claude/skills/` carries seven Agent Skills in the open
  [Agent Skills](https://agentskills.io) format - `orient`, `story-executor`, `epic-verifier`,
  `adversarial-cases`, `risk-gate`, `rust-kernel-guard`, `evidence-packet` - so the same pack
  loads for the executor and for the independent reviewer, which `AGENTS.md` assigns to a
  different agent. A skill is a runner over the contract and never a second copy of it;
  `scripts/check_agent_skills.py` enforces that by requiring every repository path and every
  `scripts/*.py` command a skill names to resolve.
- A `PreToolUse` hook refuses a hand-edit to a generated document at the moment it is attempted,
  returning the regeneration command, instead of letting CI discover it after the work is done.
  It matches on the real, project-relative, case-folded path - `docs/BACKLOG.md`,
  `docs/backlog.md` and `docs/adrs/../BACKLOG.md` are one file on APFS - and fails open on any
  input it cannot interpret. It covers the structured file-writing tools only; the CI drift
  check remains the authority.

### Changed

- `AGENTS.md` states **diff discipline** alongside story discipline: no unrelated reformatting,
  no removal of pre-existing dead code (a safety rule here, since code that looks unreachable may
  be a barrier whose reachability is what is in dispute), removal limited to what a change
  orphaned, and flagging rather than fixing what is found outside the story.

No user-visible product behavior changes from E24.

## [1.13.0] - 2026-09-12

### Added

- Added a real peak-memory regression gate for the shipped discovery path (E10-S02, CR1). New
  `cancellai-cli/tests/performance_memory.rs` runs on every `cargo test` on Linux and asserts
  real peak RSS (`/proc/self/status`'s `VmHWM`, no new dependency) against a 128 MiB regression
  budget for the same synthetic tree `performance_shipped_path.rs`'s latency gate already uses;
  the gate's own module does not exist on macOS/Windows rather than reporting a fabricated
  number there. `performance_scheduled_shipped.rs`'s heavy-dataset trend artifact gained a
  `peak_rss_bytes` field (published for trend visibility, `None` off Linux, never gated) so the
  10k/100k/1M scheduled runs report memory alongside latency without blocking ordinary PRs on a
  noisy metric.
- Added a reclaimability estimator distinguishing logical size, allocated size, and clone/
  reflink-sharing uncertainty (E10-S01, CR2, library-level, no CLI/TUI surface yet). New
  `cancellai-platform::filesystem_kind::CloneSemantics` classifies a scope root's filesystem as
  `NotKnownToShare`, `PossiblyShared` (APFS, Btrfs, XFS, ZFS, ReFS - real, documented clone/
  reflink capability that can make a naive allocated-size sum overstate what deleting files
  actually frees), or `Unsupported` - real detection ships for macOS (`libc::statfs`'s
  `f_fstypename`, `cancellai-sealedfs::observe_filesystem_name`) and Linux (reusing `wsl::
  FilesystemContextObserver`'s `/proc/mounts` parsing for the raw fstype), with Windows
  disclosed as `Unsupported` pending its own story. An unrecognized filesystem name defaults to
  `PossiblyShared`, never the more confident label, so a future/unrecognized clone-capable
  filesystem is never silently trusted. New `cancellai-inventory::reclaim::estimate_reclaim`
  aggregates `FileFacts` into a `ReclaimEstimate`, excluding (and counting, never substituting
  with logical size) any file whose allocated size was itself unobservable, and labeling the
  result `Verified` only when every size was known and the filesystem is `NotKnownToShare` -
  any clone-capable or undetermined filesystem, or any excluded file, downgrades the estimate to
  `Estimated` with a named reason (this story's AC2: unknown APFS/reflink/shared-block effects
  are never presented as guaranteed savings). A self-review found and this same story's own fix
  closed a real Windows CI break before release: the classifier and its backing constants
  carried no `cfg` gate, so they were dead code on Windows (nothing there called them) and broke
  the mandatory `cargo clippy -D warnings` gate on `windows-latest` - now gated to
  `cfg(any(test, target_os = "macos", target_os = "linux"))`, the precedent this crate's own
  `wsl::classify_fstype` already set for the identical shape of gap.

- Added a real Atlas TUI shell and keyboard-first navigation to `cancellai-tui` (E09-S01, CR1,
  observational only), replacing the E02-S01 placeholder skeleton: `Tab`/`Shift+Tab`/`1`-`4`
  cycle four screens (`Home`, and stubs for `Atlas`/`Explain`/`Plan` pending E09-S02/S03/S04),
  `?` toggles a help overlay, `q`/`Esc` quits. Terminal capability detection
  (`NO_COLOR`/`TERM`/`COLORTERM` color tiers, `LANG`/`LC_ALL`/`LC_CTYPE` Unicode-vs-ASCII
  box-drawing, plus a `CANCELLAI_TUI_ASCII` escape hatch) degrades gracefully on any missing or
  unrecognized signal, and a too-small terminal renders a message instead of panicking. Built on
  `ratatui`/`crossterm` (outer-ring dependencies named for this epic in
  [ADR-0019](docs/adrs/0019-dependency-rings-per-crate.md)); the crate depends on no
  `cancellai-*` crate at all yet - no provider/filesystem access is possible from it by
  construction, and `cancellai-policy`'s engine query API is reintroduced only once E09-S02 has
  real view data to render.
- Added the Machine and project atlas screen to `cancellai-tui` (E09-S02, CR1, observational
  only): total footprint and an estimated-reclaimable subset shown as two visually distinct
  values (never blended), per-provider and per-project breakdowns with an explicit
  "Unattributed" bucket, and the individually largest contributors. An incomplete or unknown
  provider scan is flagged prominently rather than hidden inside the totals it could be
  undercounting. New `cancellai_policy::atlas::summarize` computes the summary from
  already-classified inventory (reusing the exact reclaimability test `plan`/`clean` already
  apply); `cancellai-tui` reintroduces `cancellai-policy` as its only new dependency to consume
  it - still no provider adapter or filesystem crate. Live-scan wiring into the running binary
  remains deferred; the screen shows an explicit "not loaded yet" state until that follow-up
  lands.
- Added the Artifact explain view to `cancellai-tui` (E09-S03, CR1, observational only): a
  selectable list of artifacts with, for the selected one, why it exists (project attribution and
  structural relationships), classification, evidence, risk, reversibility, allowed authority,
  and the concrete policy outcome. A destructive policy outcome always shows the real,
  human-readable reason `cancellai_policy::retention::build_actions` already produces for it -
  new `cancellai_policy::explain::explain` surfaces that reason rather than inventing a second
  explanation mechanism. Any confidence weaker than fully verified (an artifact's own or its
  project attribution's, independently) is flagged with a `[low confidence]` marker in the text
  itself, not only by color. Fixed a real rendering bug this story's own tests caught along the
  way: a long policy reason or provider summary line could be silently clipped instead of
  wrapping - both the Explain and Atlas detail panels now wrap.
- Added the Plan review workflow to `cancellai-tui` (E09-S04, CR3, SI-016): the Plan screen
  reviews the same selected artifact the Explain screen shows and requires stronger
  confirmation for irreversible actions than for others - one keypress confirms a
  non-irreversible recommendation, an irreversible one needs a second, and changing the
  selection or leaving the screen cancels any pending or completed confirmation. This is a
  review-only workflow: `cancellai-tui` still depends on neither `cancellai-safety` nor
  `cancellai-platform`, so nothing in it can construct a plan or execute a mutation - the
  confirmed state names `cancellai-cli clean` as the real, separate execution path rather than
  claiming to execute anything itself. `docs/architecture/TARGET.md` and
  `docs/security/SAFETY_INVARIANTS.md` (SI-016) record this scope decision explicitly. A
  self-review found and this same story's own fix closed a real input-handling gap before
  release: the confirmation guard matched `c` regardless of modifiers, so a real terminal's
  Ctrl+C (delivered as `Char('c')` + `CONTROL` once raw mode disables `ISIG`) could complete a
  pending irreversible confirmation instead of cancelling it - now a shared `is_plain_c` check
  makes any modified `c`, Ctrl+C included, always cancel and never arm/confirm.

## [1.12.0] - 2026-09-11

### Added

- Defined the canonical release artifact manifest contract (E17-S01):
  `project/schemas/release_manifest.schema.json` is a machine-verifiable, versioned document
  naming every distributed binary exactly once (`name`/`target_triple`/`sha256`), alongside
  `channel`, `source_sha`, `build_identity`, and `knowledge_compatibility`.
  `scripts/release_manifest.py check` validates it against a golden fixture corpus
  (`tests/fixtures/release_manifest/golden/`) and runs in `pre-commit` and CI. Generating a
  manifest from a real multi-platform build is E17-S02/E17-S03 scope.
- Automated the tier-1 cross-platform release build (E17-S02,
  [ADR-0021](docs/adrs/0021-hand-rolled-release-build-matrix.md)): `.github/workflows/release.yml`
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
  [ADR-0022](docs/adrs/0022-cyclonedx-sbom-via-cargo-cyclonedx.md)): `build-artifacts`
  generates a per-target CycloneDX 1.5 SBOM (`cargo-cyclonedx`, target-accurate - reflects
  each leg's own conditional dependencies rather than the host toolchain's) and signs a
  build-provenance attestation and an SBOM attestation for each canonical archive
  (`actions/attest-build-provenance`, `actions/attest` with `sbom-path` - not the deprecated
  `actions/attest-sbom`). A new `attestation-verify` job independently re-fetches and
  cryptographically verifies both attestations for every archive before `publish`, which now
  depends on it: a missing or invalid attestation fails the release rather than shipping
  silently.
- Bound release channel to maximum default authority (E17-S05, CR4, SI-030,
  [ADR-0023](docs/adrs/0023-release-channel-authority-as-opt-in-function.md)):
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
  [ADR-0024](docs/adrs/0024-ed25519-dalek-for-knowledge-bundle-signatures.md),
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

## [1.11.0] - 2026-09-08

### Fixed

- `.github/workflows/release.yml`'s `verify` job now checks out full git history
  (`fetch-depth: 0`) instead of GitHub Actions' default shallow checkout, so
  `scripts/check_platforms.py check`'s ancestor validation
  (`git merge-base --is-ancestor <verified_commit> HEAD`) can find each platform's
  `verified_commit` object at all. A shallow checkout made an older `verified_commit`
  entirely absent from the tagged commit's local history (not merely unreachable), which
  failed the `v1.10.0` tag's release workflow after the tag had already been pushed - the
  GitHub release was never published (`gh run 34252829459`). `scripts/check_workflows.py`
  now fails statically if the checkout step in the job running that provenance gate reverts
  to anything but `fetch-depth: 0` (E23-S01).

## [1.10.0] - 2026-09-08

### Added

- `cancellai-cli`'s target-engine Rust core now observes real Windows file/volume identity
  (`GetFileInformationByHandle`, via a new `windows-sys`-backed capability in
  `cancellai-sealedfs`) instead of reporting every Windows path as identity-`Unsupported`
  (E20-S01, [ADR-0020](docs/adrs/0020-windows-native-identity-via-windows-sys.md)). A
  Windows reparse point (symlink, junction, or any other reparse tag) is classified from its
  own attributes and never treated as, or compared using, Unix symlink semantics; the
  `cancellai-safety` filesystem/volume-boundary check (SI-018) now enforces a genuine Windows
  volume boundary instead of refusing unconditionally. As a direct consequence,
  `cancellai-inventory`'s scanner can now descend below a scope root on Windows (previously an
  accepted limitation, E20-S04). Allocated-size reporting, `cancellai-sealedfs::SealedRoot`'s
  no-follow root-establishment walk, Windows process observation, and real Windows file
  deletion were deliberately out of this story's scope and remained unimplemented at the time
  (see the E20-S05 entry below, which closes them).
- `cancellai-platform` now detects a WSL2 runtime environment explicitly (rather than treating
  it as generic Linux) and classifies a path's filesystem context as native Linux, a
  Windows-drive mount (`drvfs`, e.g. `/mnt/c`), or an unrecognized `Other` mount (E20-S02).
  Library-level only for now - no CLI surface yet, and no change to mutation/quarantine
  authority (the existing Unix device-identity boundary check already refuses crossing into a
  `/mnt/c`-style mount, since it is genuinely a different device).
- `docs/PLATFORMS.md` is now generated (`scripts/check_platforms.py`, from
  `project/platforms.json`) rather than hand-authored aspiration: each platform's tier is
  cross-validated against `.github/workflows/rust.yml`'s real CI matrix and against named test
  functions this script confirms actually exist, so a platform cannot be declared tier 1
  without CI and verified destructive-mutation evidence (E20-S03). Per that real bar, macOS,
  Linux, and (since E20-S05) Windows are tier 1; WSL2 (no dedicated CI runner) is tier 2.
- `cancellai-cli`'s target-engine Rust core now implements the remaining Windows capabilities
  E20-S01 scoped out (E20-S05): real running-process observation
  (`CreateToolhelp32Snapshot`, replacing an unconditional "incomplete" result), real
  allocated-size reporting (`GetFileInformationByHandleEx(FileStandardInfo)`, distinct from
  logical size), a verified no-follow, handle-relative root-establishment walk in
  `cancellai-sealedfs` for Windows (`NtCreateFile` via `OBJECT_ATTRIBUTES.RootDirectory`, the
  Windows analogue of `openat`, used by `configure` and by `clean`'s default-root
  re-verification), and real, identity-confirmed Windows file deletion
  (`FILE_DISPOSITION_INFO`/`SetFileInformationByHandle`, following the same open/re-check/
  handle-relative-delete/post-delete-corroboration shape ADR-0017/E21-S07 established on Unix,
  with the atomic-rename step using the native `NtSetInformationFile` rather than the Win32
  `SetFileInformationByHandle` wrapper - the latter does not reliably honor a non-null
  `RootDirectory` for handle-relative rename). Confirmed on real Windows CI (ADR-0020's own
  dated investigation record); `docs/PLATFORMS.md` now records Windows mutation as `verified`
  and the platform as tier 1, alongside macOS and Linux.

## [1.9.0] - 2026-09-04

### Note on how E22 was verified

Round 1 of independent adversarial review (Codex) returned `FAIL` for five of the six stories
(`project/evidence/E22-VERIFIER-REVIEW.md`): six independently reproduced `release.yml` drift
regressions the static checker missed, a real Codex retention semantic divergence (a stale
subagent-tree root beside a recent child was still an individual delete candidate), a golden
CLI snapshot gap, and a docs section a prior story had silently removed. Every finding was
repaired and re-verified, including real GitHub Actions/CodeQL/Dependabot evidence gathered
after pushing the repair commit (`project/evidence/E22-S0{1,2,3,4,5}/ROUND2-REPAIR.md`). The
epic was then closed by owner decision without spending the second independent review round
ADR-0014 permits - the same pattern v1.8.0's E21 closure used - so these repairs carry no
independent re-confirmation beyond the executor's own re-run of every gate the round-1 review
specified; `project/evidence/RELEASE-v1.9.0.md` records the residual risk this accepts.

### Added

- `cancellai-cli` (the beta target-engine CLI) now has a real `--help`/`-h`/`--version`
  surface and per-command help (`cancellai-cli clean --help`, etc.), matching the reference
  CLI's own top-level surface (E22-S03, `CR-TE-07`). Argument parsing moved from a hand-rolled
  loop in `main.rs` to `clap` ([ADR-0019](docs/adrs/0019-dependency-rings-per-crate.md)).

### Changed

- `cancellai-cli` now refuses a flag irrelevant to the chosen command (e.g. `status --dry-run`,
  `clean --claude-retention`) with exit code 2, instead of silently accepting and ignoring it
  as every command's flags did before this release. `--help`/`-h`/`--version` are an explicit
  exception: wherever they appear, they still short-circuit remaining validation and exit
  before any command runs, matching `clap`'s own precedence and common CLI convention (`git`,
  `cargo`) - see `docs/CLI_RUST.md`'s "Argument parsing" section.

### Fixed

- **A Codex subagent tree with a stale root and a recently-touched child is no longer an
  individual delete candidate for the stale member** (E22-S04). `cancellai-policy::retention`
  gated `--keep-latest` pinning on the tree's effective (max-of-members) mtime but evaluated
  each member's own staleness independently, so a tree the reference protects in full - any
  recent member protects the whole tree, not just the pinning rail - could still surface the
  old-looking member as a `Delete` action in the target engine. `resolve_codex` now applies the
  same tree-level cutoff gate `cancellai.py::choose_codex_old_sessions` does before classifying
  any member's staleness.

### Documentation

- Recorded that `cancellai-cli clean` deletes Codex sessions at the filesystem level only,
  even when the installed `codex` CLI advertises its own `--force`-capable delete: this is now
  a stated, permanent divergence from `cancellai.py` (which prefers the vendor command) rather
  than an unstated gap (E22-S05, `CR-TE-10`). See `docs/CLI_RUST.md`'s "Known gaps" for why -
  wiring it would add a second mutation primitive to the safety kernel and is deferred to a
  dedicated future story, not a side effect of this one.

## [1.8.0] - 2026-09-03

### Fixed

- **A directory the scan could not read no longer authorizes deletion** (E21-S03, `CR-TE-01`).
  On an ordinary tree - one directory without read permission - `cancellai-cli` deleted an
  eligible artifact and exited `0` while reporting the scope complete and its knowledge
  verified; `cancellai.py` withheld every destructive action for that tool and exited `4`. Both
  provider adapters discarded every failure to observe part of the tree with a bare
  `else { continue }`. They now record each one with a cause and a path, withhold the whole
  tool, and degrade `knowledge_confidence` for every artifact in the scope (SI-008, SI-009,
  SI-010, C-02). The Claude side was broader than previously recorded: E06-S02 had repaired only
  the companion-payload branch, and an unreadable **project** directory still passed silently,
  disclosed nowhere.
- `scan_completeness[].error_count` reports the real number of unobservable paths instead of
  `u32::from(!complete)`, which was only ever `0` or `1` (`CR-TE-10`).
- **The delete path prevents the path-swap race instead of detecting it** (E21-S07, `CR-TE-05`).
  The unlink is issued through `cancellai-sealedfs`'s handle-relative `unlinkat` against a
  directory descriptor opened once with `O_NOFOLLOW` at every component, so a rename or
  symlink-swap after validation cannot redirect it. Consequence, intended and user-visible: a
  provider root reached through a symlinked path component can no longer be cleaned - the rule
  E07-S09 already set for root establishment, now holding at the moment of mutation.
  `MutationOperation`'s two unconfirmed, unreachable variants (`Quarantine`,
  `DeleteDirectoryTree`) were removed rather than left armed for E12 to inherit (`CR-TE-11`).
- Rollout metadata reading honours the 512 KiB bound it documents instead of loading the whole
  transcript (E21-S06, `CR-TE-04`). Measured on a single 287 MB rollout: peak RSS **2.9 MB**,
  against 303 MB before - and below the Python reference's own 27.7 MB.

### Added

- Two fixtures the corpus never had - `codex-partial-tree` and `claude-partial-project` - both
  `NORMATIVE` and running through the differential gate in both root-origin scenarios (E21-S02).
  They were written to **fail** against the unrepaired engine, and did; the failing run is
  committed in their evidence packet. `scripts/check_fixtures.py` now refuses an undeclared
  category asymmetry between the two reference providers, because the corpus carrying
  `partial_tree` for Claude and not for Codex is what let the gate stay green while the engine
  deleted (`CR-TE-03`).
- Scope completeness is a shared *type* obligation on every provider adapter
  ([ADR-0018](docs/adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md),
  E21-S04). `ProviderResolution` hands out planning candidates only through a value carrying the
  scope's completeness, with a `compile_fail` regression proving the bare-candidates route is
  unreachable; `scripts/check_rust_workspace.py` fails if `cancellai-cli` stops being able to
  reach `cancellai-inventory` at all, which is how `CR-TE-02` went unnoticed.
- A performance gate on the discovery path the CLI actually executes (E21-S05). Every timing
  assertion is paired with an assertion on what the resolution produced, so a benchmark
  measuring an empty tree fails instead of reporting an excellent number.

### Note on how E21 was verified

Round 1 of independent adversarial review (Codex) returned `FAIL` for five of the seven stories
and reproduced a real escape the first implementation had left open: an unreadable Claude
`projects/` root was converted into a clean empty scan, so `clean --yes` exited `0` where the
frozen reference exits `4`. The completeness was computed correctly in discovery and discarded
one layer up - the same class of defect this epic exists to close. Every finding is repaired and
pinned by a regression written against the verifier's own reproduction; repairing them surfaced
one more instance of the same pattern (`Path::exists()` collapsing "not installed" into "not
readable"), also closed. The epic was closed by owner decision without spending the second
review round, which means these repairs carry no independent confirmation -
`project/evidence/E21-CLOSURE.md` records that and the residual risk it accepts.

- An independent target-engine review is committed as
  [`docs/audits/2026-09-03-CODE_REVIEW.md`](docs/audits/2026-09-03-CODE_REVIEW.md), with
  thirteen findings (`CR-TE-01`..`CR-TE-13`) converted into story contracts under two new
  epics rather than left as prose: **E21 Target Engine Trust Remediation** and **E22
  Engineering System Hardening**. They are new epics, not additions to E06, because E06 has
  already used both independent review rounds ADR-0014 permits.
- [ADR-0018](docs/adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md):
  scope completeness becomes a shared *type* obligation on every provider adapter, while the
  adapters keep their layout-specific traversal. `cancellai-inventory`'s completeness model is
  currently unreachable from the shipped binary, which is why the defect its own reviewer
  rejected in E04-S03 reappeared in the adapters that replaced it.
- [ADR-0019](docs/adrs/0019-dependency-rings-per-crate.md): the safety kernel stays
  dependency-free except by dedicated ADR; the experience and persistence crates may use
  mature, licence-checked libraries. This is what will give `cancellai-cli` the `--help`/
  `--version` surface it currently lacks entirely.

### Fixed

- `project/roadmap.json` declared `current_phase: "P0"` while both P0 epics were `done` and P1
  stood one epic from closing, so `PROJECT_STATUS.md` generated a phase the project had already
  left. Corrected to `P1` (`CR-TE-12`).

### Note

No user-visible behavior changed in this entry. `cancellai.py` remains the sole canonical,
shipping engine; the findings concern the beta Rust engine and the gates around it. `E06-S04`
now records `E21` and `E22-S01` among its blockers, so this file does not read, by omission, as
though the cutover checklist were unchanged.

## [1.7.0] - 2026-09-02

### Added

- E06-S01: `cancellai-cli` gains its first real command surface -
  `status`/`inspect`/`plan`/`clean`/`configure`/`version` against the Rust engine
  (`docs/CLI_RUST.md`). `status` is the read-only default (no subcommand or flag ever implies
  `clean`); `clean` is the only mutating command, gated by `--dry-run`/`--yes`/interactive
  confirmation and routed exclusively through `cancellai-safety`'s single mutation boundary.
  This is a beta command surface, not yet the canonical engine (`docs/development/
  MIGRATION_PYTHON_RUST.md`) - `cancellai.py` remains the shipping reference until E06 closes.
- E06-S02: a differential parity gate (`scripts/rust_python_parity.py`) runs the Python
  reference and the Rust CLI over the full `NORMATIVE` fixture corpus, comparing which
  sessions each engine would delete. Wired into pre-commit/CI. Building it surfaced and fixed
  two real E06-S01 defects: an incomplete companion-payload scan only withheld the one
  affected session instead of the whole tool (SI-008/SI-009), and a Claude home with no
  `projects/` directory was misreported as an incomplete scan instead of legitimately empty.
- E06-S03: documents and proves the beta side-by-side model for `cancellai-cli` -
  `version` identifies the engine, and `cancellai`/`cancellai-cli` share no install path or
  local state (`docs/RELEASING.md`, `docs/development/MIGRATION_PYTHON_RUST.md`), so rollback
  during beta is simply not invoking the Rust binary. Proven with new smoke tests
  (`rust/crates/cancellai-cli/tests/install_rollback.rs`): every read-only command, and even a
  real `clean`, touches nothing under `$HOME` outside the provider artifacts explicitly
  targeted.
- E06-S04: records the Rust cutover gate checklist (`docs/development/RELEASE_GATES.md` "Rust
  cutover gate status") and its current verdict - **not ready**; `cancellai.py` remains the
  sole canonical, shipping engine. No user-visible behavior changed in this entry; it exists so
  this file does not read, by omission, as though cutover had happened.
- E07-S07: `cancellai-cli clean`/`configure` refuse a default-named root
  (`$HOME/.claude`/`$HOME/.codex`, no override) that is itself a symlink/reparse point,
  independently re-checked immediately before establishing the root or writing configuration -
  not only at classification time (`docs/architecture/PLATFORM_MODEL.md` "Default-root
  authority never rests on a lexical name alone"). Closes an E06 verifier review round 2
  finding: authority previously followed the lexical `$HOME/.claude` name alone, so a symlinked
  default root was still treated as mutation-eligible.
- E07-S07 (round 2): closes an E07-S07 round-1 independent verifier review finding - `configure`'s
  own re-check above narrowed but did not close its TOCTOU: a default root swapped to a symlink
  *after* that check and before the raw path-based settings write reached outside the approved
  root. `configure` now routes every read/write through a new `cancellai-sealedfs` crate
  (`docs/adrs/0017-sealed-root-handle-for-configuration-writes.md`): the root is opened exactly
  once with `O_NOFOLLOW` and retained, with every following operation issued via
  `openat`/`renameat` against that descriptor rather than the original path, closing the race by
  construction. **Behavior change**: `configure` now refuses outright (rather than attempting an
  unprotected write) on every platform without a verified no-follow/handle-relative
  implementation - today, every non-Unix platform - matching `clean`'s existing fail-closed
  posture there.
- E07-S09: closes an E07-S07 round-2 independent verifier review finding - round-1's
  `O_NOFOLLOW` bound only `configure`'s final root component, so a *default* root reached
  through an intermediate symlink (e.g. `$HOME` itself being a link, with a real, non-symlink
  leaf directory underneath it) was still silently followed and written through
  (`docs/architecture/PLATFORM_MODEL.md` "Intermediate components need the same no-follow
  treatment as the leaf"). `cancellai-sealedfs::SealedRoot::establish` now walks every path
  component handle-relatively from the filesystem root, refusing the moment any component -
  intermediate or final - is a symlink/reparse point, and creating only the final absent
  component via `mkdirat` against an already-held parent descriptor. E07-S09's own round-1
  independent verifier review found this closure reached only `configure`: `clean` establishes
  its root through the separate `ApprovedRoot` capability, whose `canonicalize()` step still
  silently resolved through the identical intermediate link, so `clean --yes` could still purge
  a stale session reachable only through a symlinked `$HOME` (`docs/architecture/
  PLATFORM_MODEL.md` "The fix had to reach `clean`, not only `configure`"). Round 2 exports a
  read-only counterpart, `verify_no_intermediate_links`, used by `establish_verified_root`
  before `ApprovedRoot::establish` for the default root. The owner-authorized combined closure
  review found one further race in that handoff: a component could be swapped after the walk
  but before canonicalization. The walk now returns a retained final-directory handle, and
  cleanup refuses unless the subsequently established root has the same device/inode identity.
- E07-S08: `scripts/rust_python_parity.py`'s divergence allow-list is now structured
  (fixture/scenario/field-scoped, citation content-checked) rather than free-text, and its
  comparison surface grew from six to eight fields covering every discovered identity record,
  protection coverage, and root authority for every `NORMATIVE` fixture - closing an E06
  verifier review round 2 finding where any real, accepted ADR citation could suppress an
  unrelated divergence regardless of what it actually authorized
  (`docs/development/MIGRATION_PYTHON_RUST.md` M6).
- E07-S05: closes the intermittent Linux CI failure of `cancellai-platform`'s
  `identity::tests::toctou_file_deleted_and_recreated_with_identical_content_still_changes_
  identity` and `mutation::tests::confirmed_delete_rejects_a_target_already_swapped_before_open`.
  Reproduced natively in a real Linux container (not hypothesized): a zero-delay
  delete-and-recreate reuses the freed inode in ~98% of iterations and lands within the same
  ~1ms mtime clock tick, so `device`+`inode`+`kind`+whole-second-`modified` alone cannot always
  distinguish the two objects - a real `IdentityToken` gap, not only a fixture one.
  `IdentityToken::Unix` gains `modified_nanos` (the raw `st_mtime_nsec` sub-second remainder,
  not derivable from the shared whole-second `Timestamp` clock/retention type);
  `cancellai-platform::mutation`'s `confirmed_delete_file_inner` - which compared device+inode
  only, bypassing `IdentityToken` entirely - now also compares it at both its open-time and
  immediately-before-unlink checks (SI-013/SI-017). The two fixtures also had an
  over-specific/false-on-Linux assertion ("recreation must allocate a new inode") removed and
  gained a small real-world-realistic delay in place of an unrealistic zero-delay recreate,
  without weakening the byte-identical-content case either test verifies. Verified with 60
  consecutive passing runs (30 iterations of both tests) in a real Linux container - exceeding
  the story's own 20-consecutive-run bar.

### Fixed

- E20-S04 (formerly E07-S06): identified why `cancellai-inventory`'s
  `completeness::tests::ac1_a_fully_readable_tree_is_complete` and
  `scan::tests::ac1_one_traversal_visits_every_directory_exactly_once` fail on real Windows CI -
  `scan::walk_directory` only recurses into a child whose identity is *confirmed*
  (`IdentityObservation::Identity`, never `Unsupported`, per SI-017), and
  `SystemIdentityObserver` reports `Unsupported` unconditionally on Windows (E03-S01's
  pre-existing residual), so a real Windows scan currently visits only the scope root -
  correct, safety-driven behavior, not a traversal bug; weakening the identity-confirmed gate
  to make the old assertions pass would have been the wrong fix. Both tests gated `#[cfg(unix)]`
  with `#[cfg(windows)]` counterparts added asserting the actual current behavior
  (`Partial`/`directories_visited == 1`), and `docs/architecture/PLATFORM_MODEL.md` gains an
  "Accepted limitation" subsection - real Windows traversal depth requires E20-S01's native
  identity implementation.
- `cancellai-inventory/tests/performance_micro.rs`'s
  `scan_scope_completes_within_budget_for_a_small_dataset` had the identical E20-S04
  Windows-traversal assumption (an exact `paths_observed` count only reachable with confirmed
  identity) - gated that specific assertion `#[cfg(unix)]`; the time-budget and
  views-do-not-re-walk checks it also makes remain meaningful and run on every platform.
- `cancellai-safety`'s `mutation_executor`/`root_capability`/`sealed_plan` had 19 tests
  (`mutation_executor`'s entire test module, plus `root_capability::tests::bind_a_plain_child_
  succeeds`/`bind_the_root_itself_is_rejected`/`bind_a_path_outside_the_root_is_rejected`, plus
  `sealed_plan::tests::seal_derives_root_and_artifact_identity_from_real_capabilities`) that
  construct a real `ApprovedRoot` via the real `SystemIdentityObserver` and were not
  `#[cfg(unix)]`-gated - real Windows CI failed every one of them with the same
  `CandidateIdentityUnsupported` error (E03-S01's pre-existing residual, unrelated to and
  predating this session). `mutation_executor`'s entire `mod tests` is now `#[cfg(unix)]`
  (every test in it depended on the same real-root helper); the three `root_capability` tests
  and the one `sealed_plan` test are individually gated.
- `cancellai-cli/tests/cli_behavior.rs`'s
  `configure_writes_the_native_claude_retention_setting_and_preserves_other_keys` was not
  `#[cfg(unix)]`-gated, so real Windows CI ran it expecting a successful write - but
  `configure`'s write capability (`SealedRoot`) has no verified handle-relative implementation
  on non-Unix platforms and fails closed there by design (`docs/CLI_RUST.md`'s own "Known
  gaps", unrelated to this session's other changes). Gated the success-path test `#[cfg(unix)]`
  and added a `#[cfg(windows)]` counterpart asserting the disclosed refusal instead, matching
  the existing pattern for the symlinked-`$HOME` configure/clean tests.
- `cancellai-sealedfs` failed to build on Windows: `validate_child_name` and its `CString`
  import lived outside the `#[cfg(unix)]` boundary, so they became genuine dead code once
  `unix_impl` (the only caller) stopped compiling in on non-Unix targets - found on real
  Windows CI while verifying E07-S09, not caught locally since this executor's environment is
  macOS. Both are now `#[cfg(unix)]`-gated with the rest of the module they belong to.
- `cancellai-provider-codex::native_delete`'s `FakeCli`-based tests
  (`ac2_a_fake_cli_advertising_force_is_reported_supported` and three others) intermittently
  failed on Linux CI with `ProbeFailed { reason: "Text file busy (os error 26)" }` - reproduced
  directly in a real Linux container (not hypothesized): writing a fresh script then executing
  it from a highly parallel `cargo test` run can race a *different*, concurrently-forking test
  thread on the process's own shared file-descriptor table. Confirmed serial-safe (0/200
  failures at `--test-threads=1`) and only observed under real concurrency, so the fix is a
  bounded retry-on-`ProbeFailed` in the test harness itself (`codex_delete_supported_retrying`)
  rather than any change to `codex_delete_supported`'s production logic - `ProbeFailed` is
  already the correct, conservative production answer for "could not tell". Verified with 60
  consecutive passing runs at `--test-threads=8` in the same container (previously ~14% flake
  rate).
- `directory_size`/`safe_lstat_size` no longer count a symlink's own `lstat().st_size` toward
  a reported byte total. For a symlink that value is the byte length of the stored target path
  string, not real disk footprint - reporting it as "size" made coverage/size output for any
  entry containing a symlink depend on the absolute path length of wherever the symlink
  happened to live, silently differing by machine and even by which temp-directory prefix a
  test run used. Found via the `codex-symlink-escape`/`claude-symlink-protected-name`
  characterization fixtures diverging between macOS and Linux CI; a symlink already contributes
  nothing to deletion or discovery accounting elsewhere (E00-S02 / ADR-0013) and now
  consistently contributes nothing to size accounting either.

## [1.6.0] - 2026-08-31

### Changed

- Epic E05 implemented the Provider API and Reference Adapters: a nine-capability
  `ProviderCapabilities` contract (`cancellai-provider-api`) where capability absence is
  explicit and never inferred from provider identity, and every response carries evidence and
  confidence by construction; Built-in Verified/Community Verified/Local Custom/Untrusted
  provider trust wired into the Effective Authority lattice as its own constraint, gated by
  `TrustedTier`, an opaque type whose only public constructors are the safe `Untrusted` default
  and a checked, evidence-requiring promotion - closing the `ProviderTrustAuthority` gap
  `docs/architecture/DOMAIN_MODEL.md` had called out since E03-S04; Claude Code and Codex CLI
  reference adapters porting `cancellai.py`'s discovery/classification/session-relationship
  logic to Rust (root fingerprinting, the Unicode-canonical-caseless protected-name barrier,
  session/subagent-graph discovery, native-delete capability detection), each checked against
  the committed Python characterization corpus by reproducing its fixtures directly; and a
  generated, per-capability reference-provider compatibility matrix in `docs/PROVIDERS.md`. An
  independent review round found and this epic's own repair cycle closed a CR4 defect before
  close: the first version of provider trust typed its authority-lattice input as a bare,
  publicly-constructible enum, so an external caller could self-assign the highest trust tier
  with no promotion evidence at all - the exact self-assignment SI-021 prohibits. `cancellai.py`'s
  own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface
  yet.

## [1.5.0] - 2026-08-29

### Changed

- Epic E04 implemented the Single-Pass Inventory Engine: `FileFacts`, a per-path evidence record (`rust/crates/cancellai-inventory`) composed from independently-observed logical size, allocated/physical size (a new `AllocationObserver` platform seam distinguishing sparse/cloned/compressed allocation from logical length), identity, and filesystem-boundary facts, with every unsupported metric an explicit typed value rather than a fabricated zero or borrowed metric; `scan_scope`, a single recursive walk per scope whose status/top-consumers/planning report views are pure reads over one snapshot, never a re-walk, and which never follows a symlink or descends across a device/filesystem boundary (SI-018); scope-level completeness classification (`Complete`/`Partial`/`Unknown` with named permission/I/O/disappearance/unsupported-feature reasons, SI-008/SI-009) that a planning-facing view cannot be obtained without, enforced by construction and by a `compile_fail` regression proving the bare-candidates accessor is unreachable outside the crate; and a performance baseline (a CI microbenchmark plus scheduled 10k/100k/1M-entry benchmarks with a machine-readable trend artifact). An independent review round found and this epic's own repair cycle closed a CR3 defect before close: a `read_dir`-listed entry's unreadable/vanished observation was silently dropped instead of degrading scope completeness, and the bare planning-candidates accessor was reachable without the completeness it should always carry. `cancellai.py`'s own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface yet.

## [1.4.0] - 2026-08-29

### Changed

- Epic E03 implemented the Formal Safety Kernel: cross-platform artifact identity tokens with fail-closed Windows refusal (`rust/crates/cancellai-platform`, SI-013/SI-017); an immutable `SealedPlan` sealed only from a verified root/target capability pair, with fail-closed identity/root revalidation (SI-013/SI-016); typed `ApprovedRoot`/`BoundedPath` root and filesystem-boundary capabilities rejecting root-self deletion, escapes, symlink-escape tricks, and cross-device mounts (SI-002/SI-003/SI-018); a monotonic-minimum Effective Authority lattice with a deterministic explanation trace, collapsing unknown/active/protected/partial state to non-destructive authority (SI-001/SI-007/SI-008/SI-009); and the mutation executor itself - the sole, statically-enforced path to real filesystem deletion, checking root binding, authority, and reversibility before mutating, and confirming a plain file's identity via an open file descriptor immediately around the delete syscall (SI-013/SI-019/SI-020). An independent review round found and this epic's own repair cycle closed three CR4 defects before close: a raw mutation capability bypassing every one of the above checks, a plan executable against a target from a different root, and an executor that never consulted its own recorded authority/reversibility. `cancellai.py`'s own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface yet.

## [1.3.0] - 2026-08-29

### Changed

- Epic E02 bootstrapped the target Rust workspace ahead of the spec-first migration: twelve crates (`docs/architecture/TARGET.md`) with an acyclic dependency graph and no provider-specific code in `cancellai-model`/`cancellai-safety`; a quality baseline enforcing `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, and `cargo deny` (license allow-list, unknown-registry/git denial, MSRV 1.85.0) across macOS/Linux/Windows CI (`rust/deny.toml`, ADR-0015); a typed diagnostic model separating invalid-input/safety-block/incomplete-inventory/compatibility/mutation-failure/internal-fault with stable human/JSON error codes; and deterministic `Clock`/`FsObserver` seams (`rust/crates/cancellai-platform`) that keep the Python reference's absent-vs-unreadable filesystem distinction (SI-008/SI-009/SI-010) as a typed contract, including for a modification time the OS cannot report or represent - never silently substituted with a credible-looking epoch timestamp. `cancellai.py`'s own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface yet.

## [1.2.0] - 2026-08-28

### Changed

- Epic E01 turned the Python v1 CLI into a characterized, versioned executable reference ahead of the Rust migration: canonical domain vocabulary (`docs/architecture/DOMAIN_MODEL.md`), a synthetic Claude/Codex provider-layout fixture corpus (`tests/fixtures/`), versioned inventory/plan/explanation/result JSON contracts with an explicit compatibility policy (`docs/architecture/JSON_CONTRACTS.md`), a committed characterization of Python's actual behavior on that corpus classified normative/intentional-divergence/legacy-only/known-defect (`scripts/characterize.py`), and a differential comparison contract and self-testing harness for the eventual Python-vs-Rust migration gate (`scripts/diff_harness.py`, `docs/development/VERIFICATION_STRATEGY.md`). `cancellai.py`'s own runtime behavior is unchanged.
- `cancellai.py` is now maintenance-only (the Python reference freeze, `AGENTS.md`): only parity fixes against the committed characterization, safety/security fixes, and migration-support tooling are accepted going forward, not merely until this epic closed. New product capability targets the Rust implementation.

## [1.1.0] - 2026-08-28

### Security

- Protected names (`CLAUDE_PROTECTED_NAMES` / `CODEX_PROTECTED_NAMES`) are now an executable barrier instead of documentation. They are enforced when the plan is built and again inside `safe_remove`, immediately before any deletion, so a future discovery change cannot silently invalidate them. Comparison uses the Unicode canonical caseless form (NFD, casefold, NFD): APFS is case-insensitive and stores decomposed filenames, so neither raw string equality nor case folding alone is filename comparison (E00-S01).
- `--aggressive` no longer bypasses the age cutoff for Claude legacy directories and rebuildable cache files. It widens which categories are eligible; retention is applied independently (E00-S03).
- Only the provider's own default directory is mutated. A root relocated with `CODEX_HOME` or `CLAUDE_CONFIG_DIR` is fully inspectable but is never deleted from or written to: nothing observable in a filesystem proves a directory belongs to a provider, so this release refuses to act on structural resemblance. Two weaker schemes were tried and rejected by independent review before this one (E00-S02, ADR-0013 superseding ADR-0012).
- The protected-name barrier is applied to the path as written as well as after resolution, and matches case-insensitively. Previously a protected entry that was itself a symlink lost its protection entirely, and a candidate spelled `Plugins` bypassed the barrier on case-insensitive APFS (E00-S01).
- An unusable process observation is no longer read as "no provider is running". `ps` output that does not contain this process is not a full listing, so a missing, failing, filtered or stubbed enumeration refuses cleanup unless `--allow-running` is given (E00-S09).
- `history.jsonl` is never rewritten through a symlink. `os.replace` would have swapped the link for a regular file and silently detached whatever it pointed at (E00-S06).
- Filesystem observation errors are no longer silently flattened into zero. Every discovery guard goes through an `lstat` that separates "not there" from "could not look" - `Path.exists()` answers False for both, so using it as a guard turned an unreadable directory into an empty one. An unreadable path now withholds destructive authority for that provider, and `status` lists the unreadable paths and prints partial totals as lower bounds (E00-S05).
- Claude `history.jsonl` trimming now streams bytes instead of loading and re-encoding the file, so retained lines - including CRLF endings and a missing trailing newline - are preserved verbatim. It re-identifies the source immediately before the atomic replace and abandons the rewrite if a provider wrote concurrently. Trimming is skipped entirely while a Claude process is running, even under `--allow-running`, and a failed trim is reported instead of looking like "nothing to do" (E00-S06).

### Changed

- **Breaking:** flags without a subcommand no longer normalize to `clean`. `cancellai --days 14` now runs the read-only `status` view; deletion requires typing `clean`. An unrecognized verb is a usage error (E00-S04).
- **Breaking:** a relocated `$CODEX_HOME` / `$CLAUDE_CONFIG_DIR` can no longer be cleaned or configured, only inspected. This is a capability regression, taken deliberately: see ADR-0013. Default roots are unaffected.
- **Breaking:** `clean` exits `3` on mutation failure (previously `2`) and `4` when safety blocked or deferred the requested work. No failure path escapes the taxonomy: an unexpected bug also reports `3` rather than Python's exit code `1`, which automation cannot distinguish from a declined prompt. Exit `2` is now reserved for invalid usage and refused configuration roots. `--json` output carries `exit_code`, `blocked_tools` and `deferred` (E00-S04).
- `status` reads each provider root in a single pass instead of traversing it for the total and again for the largest entries.
- `status --json` and `clean --json` now report per-root `origin`, `confidence`, provider `markers` and `destructive_allowed`, plus a `scan` object and `withheld_tools`.

### Added

- `status --coverage` classifies every top-level provider entry as `selective`, `selective-aggressive`, `aggressive-only`, `trimmed`, `protected`, `reported` or `unknown`, with a legend. There is deliberately no state meaning "deleted as it stands", because no top-level entry is treated that way: `projects/` and `sessions/` are containers whose *contents* are selected by age and policy, and `history.jsonl` is trimmed rather than deleted. Unknown entries are reported so provider layout drift stays visible and are never cleanup candidates. The same classification is exposed in `status --json` (E00-S08).

### Changed

- Added the cancellAI Engineering Operating System (cEOS): product constitution, decision register, target architecture, threat model, safety invariants, evidence-gated development model, Claude/Codex executor-verifier protocol, and machine-readable roadmap/backlog control plane.
- Reframed the long-term product from a macOS Claude/Codex cleanup script to a local-first, cross-platform, provider-agnostic Agent State Control Plane while clearly separating that target from the currently released Python v1 feature set.
- Documented the spec-first Python-to-Rust migration and the P0 trust-floor work that must land before the reference implementation is frozen.
- Required status-check names in branch protection are now verified against the contexts the workflows can actually report. A required check named `test` was blocking every pull request permanently while a matrix produced `test (3.10)` and `test (3.14)`; a name that matches no job never reports and is indistinguishable from a slow check.
- Added governance/document integrity automation, story-specific executor/verifier briefs, CodeQL scanning, CODEOWNERS, incident response, synthetic-fixture policy, and supply-chain-aware CI foundations.
- Bumped the pinned `pytest` development dependency to 9.0.3, closing a Dependabot advisory about vulnerable tmpdir handling. Development tooling only; the shipped tool has no runtime dependencies.
- Replaced automatic Dependabot merge behavior with review-gated dependency updates and pinned first-party GitHub Actions to immutable revisions in active workflows.


## [1.0.2] - 2026-08-27

### Fixed

- `CODEX_PROTECTED_NAMES` now includes `plugins`, matching
  `CLAUDE_PROTECTED_NAMES`. Found by dogfooding against a real `~/.codex`:
  `plugins/` holds genuine installed-plugin state (`plugins/cache`,
  `plugins/.plugin-appserver`), not disposable cache. No code path sweeps
  it today, so this is a defense-in-depth fix, not a behavior change.

## [1.0.1] - 2026-08-27

### Added

- `AGENTS.md` / `CLAUDE.md`: repo-specific instructions for AI coding agents.
- `.github/CONTRIBUTING.md`, `.github/SECURITY.md`, `.github/CODE_OF_CONDUCT.md`,
  issue and pull request templates, and an issue template chooser that
  disables blank issues.
- `docs/ARCHITECTURE.md` and `docs/RELEASING.md`.
- `docs/CLI.md`: a command reference generated directly from the argparse
  definitions by the new `scripts/gen_docs.py`, checked for drift in CI.
- `pyproject.toml` dev-tooling config (`ruff`, `mypy` in strict mode) and a
  matching `.pre-commit-config.yaml`.
- `.editorconfig` and `.github/dependabot.yml` (GitHub Actions ecosystem).
- `.github/workflows/dependabot-auto-merge.yml`: auto-merges Dependabot PRs
  once the required `test`/`lint`/`homebrew` checks pass.
- CI now also runs `ruff check`, `ruff format --check`, `mypy --strict`, and
  the docs-drift check, in addition to the existing test suite.
- Repository hardening: branch protection on `main` (required status
  checks, no force-push/deletion), squash-only merges, Dependabot
  vulnerability alerts + security updates + automated fixes, private
  vulnerability reporting, and repo topics/description for discoverability.

### Changed

- Reorganized repository layout: `test_cancellai.py` moved to
  `tests/test_cancellai.py`; `CONTRIBUTING.md`, `SECURITY.md`, and
  `CODE_OF_CONDUCT.md` moved to `.github/` (a location GitHub recognizes
  natively for these files), decluttering the repo root.
- Modernized type hints to PEP 604 syntax (`X | None` instead of
  `Optional[X]`) and moved `Iterator`/`Sequence` imports to
  `collections.abc`.
- `active_processes()` now resolves `ps` to an absolute path via
  `shutil.which` instead of relying on `$PATH` resolution at call time.
- Replaced an internal `assert` in `delete_codex_via_cli` with an explicit
  `ValueError` guard (assertions can be optimized away with `python -O`;
  this is a real invariant, not a debug check).
- Simplified several `try`/`except ...: pass` blocks to
  `contextlib.suppress(...)`.
- `cancellai.py` is now tracked as executable in git (it has a shebang).

### Fixed

- The `tests` CI job never installed `pytest`, so it failed on every run
  since it was added; every CI job now also invokes tools via
  `python3 -m <tool>` so the installer and the invocation always share the
  same interpreter.
- `.gitignore` now excludes the local `.claude/` session directory so it
  can never end up tracked by accident.

## [1.0.0] - 2026-08-27

Initial public release.

### Added

- Safe cleanup CLI for old Codex CLI and Claude Code session data:
  `status` (read-only, default), `clean` (with dry-run, confirmation
  prompt, age cutoff, and keep-latest safety rail), and `configure` (sets
  Claude Code's own `cleanupPeriodDays`).
- Conservative-by-default safety model: protected name lists for
  auth/config/plugins/skills/memory, symlink-safe deletion, config-root
  validation, running-process detection, and preference for the official
  `codex delete --force` backend over raw filesystem deletion.
- MIT license, README, and a Homebrew formula (`Formula/cancellai.rb`) so
  the tool installs via `brew tap matteo-dritara/cancellai && brew install
  cancellai`.

[Unreleased]: https://github.com/matteo-dritara/homebrew-cancellai/compare/v1.0.2...HEAD
[1.0.2]: https://github.com/matteo-dritara/homebrew-cancellai/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/matteo-dritara/homebrew-cancellai/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/matteo-dritara/homebrew-cancellai/releases/tag/v1.0.0

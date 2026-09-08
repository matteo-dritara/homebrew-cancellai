# ADR-0021: Hand-rolled per-target release build matrix instead of cargo-dist (for now)

- Status: Accepted
- Date: 2026-09-08
- Owners: project owner / cEOS
- Related: PD-017, ADR-0009, ADR-0015, E17-S02

## Context

`docs/security/SUPPLY_CHAIN.md`'s "Release automation" section names `dist`/cargo-dist as the
tool the target Rust release factory should evaluate, and requires the actual tool decision to
go through an ADR because release infrastructure is security-sensitive.

`cargo-dist` is installed and driven via `cargo install cargo-dist` (or a prebuilt installer
script), and its normal workflow is `dist init` / `dist generate`, which writes its own
GitHub Actions workflow from a template this repository would then need to audit and adapt to
coexist with the still-shipping Python/Homebrew release path (`docs/RELEASING.md`'s "Beta
side-by-side" section - `cancellai-cli` is a beta artifact today, built from source, with no
packaged release of its own yet).

**2026-09-08 correction:** this ADR originally claimed the executing environment had no
outbound network access to `crates.io`, inferred solely from `curl https://crates.io` returning
HTTP 403 - `cargo install cargo-dist` itself was never actually attempted before that claim was
written. During E17-S03 (same session), `cargo install cargo-cyclonedx --locked` and, to check
this ADR's own premise directly, `cargo install cargo-dist --locked` were both attempted for
real and both **succeeded** (`cargo-dist v0.32.0` installed cleanly): `cargo`'s registry/index
and crate-download traffic reaches `crates.io` through infrastructure the plain website `curl`
request does not, and the two are not the same reachability question. The "no network access"
premise below is therefore false and is left struck through rather than silently deleted, per
this repository's own norm that a defect gets corrected in the record, not quietly patched over
(see `docs/development/AGENT_PROTOCOL.md`'s Failure cycle). The decision itself - hand-roll the
matrix for E17-S02 rather than adopt cargo-dist immediately - stands, but on the corrected
grounds in "Alternatives considered" below: this was reconfirmed with the project owner
(2026-09-08) as the preferred path specifically because E17-S02 was already implemented,
tested, and committed under the original (mistaken) premise by the time the mistake was found,
and re-deriving it around cargo-dist's generated workflow now would cost materially more of this
epic's remaining budget than finishing E17-S03/S05/S06 does, not because cargo-dist remains
unreachable.

~~The executing environment for this story has no outbound network access to `crates.io`
(`cargo install cargo-dist` fails to resolve the registry), so `cargo-dist`'s generated output
cannot be produced, inspected, or verified locally in this session - only asserted from
documentation, which is not good enough for a supply-chain decision this repository's own
policy calls security-sensitive.~~

E17-S01 already fixed the artifact manifest contract (`project/schemas/release_manifest.schema.json`)
independently of which build tool produces the artifacts it describes, so the build-tool choice
does not block on it, and nothing about the manifest contract needs to change if this decision
is revisited later.

## Decision

E17-S02 implements the tier-1 cross-platform build matrix as hand-written GitHub Actions jobs
in `.github/workflows/release.yml` (`build-artifacts`, `release-manifest-generate`) rather than
adopting cargo-dist now:

- native `cargo build --release --locked -p cancellai-cli --target <triple>` per tier-1 target
  (`docs/PLATFORMS.md`: `aarch64-apple-darwin`, `x86_64-apple-darwin` on `macos-latest`,
  `x86_64-unknown-linux-gnu` on `ubuntu-latest`, `x86_64-pc-windows-msvc` on `windows-latest`);
- a packaged archive (`.tar.gz`/`.zip`) and a SHA-256 checksum sidecar per target;
- an "installer smoke test" step that unpacks the archive and runs the packaged binary's
  `version` command on the same runner that built it, proving the artifact this job produces
  actually executes on that platform, not merely that it compiled;
- a `release-manifest-generate` job that assembles `release-manifest.json` from the real,
  just-built artifacts via `scripts/release_manifest.py generate` (E17-S01's contract), and
  refuses to publish if the assembled manifest fails its own checksum round-trip
  (`release_manifest.py verify-checksums`) against the downloaded artifact bytes.

Every action this adds (`actions/upload-artifact`, `actions/download-artifact`) is pinned to a
real, `gh api`-resolved 40-hex commit SHA, matching `scripts/check_workflows.py`'s existing
immutable-pin policy - this ADR does not relax that policy for a new tool's generated output.

## Alternatives considered

### Adopt cargo-dist now (as of the 2026-09-08 correction, this is genuinely reachable, not blocked)

Rejected for E17-S02 specifically, on schedule/audit-cost grounds rather than access: adopting
it means running `dist init`/`dist generate`, then auditing its entire generated GitHub Actions
workflow line-by-line against this repository's own supply-chain policy
(`scripts/check_workflows.py`: every third-party action pinned to a real, verified 40-hex
commit SHA; least-privilege `permissions:`; no silently-skippable gate) before committing it -
cargo-dist's own workflow pulls in a number of its own actions and steps this repository has
not yet individually verified. That is a materially larger task than the hand-rolled matrix
this ADR ships, which is fully inspectable today with nothing generated to audit. A follow-up
ADR can still adopt cargo-dist once that audit is done against real, reachable output - the two
are not mutually exclusive long-term; cargo-dist can replace this baseline without changing what
`release_manifest.schema.json` describes.

### Wait for E17-S02 until cargo-dist can be evaluated

Rejected: E17-S03 (provenance/SBOM/signing), E17-S04 (installation-source awareness), E17-S05
(channel authority), and E17-S06 (repository topology) all depend on E17-S02, directly or
transitively. Blocking the whole remainder of the epic on a full cargo-dist audit, when an
equivalent (`docs/BACKLOG.md`'s own wording: "a release toolchain such as dist/cargo-dist or
equivalent") satisfies the story's acceptance criteria today, is a worse trade than shipping the
hand-rolled matrix now and revisiting the tool choice later.

## Consequences

### Positive

- unblocks E17-S03 through E17-S06 without a large mid-epic tool-audit detour;
- every line of the build/package/checksum/manifest pipeline is inspectable in this repository
  today, with no generated-and-vendored third-party workflow to audit;
- reuses `release_manifest.schema.json` (E17-S01) unchanged - the manifest contract is decoupled
  from whichever tool eventually produces its artifacts.

### Negative / cost

- no native installer formats yet (`.pkg`, `.msi`, `.deb`) - only archives plus checksums;
  cargo-dist (or an equivalent) would be needed to add those without hand-writing per-platform
  installer generation, which this ADR does not attempt;
- a second, independent implementation of "build the tier-1 matrix" exists if cargo-dist is
  adopted later; the migration is a deletion of this ADR's jobs plus cargo-dist's generated
  replacement, not a merge of the two.

### Neutral / follow-up

- revisit cargo-dist adoption via a follow-up ADR once (a) installer-format needs exceed plain
  archives, or (b) someone runs `dist init`/`dist generate` for real and audits the output
  against this repository's workflow policy (`scripts/check_workflows.py`) - network access is
  no longer the blocker per the 2026-09-08 correction above; the audit itself is.

## Safety and compatibility impact

- Change Risk implication: CR2 (E17-S02) - this is release-process automation, not a runtime
  authority or mutation-path change; it produces artifacts nobody is instructed to install yet
  (`cancellai-cli` stays a beta, source-built artifact per `docs/RELEASING.md` until E06-S04).
- Safety Invariants affected: none directly. `release-manifest-generate`'s checksum-round-trip
  refusal-to-publish is defense in depth for supply-chain integrity, not a safety-kernel
  authority decision.
- Migration/rollback: reverting to no automated cross-platform build is a revert of the two new
  `release.yml` jobs; no state migration exists because no channel currently distributes these
  artifacts.

## Supersession

If cargo-dist (or another tool) replaces this hand-rolled matrix later, keep this ADR and mark
it superseded by the ADR that makes that change.

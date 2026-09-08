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
packaged release of its own yet). The executing environment for this story has no outbound
network access to `crates.io` (`cargo install cargo-dist` fails to resolve the registry), so
`cargo-dist`'s generated output cannot be produced, inspected, or verified locally in this
session - only asserted from documentation, which is not good enough for a supply-chain
decision this repository's own policy calls security-sensitive.

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

### Adopt cargo-dist now

Rejected for this story: its generated workflow cannot be produced or inspected in this
session (no network path to `crates.io`), and a supply-chain workflow this repository has not
been able to read line-by-line is not something to commit under a policy that exists
specifically because "release tooling becomes part of the security boundary" (ADR-0009). A
follow-up ADR can adopt cargo-dist once someone with real network access can generate, review,
and diff its output against this hand-rolled baseline - the two are not mutually exclusive
long-term; cargo-dist can replace this baseline without changing what
`release_manifest.schema.json` describes.

### Wait for E17-S02 until cargo-dist can be evaluated

Rejected: E17-S03 (provenance/SBOM/signing), E17-S04 (installation-source awareness), E17-S05
(channel authority), and E17-S06 (repository topology) all depend on E17-S02, directly or
transitively. Blocking the whole remainder of the epic on one external tool's availability, when
an equivalent (`docs/BACKLOG.md`'s own wording: "a release toolchain such as dist/cargo-dist or
equivalent") satisfies the story's acceptance criteria today, is a worse trade than shipping the
hand-rolled matrix now and revisiting the tool choice later.

## Consequences

### Positive

- unblocks E17-S03 through E17-S06 without waiting on network access this session does not have;
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
  archives, or (b) a session with real `crates.io` network access can generate and review its
  output against this baseline.

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

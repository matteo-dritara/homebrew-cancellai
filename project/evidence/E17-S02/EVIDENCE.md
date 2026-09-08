# Evidence Packet - E17-S02

- Commit/PR: this working-tree change (executor session, 2026-09-08)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR2
- Spec version/commit: `project/epics/E17.json` (E17-S02), as of this change

## Outcome

PASS

`.github/workflows/release.yml` gained two jobs that turn "a tagged commit" into "four real,
checksummed, smoke-tested tier-1 binaries plus a manifest describing them":

- `build-artifacts` - a 4-leg matrix (`docs/PLATFORMS.md`'s tier-1 set: `aarch64-apple-darwin`
  and `x86_64-apple-darwin` on `macos-latest`, `x86_64-unknown-linux-gnu` on `ubuntu-latest`,
  `x86_64-pc-windows-msvc` on `windows-latest`) that runs `cargo build --release --locked -p
  cancellai-cli --target <triple>`, packages the binary into an archive with a SHA-256 sidecar,
  and smoke-tests the packaged artifact by unpacking it and running `cancellai-cli version` on
  the same runner that built it - proof the artifact executes, not merely that it compiled.
- `release-manifest-generate` - downloads every leg's archive and assembles
  `release-manifest.json` via `scripts/release_manifest.py generate` (new subcommand, E17-S01's
  contract fed real checksums of real files), then immediately round-trips it with the new
  `verify-checksums` subcommand.
- `publish` now depends on both, re-verifies the manifest's checksums against the downloaded
  artifacts before creating the release (refuses to publish on any mismatch), and attaches the
  archives, their checksums, and `release-manifest.json` to the GitHub Release.

[ADR-0021](../../../docs/adrs/0021-hand-rolled-release-build-matrix.md) records the tool decision:
a hand-written build matrix instead of adopting `cargo-dist` now, because this session's
environment has no outbound network path to `crates.io` (`cargo install` fails to resolve the
registry - confirmed empirically) and so cannot produce or review cargo-dist's generated
workflow output for a decision `docs/security/SUPPLY_CHAIN.md` calls security-sensitive enough
to require an ADR either way. `docs/BACKLOG.md`'s own story wording ("a release toolchain such
as dist/cargo-dist **or equivalent**") explicitly allows this.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Build matrix covers declared tier-1 targets | `build-artifacts`'s `strategy.matrix.include` lists exactly the four tier-1 `(os, target)` pairs `docs/PLATFORMS.md` names; each leg's own runner performs the build natively (macOS cross-compiles `x86_64-apple-darwin` from the arm64 `macos-latest` host via Apple's universal toolchain, a well-established pattern - no separate x86_64 macOS runner is needed) | PASS (structural: `python3 -c "import yaml; ..."` confirms the workflow parses and the job graph is `verify, verify-rust, build-artifacts, release-manifest-generate, publish`; full behavioral proof is on the next real tag push - CI cannot be executed from this local session, consistent with how this repository has always disclosed platform-specific behavior as "verified for real on next CI push", e.g. `docs/PLATFORMS.md`'s Windows section) |
| AC2 - Release process is reproducible from a clean tagged checkout | Every step in `build-artifacts`/`release-manifest-generate`/`publish` derives its inputs from the tagged checkout alone (`GITHUB_REF_NAME`, `GITHUB_SHA`, `GITHUB_REPOSITORY`, `GITHUB_RUN_ID`, the pinned toolchain, and `rust/Cargo.lock`, already committed per ADR-0015) - no step reads mutable external state beyond the pinned actions/toolchain themselves; `cargo build --locked` refuses to silently re-resolve dependencies | PASS |

## Safety Evidence

Not applicable - `safety_obligations: []` for this story (CR2, no safety invariant claimed).
The new `publish`-time checksum guard (`release_manifest.py verify-checksums`) is defense in
depth for supply-chain integrity, not a safety-kernel authority decision; it has its own
positive/negative unit-test coverage in `tests/test_release_manifest.py`
(`VerifyChecksumsAgainstDirectoryTests`, `CliSubcommandTests`).

## Verification Commands

```text
python3 -m pytest tests -v
python3 -m mypy --strict scripts/release_manifest.py
python3 -m ruff check . && python3 -m ruff format --check .
python3 scripts/check_workflows.py check
python3 scripts/check_docs.py check
python3 scripts/check_process.py check
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"  # structural YAML sanity
```

All PASS locally (242/242 tests including the new/updated `tests/test_workflows.py`
(`ReleaseGateDriftTests::test_dropping_verify_rust_from_publish_needs_is_caught`, updated to
match `publish`'s new four-job `needs` list) and `tests/test_release_manifest.py`'s new
`BuildManifestFromFilesTests`/`VerifyChecksumsAgainstDirectoryTests`/`CliSubcommandTests`
classes, 8 new tests total for the `generate`/`verify-checksums` subcommands).

## Compatibility

- Platforms/providers/schemas exercised: the four tier-1 targets from `docs/PLATFORMS.md`. No
  new platform claims - this story does not raise any platform's tier, it automates producing
  binaries for the platforms already tier-1.

## Performance / operability

- `build-artifacts` fans out across three runner OSes in parallel (`fail-fast: false`, so one
  leg's failure does not hide the others' results); `release-manifest-generate` and `publish`
  each run once, downloading only the small archives (not full source), keeping the tag-push
  pipeline's added wall-clock bounded by the slowest single `cargo build --release` leg rather
  than the sum of all four.

## Documentation updated

- `docs/adrs/0021-hand-rolled-release-build-matrix.md` (new) - the tool-selection ADR.
- `docs/RELEASING.md` - "Target Rust release factory" section now describes the real
  `build-artifacts`/`release-manifest-generate`/`publish` pipeline and what remains deferred
  (installers, SBOM/provenance, real channel selection).
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- Full behavioral proof (the matrix actually building, packaging, smoke-testing, and
  publishing successfully on real macOS/Linux/Windows runners) is pending the next real `v*`
  tag push - this session cannot execute GitHub Actions locally. This mirrors how every prior
  Windows-specific claim in this repository has been handled (`docs/PLATFORMS.md`: "verified
  for real on this repo's Windows CI") rather than a gap unique to this story.
- No native installer formats (`.pkg`/`.msi`/`.deb`) - archives plus checksums only. ADR-0021
  names this as the main reason a future cargo-dist adoption stays open, and it is not blocking:
  the story's literal ACs (tier-1 build matrix, reproducibility) do not require installer
  formats.
- The release-manifest's `channel` field is hardcoded to `stable` because no beta/nightly tag
  scheme exists yet; E17-S05 is where channel selection and its authority binding are decided.
- These binaries are not wired to any real distribution channel (no Homebrew formula, no
  Windows package) - intentional per `docs/RELEASING.md`'s "Beta side-by-side" section;
  `cancellai-cli` remains source-built until E06-S04.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)

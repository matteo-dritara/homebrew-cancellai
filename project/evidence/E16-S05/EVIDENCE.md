# Evidence Packet - E16-S05

- Commit/PR: this working-tree change (executor session, 2026-09-08)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR2
- Spec version/commit: `project/epics/E16.json` (E16-S05), as of this change

## Outcome

PASS

`rust/crates/cancellai-provider-api/manifests/opencode.json` is the first real
[`ProviderManifest`](../E16-S01/EVIDENCE.md) instance, exposed via
`cancellai-provider-api::opencode_manifest()` (`src/builtin.rs`, `include_str!`-compiled).

**Its layout is not assumed - it was researched against the real tool's source during this
story**, via `gh api` against `anomalyco/opencode` (the actual repository behind `sst/opencode`,
confirmed by a redirect; ~206k stars, matches this repository's own
`docs/research/MARKET_AND_STANDARDS_2026-08.md` citation):

- `packages/core/src/global.ts` - confirms the four XDG-based roots (`data`, `cache`, `config`,
  `state`) and that `config` alone has an app-specific override, `Flag.OPENCODE_CONFIG_DIR`.
- `packages/opencode/src/cli/cmd/uninstall.ts` - confirms `Global.Path.{data,cache,config,state}`
  are exactly the four directories the tool itself considers its own removable footprint.
- `packages/opencode/src/storage/storage.ts` - confirms the storage root is
  `path.join(Global.Path.data, "storage")` and its **current** (post-migration) on-disk shape:
  `project/<id>.json`, `session/<projectID>/<sessionID>.json`, `message/<sessionID>/<messageID>.json`,
  `part/<messageID>/<partID>.json`, `session_diff/<sessionID>.json`, plus a `migration` marker
  file this manifest deliberately does not match (internal version state, not a session
  artifact). The file's own `MIGRATIONS` array documents an *older*, now-obsolete per-project
  layout (`storage/session/info/*.json` etc.) this manifest correctly does not target.
- `packages/opencode/src/auth/index.ts` - confirms `auth.json` at the data root holds
  OAuth/API credentials (`path.join(Global.Path.data, "auth.json")`).
- `packages/web/src/content/docs/config.mdx` - confirms the global config file name/location
  (`~/.config/opencode/opencode.json`, JSON or JSONC) and precedence order, and that
  `.opencode/` project-local directories are a *separate*, per-project concern this manifest
  does not model (cancellAI's existing Claude/Codex adapters are also scoped to the global home
  directory, not per-project state - consistent precedent, not a new decision this story makes).

This is the same evidentiary bar `docs/architecture/PROVIDER_MODEL.md` already sets for
Claude/Codex's own root markers (real, cited layout knowledge, not guessed from the tool's
name) - AGENTS.md's "evidence before action" principle applied to a manifest, not only to code.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Unknown layouts remain inspection-only | `builtin::tests::ac_unknown_layouts_remain_inspection_only` drives `capability_report` against an empty directory and asserts every one of the nine capabilities is `Unsupported` - not merely the three the manifest schema can answer; `ac_a_realistic_layout_still_never_reports_capabilities_beyond_the_manifest_only_ceiling` proves the *reverse* is also true (even a fully-recognized, realistic layout still never exceeds the manifest-only ceiling for `SESSION_GRAPH`/`PROJECT_ATTRIBUTION`/etc.) | PASS |
| AC2 - Community contribution path exercises same trust pipeline | This manifest ships compiled-in (`include_str!`) but through the *identical* `parse_manifest`/`ManifestProvider`/`TrustedTier` machinery E16-S01 built - no code in `builtin.rs` bypasses or shortcuts it (documented explicitly in that module's own doc comment); `opencode_manifest()`'s `ManifestProvider` defaults to `TrustedTier::untrusted()` (E16-S01's own default, not overridden here) - a real community-contributed manifest for a different tool would go through the same construction with no separate, more-trusted path available to it | PASS |

## Safety Evidence

Not applicable in the CR3/CR4 sense - `safety_obligations: []`, CR2. The relevant care taken:
this story adds real, filesystem-facing classification logic (session vs. protected
categorization) for a specific third-party tool, so `credentials_are_classified_protected_not_session`
directly proves `auth.json` - the one file in this layout whose loss is materially worse than
"the tool starts fresh" - is declared `Protected`, not `Session`, in the committed manifest
itself (a regression guard on the manifest *data*, not only the schema/engine code).

## Verification Commands

```text
cd rust
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
cd ..
python3 scripts/check_rust_workspace.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/check_provider_compatibility.py check
python3 scripts/rust_python_parity.py self-test
python3 scripts/rust_python_parity.py check
python3 scripts/check_platforms.py check
python3 scripts/check_docs.py check
# research citations (re-run to reproduce the exact evidence this packet cites):
gh api repos/anomalyco/opencode --jq '{full_name, stargazers_count}'
gh api repos/anomalyco/opencode/contents/packages/core/src/global.ts -H "Accept: application/vnd.github.raw"
gh api repos/anomalyco/opencode/contents/packages/opencode/src/storage/storage.ts -H "Accept: application/vnd.github.raw"
gh api repos/anomalyco/opencode/contents/packages/opencode/src/auth/index.ts -H "Accept: application/vnd.github.raw"
gh api repos/anomalyco/opencode/contents/packages/web/src/content/docs/config.mdx -H "Accept: application/vnd.github.raw"
```

All PASS locally. `cargo test -p cancellai-provider-api`: 66/66 (8 new in `builtin.rs`). Full
workspace `cargo test --workspace`: every crate green.

## Compatibility

- No existing public API changed; this story adds a new module (`builtin.rs`) and a new data
  file (`manifests/opencode.json`), both purely additive.

## Documentation updated

- `docs/PROVIDERS.md` - new "OpenCode (manifest-only, E16-S05)" subsection under "Tier 2
  ecosystem providers", naming the exact source files this manifest's layout was confirmed
  against and disclosing this schema version's scope gaps (config/cache declared but not
  scanned; no `state` root; `XDG_CONFIG_HOME` not modeled alongside `OPENCODE_CONFIG_DIR`).
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- **`config`/`cache` roots are declared for `DISCOVERY` only, not scanned for artifacts.**
  Their contents (user configuration, cached binaries) are lower-value cleanup targets than
  session data and were judged out of this story's minimum-viable scope; a future story can add
  artifact patterns for them once there is a concrete reason to (e.g. a `cache/bin` retention
  policy).
- **`XDG_CONFIG_HOME` is not modeled for the `config` root** - only the app-specific
  `OPENCODE_CONFIG_DIR` override is, because `ManifestRoot` (E16-S01) supports one environment
  variable per root and `OPENCODE_CONFIG_DIR` is the more direct analogue to `CLAUDE_CONFIG_DIR`/
  `CODEX_HOME`. A machine with `XDG_CONFIG_HOME` set but not `OPENCODE_CONFIG_DIR` would see
  this manifest resolve `config` to `~/.config/opencode` (the common, unset-XDG-vars case)
  rather than the real `$XDG_CONFIG_HOME/opencode` - disclosed, and irrelevant to the `data`
  root the artifact patterns actually target.
- **No `state` root is declared** (`$XDG_STATE_HOME/opencode`, used by the real tool for lock
  files per `global.ts`) - no marker file specific enough to fingerprint with real confidence
  was found during this story's research; an undeclared root is judged more honest than one
  with an empty marker table that could never reach usable confidence.
- **Not wired into `cancellai-cli`'s command surface** - `status`/`plan`/`clean` do not yet
  recognize or scan OpenCode; this story proves the manifest and engine work correctly against
  a real layout in isolation, not that a user can invoke this today. Wiring a manifest-driven
  provider into the CLI's provider list is a distinct, larger surface-area change than either
  this story's or E16-S01's own ACs require.
- **No genuine OpenCode installation was available to test against** - the realistic fixture
  tree this story's tests build was constructed from the source-code evidence above, not
  captured from a real running instance. This is the same class of residual
  `docs/architecture/PROVIDER_MODEL.md`'s "Tested compatibility matrix" already discloses for
  any layout claim not backed by a live install (`docs/PROVIDERS.md`'s own header: "real
  per-version/layout compatibility evidence beyond the two reference adapters' own current
  output remains future P1/P2 work").

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)

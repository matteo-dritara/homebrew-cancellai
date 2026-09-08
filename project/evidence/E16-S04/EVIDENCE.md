# Evidence Packet - E16-S04

- Commit/PR: this working-tree change (executor session, 2026-09-09)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR2
- Spec version/commit: `project/epics/E16.json` (E16-S04), as of this change

## Outcome

PASS

`rust/crates/cancellai-provider-api/manifests/github-copilot-cli.json` is the third real
[`ProviderManifest`](../E16-S01/EVIDENCE.md) instance on the same "Manifest-only" engine
[E16-S05](../E16-S05/EVIDENCE.md) first proved out. Exposed via
`cancellai-provider-api::github_copilot_manifest()` (`src/builtin.rs`, `include_str!`-compiled).

**Its layout is not assumed - it was researched against GitHub's own published documentation
during this story**: GitHub Copilot CLI is closed-source, so unlike E16-S05's/E16-S03's
source-code citations, the authoritative evidence here is
`docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference` -
`AGENTS.md`'s "evidence before action" bar applied with the kind of evidence actually available
for a closed-source tool, not a lower bar:

- Default config directory `~/.copilot`, overridable in full by `COPILOT_HOME` - the same
  full-path-override shape this crate's `cancellai-provider-claude`/`cancellai-provider-codex`
  adapters already use for `CLAUDE_CONFIG_DIR`/`CODEX_HOME`, not a new resolution rule.
- `config.json` (authentication/plugin metadata) and `settings.json` (user configuration) at the
  root - both credential/config state, `Protected`.
- `session-state/<sessionID>/events.jsonl` - per-session event logs, the cleanup surface,
  `Session`.
- A separate, platform-conditional cache directory (`COPILOT_CACHE_HOME`) documented alongside
  the config directory - deliberately not modeled as a root in this schema version (see
  Residual risks).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - SQLite/internal state is not mutated without documented native capability | This is a manifest-only integration: `ManifestProvider` has no mutation path of any kind - `native_delete_capability` and every other capability besides `DETECT`/`FINGERPRINT_ROOT`/`INVENTORY_MAP` report `Unsupported` unconditionally (`MANIFEST_ONLY_UNSUPPORTED`, enforced in `manifest_provider.rs`, not per-manifest). There is no internal/SQLite state in this tool's documented layout to begin with (config/settings are JSON, session state is JSONL) - the AC holds vacuously and by construction, not by restraint that could later lapse | PASS |
| AC2 - Project/session mapping remains evidence-based | `a_realistic_copilot_home_is_detected_and_its_session_events_inventoried` proves inventory only ever reports files the manifest's own declared `session-state/*/events.jsonl` glob matches against a realistic fixture tree - no inference beyond that literal pattern; `copilot_config_json_is_classified_protected_not_session` proves `config.json` is never miscategorized as session data | PASS |

## Safety Evidence

Not applicable in the CR3/CR4 sense - `safety_obligations: []`, CR2. The relevant care taken:
`copilot_config_json_is_classified_protected_not_session` directly proves the credential file in
this layout is declared `Protected` in the committed manifest data itself, mirroring E16-S05's
`credentials_are_classified_protected_not_session` and E16-S03's
`gemini_oauth_credentials_are_classified_protected` regression guards.

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
# research citation (re-run to reproduce the exact evidence this packet cites):
# https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference
```

All PASS locally. `cargo test -p cancellai-provider-api`: 78/78 total (includes both E16-S03's
and E16-S04's new tests). Full workspace `cargo test --workspace`: every crate green.

## Compatibility

- No existing public API changed; this story adds one new data file
  (`manifests/github-copilot-cli.json`) and one new exported function
  (`github_copilot_manifest`), both purely additive.

## Documentation updated

- `docs/PROVIDERS.md` - new "GitHub Copilot CLI (manifest-only, E16-S04)" subsection under
  "Tier 2 ecosystem providers", naming the exact documentation source this manifest's layout was
  confirmed against and disclosing this schema version's scope gap.
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- **Cache directory (`COPILOT_CACHE_HOME`) not modeled** - `ManifestRoot` (E16-S01) supports one
  environment variable and one default-relative-to-home path per root, with no per-platform
  branching; GitHub's documentation gives this directory a different default path per operating
  system, which this schema version cannot express faithfully as a single root. An undeclared
  root was judged more honest than one whose default is only correct on some platforms, and a
  cache directory carries no session-cleanup value that would justify the added engine
  complexity now.
- **Evidence source is documentation, not source code** - Copilot CLI being closed-source means
  this manifest's layout claims rest on GitHub's published reference page rather than a citable
  source file; disclosed as a difference in evidence kind, consistent with how
  `docs/architecture/PROVIDER_MODEL.md` already distinguishes evidence tiers by what is actually
  available per provider.
- **No genuine GitHub Copilot CLI installation was available to test against** - same disclosed
  residual class as E16-S05/E16-S03; the realistic fixture tree was built from the documented
  layout above, not captured from a live install.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)

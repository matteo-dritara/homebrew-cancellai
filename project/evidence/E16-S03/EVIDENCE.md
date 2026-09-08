# Evidence Packet - E16-S03

- Commit/PR: this working-tree change (executor session, 2026-09-09)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR2
- Spec version/commit: `project/epics/E16.json` (E16-S03), as of this change

## Outcome

PASS

`rust/crates/cancellai-provider-api/manifests/gemini-cli.json` is the second real
[`ProviderManifest`](../E16-S01/EVIDENCE.md) instance, built on the same "Manifest-only" engine
[E16-S05](../E16-S05/EVIDENCE.md) first proved out - not a new adapter code path. Exposed via
`cancellai-provider-api::gemini_manifest()` (`src/builtin.rs`, `include_str!`-compiled).

**Its layout is not assumed - it was researched against the real tool's source during this
story**, against `google-gemini/gemini-cli`:

- `packages/core/src/utils/paths.ts` - confirms `GEMINI_CLI_HOME` replaces the *home* concept
  wholesale (`homedir()` itself is substitutable via the override) rather than naming this
  provider's root directly, with `.gemini` joined on unconditionally afterward - the
  `default_relative_to_home: ""` + `subdir: ".gemini"` shape E16-S01's own manifest schema had
  to be extended to express (see that story's evidence for the validation change).
- `packages/core/src/config/storage.ts` - confirms `oauth_creds.json`, `google_accounts.json`,
  `settings.json`, `trustedFolders.json`, `projects.json` at the root, all credential/config
  state whose loss is worse than "the tool starts fresh" - `Protected`, not `Session`.
- `chatRecordingService.ts` - confirms the real, current filename construction for chat
  recordings under `tmp/<projectIdentifier>/chats/`, the basis for this manifest's two
  `SESSION` glob patterns (`tmp/*/chats/*.jsonl` and the nested `tmp/*/chats/*/*.jsonl` shape).
- `docs/cli/session-management.md` - documents Gemini CLI's own built-in session retention
  policy (`settings.json`'s `general.sessionRetention`: `enabled`/`maxAge`/`maxCount`), cited
  verbatim in this manifest's new `vendor_notes` field.

Same evidentiary bar as E16-S05: real, cited layout knowledge, not guessed from the tool's name.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Adapter starts at truthful capability subset | `ac_unknown_layouts_remain_inspection_only`-equivalent coverage: `a_realistic_gemini_home_is_detected_and_its_chats_inventoried` proves `DETECT`/`FINGERPRINT_ROOT`/`INVENTORY_MAP` answer from real evidence against a realistic fixture tree, while `builtin.rs`'s cross-cutting `every_builtin_manifests_provider_defaults_to_untrusted_authority` and the shared `MANIFEST_ONLY_UNSUPPORTED` ceiling (enforced in `manifest_provider.rs`, not per-manifest) prove every other capability, including this manifest, never exceeds `Unsupported` | PASS |
| AC2 - Vendor-native retention is detected/explained where relevant | `gemini_explain_cites_the_vendor_documented_retention_policy` asserts `EXPLAIN`'s evidence text contains the `vendor_notes` content when present, while `EXPLAIN` itself still returns `Unsupported` - the manifest explains the vendor's documented policy without claiming to have detected the user's actual configured value, which stays outside the manifest-only ceiling | PASS |

## Safety Evidence

Not applicable in the CR3/CR4 sense - `safety_obligations: []`, CR2. The relevant care taken:
`gemini_oauth_credentials_are_classified_protected` directly proves the OAuth credential file -
the one artifact in this layout whose loss is materially worse than "the tool starts fresh" - is
declared `Protected` in the committed manifest data itself, mirroring E16-S05's own
`credentials_are_classified_protected_not_session` regression guard.

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
gh api repos/google-gemini/gemini-cli/contents/packages/core/src/utils/paths.ts -H "Accept: application/vnd.github.raw"
gh api repos/google-gemini/gemini-cli/contents/packages/core/src/config/storage.ts -H "Accept: application/vnd.github.raw"
gh api repos/google-gemini/gemini-cli/contents/docs/cli/session-management.md -H "Accept: application/vnd.github.raw"
```

All PASS locally. `cargo test -p cancellai-provider-api`: 78/78 total (includes both E16-S03's
and E16-S04's new tests). Full workspace `cargo test --workspace`: every crate green.

## Compatibility

- No existing public API changed; this story adds one new data file
  (`manifests/gemini-cli.json`), one new exported function (`gemini_manifest`), and one new
  manifest field (`vendor_notes`, `#[serde(default)]` so E16-S01's/E16-S05's already-committed
  manifests parse unchanged without it).

## Documentation updated

- `docs/PROVIDERS.md` - new "Gemini CLI (manifest-only, E16-S03)" subsection under "Tier 2
  ecosystem providers", naming the exact source files this manifest's layout was confirmed
  against and disclosing this schema version's scope gaps.
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- **`tmp/<projectIdentifier>` naming ambiguity** - the tool has used both a legacy path-hash
  form and a newer slug form for this identifier across versions; this manifest's glob matches
  either shape positionally (`tmp/*/chats/*.jsonl`) but does not itself disambiguate which
  naming scheme produced a given directory. Disclosed, not silently assumed uniform.
- **`history` directory not declared** - shell/command history, not chat transcripts; carries no
  session-cleanup value, so declaring it as a root or artifact was judged to add scanning
  surface with no offsetting benefit.
- **`vendor_notes` is free-text, not a machine-checked claim** - it is deliberately excluded
  from any field `parse_manifest` treats as security-relevant (no `deny_unknown_fields`
  interaction, no effect on `SupportState`); a stale or inaccurate note would only make
  `EXPLAIN`'s text less useful, never grant capability or trust it should not have.
- **No genuine Gemini CLI installation was available to test against** - same disclosed
  residual class as E16-S05 (`docs/PROVIDERS.md`'s own header on real per-version compatibility
  evidence remaining future P1/P2 work); the realistic fixture tree was built from the
  source-code evidence above.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)

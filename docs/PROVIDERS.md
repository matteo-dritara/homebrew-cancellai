# Provider Support

Provider support is capability-based. This document describes the intended support tiers.
E05-S05 adds the first generated slice of capability evidence (see "Tested compatibility
matrix" below); real per-version/layout compatibility evidence beyond the two reference
adapters' own current output remains future P1/P2 work.

## Trust levels

| Trust | Meaning | Maximum default authority |
| --- | --- | --- |
| Built-in Verified | Maintainer-owned adapter/manifest with compatibility fixtures and release evidence | As allowed by artifact/policy safety ceilings |
| Community Verified | Community integration promoted after maintainer verification | Govern only where explicitly verified; irreversible authority is opt-in and evidence-gated |
| Local Custom | User-supplied local manifest/adapter configuration | Recommend/Quarantine at most unless explicitly elevated under local policy |
| Untrusted | Discovered or imported knowledge without verification | Observe only |

## Capability vocabulary

Providers may independently expose:

- `DISCOVERY`
- `ROOT_FINGERPRINT`
- `INVENTORY_MAPPING`
- `PROJECT_ATTRIBUTION`
- `SESSION_GRAPH`
- `ACTIVITY_DETECTION`
- `NATIVE_DELETE`
- `QUARANTINE`
- `RESTORE`
- `RETENTION_CONFIG`
- `EXPLAIN`
- `GUARDIAN_SIGNALS`

Absence of a capability is not an error and must never be inferred from the provider name.

`NATIVE_DELETE` is a capability-*detection* claim, not a guarantee that the engine's mutation
path actually uses it. The target-engine Rust CLI's Codex adapter probes and reports
`NATIVE_DELETE`/`native_delete_capability` accurately (see the generated matrix below - all
four `NativeDeleteSupport` outcomes stay distinct, TM-09), but `cancellai-cli clean` does not
yet route deletion through it: every deletion is filesystem-level today, regardless of what
this capability reports. That is a disclosed, permanent divergence from the Python reference
(which prefers the vendor's own `codex delete --force` when available) rather than a silent
gap - see `docs/CLI_RUST.md`'s "Known gaps" (`CR-TE-10`, E22-S05) for why closing it is kernel
mutation-boundary work (SI-019/C-07), not a capability-detection fix.

## Tested compatibility matrix

Generated from the two reference adapters' own `ProviderCapabilities` output
(`rust/crates/cancellai-cli/examples/compatibility_matrix.rs`), not hand-maintained prose -
`Known (default root)` reflects the OS-default provider directory; `Unknown (fail-closed)`
reflects a candidate root with no recognized layout evidence at all (unknown-version/layout
behavior, SI-004). Regenerate with `python3 scripts/check_provider_compatibility.py generate`;
`check` fails if this block has drifted from what the adapters currently produce. Do not edit
the block between the markers by hand.

<!-- BEGIN GENERATED: provider-compatibility-matrix -->

### `claude-code`

| Capability | Known (default root) | Unknown (fail-closed) |
| --- | --- | --- |
| `detect` | `VERIFIED` (verified) | `UNSUPPORTED` (low_unknown) |
| `fingerprint_root` | `VERIFIED` (verified) | `UNSUPPORTED` (low_unknown) |
| `inventory_map` | `SUPPORTED_OBSERVED` (observed) | `SUPPORTED_OBSERVED` (observed) |
| `project_attribution` | `UNSUPPORTED` (low_unknown) | `UNSUPPORTED` (low_unknown) |
| `session_graph` | `UNSUPPORTED` (low_unknown) | `UNSUPPORTED` (low_unknown) |
| `activity_state` | `UNSUPPORTED` (low_unknown) | `UNSUPPORTED` (low_unknown) |
| `native_delete_capability` | `UNSUPPORTED` (verified) | `UNSUPPORTED` (verified) |
| `retention_capability` | `UNSUPPORTED` (low_unknown) | `UNSUPPORTED` (low_unknown) |
| `explain` | `SUPPORTED_OBSERVED` (observed) | `SUPPORTED_OBSERVED` (observed) |

### `codex-cli`

| Capability | Known (default root) | Unknown (fail-closed) |
| --- | --- | --- |
| `detect` | `VERIFIED` (verified) | `UNSUPPORTED` (low_unknown) |
| `fingerprint_root` | `VERIFIED` (verified) | `UNSUPPORTED` (low_unknown) |
| `inventory_map` | `SUPPORTED_OBSERVED` (observed) | `SUPPORTED_OBSERVED` (observed) |
| `project_attribution` | `UNSUPPORTED` (low_unknown) | `UNSUPPORTED` (low_unknown) |
| `session_graph` | `SUPPORTED_OBSERVED` (observed) | `SUPPORTED_OBSERVED` (observed) |
| `activity_state` | `UNSUPPORTED` (low_unknown) | `UNSUPPORTED` (low_unknown) |
| `native_delete_capability` | `ERROR_PARTIAL` (low_unknown) | `ERROR_PARTIAL` (low_unknown) |
| `retention_capability` | `UNSUPPORTED` (verified) | `UNSUPPORTED` (verified) |
| `explain` | `SUPPORTED_OBSERVED` (observed) | `SUPPORTED_OBSERVED` (observed) |

<!-- END GENERATED: provider-compatibility-matrix -->

## Provider sequence

### Reference providers

- Claude Code
- OpenAI Codex

These define the first conformance corpus and differential migration contract.

### Tier 2 ecosystem providers

- Gemini CLI
- GitHub Copilot CLI
- OpenCode

They enter with the truthful minimum capability set and expand only as evidence supports it.

#### OpenCode (manifest-only, E16-S05)

The first real "Manifest-only" integration (`docs/architecture/PROVIDER_MODEL.md`) - a
declarative manifest, not native adapter code, matching this tier's own "truthful minimum
capability set" mandate exactly. `rust/crates/cancellai-provider-api/manifests/opencode.json`
records the layout confirmed directly against `anomalyco/opencode`'s (formerly `sst/opencode`)
real source during this story:

- **data** (`$XDG_DATA_HOME/opencode`, default `~/.local/share/opencode`): `auth.json`
  (credentials - `PROTECTED`, never a cleanup target) and a `storage/` tree of
  `session`/`message`/`part`/`session_diff`/`project` JSON records (`SESSION` - the actual
  cleanup surface, confirmed against `packages/opencode/src/storage/storage.ts`'s current,
  post-migration layout, not the tool's now-obsolete pre-migration per-project layout);
- **config** (`OPENCODE_CONFIG_DIR` override, default `~/.config/opencode`):
  `opencode.json`/`opencode.jsonc` - declared for `DISCOVERY` only, not scanned for artifacts in
  this schema version;
- **cache** (`$XDG_CACHE_HOME/opencode`, default `~/.cache/opencode`): declared for `DISCOVERY`
  only, same reason.

Only `DETECT`/`FINGERPRINT_ROOT`/`INVENTORY_MAP` are ever reported as anything but
`UNSUPPORTED` - the "Manifest-only" ceiling PROVIDER_MODEL.md's own section documents, enforced
structurally by `ManifestProvider`, not merely by this manifest's own restraint. The manifest
ships committed and reviewed in this repository, but that is a loading-mechanism detail, not an
authority grant: its provider still defaults to `TrustedTier::untrusted()` like every other
provider (SI-021) - promotion to a higher trust tier requires the same maintainer-owned
fixtures/compatibility evidence/threat review this document's "Trust levels" table always has,
which this story does not claim to have produced. A community-contributed manifest for a
different tool would go through the identical `parse_manifest`/`ManifestProvider`/`TrustedTier`
pipeline - E16-S05's own AC ("community contribution path exercises same trust pipeline") is
true because there is no separate path here to diverge from it.

**Disclosed v1 gaps**, not silently assumed complete: `config`/`cache` roots are declared but
not scanned for artifacts (their contents are lower-value/lower-confidence for a first pass);
`config`'s resolution models `OPENCODE_CONFIG_DIR` (the app-specific override) but not the more
generic `XDG_CONFIG_HOME` the tool's own default also respects, since this schema version's
`ManifestRoot` supports only one environment variable per root; no `state`
(`$XDG_STATE_HOME/opencode`) root is declared at all, because this story found no marker file
there specific enough to fingerprint with real confidence, and an undeclared root is a more
honest gap than a root with an empty marker table that could never reach usable confidence.

### Later providers

Other local-state agents are considered when they have material developer usage and a storage/lifecycle surface that can be safely observed. Cursor/Roo/Windsurf or future agents are not added simply to inflate a compatibility logo wall.

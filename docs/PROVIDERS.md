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

### Later providers

Other local-state agents are considered when they have material developer usage and a storage/lifecycle surface that can be safely observed. Cursor/Roo/Windsurf or future agents are not added simply to inflate a compatibility logo wall.

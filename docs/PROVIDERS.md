# Provider Support

Provider support is capability-based. This document describes the intended support tiers; exact tested versions and capability evidence will become generated adapter metadata during P1/P2.

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

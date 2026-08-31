# Provider Model

## Principle

A provider is an adapter from vendor-specific state into provider-neutral facts and capabilities. It is not a privileged deletion module.

## Capability contract

The target provider API exposes independent capability results such as:

```text
detect()
fingerprint_root()
inventory_map()
project_attribution()
session_graph()
activity_state()
native_delete_capability()
retention_capability()
explain()
```

Every capability result includes:

- support state;
- provider/version/layout evidence;
- confidence/trust;
- any authority ceiling it implies.

## Support states

Recommended vocabulary:

- `VERIFIED`
- `SUPPORTED_OBSERVED`
- `UNSUPPORTED`
- `UNKNOWN_VERSION`
- `LAYOUT_DRIFT`
- `ERROR/PARTIAL`

A provider can therefore be verified for inventory but unsupported for native delete.

E05-S01 implements this contract at `rust/crates/cancellai-provider-api/src/capability.rs`:
`CapabilityKind` enumerates the nine capabilities above by name (`ALL`, mirroring
`cancellai-model::ErrorCategory::ALL`); `ProviderCapabilities` is the trait every adapter
implements, with a single required method (`capability(kind) -> CapabilityOutcome`) that
takes no provider-identity input beyond `&self` - there is no identity-keyed lookup table
anywhere in the crate that could infer a capability's support from a provider id string, which
is what makes capability absence first-class rather than inferred (this story's AC1).
`CapabilityOutcome` bundles support state, `KnowledgeConfidence`, an authority ceiling, and at
least one evidence note - its only public constructor requires a first evidence string, so
there is no way to construct a response that omits evidence (AC2), the same "invariant
enforced by API shape" pattern `cancellai-safety::SealedPlan` uses. The per-capability result
*payload* (a session graph's actual shape, a project attribution's actual fields) is
deliberately not defined yet - it belongs to the adapter stories that produce real data
(E05-S03 Claude, E05-S04 Codex) and to the inventory/session-graph epics beyond E05, not to
this contract-definition story. `capability_report` runs every `CapabilityKind` against a
provider in a fixed order; it is the reusable half of this story's "mock provider contract
conformance suite" verification, meant to be driven against real adapters once they exist
rather than re-derived per adapter.

## Root fingerprinting

Destructive operation on a [`ProviderRoot`](DOMAIN_MODEL.md#providerroot) requires a credible root fingerprint. A path is not accepted merely because the environment variable names it.

Fingerprint evidence may include:

- known config file(s);
- known session/index directories;
- version metadata;
- recognizable database/header structure;
- CLI-reported config root where available.

A low-confidence custom root is inspection-only.

E05-S03 implements this root-fingerprinting posture for Claude Code at
`rust/crates/cancellai-provider-claude/src/fingerprint.rs`: `fingerprint_claude_root` ports
`cancellai.py`'s `ROOT_MARKERS["claude"]`/`fingerprint_root` marker table and
default/high/low/unknown confidence derivation. Unlike the Python reference, it takes
"is this the default root" as an explicit caller-supplied argument rather than reading
`CLAUDE_CONFIG_DIR`/`HOME` itself, keeping the fingerprinting function pure and
synthetic-filesystem-testable - a documented, narrow improvement in shape, not a behavioral
divergence. `RootConfidence::Unknown` maps to `SupportState::Unsupported` with an
`AuthorityLevel::Observe` ceiling in `ClaudeProvider::capability` - AC3's "unknown layouts
downgrade to inspection-only" (SI-004).

## Three integration levels

### Manifest-only

Declarative root/pattern/category knowledge. Appropriate for discovery/inventory and conservative classification.

### Native adapter

Code for session graphs, activity, project attribution, structured metadata, and richer compatibility checks.

E05-S03 implements the Claude Code native adapter's discovery/classification/session-graph
slice: `cancellai-provider-api::protection` (`canonical_name`/`protected_component`, shared
across adapters) ports `cancellai.py`'s Unicode-canonical-caseless protected-name barrier -
`rust/crates/cancellai-provider-claude/src/protected_names.rs` supplies Claude's own
`CLAUDE_PROTECTED_NAMES` list (settings/keybindings/memory/skills/agents/commands/rules/
workflows/output-styles/plugins), satisfying "memory/settings/plugin protected classes are
explicit artifacts... with evidence" (this story's AC2). `session.rs` ports
`discover_claude_sessions`: Claude's session relationships are a flat project → session
grouping (no subagent tree, unlike Codex's rollout graph, E05-S04's own concern) - a session
whose companion payload directory cannot be fully listed is still reported, marked in
`degraded_companions` rather than silently dropped (SI-008/SI-009's "partial observation is
never treated as absence", applied at this adapter's own layer since `cancellai-inventory`'s
`ScopeCompleteness` is not yet wired to provider-adapter discovery - a documented residual).
`rust/crates/cancellai-provider-claude/tests/claude_fixture_parity.rs` reproduces five of this
corpus's `claude-*` fixtures by hand and asserts the adapter's output against the exact values
the committed Python characterization records for each (AC1).

### Vendor-native integration

Explicit vendor command/API for operations such as delete/retention/restore when semantics are tested.

Vendor-native delete is not automatically safer if provider capability/version evidence is unknown.

## Trust chain

See [`../PROVIDERS.md`](../PROVIDERS.md). Trust is an authority input, not a popularity label.

A community manifest cannot declare itself Built-in Verified. Promotion requires maintainer-owned fixtures, compatibility evidence, threat review, and code ownership approval.

E05-S02 implements the trust tiers themselves and their authority enforcement:
`cancellai_model::ProviderTrust` (`rust/crates/cancellai-model/src/vocabulary.rs`) is the pure
four-tier vocabulary (`Untrusted < LocalCustom < CommunityVerified < BuiltinVerified`), and
`cancellai_safety::authority::effective_authority` (`rust/crates/cancellai-safety/src/authority.rs`)
wires it in as its own named constraint, `provider_trust_authority`, matching this document's
"Trust levels" table exactly: `Untrusted` caps the monotonic-minimum result at `Observe` (so
an untrusted manifest cannot reach even `Quarantine`, let alone an irreversible action -
SI-021), `LocalCustom` at `Quarantine`, `CommunityVerified` at `Govern`, and `BuiltinVerified`
at `Autopilot` (no additional cap from trust alone). `cancellai_safety::trust_promotion::promote`
is the *only* function in the workspace that can raise a `ProviderTrust` tier - it requires a
non-empty named verifier and at least one fixture reference, and refuses any request that is
not a strict upgrade, fail-closed (SI-021, SI-022). Nothing reads a trust tier out of a
manifest's own self-description and treats it as authoritative; the conservative default for
anything not promoted through that gate is `ProviderTrust::Untrusted`.

## Knowledge bundles

Federated knowledge bundles may update:

- known provider versions;
- layout fingerprints;
- artifact pattern metadata;
- compatibility warnings;
- capability-disable rules for known regressions.

They may not:

- execute code;
- define arbitrary shell commands;
- raise authority beyond local trust policy;
- bypass the local binary safety kernel.

Bundles are signed/attested, versioned, rollbackable, and content-addressed.

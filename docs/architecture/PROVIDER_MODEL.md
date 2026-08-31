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

## Three integration levels

### Manifest-only

Declarative root/pattern/category knowledge. Appropriate for discovery/inventory and conservative classification.

### Native adapter

Code for session graphs, activity, project attribution, structured metadata, and richer compatibility checks.

### Vendor-native integration

Explicit vendor command/API for operations such as delete/retention/restore when semantics are tested.

Vendor-native delete is not automatically safer if provider capability/version evidence is unknown.

## Trust chain

See [`../PROVIDERS.md`](../PROVIDERS.md). Trust is an authority input, not a popularity label.

A community manifest cannot declare itself Built-in Verified. Promotion requires maintainer-owned fixtures, compatibility evidence, threat review, and code ownership approval.

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

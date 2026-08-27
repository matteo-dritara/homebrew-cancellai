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

## Root fingerprinting

Destructive operation on a provider root requires a credible root fingerprint. A path is not accepted merely because the environment variable names it.

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

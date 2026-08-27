# Architecture

cancellAI is in a controlled architecture transition.

The current shipping product is the Python v1 reference implementation in `cancellai.py`. The target product is a provider-neutral Rust engine with CLI and TUI clients, followed by policy, quarantine, Guardian, and later remote/fleet capabilities.

Do not confuse **current implementation constraints** with **target architecture decisions**.

## Read in this order

1. [architecture/AS_IS.md](architecture/AS_IS.md) - the shipping v1 pipeline, safety-critical core, data model, exit taxonomy, and which defects are known. Start here for how the current tool actually works.
2. [architecture/TARGET.md](architecture/TARGET.md) - target components and dependency direction.
3. [architecture/DOMAIN_MODEL.md](architecture/DOMAIN_MODEL.md) - core model and authority semantics.
4. [architecture/PROVIDER_MODEL.md](architecture/PROVIDER_MODEL.md) - capability adapters/manifests and trust.
5. [architecture/PLATFORM_MODEL.md](architecture/PLATFORM_MODEL.md) - OS/filesystem abstraction requirements.
6. [architecture/PERSISTENCE_MODEL.md](architecture/PERSISTENCE_MODEL.md) - local state, event ledger, analytics, quarantine.
7. [architecture/POLICY_MODEL.md](architecture/POLICY_MODEL.md) - deterministic policy resolution.
8. [architecture/GUARDIAN_MODEL.md](architecture/GUARDIAN_MODEL.md) - predictive monitoring and bounded remediation.

The migration contract is in [development/MIGRATION_PYTHON_RUST.md](development/MIGRATION_PYTHON_RUST.md).

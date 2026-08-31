//! Provider capability contract and manifest model: `detect`, `fingerprint_root`,
//! `inventory_map`, `project_attribution`, `session_graph`, `activity_state`,
//! `native_delete_capability`, `retention_capability`, `explain` (`docs/architecture/PROVIDER_MODEL.md`).
//!
//! Provider adapters (`cancellai-provider-claude`, `cancellai-provider-codex`) implement
//! this contract; this crate defines it and must not depend on a specific adapter.
//!
//! `capability` (E05-S01) is the first real contract in this crate - the nine-capability
//! [`ProviderCapabilities`] trait and its evidence/confidence-bearing
//! [`CapabilityOutcome`]. The manifest model (declarative root/pattern/category knowledge,
//! PROVIDER_MODEL.md "Manifest-only" integration level) does not exist yet and is deferred to
//! a later E05 story.

pub mod capability;

pub use capability::{
    CapabilityKind, CapabilityOutcome, ProviderCapabilities, SupportState, capability_report,
};

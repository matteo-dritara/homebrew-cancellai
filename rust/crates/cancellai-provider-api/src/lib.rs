//! Provider capability contract and manifest model: `detect`, `fingerprint_root`,
//! `inventory_map`, `project_attribution`, `session_graph`, `activity_state`,
//! `native_delete_capability`, `retention_capability`, `explain` (`docs/architecture/PROVIDER_MODEL.md`).
//!
//! Provider adapters (`cancellai-provider-claude`, `cancellai-provider-codex`) implement
//! this contract; this crate defines it and must not depend on a specific adapter.
//!
//! `capability` (E05-S01) is the first real contract in this crate - the nine-capability
//! [`ProviderCapabilities`] trait and its evidence/confidence-bearing
//! [`CapabilityOutcome`]. `protection`/`root_probe` (E05-S03) and `root_fingerprint` (E05-S04)
//! are tool-agnostic utilities every adapter needs (protected-name comparison, provider-root
//! marker probes, root confidence vocabulary/derivation) ported from `cancellai.py`, kept here
//! rather than duplicated per adapter crate since none carries any provider-specific knowledge
//! of its own. The manifest model (declarative root/pattern/category knowledge,
//! PROVIDER_MODEL.md "Manifest-only" integration level) does not exist yet and is deferred to
//! a later E05 story.

pub mod capability;
pub mod protection;
pub mod root_fingerprint;
pub mod root_probe;

pub use capability::{
    CapabilityKind, CapabilityOutcome, ProviderCapabilities, SupportState, capability_report,
};
pub use protection::{ProtectionOutcome, canonical_name, protected_component};
pub use root_fingerprint::{RootConfidence, RootFingerprint, RootOrigin, derive_root_confidence};
pub use root_probe::{
    MAX_ROOT_PROBE_ENTRIES, contains_uuid_named_jsonl, extract_uuid, is_dir, is_json_object,
    is_jsonl_of_objects, is_nonempty_file,
};

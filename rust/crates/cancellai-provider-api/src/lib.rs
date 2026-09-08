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
//! of its own. `manifest`/`manifest_provider` (E16-S01) add the manifest model PROVIDER_MODEL.md
//! calls the "Manifest-only" integration level: declarative root/pattern/category knowledge - no
//! code, and structurally no way to claim a capability or trust level a hand-written adapter did
//! not also have to earn through the same `cancellai-safety::TrustedTier` gate (SI-021).
//! `builtin` (E16-S05, E16-S03, E16-S04) adds the first real manifest instances - OpenCode,
//! Gemini CLI, GitHub Copilot CLI - the "tier 2 ecosystem providers" `docs/PROVIDERS.md` names,
//! each entering with the truthful minimum capability set.

pub mod builtin;
pub mod capability;
pub mod manifest;
pub mod manifest_provider;
pub mod protection;
pub mod root_fingerprint;
pub mod root_probe;

pub use builtin::{gemini_manifest, github_copilot_manifest, opencode_manifest};
pub use capability::{
    CapabilityKind, CapabilityOutcome, ProviderCapabilities, SupportState, capability_report,
};
pub use manifest::{
    ArtifactCategory, ArtifactPattern, CURRENT_SCHEMA_VERSION, ManifestError, ManifestRoot, Marker,
    MarkerProbe, ProviderManifest, parse_manifest,
};
pub use manifest_provider::{
    ManifestProvider, ResolvedManifestRoot, fingerprint_manifest_root, resolve_root,
};
pub use protection::{ProtectionOutcome, canonical_name, protected_component};
pub use root_fingerprint::{RootConfidence, RootFingerprint, RootOrigin, derive_root_confidence};
pub use root_probe::{
    MAX_ROOT_PROBE_ENTRIES, contains_uuid_named_jsonl, extract_uuid, is_dir, is_json_object,
    is_jsonl_of_objects, is_nonempty_file,
};

//! Provider capability contract and manifest model: `detect`, `fingerprint_root`,
//! `inventory_map`, `project_attribution`, `session_graph`, `activity_state`,
//! `native_delete_capability`, `retention_capability`, `explain` (`docs/architecture/PROVIDER_MODEL.md`).
//!
//! Provider adapters (`cancellai-provider-claude`, `cancellai-provider-codex`) implement
//! this contract; this crate defines it and must not depend on a specific adapter.
//!
//! Skeleton crate (E02-S01) - no types defined yet.

use cancellai_model as _;

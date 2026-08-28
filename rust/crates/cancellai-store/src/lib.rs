//! The current-state SQLite store, event ledger, and analytical rollups
//! (`docs/architecture/PERSISTENCE_MODEL.md`). Never the source of truth for provider
//! state (C-10): disposable and rebuildable, and never destructive truth for mutation
//! preconditions (SI-024).
//!
//! Skeleton crate (E02-S01) - no types defined yet.

use cancellai_model as _;

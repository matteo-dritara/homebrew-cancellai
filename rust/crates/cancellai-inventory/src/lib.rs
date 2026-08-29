//! Filesystem observations and scan completeness: the `Scan`/`observe` completeness
//! channel described in `docs/architecture/AS_IS.md` and `docs/security/SAFETY_INVARIANTS.md`
//! (SI-008, SI-009, SI-010), reimplemented as a first-class typed model rather than a
//! best-effort helper.
//!
//! E04-S01 adds the first real type, [`file_facts::FileFacts`] (and its outer
//! [`file_facts::FactObservation`]): the per-path OBSERVE-stage evidence record composed from
//! `cancellai-platform`'s `FsObserver`/`IdentityObserver`/`AllocationObserver` seams.
//!
//! E04-S02 adds [`scan::scan_scope`]: one recursive walk of a scope root producing one
//! [`scan::InventorySnapshot`], with every report view (`status_summary`, `top_consumers`,
//! `planning_candidates`) a pure read over that same snapshot rather than a fresh walk.
//!
//! E04-S03 adds [`completeness`]: every scope this crate observes is classified `Complete`,
//! `Partial`, or `Unknown` with named reasons (SI-008, SI-009), and [`completeness::PlanningView`]
//! is the only way to hand a caller planning candidates - it always carries completeness
//! alongside them, by construction.

pub mod completeness;
pub mod file_facts;
pub mod scan;

pub use completeness::{
    CompletenessReason, PlanningView, ScopeCompleteness, derive_completeness, planning_view,
};
pub use file_facts::{
    FactConfidence, FactObservation, FileFacts, ScopeBoundary, SizeMetric, observe_file_facts,
};
pub use scan::{DirectoryError, DirectoryErrorKind, InventorySnapshot, StatusSummary, scan_scope};

use cancellai_model as _;

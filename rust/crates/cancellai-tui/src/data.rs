//! The one seam this crate touches `cancellai-policy`'s "Engine / Query API" through (E09-S02).
//! `ui::draw` takes an [`EngineData`] rather than importing `cancellai_policy` types in every
//! screen-drawing function - a future story (E09-S03's explain view, E09-S04's plan review)
//! extends this one struct instead of widening how many places in this crate know the engine
//! crate's shape.

pub use cancellai_policy::AtlasSummary;

/// Everything the shell's screens can render, gathered once by whatever assembles real data.
/// Wiring a live scan into `main.rs` is deferred (see that file's own doc); `Default` (every
/// field `None`) is what a not-yet-loaded shell renders, as an explicit state rather than an
/// empty/zeroed summary standing in for "nothing scanned yet".
#[derive(Debug, Default)]
pub struct EngineData<'a> {
    pub atlas: Option<AtlasSummary<'a>>,
}

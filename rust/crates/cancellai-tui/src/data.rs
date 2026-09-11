//! The one seam this crate touches `cancellai-policy`'s "Engine / Query API" through (E09-S02).
//! `ui::draw` takes an [`EngineData`] rather than importing `cancellai_policy` types in every
//! screen-drawing function - a future story (E09-S04's plan review) extends this one struct
//! instead of widening how many places in this crate know the engine crate's shape.

pub use cancellai_policy::{AtlasSummary, ExplainView};

/// Everything the shell's screens can render, gathered once by whatever assembles real data.
/// Wiring a live scan into `main.rs` is deferred (see that file's own doc); `Default` (every
/// field empty/`None`) is what a not-yet-loaded shell renders, as an explicit state rather than
/// an empty/zeroed summary standing in for "nothing scanned yet".
#[derive(Debug, Default)]
pub struct EngineData<'a> {
    pub atlas: Option<AtlasSummary<'a>>,
    /// One entry per artifact the Explain screen (E09-S03) can select and show. An empty `Vec`
    /// (the `Default`) renders as "nothing to explain yet", the same explicit-not-loaded
    /// posture `atlas`'s `None` already establishes.
    pub explain: Vec<ExplainView<'a>>,
}

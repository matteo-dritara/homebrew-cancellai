//! A minimal, deterministic combination of a clock reading and a set of path observations.
//!
//! This is **not** the real plan builder - `SealedPlan` generation belongs to the safety
//! kernel (E03) and inventory engine (E04), neither of which exists yet. `Snapshot` exists
//! only to prove, end to end, that [`crate::Clock`] and [`crate::FsObserver`] compose
//! deterministically when both are frozen/synthetic: the actual verification contract this
//! story names ("determinism test repeats plan generation byte-for-byte") is proven here
//! against this stand-in, and the same composition is what the real plan builder will reuse.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::clock::{Clock, Timestamp};
use crate::fs_observer::{FsObserver, Observation};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Snapshot {
    pub taken_at: Timestamp,
    // BTreeMap, not HashMap: iteration (and therefore serialization) order must be
    // deterministic, or two runs over the same facts could still produce different bytes.
    pub observations: BTreeMap<PathBuf, Observation>,
}

pub fn build_snapshot(clock: &dyn Clock, observer: &dyn FsObserver, paths: &[PathBuf]) -> Snapshot {
    let observations = paths
        .iter()
        .map(|path| (path.clone(), observer.observe(path)))
        .collect();
    Snapshot {
        taken_at: clock.now(),
        observations,
    }
}

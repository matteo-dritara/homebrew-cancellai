//! Later user-service runtime (`docs/architecture/GUARDIAN_MODEL.md`): predictive
//! pressure/anomaly signals and bounded, safety-floor-respecting remediation
//! (`SI-027`, `SI-028` - detection severity and Guardian never self-escalate authority).
//!
//! Split into a library and a thin binary (`main.rs`), matching `cancellai-tui`'s pattern: the
//! modules here are pure detection logic with no runtime/service surface yet, so they are
//! exercised directly by unit tests rather than through the (still unimplemented) compiled
//! binary.

pub mod forecast;
pub mod pressure;

//! Later user-service runtime (`docs/architecture/GUARDIAN_MODEL.md`): predictive
//! pressure/anomaly signals and bounded, safety-floor-respecting remediation
//! (`SI-027`, `SI-028` - detection severity and Guardian never self-escalate authority).
//!
//! Skeleton crate (E02-S01) - no runtime defined yet; not part of the current roadmap phase
//! (P4). Created now only so the workspace's crate graph matches `docs/architecture/TARGET.md`
//! from the start.

use cancellai_model as _;
use cancellai_policy as _;
use cancellai_safety as _;
use cancellai_store as _;

fn main() {
    println!("cancellai-guardian: workspace skeleton (E02-S01), not yet implemented");
}

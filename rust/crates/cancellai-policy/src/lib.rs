//! Typed policy and the deterministic `EffectivePolicy` resolver
//! (`docs/architecture/POLICY_MODEL.md`). Depends on `cancellai-safety` because policy can
//! only select within the authority ceiling safety computes; it can never raise it
//! (`SI-025`).
//!
//! Skeleton crate (E02-S01) - no types defined yet.

use cancellai_model as _;
use cancellai_safety as _;

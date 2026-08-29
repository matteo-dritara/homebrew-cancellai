//! The authority lattice, root capabilities, and sealed plans: the only crate whose future
//! code is allowed to authorize a mutation. See `docs/architecture/DOMAIN_MODEL.md`
//! (`Effective Authority`, `SealedPlan`) and `docs/security/SAFETY_INVARIANTS.md`.
//!
//! Forbidden dependency direction (`docs/architecture/TARGET.md`): provider adapters may not
//! bypass this crate; this crate may not depend on UI or provider implementation crates.
//!
//! E03-S02 adds the first real type, [`SealedPlan`], and [`revalidate`] - the fail-closed
//! SI-013 precondition check E03-S05 (Mutation executor isolation) will call immediately
//! before any real mutation exists to perform. Nothing in this crate has filesystem access;
//! it consumes `cancellai-platform`'s `IdentityObserver`-produced facts as plain data
//! (`docs/architecture/PLATFORM_MODEL.md`: "domain and policy code consume capability
//! results, not OS-specific syscalls").

pub mod sealed_plan;

pub use sealed_plan::{RevalidationOutcome, SealedPlan, revalidate};

//! The authority lattice, root capabilities, and sealed plans: the only crate whose future
//! code is allowed to authorize a mutation. See `docs/architecture/DOMAIN_MODEL.md`
//! (`Effective Authority`, `SealedPlan`) and `docs/security/SAFETY_INVARIANTS.md`.
//!
//! Forbidden dependency direction (`docs/architecture/TARGET.md`): provider adapters may not
//! bypass this crate; this crate may not depend on UI or provider implementation crates.
//!
//! E03-S02 adds the first real type, [`SealedPlan`], and [`revalidate`] - the fail-closed
//! SI-013 precondition check [`mutation_executor::execute`] (E03-S05) calls immediately
//! before mutation. E03-S03 adds [`ApprovedRoot`]/[`BoundedPath`], the SI-002/SI-003/SI-018
//! boundary capability. E03-S04 adds
//! [`effective_authority`]/[`compute_effective_authority`], the SI-001/SI-007/SI-008/SI-009
//! monotonic-minimum Effective Authority lattice. E03-S05 adds
//! [`mutation_executor::execute`]/[`mutation_executor::execute_all`], composing all three
//! into the one path from a `SealedPlan` to a real mutation (SI-019, SI-020, C-07). This
//! crate performs no OS calls of its own; every OS-facing operation goes through a
//! `cancellai-platform` capability (`IdentityObserver`, `PathResolver`, `MutationExecutor`)
//! consumed as plain data (`docs/architecture/PLATFORM_MODEL.md`: "domain and policy code
//! consume capability results, not OS-specific syscalls").

pub mod authority;
pub mod mutation_executor;
pub mod root_capability;
pub mod sealed_plan;

pub use authority::{
    AuthorityConstraint, AuthorityInputs, EffectiveAuthority, compute_effective_authority,
    effective_authority,
};
pub use mutation_executor::{ActionResult, execute, execute_all};
pub use root_capability::{ApprovedRoot, BoundaryError, BoundedPath};
pub use sealed_plan::{RevalidationOutcome, SealedPlan, revalidate};

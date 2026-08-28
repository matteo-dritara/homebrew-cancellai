//! The authority lattice, root capabilities, and sealed plans: the only crate whose future
//! code is allowed to authorize a mutation. See `docs/architecture/DOMAIN_MODEL.md`
//! (`Effective Authority`, `SealedPlan`) and `docs/security/SAFETY_INVARIANTS.md`.
//!
//! Forbidden dependency direction (`docs/architecture/TARGET.md`): provider adapters may not
//! bypass this crate; this crate may not depend on UI or provider implementation crates.
//!
//! Skeleton crate (E02-S01) - no types defined yet. The mutation authority this crate will
//! own has E03 (Formal Safety Kernel) as its own dedicated epic; nothing here yet grants any.

use cancellai_model as _;

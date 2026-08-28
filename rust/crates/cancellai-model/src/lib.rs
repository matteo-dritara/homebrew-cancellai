//! Pure domain types and invariants for cancellAI: `AgentArtifact`, evidence, lifecycle,
//! risk, and authority. See `docs/architecture/DOMAIN_MODEL.md`.
//!
//! Forbidden dependency direction (`docs/architecture/TARGET.md`): this crate must never
//! depend on a provider adapter, UI, or store crate. It is the bottom of the dependency
//! graph other than the standard library.
//!
//! `AgentArtifact`/`ProviderRoot`/`SealedPlan`/`Results` etc. are not defined yet (E02-S01
//! skeleton); `diagnostic` (E02-S03) is the first real domain type in this crate.

pub mod diagnostic;

pub use diagnostic::{Diagnostic, ErrorCategory};

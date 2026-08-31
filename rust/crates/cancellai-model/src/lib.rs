//! Pure domain types and invariants for cancellAI: `AgentArtifact`, evidence, lifecycle,
//! risk, and authority. See `docs/architecture/DOMAIN_MODEL.md`.
//!
//! Forbidden dependency direction (`docs/architecture/TARGET.md`): this crate must never
//! depend on a provider adapter, UI, or store crate. It is the bottom of the dependency
//! graph other than the standard library.
//!
//! `diagnostic` (E02-S03) is the first real domain type in this crate, `vocabulary`
//! (E03-S02) is the second - `SealedPlan` itself lives in `cancellai-safety`, which depends
//! on this crate for the vocabulary it records. `evidence`/`agent_artifact`/`action` (E06-S01)
//! add the remaining domain types DOMAIN_MODEL.md names: `Evidence`, `AgentArtifact`, and the
//! plan document's `Action` envelope - deferred until E06 because, per `agent_artifact`'s own
//! module docs, classifying an artifact's `RiskClass`/lifecycle axes/`AuthorityCeiling` needs
//! provider/policy knowledge no earlier story has.

pub mod action;
pub mod agent_artifact;
pub mod diagnostic;
pub mod evidence;
pub mod vocabulary;

pub use action::{Action, ActionId, Precondition, PreconditionValue};
pub use agent_artifact::{AgentArtifact, ArtifactId};
pub use diagnostic::{Diagnostic, ErrorCategory};
pub use evidence::{Evidence, EvidenceId};
pub use vocabulary::{
    ActionClass, ActivityState, AuthorityLevel, IntegrityState, KnowledgeConfidence,
    ProtectionState, ProviderTrust, ResidencyState, Reversibility, RiskClass, RootFingerprint,
};

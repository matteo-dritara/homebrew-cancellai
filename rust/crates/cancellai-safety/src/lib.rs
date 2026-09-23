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
//! into the one path from a `SealedPlan` to a real mutation (SI-019, SI-020, C-07). E05-S02
//! adds [`TrustedTier`], an opaque wrapper around [`cancellai_model::ProviderTrust`] whose only
//! public constructors are [`TrustedTier::untrusted`] and a checked [`TrustedTier::promote`]
//! (SI-021, SI-022) - [`AuthorityInputs::provider_trust`] accepts only this type, not a bare
//! `ProviderTrust`, specifically so no external caller can supply an unpromoted trust tier
//! directly (E05 verifier review round 1 found and this repair closed exactly that gap; see
//! `trust_promotion.rs`'s module doc). E17-S05 adds [`BuildChannel`], the same split applied to
//! release channel (SI-030): an opaque wrapper around [`cancellai_model::ReleaseChannel`] whose
//! only production constructor, [`BuildChannel::from_compiled_env`], reads a compile-time
//! environment variable baked into the binary at build time, not a runtime one a user could set
//! to unlock stable-level authority from a nightly build (`build_channel.rs`'s module doc).
//! This crate performs no OS calls of its own; every OS-facing operation goes through a
//! `cancellai-platform` capability (`IdentityObserver`, `PathResolver`, `MutationExecutor`)
//! consumed as plain data (`docs/architecture/PLATFORM_MODEL.md`: "domain and policy code
//! consume capability results, not OS-specific syscalls"). E16-S02 adds
//! [`knowledge_bundle`] (SI-022, SI-029, ADR-0024): signed provider knowledge bundles,
//! verified with Ed25519 against a caller-supplied [`LocalTrustPolicy`] whose tier assignments
//! are the only source of authority a verified bundle ever carries - never the bundle itself.
//! E18-S02 adds [`remote_execution`] (SI-031, RFC-0001, ADR-0031): a remote controller's signed
//! request, verified the same way, whose only effect on [`AuthorityInputs`] is supplying
//! `user_requested` - every other input, and `effective_authority` itself, stays exactly as it
//! is for a local caller. E17-S07 adds [`incident`] (SI-022, SI-029): signed capability
//! containment carried in a knowledge bundle, which can only cap a provider at `Observe` or
//! `Recommend`, is recorded in a ledger that replay, rollback and expiry cannot shrink, and is
//! lifted only locally.

pub mod authority;
pub mod build_channel;
pub mod incident;
pub mod knowledge_bundle;
pub mod mutation_executor;
pub mod provider_layout;
pub mod remote_execution;
pub mod root_capability;
pub mod sealed_plan;
pub mod trust_promotion;

pub use authority::{
    AuthorityConstraint, AuthorityInputs, EffectiveAuthority, ProviderExecutionPermit,
    compute_effective_authority, effective_authority, effective_authority_for_channel,
    effective_authority_under_containment, resolve_provider_execution_authority,
};
pub use build_channel::BuildChannel;
pub use incident::{
    ContainmentBinding, ContainmentCeiling, ContainmentEntry, ContainmentError, ContainmentEvent,
    ContainmentLedger, ContainmentNotice, ContainmentTarget, ContainmentTrustError,
    IncidentEvidence, IncidentPlatform, IncidentSeverity, KnowledgeProvenance,
    KnowledgeUnavailable, MAX_ACTIVE_RECORDS, MAX_NOTICE_BYTES, MAX_TRACKED_PUBLISHERS,
    PROJECT_INCIDENT_PUBLISHER_ID, RefreshOutcome, ReleaseProvenance, containment_trust_policy,
    parse_notice,
};
pub use knowledge_bundle::{
    KnowledgeBundle, KnowledgeBundleError, KnowledgeStore, LocalTrustPolicy,
    SUPPORTED_SCHEMA_VERSIONS, TrustedPublisher, VerifiedKnowledgeBundle, parse_bundle,
    verify_bundle,
};
pub use mutation_executor::{ActionResult, execute_with_system_capabilities};
pub use provider_layout::LayoutSignature;
pub use remote_execution::{
    RemoteExecutionError, RemoteExecutionLog, RemoteExecutionRequest, TrustedRemoteController,
    TrustedRemoteControllers, VerifiedRemoteIntent, parse_request,
};
pub use root_capability::{ApprovedRoot, BoundaryError, BoundedPath};
pub use sealed_plan::{RevalidationOutcome, SealedPlan, revalidate};
pub use trust_promotion::{TrustPromotionError, TrustPromotionEvidence, TrustedTier};

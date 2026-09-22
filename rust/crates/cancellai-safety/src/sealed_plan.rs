//! `SealedPlan`: an immutable, identity-bound, capability-bound, policy-explained mutating
//! plan (E03-S02, `docs/architecture/DOMAIN_MODEL.md` "SealedPlan", SI-013, SI-016).
//!
//! A `SealedPlan` records exactly what it was approved against - which root capability
//! (`RootFingerprint`), which object (`IdentityToken`), what class of action, at what
//! authority, with what reversibility. Nothing here mutates anything; this crate has no
//! filesystem access at all yet (E03-S05, Mutation executor isolation, is where a plan turns
//! into a real `Result`). What this story does provide is [`revalidate`]: the fail-closed
//! answer to "does the object I am about to act on still match what I planned against?"
//! (SI-013) - `docs/architecture/AS_IS.md`'s deferred requirement "sealed identity-bound
//! plans and stronger TOCTOU defense," now real.
//!
//! Scope note: `docs/architecture/DOMAIN_MODEL.md`'s full `SealedPlan` also carries an
//! inventory snapshot ID, a batch of `Action`s (this crate models exactly one target per
//! plan for now), evidence references, and knowledge-bundle version references - none of
//! which exist as real subsystems yet (E04 inventory engine, provider knowledge). Building
//! those fields as placeholders nothing produces would not make this story's revalidation
//! logic any more correct; they land with the stories that actually populate them.
//! `artifact_identity` doubles as this plan's one implemented execution precondition (the AC
//! and SI-013 both single out identity specifically); other precondition kinds (activity
//! state, provider capability) are future stories' concern once those facts exist to check.
//!
//! `root_identity` (E03 verifier review round 1 repair): a `SealedPlan` now records the
//! *root's* identity, not only the target's - `mutation_executor::execute` (E03-S05) compares
//! it against the [`crate::BoundedPath`] actually passed at execution time, closing a gap
//! where a plan sealed against one root's fingerprint could execute against a target bound
//! under a completely different root (the two were never previously connected by anything
//! but caller-trusted, unverified strings).

use std::path::{Path, PathBuf};

use cancellai_model::{ActionClass, AuthorityLevel, Reversibility, RootFingerprint};
use cancellai_platform::provider_layout::ProviderLayoutObserver;
use cancellai_platform::{IdentityObservation, IdentityToken};

use crate::provider_layout::LayoutSignature;
use crate::root_capability::{ApprovedRoot, BoundedPath, MoveDestination};

/// An immutable, sealed mutating plan for exactly one target artifact.
///
/// Immutability is enforced by API shape, not by a runtime check: fields are private and
/// every accessor is `&self` - there is no method here that could mutate a `SealedPlan` once
/// built (SI-016). A caller that wants a "different" plan builds a new one; it cannot edit
/// this one in place.
///
/// [`SealedPlan::seal`] is the only *public* constructor: it derives `root_identity` and
/// `artifact_identity` directly from a real [`ApprovedRoot`]/[`BoundedPath`] pair rather than
/// accepting bare, caller-suppliable `IdentityToken` values a caller could fabricate
/// disconnected from any real boundary check (E03 verifier review round 1: a `SealedPlan`
/// built from loose values, never actually bound to a checked root/target pair, executed
/// successfully against a target from a different root). The lower-level field constructor
/// remains available *within this crate only* (`pub(crate)`) for tests that need to exercise
/// [`revalidate`]'s pure identity-matching logic without the overhead of a real filesystem
/// root/bind round trip - it is not part of this crate's public API.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SealedPlan {
    root: RootFingerprint,
    root_identity: IdentityToken,
    artifact_identity: IdentityToken,
    action_class: ActionClass,
    authority: AuthorityLevel,
    reversibility: Reversibility,
    process_guard: Option<&'static [&'static str]>,
    /// The move destination (E12-S01/E12-S02/E12-S03) - `Some` only for a plan built by
    /// [`Self::seal_quarantine`], [`Self::seal_restore`] or [`Self::seal_archive`]. Every other
    /// action class carries `None`: there is nothing to move anywhere else to.
    destination_path: Option<PathBuf>,
    /// The identity of the destination's own root, recorded the same way `root_identity` is -
    /// from a real [`MoveDestination`], never a bare caller-suppliable value - so
    /// `mutation_executor::execute` can compare it against `root_identity` before ever
    /// attempting a move (SI-018).
    destination_root_identity: Option<IdentityToken>,
    /// A snapshot of the provider root's structural layout, observed once when this plan was
    /// sealed (E14-S05, SI-004, SI-013's own "revalidate immediately before mutation"
    /// principle applied to a root's own layout, not only the target artifact's identity) -
    /// `None` means no platform capability could observe it at seal time (the current,
    /// disclosed Unix-only residual: `cancellai_platform::BoundLayoutObservation::observe`
    /// fails closed with `Unsupported` on every other platform). `revalidate_provider_layout`
    /// re-observes exactly this snapshot's own `root_path` and compares against its recorded
    /// `root_identity`/`signature`, treating `None` the same as a drift: there is no baseline
    /// to prove "unchanged" against, so nothing here ever grants an unconstrained default.
    ///
    /// *Which* root this snapshot describes is not always `root_identity`/the source root
    /// (E14-S05 round 2 independent review, finding F3): `Self::seal_restore`'s `root`
    /// parameter is the quarantine store, not a provider root at all - for that action class
    /// the snapshot instead describes `destination`'s own root, the real provider root the
    /// move actually writes into, since that is the root SI-004 is actually about protecting.
    provider_layout: Option<ProviderLayoutSnapshot>,
}

/// A snapshot of one specific root's observed structural layout, bound to the real path and
/// identity it was read from (E14-S05 round 2) - not merely a bare [`LayoutSignature`], which
/// on its own carries no record of *which* root it describes or where to re-observe it.
/// [`revalidate_provider_layout`] is the only consumer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct ProviderLayoutSnapshot {
    pub(crate) root_path: PathBuf,
    pub(crate) root_identity: IdentityToken,
    pub(crate) signature: LayoutSignature,
}

impl SealedPlan {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_process_guard(
        root: RootFingerprint,
        root_identity: IdentityToken,
        artifact_identity: IdentityToken,
        action_class: ActionClass,
        authority: AuthorityLevel,
        reversibility: Reversibility,
        process_guard: Option<&'static [&'static str]>,
        provider_layout: Option<ProviderLayoutSnapshot>,
    ) -> Self {
        Self::new_with_destination(
            root,
            root_identity,
            artifact_identity,
            action_class,
            authority,
            reversibility,
            process_guard,
            None,
            None,
            provider_layout,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_destination(
        root: RootFingerprint,
        root_identity: IdentityToken,
        artifact_identity: IdentityToken,
        action_class: ActionClass,
        authority: AuthorityLevel,
        reversibility: Reversibility,
        process_guard: Option<&'static [&'static str]>,
        destination_path: Option<PathBuf>,
        destination_root_identity: Option<IdentityToken>,
        provider_layout: Option<ProviderLayoutSnapshot>,
    ) -> Self {
        Self {
            root,
            root_identity,
            artifact_identity,
            action_class,
            authority,
            reversibility,
            process_guard,
            destination_path,
            destination_root_identity,
            provider_layout,
        }
    }

    /// Seal a plan for `target`, bound under `root` (E03-S03's capabilities - not raw
    /// paths). `root_identity`/`artifact_identity` are read from `root`/`target` themselves,
    /// never accepted as independent caller-supplied values. `layout_observer` (E14-S05) is
    /// used the same way: `root`'s provider layout is observed internally, from `root.path()`
    /// itself, never accepted as a pre-built observation a caller could have paired with an
    /// unrelated root (the exact class of defect ADR-0036's round 5 review found and closed
    /// one layer down, in `BoundLayoutObservation::observe` itself - this constructor does not
    /// reopen it one layer up).
    pub fn seal(
        root: &ApprovedRoot,
        root_fingerprint: RootFingerprint,
        target: &BoundedPath,
        action_class: ActionClass,
        authority: AuthorityLevel,
        reversibility: Reversibility,
        layout_observer: &dyn ProviderLayoutObserver,
    ) -> Self {
        Self::seal_with_process_guard(
            root,
            root_fingerprint,
            target,
            action_class,
            authority,
            reversibility,
            None,
            layout_observer,
        )
    }

    /// Same as [`Self::seal`], additionally recording which provider process name(s) must be
    /// confirmed *not running* immediately before mutation (`execute`'s own re-check, not this
    /// constructor - E06 verifier review round 1: `execution_preconditions`'s
    /// `process_not_running` entry was recorded in the emitted plan document but never actually
    /// revalidated at the one moment that matters, unlike `artifact_identity` which already had
    /// a real TOCTOU-closing revalidation). `None` means this action class has no such
    /// precondition to check (matches every existing call site/test unaffected by this addition).
    #[allow(clippy::too_many_arguments)]
    pub fn seal_with_process_guard(
        root: &ApprovedRoot,
        root_fingerprint: RootFingerprint,
        target: &BoundedPath,
        action_class: ActionClass,
        authority: AuthorityLevel,
        reversibility: Reversibility,
        process_guard: Option<&'static [&'static str]>,
        layout_observer: &dyn ProviderLayoutObserver,
    ) -> Self {
        Self::new_with_process_guard(
            root_fingerprint,
            root.identity().clone(),
            target.identity().clone(),
            action_class,
            authority,
            reversibility,
            process_guard,
            observe_provider_layout(root.path(), layout_observer),
        )
    }

    /// Seal a quarantine plan for `target`, additionally recording `destination` (E12-S01) -
    /// the field `mutation_executor::execute` needs to perform the move and to compare the
    /// destination root's identity against `root`'s own (SI-018) before ever attempting it.
    /// `destination_root_identity`/`destination_path` are read from a real
    /// [`MoveDestination`] (produced only by [`ApprovedRoot::prepare_destination`]),
    /// never accepted as bare caller-supplied values - the same principle [`Self::seal`]
    /// already applies to `root_identity`/`artifact_identity`.
    #[allow(clippy::too_many_arguments)]
    pub fn seal_quarantine(
        root: &ApprovedRoot,
        root_fingerprint: RootFingerprint,
        target: &BoundedPath,
        destination: &MoveDestination,
        authority: AuthorityLevel,
        reversibility: Reversibility,
        layout_observer: &dyn ProviderLayoutObserver,
    ) -> Self {
        Self::new_with_destination(
            root_fingerprint,
            root.identity().clone(),
            target.identity().clone(),
            ActionClass::Quarantine,
            authority,
            reversibility,
            None,
            Some(destination.path().to_path_buf()),
            Some(destination.root_identity().clone()),
            observe_provider_layout(root.path(), layout_observer),
        )
    }

    /// Seal a restore plan for `target` - an artifact currently inside the quarantine store,
    /// bound under `root` (the quarantine store's own [`ApprovedRoot`], not the original
    /// provider root) - moving it to `destination` (E12-S02). The reverse of
    /// [`Self::seal_quarantine`]: same shape, same SI-018 boundary comparison at execution
    /// time, `destination` here names a location outside the quarantine store rather than
    /// inside it.
    ///
    /// The provider-layout snapshot (E14-S05 round 2 independent review, finding F3) is
    /// observed against **`destination`'s own root**, not `root` - `root` here is the
    /// quarantine store, which is not a provider root at all, so observing it would silently
    /// protect nothing SI-004 actually cares about. `destination`'s root is the real provider
    /// location this move writes into, so that is what a fresh observation must re-check
    /// immediately before mutation.
    #[allow(clippy::too_many_arguments)]
    pub fn seal_restore(
        root: &ApprovedRoot,
        root_fingerprint: RootFingerprint,
        target: &BoundedPath,
        destination: &MoveDestination,
        authority: AuthorityLevel,
        reversibility: Reversibility,
        layout_observer: &dyn ProviderLayoutObserver,
    ) -> Self {
        Self::new_with_destination(
            root_fingerprint,
            root.identity().clone(),
            target.identity().clone(),
            ActionClass::Restore,
            authority,
            reversibility,
            None,
            Some(destination.path().to_path_buf()),
            Some(destination.root_identity().clone()),
            observe_provider_layout(destination.root_path(), layout_observer),
        )
    }

    /// Seal an archive plan for `target`, additionally recording `destination` (E12-S03) -
    /// same shape as [`Self::seal_quarantine`]: a second, cancellAI-controlled root (the
    /// archive store), the same SI-018 same-device comparison at execution time.
    #[allow(clippy::too_many_arguments)]
    pub fn seal_archive(
        root: &ApprovedRoot,
        root_fingerprint: RootFingerprint,
        target: &BoundedPath,
        destination: &MoveDestination,
        authority: AuthorityLevel,
        reversibility: Reversibility,
        layout_observer: &dyn ProviderLayoutObserver,
    ) -> Self {
        Self::new_with_destination(
            root_fingerprint,
            root.identity().clone(),
            target.identity().clone(),
            ActionClass::Archive,
            authority,
            reversibility,
            None,
            Some(destination.path().to_path_buf()),
            Some(destination.root_identity().clone()),
            observe_provider_layout(root.path(), layout_observer),
        )
    }

    pub fn root(&self) -> &RootFingerprint {
        &self.root
    }

    /// The identity of the root this plan was sealed against - compared against a
    /// [`BoundedPath`]'s own [`BoundedPath::root_identity`] at execution time, not merely at
    /// sealing time (a caller could otherwise pass a *different* `BoundedPath` to `execute`
    /// than the one used to seal the plan).
    pub fn root_identity(&self) -> &IdentityToken {
        &self.root_identity
    }

    /// The identity this plan was sealed against - also this plan's execution precondition
    /// (see module docs). [`revalidate`] compares a fresh [`IdentityObservation`] to this.
    pub fn artifact_identity(&self) -> &IdentityToken {
        &self.artifact_identity
    }

    pub fn action_class(&self) -> ActionClass {
        self.action_class
    }

    pub fn authority(&self) -> AuthorityLevel {
        self.authority
    }

    pub fn reversibility(&self) -> Reversibility {
        self.reversibility
    }

    /// Provider process name(s) that must be confirmed not-running immediately before this
    /// plan mutates anything - `None` when this plan carries no such precondition.
    pub fn process_guard(&self) -> Option<&'static [&'static str]> {
        self.process_guard
    }

    /// The quarantine move destination this plan was sealed against - `None` for every action
    /// class except `Quarantine` (see [`Self::seal_quarantine`]).
    pub fn destination_path(&self) -> Option<&Path> {
        self.destination_path.as_deref()
    }

    /// The identity of the quarantine destination's own root - `None` for every action class
    /// except `Quarantine`. Compared against [`Self::root_identity`] before a move is ever
    /// attempted (SI-018).
    pub fn destination_root_identity(&self) -> Option<&IdentityToken> {
        self.destination_root_identity.as_ref()
    }

    /// The provider root's layout signature observed when this plan was sealed - `None` if no
    /// platform capability could observe it then (E14-S05). [`revalidate_provider_layout`]
    /// compares a fresh observation of the same root to this immediately before mutation.
    pub fn provider_layout(&self) -> Option<&LayoutSignature> {
        self.provider_layout
            .as_ref()
            .map(|snapshot| &snapshot.signature)
    }

    /// The real path of the root [`Self::provider_layout`]'s snapshot describes - `None` under
    /// the identical condition `provider_layout()` is `None`. Exposed for tests that need to
    /// assert which root this plan actually bound its layout precondition to (E14-S05 round 2:
    /// `Self::seal_restore` binds it to the destination's root, not `root`'s own).
    pub fn provider_layout_root_path(&self) -> Option<&Path> {
        self.provider_layout
            .as_ref()
            .map(|snapshot| snapshot.root_path.as_path())
    }
}

/// Observe `path`'s provider-root layout via `layout_observer`, normalized into the order-
/// independent [`LayoutSignature`] shape and bound together with the real path/identity it was
/// read from, into a [`ProviderLayoutSnapshot`] `revalidate_provider_layout` later re-observes
/// and compares against - `None` on any observation failure (an absent/unreadable root, or,
/// currently, any non-Unix platform: `cancellai_platform::BoundLayoutObservation::observe`'s
/// own disclosed residual). Never a silent empty/clean signature: an unobservable root has no
/// baseline, not an honest "no markers."
fn observe_provider_layout(
    path: &Path,
    layout_observer: &dyn ProviderLayoutObserver,
) -> Option<ProviderLayoutSnapshot> {
    let observed = layout_observer.observe(path).ok()?;
    Some(ProviderLayoutSnapshot {
        root_path: path.to_path_buf(),
        root_identity: observed.root_identity().clone(),
        signature: LayoutSignature::new(observed.markers().iter().cloned()),
    })
}

/// The result of checking a [`SealedPlan`]'s preconditions immediately before mutation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RevalidationOutcome {
    /// The freshly observed identity still matches what the plan was sealed against.
    Proceed,
    /// Something about the artifact changed - or could not be re-established - since the
    /// plan was sealed. Execution must refuse (SI-013); this is never a "probably fine."
    StalePlan { reason: String },
}

/// Revalidate a plan's identity precondition against a freshly observed identity fact.
///
/// Fail-closed by construction: this match is exhaustive over every
/// [`IdentityObservation`] variant, and exactly one arm - an exact [`IdentityToken`] match -
/// returns [`RevalidationOutcome::Proceed`]. Every other branch, including ones a future
/// variant would add, has to be handled explicitly and defaults to none of them being
/// written as "proceed anyway" (there is no wildcard `_ => Proceed` arm to silently cover a
/// case this function's author didn't think through - the compiler refuses to build this
/// function at all until every `IdentityObservation` variant is named here).
pub fn revalidate(plan: &SealedPlan, current: &IdentityObservation) -> RevalidationOutcome {
    match current {
        IdentityObservation::Identity(token) if *token == plan.artifact_identity => {
            RevalidationOutcome::Proceed
        }
        IdentityObservation::Identity(token) => RevalidationOutcome::StalePlan {
            reason: format!(
                "artifact identity changed since the plan was sealed: planned {:?}, observed {token:?}",
                plan.artifact_identity
            ),
        },
        IdentityObservation::Absent => RevalidationOutcome::StalePlan {
            reason: "artifact no longer exists".to_string(),
        },
        IdentityObservation::Unreadable { reason } => RevalidationOutcome::StalePlan {
            reason: format!(
                "artifact could not be re-examined immediately before mutation: {reason}"
            ),
        },
        IdentityObservation::Unsupported { reason } => RevalidationOutcome::StalePlan {
            reason: format!(
                "platform cannot re-verify artifact identity immediately before mutation: {reason}"
            ),
        },
    }
}

/// Revalidate a plan's provider-layout precondition against a freshly observed layout fact
/// (E14-S05, SI-004, SI-013) - [`revalidate`]'s own principle, applied to the provider root's
/// structural layout instead of the target artifact's identity: a plan sealed while a layout
/// was one shape is refused if a fresh observation, taken immediately before mutation, shows a
/// different shape now - never permitted merely because the plan's own seal-time snapshot
/// still says what it used to.
///
/// Takes `layout_observer` directly and re-observes exactly the path the plan's own snapshot
/// recorded (`ProviderLayoutSnapshot::root_path`) - not a path the caller supplies separately
/// (E14-S05 round 2 independent review, finding F3: a caller-supplied path could name the wrong
/// root, as `mutation_executor::execute` previously did for every `Restore` plan by always
/// re-observing `target`'s own bound root - the quarantine store - instead of the plan's actual
/// destination provider root).
///
/// Fail-closed on every branch but one: the plan never recording a snapshot at seal time
/// (`None`, e.g. sealed on a platform with no verified layout-observation capability yet), the
/// fresh observation failing outright, the fresh root identity no longer matching what was
/// recorded, or the fresh signature differing from the recorded one - each is `StalePlan`. Only
/// a fresh, successful observation of the plan's own recorded root whose identity and
/// normalized markers both still match is `Proceed`.
pub fn revalidate_provider_layout(
    plan: &SealedPlan,
    layout_observer: &dyn ProviderLayoutObserver,
) -> RevalidationOutcome {
    let Some(baseline) = &plan.provider_layout else {
        return RevalidationOutcome::StalePlan {
            reason: "provider root layout was not observable when the plan was sealed".to_string(),
        };
    };
    let fresh = match layout_observer.observe(&baseline.root_path) {
        Err(reason) => {
            return RevalidationOutcome::StalePlan {
                reason: format!(
                    "provider root layout could not be observed immediately before mutation: {reason:?}"
                ),
            };
        }
        Ok(observation) => observation,
    };
    if fresh.root_identity() != &baseline.root_identity {
        return RevalidationOutcome::StalePlan {
            reason: "provider root identity changed since the plan was sealed".to_string(),
        };
    }
    let fresh_signature = LayoutSignature::new(fresh.markers().iter().cloned());
    match fresh_signature == baseline.signature {
        true => RevalidationOutcome::Proceed,
        false => RevalidationOutcome::StalePlan {
            reason: "provider root layout changed since the plan was sealed".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_model::KnowledgeConfidence;
    use cancellai_platform::{
        Clock, FileKind, FrozenClock, IdentityObserver, SyntheticIdentityObserver,
    };

    fn fingerprint() -> RootFingerprint {
        RootFingerprint {
            root_id: "root-1".into(),
            provider_id: "codex".into(),
            confidence: KnowledgeConfidence::Verified,
        }
    }

    fn token(inode: u64) -> IdentityToken {
        IdentityToken::Unix {
            device: 1,
            inode,
            kind: FileKind::File,
            modified: FrozenClock::at(1_000).now(),
            modified_nanos: 0,
        }
    }

    fn root_token() -> IdentityToken {
        IdentityToken::Unix {
            device: 1,
            inode: 0,
            kind: FileKind::Directory,
            modified: FrozenClock::at(1_000).now(),
            modified_nanos: 0,
        }
    }

    fn plan_with(identity: IdentityToken) -> SealedPlan {
        SealedPlan::new_with_process_guard(
            fingerprint(),
            root_token(),
            identity,
            ActionClass::Delete,
            AuthorityLevel::Govern,
            Reversibility::Irreversible,
            None,
            None,
        )
    }

    #[test]
    fn proceeds_when_identity_is_unchanged() {
        // Not vacuously fail-closed: prove the mechanism actually allows the matching case
        // through, or every other test in this module would be meaningless.
        let plan = plan_with(token(1));
        let outcome = revalidate(&plan, &IdentityObservation::Identity(token(1)));
        assert_eq!(outcome, RevalidationOutcome::Proceed);
    }

    #[test]
    fn blocks_when_identity_token_differs() {
        let plan = plan_with(token(1));
        let outcome = revalidate(&plan, &IdentityObservation::Identity(token(2)));
        assert!(matches!(outcome, RevalidationOutcome::StalePlan { .. }));
    }

    #[test]
    fn blocks_when_artifact_became_absent() {
        let plan = plan_with(token(1));
        let outcome = revalidate(&plan, &IdentityObservation::Absent);
        assert!(matches!(outcome, RevalidationOutcome::StalePlan { .. }));
    }

    #[test]
    fn blocks_when_artifact_became_unreadable() {
        let plan = plan_with(token(1));
        let outcome = revalidate(
            &plan,
            &IdentityObservation::Unreadable {
                reason: "permission denied".into(),
            },
        );
        assert!(matches!(outcome, RevalidationOutcome::StalePlan { .. }));
    }

    #[test]
    fn blocks_when_platform_identity_is_unsupported() {
        let plan = plan_with(token(1));
        let outcome = revalidate(
            &plan,
            &IdentityObservation::Unsupported {
                reason: "no verified Windows identity yet".into(),
            },
        );
        assert!(matches!(outcome, RevalidationOutcome::StalePlan { .. }));
    }

    #[test]
    fn end_to_end_toctou_through_a_real_synthetic_observer_fails_closed() {
        // Ties E03-S01's observer directly to E03-S02's revalidation: plan against one
        // token, then have the *observer itself* report a different fact at "execute" time
        // (standing in for SystemIdentityObserver re-observing a real filesystem swap,
        // which identity.rs's own tests already cover against the real filesystem).
        let path = std::path::PathBuf::from("/synthetic/target");
        let mut observer = SyntheticIdentityObserver::new();
        observer.set(&path, IdentityObservation::Identity(token(1)));
        let plan = plan_with(token(1));

        observer.set(&path, IdentityObservation::Identity(token(2)));
        let revalidated = observer.observe(&path);
        assert_eq!(
            revalidate(&plan, &revalidated),
            RevalidationOutcome::StalePlan {
                reason: format!(
                    "artifact identity changed since the plan was sealed: planned {:?}, observed {:?}",
                    token(1),
                    token(2)
                )
            }
        );
    }

    #[test]
    fn sealed_plan_exposes_every_field_the_acceptance_criteria_names() {
        let plan = plan_with(token(1));
        assert_eq!(plan.root(), &fingerprint());
        assert_eq!(plan.root_identity(), &root_token());
        assert_eq!(plan.artifact_identity(), &token(1));
        assert_eq!(plan.action_class(), ActionClass::Delete);
        assert_eq!(plan.authority(), AuthorityLevel::Govern);
        assert_eq!(plan.reversibility(), Reversibility::Irreversible);
    }

    #[cfg(unix)]
    #[test]
    fn seal_derives_root_and_artifact_identity_from_real_capabilities() {
        // E03 verifier review round 1: `seal` must read root_identity/artifact_identity
        // from a real ApprovedRoot/BoundedPath pair, not accept them as bare caller values.
        let dir = std::env::temp_dir().join(format!(
            "cancellai-sealed-plan-seal-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let file = dir.join("target.txt");
        std::fs::write(&file, b"hello").expect("create file");

        let resolver = cancellai_platform::SystemPathResolver;
        let observer = cancellai_platform::SystemIdentityObserver;
        let root = ApprovedRoot::establish(&dir, &resolver, &observer).expect("establish root");
        let target = root.bind(&file, &resolver, &observer).expect("bind target");

        let plan = SealedPlan::seal(
            &root,
            fingerprint(),
            &target,
            ActionClass::Delete,
            AuthorityLevel::Govern,
            Reversibility::Irreversible,
            &cancellai_platform::SystemProviderLayoutObserver,
        );

        assert_eq!(plan.root_identity(), root.identity());
        assert_eq!(plan.artifact_identity(), target.identity());
        assert_eq!(
            plan.provider_layout(),
            Some(&LayoutSignature::new(["target.txt".to_string()]))
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn seal_quarantine_records_the_destination_from_a_real_capability() {
        use cancellai_platform::{SystemIdentityObserver, SystemPathResolver};

        let dir = std::env::temp_dir().join(format!(
            "cancellai-sealed-plan-seal-quarantine-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let source_root_path = dir.join("source");
        std::fs::create_dir_all(&source_root_path).expect("create source root");
        let dest_root_path = dir.join("dest");
        std::fs::create_dir_all(&dest_root_path).expect("create dest root");
        let file = source_root_path.join("target.txt");
        std::fs::write(&file, b"hello").expect("create file");

        let resolver = SystemPathResolver;
        let observer = SystemIdentityObserver;
        let source_root =
            ApprovedRoot::establish(&source_root_path, &resolver, &observer).expect("source root");
        let target = source_root
            .bind(&file, &resolver, &observer)
            .expect("bind target");
        let dest_root =
            ApprovedRoot::establish(&dest_root_path, &resolver, &observer).expect("dest root");
        let destination = dest_root
            .prepare_destination("quarantined.txt", &observer)
            .expect("prepare destination");

        let plan = SealedPlan::seal_quarantine(
            &source_root,
            fingerprint(),
            &target,
            &destination,
            AuthorityLevel::Quarantine,
            Reversibility::Quarantinable,
            &cancellai_platform::SystemProviderLayoutObserver,
        );

        assert_eq!(plan.action_class(), ActionClass::Quarantine);
        assert_eq!(plan.destination_path(), Some(destination.path()));
        assert_eq!(
            plan.destination_root_identity(),
            Some(destination.root_identity())
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn seal_restore_records_the_destination_from_a_real_capability() {
        use cancellai_platform::{SystemIdentityObserver, SystemPathResolver};

        let dir = std::env::temp_dir().join(format!(
            "cancellai-sealed-plan-seal-restore-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let quarantine_root_path = dir.join("quarantine");
        std::fs::create_dir_all(&quarantine_root_path).expect("create quarantine root");
        let provider_root_path = dir.join("provider");
        std::fs::create_dir_all(&provider_root_path).expect("create provider root");
        let file = quarantine_root_path.join("quarantined.txt");
        std::fs::write(&file, b"hello").expect("create file");

        let resolver = SystemPathResolver;
        let observer = SystemIdentityObserver;
        let quarantine_root = ApprovedRoot::establish(&quarantine_root_path, &resolver, &observer)
            .expect("quarantine root");
        let target = quarantine_root
            .bind(&file, &resolver, &observer)
            .expect("bind target");
        let provider_root = ApprovedRoot::establish(&provider_root_path, &resolver, &observer)
            .expect("provider root");
        let destination = provider_root
            .prepare_destination("artifact.txt", &observer)
            .expect("prepare destination");

        let plan = SealedPlan::seal_restore(
            &quarantine_root,
            fingerprint(),
            &target,
            &destination,
            AuthorityLevel::Quarantine,
            Reversibility::Quarantinable,
            &cancellai_platform::SystemProviderLayoutObserver,
        );

        assert_eq!(plan.action_class(), ActionClass::Restore);
        assert_eq!(plan.destination_path(), Some(destination.path()));
        assert_eq!(
            plan.destination_root_identity(),
            Some(destination.root_identity())
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn seal_archive_records_the_destination_from_a_real_capability() {
        use cancellai_platform::{SystemIdentityObserver, SystemPathResolver};

        let dir = std::env::temp_dir().join(format!(
            "cancellai-sealed-plan-seal-archive-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let source_root_path = dir.join("source");
        std::fs::create_dir_all(&source_root_path).expect("create source root");
        let archive_root_path = dir.join("archive");
        std::fs::create_dir_all(&archive_root_path).expect("create archive root");
        let file = source_root_path.join("artifact.txt");
        std::fs::write(&file, b"hello").expect("create file");

        let resolver = SystemPathResolver;
        let observer = SystemIdentityObserver;
        let source_root =
            ApprovedRoot::establish(&source_root_path, &resolver, &observer).expect("source root");
        let target = source_root
            .bind(&file, &resolver, &observer)
            .expect("bind target");
        let archive_root = ApprovedRoot::establish(&archive_root_path, &resolver, &observer)
            .expect("archive root");
        let destination = archive_root
            .prepare_destination("archived.txt", &observer)
            .expect("prepare destination");

        let plan = SealedPlan::seal_archive(
            &source_root,
            fingerprint(),
            &target,
            &destination,
            AuthorityLevel::Quarantine,
            Reversibility::Archivable,
            &cancellai_platform::SystemProviderLayoutObserver,
        );

        assert_eq!(plan.action_class(), ActionClass::Archive);
        assert_eq!(plan.destination_path(), Some(destination.path()));
        assert_eq!(
            plan.destination_root_identity(),
            Some(destination.root_identity())
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_non_quarantine_plan_carries_no_destination() {
        let plan = plan_with(token(1));
        assert_eq!(plan.destination_path(), None);
        assert_eq!(plan.destination_root_identity(), None);
    }

    // --- E14-S05, SI-004, SI-013: `revalidate_provider_layout` -----------------------------

    fn synthetic_root_path() -> PathBuf {
        PathBuf::from("/synthetic/provider-root")
    }

    fn snapshot(
        root_path: &Path,
        root_identity: IdentityToken,
        markers: impl IntoIterator<Item = impl Into<String>>,
    ) -> ProviderLayoutSnapshot {
        ProviderLayoutSnapshot {
            root_path: root_path.to_path_buf(),
            root_identity,
            signature: LayoutSignature::new(markers.into_iter().map(Into::into)),
        }
    }

    fn plan_with_layout(provider_layout: Option<ProviderLayoutSnapshot>) -> SealedPlan {
        SealedPlan::new_with_process_guard(
            fingerprint(),
            root_token(),
            token(1),
            ActionClass::Delete,
            AuthorityLevel::Govern,
            Reversibility::Irreversible,
            None,
            provider_layout,
        )
    }

    /// A synthetic observer that returns `root_identity`/`markers` for `synthetic_root_path()`
    /// specifically - the same path [`snapshot`] uses by default, so a plan built from
    /// `plan_with_layout` and an observer built from this helper agree on *where* to look; only
    /// the two functions' other arguments need to differ to exercise drift.
    fn layout_observer_returning(
        root_identity: IdentityToken,
        markers: impl IntoIterator<Item = impl Into<String>>,
    ) -> cancellai_platform::SyntheticProviderLayoutObserver {
        let mut synth = cancellai_platform::SyntheticProviderLayoutObserver::new();
        synth.set_observed(synthetic_root_path(), root_identity, markers);
        synth
    }

    #[test]
    fn revalidate_provider_layout_proceeds_when_the_fresh_signature_still_matches() {
        // Not vacuously fail-closed: prove the matching case is actually let through.
        let plan = plan_with_layout(Some(snapshot(
            &synthetic_root_path(),
            root_token(),
            ["sessions/".to_string()],
        )));
        let observer = layout_observer_returning(root_token(), ["sessions/".to_string()]);
        assert_eq!(
            revalidate_provider_layout(&plan, &observer),
            RevalidationOutcome::Proceed
        );
    }

    #[test]
    fn revalidate_provider_layout_ignores_marker_order_and_duplicates() {
        // LayoutSignature normalizes both - a real `read_dir` ordering difference between two
        // observations of the identical, unchanged directory must never read as drift.
        let plan = plan_with_layout(Some(snapshot(
            &synthetic_root_path(),
            root_token(),
            ["a".to_string(), "b".to_string()],
        )));
        let observer = layout_observer_returning(
            root_token(),
            ["b".to_string(), "a".to_string(), "a".to_string()],
        );
        assert_eq!(
            revalidate_provider_layout(&plan, &observer),
            RevalidationOutcome::Proceed
        );
    }

    #[test]
    fn revalidate_provider_layout_blocks_when_the_fresh_signature_has_drifted() {
        let plan = plan_with_layout(Some(snapshot(
            &synthetic_root_path(),
            root_token(),
            ["sessions/".to_string()],
        )));
        let observer =
            layout_observer_returning(root_token(), ["totally_different_shape/".to_string()]);
        assert!(matches!(
            revalidate_provider_layout(&plan, &observer),
            RevalidationOutcome::StalePlan { .. }
        ));
    }

    #[test]
    fn revalidate_provider_layout_blocks_when_the_root_identity_itself_changed() {
        let plan = plan_with_layout(Some(snapshot(
            &synthetic_root_path(),
            root_token(),
            ["sessions/".to_string()],
        )));
        let swapped_root = IdentityToken::Unix {
            device: 1,
            inode: 999,
            kind: FileKind::Directory,
            modified: FrozenClock::at(1_000).now(),
            modified_nanos: 0,
        };
        let observer = layout_observer_returning(swapped_root, ["sessions/".to_string()]);
        assert!(matches!(
            revalidate_provider_layout(&plan, &observer),
            RevalidationOutcome::StalePlan { .. }
        ));
    }

    #[test]
    fn revalidate_provider_layout_blocks_when_the_plan_never_recorded_a_baseline() {
        // Seal-time observation failure (e.g. a non-Unix platform) must never grant an
        // unconstrained default (AC3) - there is no baseline to prove "unchanged" against.
        let plan = plan_with_layout(None);
        let observer = layout_observer_returning(root_token(), ["sessions/".to_string()]);
        assert!(matches!(
            revalidate_provider_layout(&plan, &observer),
            RevalidationOutcome::StalePlan { .. }
        ));
    }

    #[test]
    fn revalidate_provider_layout_blocks_when_the_fresh_observation_itself_fails() {
        let plan = plan_with_layout(Some(snapshot(
            &synthetic_root_path(),
            root_token(),
            ["sessions/".to_string()],
        )));
        // No `set_observed` call for `synthetic_root_path()` - an unconfigured path fails
        // closed by construction (`SyntheticProviderLayoutObserver`'s own module docs).
        let observer = cancellai_platform::SyntheticProviderLayoutObserver::new();
        assert!(matches!(
            revalidate_provider_layout(&plan, &observer),
            RevalidationOutcome::StalePlan { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn seal_restore_binds_its_provider_layout_to_the_destination_root_not_the_quarantine_source() {
        // E14-S05 round 2 independent review, finding F3: `seal_restore`'s `root` parameter is
        // the quarantine store, not a provider root - observing it would silently protect
        // nothing SI-004 actually cares about. The recorded snapshot must describe
        // `destination`'s own root instead, since that is the real provider location the move
        // writes into.
        use cancellai_platform::{SystemIdentityObserver, SystemPathResolver};

        let dir = std::env::temp_dir().join(format!(
            "cancellai-sealed-plan-seal-restore-layout-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let quarantine_root_path = dir.join("quarantine");
        std::fs::create_dir_all(&quarantine_root_path).expect("create quarantine root");
        let provider_root_path = dir.join("provider");
        std::fs::create_dir_all(&provider_root_path).expect("create provider root");
        std::fs::write(provider_root_path.join("sessions.json"), b"{}")
            .expect("seed the real provider root with a marker");
        let file = quarantine_root_path.join("quarantined.txt");
        std::fs::write(&file, b"hello").expect("create file");

        let resolver = SystemPathResolver;
        let observer = SystemIdentityObserver;
        let quarantine_root = ApprovedRoot::establish(&quarantine_root_path, &resolver, &observer)
            .expect("quarantine root");
        let target = quarantine_root
            .bind(&file, &resolver, &observer)
            .expect("bind target");
        let provider_root = ApprovedRoot::establish(&provider_root_path, &resolver, &observer)
            .expect("provider root");
        let destination = provider_root
            .prepare_destination("artifact.txt", &observer)
            .expect("prepare destination");

        let plan = SealedPlan::seal_restore(
            &quarantine_root,
            fingerprint(),
            &target,
            &destination,
            AuthorityLevel::Quarantine,
            Reversibility::Quarantinable,
            &cancellai_platform::SystemProviderLayoutObserver,
        );

        assert_eq!(
            plan.provider_layout_root_path(),
            Some(destination.root_path()),
            "the recorded layout snapshot must describe the destination provider root"
        );
        assert_ne!(
            plan.provider_layout_root_path(),
            Some(quarantine_root.path()),
            "the recorded layout snapshot must not describe the quarantine store"
        );
        assert_eq!(
            plan.provider_layout(),
            Some(&LayoutSignature::new(["sessions.json".to_string()])),
            "the recorded signature must reflect the real provider root's own markers"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}

//! Incomplete inventory propagation (E04-S03, SI-008 "Partial scan is non-destructive",
//! SI-009 "Unknown scan state is non-destructive").
//!
//! Every inventory scope this crate produces is classified `Complete`, `Partial`, or
//! `Unknown`, with the concrete reasons behind that classification (permission, I/O,
//! disappearance, or an unsupported filesystem/platform feature) - never a bare boolean. A
//! planning-facing view of a scope ([`PlanningView`]) can only be constructed *with* its
//! completeness attached: there is no accessor that returns candidates alone, so a caller
//! cannot silently drop this evidence on the way to a decision (AC2: "Planning cannot erase
//! completeness information").

use std::path::PathBuf;

use cancellai_platform::IdentityObservation;

use crate::file_facts::{FactConfidence, FactObservation, FileFacts};
use crate::scan::{DirectoryErrorKind, FactErrorKind, InventorySnapshot};

/// Why a scope's completeness is less than `Complete`. Every reason names a path and a
/// concrete cause - SI-010 requires scan errors to be visible, never summarized into an
/// opaque count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletenessReason {
    /// The scope root itself could not be observed at all - the strongest form of missing
    /// evidence this model can express.
    ScopeRootUnavailable {
        path: PathBuf,
        detail: String,
    },
    PermissionDenied {
        path: PathBuf,
    },
    /// A directory existed when listed by its parent but had vanished by the time this scan
    /// tried to read it (a listing-to-read race).
    Disappeared {
        path: PathBuf,
    },
    /// A permission/I/O failure this model cannot further classify (see the module doc on
    /// `DirectoryErrorKind::Other`, and `file_facts`'s per-file `Unreadable`, which carries
    /// only an opaque platform message).
    Io {
        path: PathBuf,
        message: String,
    },
    /// A sub-observation (identity or allocation) could not be established on this
    /// platform/filesystem - `docs/architecture/PLATFORM_MODEL.md`'s "if the platform cannot
    /// produce an identity strong enough ... authority is reduced" made explicit at the
    /// per-fact level.
    UnsupportedFilesystemFeature {
        path: PathBuf,
        feature: String,
        detail: String,
    },
}

/// A scope's aggregate completeness. `Unknown` is reserved for the case where the scope root
/// itself could not be established - anything less severe, where the root was observed but
/// some descendant was not, is `Partial`. Ordering here is deliberately *not* derived
/// (`Ord`/`PartialOrd`) - "worse than" is a judgment call this module makes explicitly in
/// [`derive_completeness`], not something callers should reconstruct themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeCompleteness {
    Complete,
    Partial { reasons: Vec<CompletenessReason> },
    Unknown { reasons: Vec<CompletenessReason> },
}

impl ScopeCompleteness {
    pub fn is_complete(&self) -> bool {
        matches!(self, ScopeCompleteness::Complete)
    }
}

/// How many distinct reasons a scope retains before it stops storing them. A hostile or simply
/// broken tree can produce a failure per entry, and an unbounded `Vec<CompletenessReason>` would
/// turn "this scan could not read anything" into memory pressure inside the process that is
/// supposed to be governing storage (C-11). E21 round-1 independent review recorded this as a
/// required fail-closed operability repair.
///
/// Retention is bounded; the *count* is not. Truncating the count would understate how much of a
/// scope went unobserved, which is the one direction SI-010 does not permit.
pub const MAX_RETAINED_REASONS: usize = 64;

/// A scope's completeness plus the truthful number of paths it could not observe.
///
/// These travel together because they answer one question and must never disagree: an
/// observation is `Complete` exactly when `unobserved_count == 0`. [`ReasonLog`] is the only way
/// to build a non-complete one, so the invariant holds by construction rather than by review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeObservation {
    completeness: ScopeCompleteness,
    unobserved_count: u32,
}

impl ScopeObservation {
    /// A scope that was observed in full.
    pub fn complete() -> Self {
        Self {
            completeness: ScopeCompleteness::Complete,
            unobserved_count: 0,
        }
    }

    pub fn completeness(&self) -> &ScopeCompleteness {
        &self.completeness
    }

    /// Every path this scope failed to observe, including any beyond [`MAX_RETAINED_REASONS`]
    /// whose individual reason was not retained.
    pub fn unobserved_count(&self) -> u32 {
        self.unobserved_count
    }

    pub fn is_complete(&self) -> bool {
        self.completeness.is_complete()
    }

    /// The retained reasons, for explanation. Never a substitute for
    /// [`unobserved_count`](Self::unobserved_count) when reporting how much went unseen.
    pub fn retained_reasons(&self) -> &[CompletenessReason] {
        match &self.completeness {
            ScopeCompleteness::Complete => &[],
            ScopeCompleteness::Partial { reasons } | ScopeCompleteness::Unknown { reasons } => {
                reasons
            }
        }
    }
}

/// Accumulates observation failures during a bespoke provider walk and turns them into one
/// [`ScopeObservation`] (ADR-0018).
///
/// `cancellai-inventory`'s own `scan_scope` derives completeness from an [`InventorySnapshot`];
/// the provider adapters keep their layout-specific traversals and have no snapshot to derive
/// from, so this is the shared accumulator they record into instead. Same vocabulary, same
/// invariants, different walk.
#[derive(Debug, Clone, Default)]
pub struct ReasonLog {
    retained: Vec<CompletenessReason>,
    total: u32,
    root_unavailable: bool,
}

impl ReasonLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one path this walk could not observe. Always counted; retained only while under
    /// [`MAX_RETAINED_REASONS`].
    pub fn record(&mut self, reason: CompletenessReason) {
        self.total = self.total.saturating_add(1);
        if self.retained.len() < MAX_RETAINED_REASONS {
            self.retained.push(reason);
        }
    }

    /// Records a failure to observe the scope *root* itself - the strongest form of missing
    /// evidence, which makes the whole observation `Unknown` rather than `Partial`. Distinct
    /// from a root that is simply absent or symlinked, which is a known-empty state and must
    /// not be recorded here at all (SI-009).
    pub fn record_root_unavailable(&mut self, reason: CompletenessReason) {
        self.root_unavailable = true;
        self.record(reason);
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    pub fn total(&self) -> u32 {
        self.total
    }

    pub fn into_observation(self) -> ScopeObservation {
        if self.total == 0 {
            return ScopeObservation::complete();
        }
        let completeness = if self.root_unavailable {
            ScopeCompleteness::Unknown {
                reasons: self.retained,
            }
        } else {
            ScopeCompleteness::Partial {
                reasons: self.retained,
            }
        };
        ScopeObservation {
            completeness,
            unobserved_count: self.total,
        }
    }
}

/// Derives a scope's completeness from everything an [`InventorySnapshot`] already recorded:
/// the root fact (including a *present-but-degraded* root, E04 round-1 repair - see below),
/// every directory-listing error, every `read_dir`-listed-but-unobservable entry
/// ([`crate::scan::FactError`], also an E04 round-1 repair), and every per-file degraded
/// confidence. Nothing here re-touches the filesystem - this is a pure rollup, matching
/// E04-S02's "one traversal per scope."
pub fn derive_completeness(snapshot: &InventorySnapshot) -> ScopeCompleteness {
    let mut reasons = Vec::new();

    match &snapshot.root_fact {
        FactObservation::Absent => {
            return ScopeCompleteness::Unknown {
                reasons: vec![CompletenessReason::ScopeRootUnavailable {
                    path: snapshot.scope_root.clone(),
                    detail: "scope root does not exist".to_string(),
                }],
            };
        }
        FactObservation::Unreadable { reason } => {
            return ScopeCompleteness::Unknown {
                reasons: vec![CompletenessReason::ScopeRootUnavailable {
                    path: snapshot.scope_root.clone(),
                    detail: reason.clone(),
                }],
            };
        }
        // The root itself was observed, but E04 round-1 verifier review found this branch
        // previously ignored whether that observation was itself degraded (e.g. the root's
        // own identity/allocation unsupported or unreadable) - an otherwise-empty scope with
        // a partial root fact reported `Complete`. The root's own reasons are folded into the
        // same rollup as every descendant's below, not treated as a separate case.
        FactObservation::Present(root_facts) => reasons.extend(fact_reasons(root_facts)),
    }

    for error in &snapshot.directory_errors {
        reasons.push(match error.kind {
            DirectoryErrorKind::Disappeared => CompletenessReason::Disappeared {
                path: error.path.clone(),
            },
            DirectoryErrorKind::PermissionDenied => CompletenessReason::PermissionDenied {
                path: error.path.clone(),
            },
            DirectoryErrorKind::Other => CompletenessReason::Io {
                path: error.path.clone(),
                message: error.message.clone(),
            },
        });
    }

    // E04 round-1 repair: a `read_dir`-listed entry whose own observation was `Absent`/
    // `Unreadable` used to be dropped entirely by `scan.rs`, so this rollup never saw it.
    // `FactError` now preserves it explicitly.
    for error in &snapshot.fact_errors {
        reasons.push(match &error.kind {
            FactErrorKind::Disappeared => CompletenessReason::Disappeared {
                path: error.path.clone(),
            },
            FactErrorKind::Unreadable { reason } => CompletenessReason::Io {
                path: error.path.clone(),
                message: reason.clone(),
            },
        });
    }

    for fact in &snapshot.facts {
        reasons.extend(fact_reasons(fact));
    }

    if reasons.is_empty() {
        ScopeCompleteness::Complete
    } else {
        ScopeCompleteness::Partial { reasons }
    }
}

fn fact_reasons(fact: &FileFacts) -> Vec<CompletenessReason> {
    let FactConfidence::Partial { .. } = &fact.confidence else {
        return Vec::new();
    };

    let mut reasons = Vec::new();
    match &fact.identity {
        IdentityObservation::Unsupported { reason } => {
            reasons.push(CompletenessReason::UnsupportedFilesystemFeature {
                path: fact.path.clone(),
                feature: "identity".to_string(),
                detail: reason.clone(),
            });
        }
        IdentityObservation::Unreadable { reason } => {
            reasons.push(CompletenessReason::Io {
                path: fact.path.clone(),
                message: format!("identity: {reason}"),
            });
        }
        IdentityObservation::Absent => {
            reasons.push(CompletenessReason::Disappeared {
                path: fact.path.clone(),
            });
        }
        IdentityObservation::Identity(_) => {}
    }
    if let crate::file_facts::SizeMetric::Unsupported { reason } = &fact.allocated_size {
        reasons.push(CompletenessReason::UnsupportedFilesystemFeature {
            path: fact.path.clone(),
            feature: "allocated_size".to_string(),
            detail: reason.clone(),
        });
    }
    reasons
}

/// A planning-facing view of one scope: candidates *and* the completeness they were produced
/// under, bundled in one struct with no bare-candidates constructor. A caller cannot obtain
/// `candidates` without also receiving `completeness` in the same value - the type shape is
/// what enforces AC2 ("planning cannot erase completeness information"), the same pattern
/// `cancellai-safety::SealedPlan` uses for its own invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningView<'a> {
    pub completeness: ScopeCompleteness,
    pub candidates: Vec<&'a FileFacts>,
}

/// The only way to build a [`PlanningView`] - always derives completeness from the same
/// snapshot the candidates come from, so the two can never drift apart or be assembled from
/// mismatched sources.
pub fn planning_view(snapshot: &InventorySnapshot) -> PlanningView<'_> {
    PlanningView {
        completeness: derive_completeness(snapshot),
        candidates: snapshot.planning_candidates(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::scan_scope;
    use cancellai_platform::{SystemAllocationObserver, SystemFsObserver, SystemIdentityObserver};

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-completeness-test-{label}-{}",
                std::process::id()
            ));
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).expect("create temp root");
            Self(dir)
        }

        fn path(&self, rel: &str) -> PathBuf {
            self.0.join(rel)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn system_scan(root: &std::path::Path) -> InventorySnapshot {
        scan_scope(
            root,
            &SystemFsObserver,
            &SystemIdentityObserver,
            &SystemAllocationObserver,
        )
    }

    // Windows still cannot reach `Complete` today, but no longer for an identity reason:
    // E20-S01 (ADR-0020) implemented real Windows file/volume identity, closing the gap
    // E03-S01 originally disclosed here. `AllocationObserver` remains `Unsupported` on
    // Windows (a separate, out-of-scope Win32 call), so a "fully readable tree" is still
    // `Partial` there - the Windows counterpart below documents that narrower remaining gap
    // rather than weakening this test's Unix assertion. Windows used to lag behind Unix here
    // (E20-S01 implemented real identity but not allocated-size, so a Windows-only variant of
    // this test asserted `Partial` pending that gap) - E20-S05 implemented real Windows
    // allocated-size too (`GetFileInformationByHandleEx(FileStandardInfo)`), closing that gap,
    // so this test is now genuinely cross-platform: real Windows CI confirmed `Complete` here
    // once both capabilities were real, which is what retired the platform-specific variant.
    #[test]
    fn ac1_a_fully_readable_tree_is_complete() {
        let tree = TempTree::new("complete");
        std::fs::create_dir_all(tree.path("a/b")).unwrap();
        std::fs::write(tree.path("a/f.txt"), b"data").unwrap();

        let snapshot = system_scan(&tree.0);
        assert_eq!(derive_completeness(&snapshot), ScopeCompleteness::Complete);
    }

    #[test]
    fn ac1_a_nonexistent_scope_root_is_unknown_not_complete_or_silently_empty() {
        let tree = TempTree::new("missing-root");
        let missing = tree.path("does-not-exist");

        let snapshot = system_scan(&missing);
        match derive_completeness(&snapshot) {
            ScopeCompleteness::Unknown { reasons } => {
                assert!(matches!(
                    reasons.as_slice(),
                    [CompletenessReason::ScopeRootUnavailable { .. }]
                ));
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn ac1_nested_permission_fixture_is_partial_with_a_permission_reason() {
        use std::os::unix::fs::PermissionsExt;

        let tree = TempTree::new("nested-permission");
        std::fs::create_dir_all(tree.path("a/blocked")).unwrap();
        std::fs::write(tree.path("a/blocked/secret.txt"), b"data").unwrap();
        std::fs::write(tree.path("a/visible.txt"), b"ok").unwrap();
        std::fs::set_permissions(
            tree.path("a/blocked"),
            std::fs::Permissions::from_mode(0o000),
        )
        .unwrap();

        let snapshot = system_scan(&tree.0);

        std::fs::set_permissions(
            tree.path("a/blocked"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();

        match derive_completeness(&snapshot) {
            ScopeCompleteness::Partial { reasons } => {
                assert!(
                    reasons.iter().any(|r| matches!(
                        r,
                        CompletenessReason::PermissionDenied { path } if *path == tree.path("a/blocked")
                    )),
                    "expected a PermissionDenied reason for a/blocked, got {reasons:?}"
                );
            }
            other => panic!("expected Partial, got {other:?}"),
        }
    }

    #[test]
    fn ac1_unsupported_identity_on_a_present_file_contributes_a_partial_reason() {
        use cancellai_platform::{
            FileKind, IdentityToken, SyntheticFsObserver, SyntheticIdentityObserver, Timestamp,
        };

        let mut fs = SyntheticFsObserver::new();
        fs.set(
            "/scope",
            cancellai_platform::Observation::Metadata(cancellai_platform::FsMetadata {
                is_dir: true,
                is_symlink: false,
                len: 0,
                modified: Timestamp(1_000),
            }),
        );
        fs.set(
            "/scope/child",
            cancellai_platform::Observation::Metadata(cancellai_platform::FsMetadata {
                is_dir: false,
                is_symlink: false,
                len: 10,
                modified: Timestamp(1_000),
            }),
        );
        let mut identity = SyntheticIdentityObserver::new();
        identity.set(
            "/scope",
            IdentityObservation::Identity(IdentityToken::Unix {
                device: 1,
                inode: 1,
                kind: FileKind::Directory,
                modified: Timestamp(1_000),
                modified_nanos: 0,
            }),
        );
        identity.set(
            "/scope/child",
            IdentityObservation::Unsupported {
                reason: "no verified Windows identity yet".into(),
            },
        );
        let allocation = SystemAllocationObserver;

        // scan_scope needs read_dir against a real filesystem for directory listing; build
        // the snapshot by hand for this synthetic-identity case instead of a real temp tree.
        let root_fact = crate::file_facts::observe_file_facts(
            std::path::Path::new("/scope"),
            &fs,
            &identity,
            &allocation,
            None,
        );
        let child_fact = crate::file_facts::observe_file_facts(
            std::path::Path::new("/scope/child"),
            &fs,
            &identity,
            &allocation,
            Some(1),
        );
        let snapshot = InventorySnapshot {
            scope_root: PathBuf::from("/scope"),
            root_device: Some(1),
            root_fact,
            facts: vec![match child_fact {
                FactObservation::Present(f) => *f,
                other => panic!("expected Present, got {other:?}"),
            }],
            directory_errors: Vec::new(),
            fact_errors: Vec::new(),
            directories_visited: 1,
            paths_observed: 1,
        };

        match derive_completeness(&snapshot) {
            ScopeCompleteness::Partial { reasons } => {
                assert!(reasons.iter().any(|r| matches!(
                    r,
                    CompletenessReason::UnsupportedFilesystemFeature { feature, .. } if feature == "identity"
                )));
            }
            other => panic!("expected Partial, got {other:?}"),
        }
    }

    #[test]
    fn ac2_planning_view_always_carries_completeness_alongside_candidates() {
        let tree = TempTree::new("planning");
        std::fs::write(tree.path("f.txt"), b"data").unwrap();
        let snapshot = system_scan(&tree.0);

        let view = planning_view(&snapshot);
        assert_eq!(view.completeness, derive_completeness(&snapshot));
        assert_eq!(view.candidates.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn ac2_a_degraded_scope_planning_view_still_reports_partial_not_complete() {
        use std::os::unix::fs::PermissionsExt;

        let tree = TempTree::new("planning-degraded");
        std::fs::create_dir_all(tree.path("blocked")).unwrap();
        std::fs::write(tree.path("blocked/secret.txt"), b"data").unwrap();
        std::fs::set_permissions(tree.path("blocked"), std::fs::Permissions::from_mode(0o000))
            .unwrap();

        let snapshot = system_scan(&tree.0);
        std::fs::set_permissions(tree.path("blocked"), std::fs::Permissions::from_mode(0o755))
            .unwrap();

        let view = planning_view(&snapshot);
        assert!(
            !view.completeness.is_complete(),
            "a scope with a permission-blocked subdirectory must never report Complete"
        );
    }

    #[test]
    fn idempotent_completeness_across_two_scans_of_an_unchanged_tree() {
        let tree = TempTree::new("idempotent");
        std::fs::create_dir_all(tree.path("a")).unwrap();
        std::fs::write(tree.path("a/f.txt"), b"stable").unwrap();

        let first = derive_completeness(&system_scan(&tree.0));
        let second = derive_completeness(&system_scan(&tree.0));
        assert_eq!(first, second);
    }

    #[test]
    fn a_disappeared_directory_is_classified_distinctly_from_permission_denied() {
        // A true listing-to-read disappearance race is impractical to construct reliably
        // against a real filesystem in a sandboxed test; the classification itself
        // (`classify_io_error` in scan.rs) is exercised directly here against a
        // synthesized NotFound error, proving Disappeared and PermissionDenied produce
        // distinct CompletenessReason variants rather than collapsing into one generic
        // "directory error" (documented residual: no end-to-end race is exercised).
        let disappeared_error = crate::scan::DirectoryError {
            path: PathBuf::from("/scope/vanished"),
            kind: DirectoryErrorKind::Disappeared,
            message: "No such file or directory".to_string(),
        };
        let permission_error = crate::scan::DirectoryError {
            path: PathBuf::from("/scope/locked"),
            kind: DirectoryErrorKind::PermissionDenied,
            message: "Permission denied".to_string(),
        };

        let snapshot = InventorySnapshot {
            scope_root: PathBuf::from("/scope"),
            root_device: Some(1),
            root_fact: FactObservation::Present(Box::new(FileFacts {
                path: PathBuf::from("/scope"),
                kind: cancellai_platform::FileKind::Directory,
                identity: IdentityObservation::Identity(cancellai_platform::IdentityToken::Unix {
                    device: 1,
                    inode: 1,
                    kind: cancellai_platform::FileKind::Directory,
                    modified: cancellai_platform::Timestamp(1_000),
                    modified_nanos: 0,
                }),
                logical_size: crate::file_facts::SizeMetric::Known { bytes: 0 },
                allocated_size: crate::file_facts::SizeMetric::Known { bytes: 0 },
                modified: Some(cancellai_platform::Timestamp(1_000)),
                boundary: crate::file_facts::ScopeBoundary::Unscoped,
                provider_hint: None,
                category_hint: None,
                confidence: FactConfidence::Complete,
            })),
            facts: Vec::new(),
            directory_errors: vec![disappeared_error, permission_error],
            fact_errors: Vec::new(),
            directories_visited: 1,
            paths_observed: 0,
        };

        match derive_completeness(&snapshot) {
            ScopeCompleteness::Partial { reasons } => {
                assert!(reasons
                    .iter()
                    .any(|r| matches!(r, CompletenessReason::Disappeared { path } if path == std::path::Path::new("/scope/vanished"))));
                assert!(reasons
                    .iter()
                    .any(|r| matches!(r, CompletenessReason::PermissionDenied { path } if path == std::path::Path::new("/scope/locked"))));
            }
            other => panic!("expected Partial, got {other:?}"),
        }
    }

    #[test]
    fn ac1_an_unreadable_listed_child_makes_the_scope_partial_not_complete() {
        // E04 round-1 verifier repair: reproduces the exact reported bypass - a child listed
        // by `read_dir` but reported `Unreadable` by direct observation used to vanish from
        // the snapshot entirely, so this scope was wrongly reported `Complete`.
        use crate::test_doubles::OverrideFsObserver;
        use cancellai_platform::Observation;

        let tree = TempTree::new("unreadable-child-completeness");
        std::fs::write(tree.path("child.txt"), b"data").unwrap();

        let real_fs = SystemFsObserver;
        let mut fs = OverrideFsObserver::new(&real_fs);
        fs.set(
            tree.path("child.txt"),
            Observation::Unreadable {
                reason: "injected: permission denied".into(),
            },
        );

        let snapshot = scan_scope(
            &tree.0,
            &fs,
            &SystemIdentityObserver,
            &SystemAllocationObserver,
        );

        match derive_completeness(&snapshot) {
            ScopeCompleteness::Partial { reasons } => {
                assert!(
                    reasons.iter().any(|r| matches!(
                        r,
                        CompletenessReason::Io { path, .. } if *path == tree.path("child.txt")
                    )),
                    "expected an Io reason for the unreadable child, got {reasons:?}"
                );
            }
            other => panic!("an unreadable listed child must never report Complete, got {other:?}"),
        }
    }

    #[test]
    fn ac1_a_child_that_disappears_between_listing_and_observation_makes_the_scope_partial() {
        use crate::test_doubles::OverrideFsObserver;
        use cancellai_platform::Observation;

        let tree = TempTree::new("disappeared-child-completeness");
        std::fs::write(tree.path("child.txt"), b"data").unwrap();

        let real_fs = SystemFsObserver;
        let mut fs = OverrideFsObserver::new(&real_fs);
        fs.set(tree.path("child.txt"), Observation::Absent);

        let snapshot = scan_scope(
            &tree.0,
            &fs,
            &SystemIdentityObserver,
            &SystemAllocationObserver,
        );

        match derive_completeness(&snapshot) {
            ScopeCompleteness::Partial { reasons } => {
                assert!(reasons.iter().any(|r| matches!(
                    r,
                    CompletenessReason::Disappeared { path } if *path == tree.path("child.txt")
                )));
            }
            other => panic!("a disappeared listed child must never report Complete, got {other:?}"),
        }
    }

    #[test]
    fn ac1_a_degraded_empty_root_is_partial_not_complete() {
        // E04 round-1 verifier repair: `derive_completeness` previously only inspected
        // `snapshot.facts` (descendants), never the root fact's own confidence - an
        // otherwise-empty scope whose root identity could not be established reported
        // `Complete` (there was nothing in `facts` to contribute a reason).
        use crate::test_doubles::OverrideIdentityObserver;
        use cancellai_platform::IdentityObservation;

        let tree = TempTree::new("degraded-empty-root");

        let real_identity = SystemIdentityObserver;
        let mut identity = OverrideIdentityObserver::new(&real_identity);
        identity.set(
            &tree.0,
            IdentityObservation::Unsupported {
                reason: "injected: no verified identity for this root".into(),
            },
        );

        let snapshot = scan_scope(
            &tree.0,
            &SystemFsObserver,
            &identity,
            &SystemAllocationObserver,
        );

        assert!(
            snapshot.facts.is_empty(),
            "this fixture is deliberately an empty scope"
        );
        match derive_completeness(&snapshot) {
            ScopeCompleteness::Partial { reasons } => {
                assert!(reasons.iter().any(|r| matches!(
                    r,
                    CompletenessReason::UnsupportedFilesystemFeature { path, feature, .. }
                        if *path == tree.0 && feature == "identity"
                )));
            }
            other => panic!("a degraded empty root must never report Complete, got {other:?}"),
        }
    }

    #[test]
    fn ac2_planning_view_of_a_degraded_scope_never_hides_completeness_behind_empty_candidates() {
        // The strongest form of AC2: even when `candidates` is empty (nothing to plan over),
        // `completeness` must still surface the degradation - an empty `Vec` must never be
        // mistaken for "nothing was wrong."
        use crate::test_doubles::OverrideIdentityObserver;
        use cancellai_platform::IdentityObservation;

        let tree = TempTree::new("degraded-empty-root-planning");
        let real_identity = SystemIdentityObserver;
        let mut identity = OverrideIdentityObserver::new(&real_identity);
        identity.set(
            &tree.0,
            IdentityObservation::Unsupported {
                reason: "injected: no verified identity for this root".into(),
            },
        );

        let snapshot = scan_scope(
            &tree.0,
            &SystemFsObserver,
            &identity,
            &SystemAllocationObserver,
        );
        let view = planning_view(&snapshot);

        assert!(view.candidates.is_empty());
        assert!(!view.completeness.is_complete());
    }

    // ----------------------------------------------------------------------------------
    // E21 round-1 independent review: an unbounded reason vector is its own operability
    // failure on a hostile or simply broken tree (C-11). Retention is bounded; the count is not.
    // ----------------------------------------------------------------------------------

    fn permission_reason(index: usize) -> CompletenessReason {
        CompletenessReason::PermissionDenied {
            path: PathBuf::from(format!("/synthetic/{index}")),
        }
    }

    #[test]
    fn an_empty_reason_log_observes_a_complete_scope() {
        let observation = ReasonLog::new().into_observation();
        assert_eq!(observation, ScopeObservation::complete());
        assert_eq!(observation.unobserved_count(), 0);
        assert!(observation.retained_reasons().is_empty());
    }

    #[test]
    fn reason_retention_is_bounded_but_the_count_is_not() {
        let mut log = ReasonLog::new();
        let recorded = MAX_RETAINED_REASONS * 10 + 7;
        for index in 0..recorded {
            log.record(permission_reason(index));
        }
        let observation = log.into_observation();

        assert_eq!(
            observation.unobserved_count(),
            recorded as u32,
            "truncating the count would understate how much of the scope went unobserved - the \
             one direction SI-010 does not permit"
        );
        assert_eq!(
            observation.retained_reasons().len(),
            MAX_RETAINED_REASONS,
            "retention must stop at the cap so a failure-per-entry tree cannot grow this vector \
             without bound"
        );
        assert!(!observation.is_complete());
    }

    #[test]
    fn a_root_failure_makes_the_observation_unknown_not_partial() {
        let mut log = ReasonLog::new();
        log.record_root_unavailable(CompletenessReason::ScopeRootUnavailable {
            path: PathBuf::from("/synthetic/root"),
            detail: "permission denied".to_string(),
        });
        log.record(permission_reason(1));
        let observation = log.into_observation();

        assert!(
            matches!(
                observation.completeness(),
                ScopeCompleteness::Unknown { .. }
            ),
            "a scope whose root could not be observed is Unknown: nothing at all was seen"
        );
        assert_eq!(observation.unobserved_count(), 2);
    }

    #[test]
    fn a_non_root_failure_makes_the_observation_partial() {
        let mut log = ReasonLog::new();
        log.record(permission_reason(1));
        assert!(matches!(
            log.into_observation().completeness(),
            ScopeCompleteness::Partial { .. }
        ));
    }
}

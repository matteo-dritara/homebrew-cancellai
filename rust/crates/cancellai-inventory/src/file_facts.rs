//! `FileFacts`: the per-path observed evidence record (E04-S01,
//! `docs/architecture/DOMAIN_MODEL.md`'s `AgentArtifact` "Minimum conceptual fields" -
//! `LogicalSize`, `AllocatedSize?`, "Observed timestamps", `ArtifactType`, `IdentityToken`).
//!
//! `FileFacts` is deliberately *not* `AgentArtifact` itself. `AgentArtifact` also carries
//! `RiskClass`, `Reversibility`, `KnowledgeConfidence`, the lifecycle axes, and
//! `AuthorityCeiling` - classification decisions that require provider/policy knowledge
//! (E05/E06) this crate does not have. `FileFacts` is the OBSERVE-stage evidence
//! (`docs/architecture/TARGET.md`'s "Core loop") that a future CLASSIFY stage will consume to
//! build an `AgentArtifact`; it never invents a classification of its own.
//!
//! Every metric that a platform/filesystem cannot report is an explicit typed variant, never
//! a fabricated zero or a silent substitution of a different metric (SI-008, SI-009, SI-010) -
//! this mirrors `cancellai-platform`'s own `Observation`/`IdentityObservation`/
//! `AllocationObservation` split, which `observe_file_facts` composes rather than
//! reimplements.

use std::path::{Path, PathBuf};

use cancellai_platform::{
    AllocationObservation, AllocationObserver, FileKind, FsObserver, IdentityObservation,
    IdentityObserver, Observation, Timestamp,
};

/// A size metric that a platform/filesystem may be unable to report. Never collapsed to `0`
/// when unsupported - `SizeMetric::Unsupported` is a distinct, explicit value (SI-008/SI-009
/// generalized to per-metric evidence, per this story's AC2).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SizeMetric {
    Known { bytes: u64 },
    Unsupported { reason: String },
}

/// Whether this path sits within the same filesystem/volume as the scope it is being
/// observed for, or crosses a boundary (`docs/architecture/PLATFORM_MODEL.md` "Boundary
/// rules", SI-018). Computed relative to a caller-supplied scope device, not invented here -
/// crossing a boundary is a traversal-level decision (E04-S02), this only records the fact.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ScopeBoundary {
    /// No scope device was supplied to compare against (e.g. this fact is itself the scope
    /// root, or the caller has not established a scope yet).
    Unscoped,
    WithinScope,
    CrossesBoundary,
    /// This path's own device could not be established, so boundary membership cannot be
    /// determined either (SI-017: an unsupported/unreadable identity is never treated as
    /// "same device" by default).
    Unknown {
        reason: String,
    },
}

/// How much of `FileFacts` was actually observed successfully. A per-path echo of exactly
/// which sub-observations degraded, so a scope-level completeness rollup (E04-S03) has real
/// evidence to aggregate rather than an opaque boolean.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FactConfidence {
    /// Every sub-observation (metadata, identity, allocation) succeeded.
    Complete,
    /// The path itself was observed, but at least one sub-observation degraded. Every
    /// reason names which one and why - never summarized away (SI-010).
    Partial { reasons: Vec<String> },
}

/// One observed path's evidence. Only constructed for a path that exists in some observable
/// form - see [`FactObservation`] for the absent/unreadable outer states.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FileFacts {
    pub path: PathBuf,
    pub kind: FileKind,
    /// The full identity observation, not just a token - `Unreadable`/`Unsupported` here is
    /// itself safety-relevant evidence a caller must see, not a detail to unwrap and discard.
    pub identity: IdentityObservation,
    pub logical_size: SizeMetric,
    pub allocated_size: SizeMetric,
    pub modified: Option<Timestamp>,
    pub boundary: ScopeBoundary,
    /// Populated by a future provider-adapter epic (E05); always `None` here. The field
    /// exists now so `FileFacts`'s shape does not need to change when that epic lands.
    pub provider_hint: Option<String>,
    /// Populated by a future classification stage; always `None` here, same rationale as
    /// `provider_hint`.
    pub category_hint: Option<String>,
    pub confidence: FactConfidence,
}

/// The outer result of observing one path. Mirrors
/// [`cancellai_platform::Observation`]/[`IdentityObservation`]'s absent-vs-unreadable split:
/// a path that does not exist and a path that could not be examined are never conflated, and
/// neither collapses into an empty/default `FileFacts` (SI-008/SI-009/SI-010).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FactObservation {
    Absent,
    Present(FileFacts),
    Unreadable { reason: String },
}

/// Observes one path's facts by composing three independent platform seams
/// (`FsObserver`, `IdentityObserver`, `AllocationObserver`) - each contributes only the
/// metric it owns, and none is invented from another when it is unavailable.
///
/// `scope_device` is the device of the traversal scope this path is being observed for
/// (`None` if the caller has not established one, e.g. observing a single path outside any
/// scope). It is used only to compute [`ScopeBoundary`]; it never gates whether a fact is
/// produced at all.
pub fn observe_file_facts(
    path: &Path,
    fs: &dyn FsObserver,
    identity: &dyn IdentityObserver,
    allocation: &dyn AllocationObserver,
    scope_device: Option<u64>,
) -> FactObservation {
    let metadata = match fs.observe(path) {
        Observation::Absent => return FactObservation::Absent,
        Observation::Unreadable { reason } => return FactObservation::Unreadable { reason },
        Observation::Metadata(metadata) => metadata,
    };

    let mut reasons = Vec::new();

    let identity_observation = identity.observe(path);
    let kind = match &identity_observation {
        IdentityObservation::Identity(token) => token.kind(),
        _ => {
            // Fall back to FsObserver's own coarse kind (it cannot distinguish "other", but
            // file/dir/symlink are enough to still report a usable fact) - identity's
            // richer classification, or its absence, is recorded separately below.
            if metadata.is_symlink {
                FileKind::Symlink
            } else if metadata.is_dir {
                FileKind::Directory
            } else {
                FileKind::File
            }
        }
    };
    match &identity_observation {
        IdentityObservation::Identity(_) => {}
        IdentityObservation::Absent => {
            // A TOCTOU race between the two observations above (the path vanished between
            // the FsObserver stat and the IdentityObserver stat) - not a classification
            // this fact can trust; recorded as degraded rather than silently using the
            // FsObserver's now-stale metadata as if identity had confirmed it.
            reasons.push("identity observation raced: path present for FsObserver but absent for IdentityObserver".to_string());
        }
        IdentityObservation::Unreadable { reason } => {
            reasons.push(format!("identity unreadable: {reason}"));
        }
        IdentityObservation::Unsupported { reason } => {
            reasons.push(format!("identity unsupported: {reason}"));
        }
    }

    let allocated_size = match allocation.observe(path) {
        AllocationObservation::Allocated(bytes) => SizeMetric::Known { bytes },
        AllocationObservation::Absent => {
            reasons.push(
                "allocation observation raced: path present for FsObserver but absent for AllocationObserver"
                    .to_string(),
            );
            SizeMetric::Unsupported {
                reason: "allocation observer reported the path absent".to_string(),
            }
        }
        AllocationObservation::Unreadable { reason } => {
            reasons.push(format!("allocation unreadable: {reason}"));
            SizeMetric::Unsupported {
                reason: reason.clone(),
            }
        }
        AllocationObservation::Unsupported { reason } => {
            reasons.push(format!("allocation unsupported: {reason}"));
            SizeMetric::Unsupported { reason }
        }
    };

    let boundary = match (scope_device, &identity_observation) {
        (None, _) => ScopeBoundary::Unscoped,
        (Some(scope_device), IdentityObservation::Identity(token)) => {
            if token.device() == scope_device {
                ScopeBoundary::WithinScope
            } else {
                ScopeBoundary::CrossesBoundary
            }
        }
        (Some(_), IdentityObservation::Unsupported { reason }) => ScopeBoundary::Unknown {
            reason: reason.clone(),
        },
        (Some(_), IdentityObservation::Unreadable { reason }) => ScopeBoundary::Unknown {
            reason: reason.clone(),
        },
        (Some(_), IdentityObservation::Absent) => ScopeBoundary::Unknown {
            reason: "identity observation raced: path became absent".to_string(),
        },
    };

    let confidence = if reasons.is_empty() {
        FactConfidence::Complete
    } else {
        FactConfidence::Partial { reasons }
    };

    FactObservation::Present(FileFacts {
        path: path.to_path_buf(),
        kind,
        identity: identity_observation,
        logical_size: SizeMetric::Known {
            bytes: metadata.len,
        },
        allocated_size,
        modified: Some(metadata.modified),
        boundary,
        provider_hint: None,
        category_hint: None,
        confidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_platform::{
        FileKind, FsMetadata, IdentityToken, SyntheticAllocationObserver, SyntheticFsObserver,
        SyntheticIdentityObserver,
    };

    struct Fixture {
        fs: SyntheticFsObserver,
        identity: SyntheticIdentityObserver,
        allocation: SyntheticAllocationObserver,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                fs: SyntheticFsObserver::new(),
                identity: SyntheticIdentityObserver::new(),
                allocation: SyntheticAllocationObserver::new(),
            }
        }

        fn observe(&self, path: &str, scope_device: Option<u64>) -> FactObservation {
            observe_file_facts(
                Path::new(path),
                &self.fs,
                &self.identity,
                &self.allocation,
                scope_device,
            )
        }
    }

    fn present(observation: FactObservation) -> FileFacts {
        match observation {
            FactObservation::Present(facts) => facts,
            other => panic!("expected Present, got {other:?}"),
        }
    }

    #[test]
    fn ac1_a_fully_observed_file_distinguishes_logical_from_allocated_size() {
        let mut fixture = Fixture::new();
        fixture.fs.set(
            "/f",
            Observation::Metadata(FsMetadata {
                is_dir: false,
                is_symlink: false,
                len: 10_000_000,
                modified: Timestamp(1_000),
            }),
        );
        fixture.identity.set(
            "/f",
            IdentityObservation::Identity(IdentityToken::Unix {
                device: 1,
                inode: 42,
                kind: FileKind::File,
                modified: Timestamp(1_000),
            }),
        );
        // A sparse file: allocated is far smaller than logical - these must not be equal
        // by construction, proving the two metrics are genuinely independent.
        fixture
            .allocation
            .set("/f", AllocationObservation::Allocated(4_096));

        let facts = present(fixture.observe("/f", None));
        assert_eq!(facts.logical_size, SizeMetric::Known { bytes: 10_000_000 });
        assert_eq!(facts.allocated_size, SizeMetric::Known { bytes: 4_096 });
        assert_ne!(facts.logical_size, facts.allocated_size);
        assert_eq!(facts.confidence, FactConfidence::Complete);
    }

    #[test]
    fn ac2_unsupported_allocation_is_an_explicit_value_never_a_fabricated_zero_or_logical_copy() {
        let mut fixture = Fixture::new();
        fixture.fs.set(
            "/f",
            Observation::Metadata(FsMetadata {
                is_dir: false,
                is_symlink: false,
                len: 500,
                modified: Timestamp(1_000),
            }),
        );
        fixture.identity.set(
            "/f",
            IdentityObservation::Identity(IdentityToken::Unix {
                device: 1,
                inode: 1,
                kind: FileKind::File,
                modified: Timestamp(1_000),
            }),
        );
        fixture.allocation.set(
            "/f",
            AllocationObservation::Unsupported {
                reason: "no allocation metric on this filesystem".into(),
            },
        );

        let facts = present(fixture.observe("/f", None));
        assert_eq!(
            facts.allocated_size,
            SizeMetric::Unsupported {
                reason: "no allocation metric on this filesystem".into()
            }
        );
        assert_ne!(facts.allocated_size, SizeMetric::Known { bytes: 0 });
        assert_ne!(facts.allocated_size, facts.logical_size);
        match facts.confidence {
            FactConfidence::Partial { reasons } => {
                assert!(reasons.iter().any(|r| r.contains("allocation unsupported")));
            }
            other => panic!("expected Partial confidence, got {other:?}"),
        }
    }

    #[test]
    fn absent_path_produces_no_fact() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.observe("/never/configured", None),
            FactObservation::Absent
        );
    }

    #[test]
    fn unreadable_path_is_reported_not_collapsed_to_absent_or_empty() {
        let mut fixture = Fixture::new();
        fixture.fs.set(
            "/locked",
            Observation::Unreadable {
                reason: "permission denied".into(),
            },
        );
        assert_eq!(
            fixture.observe("/locked", None),
            FactObservation::Unreadable {
                reason: "permission denied".into()
            }
        );
    }

    #[test]
    fn unsupported_identity_still_produces_a_usable_fact_with_degraded_confidence() {
        let mut fixture = Fixture::new();
        fixture.fs.set(
            "/f",
            Observation::Metadata(FsMetadata {
                is_dir: false,
                is_symlink: false,
                len: 100,
                modified: Timestamp(1_000),
            }),
        );
        fixture.identity.set(
            "/f",
            IdentityObservation::Unsupported {
                reason: "no verified Windows identity yet".into(),
            },
        );
        fixture
            .allocation
            .set("/f", AllocationObservation::Allocated(512));

        let facts = present(fixture.observe("/f", None));
        assert_eq!(facts.logical_size, SizeMetric::Known { bytes: 100 });
        assert_eq!(
            facts.identity,
            IdentityObservation::Unsupported {
                reason: "no verified Windows identity yet".into()
            }
        );
        match facts.confidence {
            FactConfidence::Partial { reasons } => {
                assert!(reasons.iter().any(|r| r.contains("identity unsupported")));
            }
            other => panic!("expected Partial confidence, got {other:?}"),
        }
    }

    #[test]
    fn boundary_within_scope_when_device_matches() {
        let mut fixture = Fixture::new();
        fixture.fs.set(
            "/scope/child",
            Observation::Metadata(FsMetadata {
                is_dir: false,
                is_symlink: false,
                len: 1,
                modified: Timestamp(1_000),
            }),
        );
        fixture.identity.set(
            "/scope/child",
            IdentityObservation::Identity(IdentityToken::Unix {
                device: 7,
                inode: 1,
                kind: FileKind::File,
                modified: Timestamp(1_000),
            }),
        );
        fixture
            .allocation
            .set("/scope/child", AllocationObservation::Allocated(512));

        let facts = present(fixture.observe("/scope/child", Some(7)));
        assert_eq!(facts.boundary, ScopeBoundary::WithinScope);
    }

    #[test]
    fn boundary_crosses_when_device_differs_a_mount_point_child() {
        let mut fixture = Fixture::new();
        fixture.fs.set(
            "/scope/mounted",
            Observation::Metadata(FsMetadata {
                is_dir: true,
                is_symlink: false,
                len: 0,
                modified: Timestamp(1_000),
            }),
        );
        fixture.identity.set(
            "/scope/mounted",
            IdentityObservation::Identity(IdentityToken::Unix {
                device: 99,
                inode: 1,
                kind: FileKind::Directory,
                modified: Timestamp(1_000),
            }),
        );
        fixture
            .allocation
            .set("/scope/mounted", AllocationObservation::Allocated(0));

        let facts = present(fixture.observe("/scope/mounted", Some(7)));
        assert_eq!(facts.boundary, ScopeBoundary::CrossesBoundary);
    }

    #[test]
    fn boundary_unknown_when_identity_cannot_be_established_never_assumed_within_scope() {
        let mut fixture = Fixture::new();
        fixture.fs.set(
            "/scope/weird",
            Observation::Metadata(FsMetadata {
                is_dir: false,
                is_symlink: false,
                len: 1,
                modified: Timestamp(1_000),
            }),
        );
        fixture.identity.set(
            "/scope/weird",
            IdentityObservation::Unsupported {
                reason: "no verified Windows identity yet".into(),
            },
        );
        fixture
            .allocation
            .set("/scope/weird", AllocationObservation::Allocated(1));

        let facts = present(fixture.observe("/scope/weird", Some(7)));
        // SI-017: an unsupported identity is never silently treated as "same device".
        assert_ne!(facts.boundary, ScopeBoundary::WithinScope);
        match facts.boundary {
            ScopeBoundary::Unknown { .. } => {}
            other => panic!("expected Unknown boundary, got {other:?}"),
        }
    }

    #[test]
    fn unscoped_when_caller_supplies_no_scope_device() {
        let mut fixture = Fixture::new();
        fixture.fs.set(
            "/f",
            Observation::Metadata(FsMetadata {
                is_dir: false,
                is_symlink: false,
                len: 1,
                modified: Timestamp(1_000),
            }),
        );
        fixture.identity.set(
            "/f",
            IdentityObservation::Identity(IdentityToken::Unix {
                device: 1,
                inode: 1,
                kind: FileKind::File,
                modified: Timestamp(1_000),
            }),
        );
        fixture
            .allocation
            .set("/f", AllocationObservation::Allocated(1));

        let facts = present(fixture.observe("/f", None));
        assert_eq!(facts.boundary, ScopeBoundary::Unscoped);
    }
}

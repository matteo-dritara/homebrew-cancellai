//! Reclaimability estimator (E10-S01, `docs/architecture/PLATFORM_MODEL.md`'s "logical and
//! allocated-size observation" extended to reclaim accounting).
//!
//! [`crate::file_facts::FileFacts`] already keeps logical size and allocated size as two
//! distinct, honestly-observed metrics (E04-S01). Summing allocated size across many files is
//! a *further* claim - "deleting these files would free approximately this many bytes" - that
//! is only as trustworthy as two things this module makes explicit rather than assumed:
//!
//! 1. Every file actually had a known allocated size. A file whose allocated size could not be
//!    observed is never silently treated as `0`, and its logical size is never silently
//!    substituted for it (`cancellai_platform::allocation`'s own contract) - it is excluded
//!    from the sum and counted, so the reported total is a disclosed lower bound rather than
//!    an unknowingly wrong one.
//! 2. The underlying filesystem does not let two files share the same disk blocks. APFS
//!    clones, Btrfs/XFS reflinks, and ZFS clones/dedup can all make this false: deleting one
//!    file may free none of its "allocated" bytes if a surviving clone still references the
//!    same blocks (`cancellai_platform::filesystem_kind::CloneSemantics`). Whenever that
//!    cannot be ruled out, this module labels the whole estimate `Estimated`, never `Verified` -
//!    this is this story's AC2: unknown clone/shared-block effects are never presented as
//!    guaranteed savings.

use cancellai_platform::CloneSemantics;

use crate::file_facts::{FileFacts, SizeMetric};

/// How much to trust an aggregated [`ReclaimEstimate`]. Never a stronger claim than the
/// underlying observations support (SI-008/SI-009 generalized to reclaim accounting).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ReclaimConfidence {
    /// Every counted file had a known allocated size, and the filesystem is not known to
    /// support clone/reflink sharing between files - `allocated_bytes` is a real reclaim
    /// estimate, not merely a lower bound.
    Verified,
    /// At least one condition above did not hold. Every reason names which one and why -
    /// never summarized away (matching `FactConfidence::Partial`'s own contract).
    Estimated { reasons: Vec<String> },
}

/// An aggregated reclaim estimate over a set of [`FileFacts`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReclaimEstimate {
    /// Sum of every counted file's known logical size. Files with an unknown logical size are
    /// excluded, matching `InventorySnapshot::total_logical_size`'s existing convention -
    /// never fabricated as `0`.
    pub logical_bytes: u64,
    /// Sum of every counted file's known allocated size only. Files with an unknown allocated
    /// size are excluded (see `files_with_unknown_allocated_size`) rather than having their
    /// logical size substituted, so this can only ever be a true sum or a disclosed
    /// undercount - never a number inflated by a metric this build never actually observed.
    pub allocated_bytes: u64,
    /// How many counted files had `SizeMetric::Unsupported` allocated size and were therefore
    /// excluded from `allocated_bytes` - the true allocated total for this set is at least
    /// `allocated_bytes`, possibly more.
    pub files_with_unknown_allocated_size: u32,
    pub confidence: ReclaimConfidence,
}

/// Aggregate a reclaim estimate over `facts`, given the clone/reflink capability of the
/// filesystem they live on (one call per scope root - see
/// `cancellai_platform::filesystem_kind::FilesystemKindObserver`, not once per file: a scan
/// scope is bounded to one device, SI-018).
pub fn estimate_reclaim<'a>(
    facts: impl IntoIterator<Item = &'a FileFacts>,
    filesystem: &CloneSemantics,
) -> ReclaimEstimate {
    let mut logical_bytes = 0u64;
    let mut allocated_bytes = 0u64;
    let mut files_with_unknown_allocated_size = 0u32;
    let mut reasons = Vec::new();

    for fact in facts {
        if let SizeMetric::Known { bytes } = fact.logical_size {
            logical_bytes = logical_bytes.saturating_add(bytes);
        }
        match &fact.allocated_size {
            SizeMetric::Known { bytes } => {
                allocated_bytes = allocated_bytes.saturating_add(*bytes);
            }
            SizeMetric::Unsupported { reason } => {
                files_with_unknown_allocated_size += 1;
                reasons.push(format!(
                    "{}: allocated size unknown ({reason}), excluded from allocated_bytes",
                    fact.path.display()
                ));
            }
        }
    }

    match filesystem {
        CloneSemantics::PossiblyShared { filesystem } => {
            reasons.push(format!(
                "filesystem {filesystem} may support clone/reflink sharing between files; \
                 deleting these files may not free all of allocated_bytes if a surviving clone \
                 still references the same blocks"
            ));
        }
        CloneSemantics::Unsupported { reason } => {
            reasons.push(format!(
                "filesystem clone/reflink capability could not be determined ({reason}); \
                 allocated_bytes may overstate real reclaim"
            ));
        }
        CloneSemantics::NotKnownToShare { .. } => {}
    }

    let confidence = if reasons.is_empty() {
        ReclaimConfidence::Verified
    } else {
        ReclaimConfidence::Estimated { reasons }
    };

    ReclaimEstimate {
        logical_bytes,
        allocated_bytes,
        files_with_unknown_allocated_size,
        confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_facts::{FactConfidence, ScopeBoundary};
    use cancellai_platform::{IdentityObservation, IdentityToken, Timestamp};
    use std::path::PathBuf;

    fn fact(path: &str, logical: SizeMetric, allocated: SizeMetric) -> FileFacts {
        FileFacts {
            path: PathBuf::from(path),
            kind: cancellai_platform::FileKind::File,
            identity: IdentityObservation::Identity(IdentityToken::Unix {
                device: 1,
                inode: 1,
                kind: cancellai_platform::FileKind::File,
                modified: Timestamp(0),
                modified_nanos: 0,
            }),
            logical_size: logical,
            allocated_size: allocated,
            modified: Some(Timestamp(0)),
            boundary: ScopeBoundary::Unscoped,
            provider_hint: None,
            category_hint: None,
            confidence: FactConfidence::Complete,
        }
    }

    #[test]
    fn ac1_a_fully_known_estimate_on_a_non_sharing_filesystem_is_verified() {
        let facts = vec![
            fact(
                "/a",
                SizeMetric::Known { bytes: 10_000 },
                SizeMetric::Known { bytes: 4_096 },
            ),
            fact(
                "/b",
                SizeMetric::Known { bytes: 20_000 },
                SizeMetric::Known { bytes: 8_192 },
            ),
        ];
        let filesystem = CloneSemantics::NotKnownToShare {
            filesystem: "ext4".to_string(),
        };

        let estimate = estimate_reclaim(&facts, &filesystem);

        assert_eq!(estimate.logical_bytes, 30_000);
        assert_eq!(estimate.allocated_bytes, 12_288);
        assert_eq!(estimate.files_with_unknown_allocated_size, 0);
        assert_eq!(estimate.confidence, ReclaimConfidence::Verified);
    }

    #[test]
    fn ac2_a_clone_capable_filesystem_is_never_verified_even_with_every_size_known() {
        let facts = vec![fact(
            "/a",
            SizeMetric::Known { bytes: 10_000 },
            SizeMetric::Known { bytes: 10_000 },
        )];
        let filesystem = CloneSemantics::PossiblyShared {
            filesystem: "apfs".to_string(),
        };

        let estimate = estimate_reclaim(&facts, &filesystem);

        assert_eq!(estimate.allocated_bytes, 10_000);
        match estimate.confidence {
            ReclaimConfidence::Estimated { reasons } => {
                assert!(reasons.iter().any(|r| r.contains("apfs")));
            }
            other => panic!("expected Estimated for a clone-capable filesystem, got {other:?}"),
        }
    }

    #[test]
    fn ac2_undetermined_filesystem_capability_is_never_verified() {
        let facts = vec![fact(
            "/a",
            SizeMetric::Known { bytes: 1 },
            SizeMetric::Known { bytes: 1 },
        )];
        let filesystem = CloneSemantics::Unsupported {
            reason: "no verified detector on this platform".to_string(),
        };

        let estimate = estimate_reclaim(&facts, &filesystem);

        match estimate.confidence {
            ReclaimConfidence::Estimated { reasons } => {
                assert!(
                    reasons
                        .iter()
                        .any(|r| r.contains("could not be determined"))
                );
            }
            other => {
                panic!("expected Estimated when filesystem capability is unknown, got {other:?}")
            }
        }
    }

    #[test]
    fn an_unknown_allocated_size_is_excluded_never_substituted_with_logical_size() {
        let facts = vec![fact(
            "/a",
            SizeMetric::Known { bytes: 10_000_000 },
            SizeMetric::Unsupported {
                reason: "no allocation metric on this filesystem".to_string(),
            },
        )];
        let filesystem = CloneSemantics::NotKnownToShare {
            filesystem: "ext4".to_string(),
        };

        let estimate = estimate_reclaim(&facts, &filesystem);

        // Never the fabricated logical-size substitute this crate's own AllocationObserver
        // docs explicitly forbid.
        assert_eq!(estimate.allocated_bytes, 0);
        assert_eq!(estimate.files_with_unknown_allocated_size, 1);
        match estimate.confidence {
            ReclaimConfidence::Estimated { reasons } => {
                assert!(
                    reasons
                        .iter()
                        .any(|r| r.contains("excluded from allocated_bytes"))
                );
            }
            other => panic!("expected Estimated for an unknown allocated size, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_set_on_a_non_sharing_filesystem_is_a_verified_zero() {
        let facts: Vec<FileFacts> = Vec::new();
        let filesystem = CloneSemantics::NotKnownToShare {
            filesystem: "ext4".to_string(),
        };

        let estimate = estimate_reclaim(&facts, &filesystem);

        assert_eq!(estimate.logical_bytes, 0);
        assert_eq!(estimate.allocated_bytes, 0);
        assert_eq!(estimate.confidence, ReclaimConfidence::Verified);
    }
}

//! Shared root-fingerprint vocabulary (ported from `cancellai.py`'s `RootAuthority`, which is
//! itself one dataclass shared across both tools via a `tool: str` field - not duplicated per
//! provider). E05-S04 factors this out of `cancellai-provider-claude` (E05-S03) once a second
//! adapter needed the identical origin/confidence vocabulary and derivation rule, rather than
//! duplicating both per adapter crate.

/// Whether a candidate root is the provider's OS-default directory or an operator-supplied
/// custom one (`cancellai.py`'s `RootAuthority.origin`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootOrigin {
    Default,
    Custom,
}

/// A root's fingerprint confidence (`cancellai.py`'s `RootAuthority.confidence` four-value
/// vocabulary, unchanged): this is the source vocabulary an adapter's
/// `ProviderCapabilities::capability` maps onto `SupportState`/`KnowledgeConfidence`, not a
/// replacement for either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootConfidence {
    /// The provider's own default directory - authoritative by definition, including when
    /// empty or absent on a fresh machine.
    Default,
    /// A custom root with at least one identifying marker and at least two markers overall.
    High,
    /// A custom root with some marker evidence, but not enough to be `High`.
    Low,
    /// A custom root with no recognized marker at all.
    Unknown,
}

/// A candidate root's fingerprint: which markers were found (an adapter sorts and supplies
/// these; this type does not enforce sort order itself) and the origin/confidence derived from
/// them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootFingerprint {
    pub origin: RootOrigin,
    pub confidence: RootConfidence,
    pub markers: Vec<&'static str>,
}

/// The confidence-derivation rule every adapter's `fingerprint_root` uses, identical to
/// `cancellai.py`'s own (shared, not duplicated per tool there either): the default root is
/// always `Default`; otherwise at least one identifying marker plus at least two markers
/// overall reaches `High`; any marker evidence at all without meeting that bar is `Low`; no
/// marker evidence is `Unknown`.
pub fn derive_root_confidence(
    is_default_root: bool,
    identifying_markers_found: usize,
    total_markers_found: usize,
) -> RootConfidence {
    if is_default_root {
        RootConfidence::Default
    } else if identifying_markers_found >= 1 && total_markers_found >= 2 {
        RootConfidence::High
    } else if total_markers_found > 0 {
        RootConfidence::Low
    } else {
        RootConfidence::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_root_is_always_default_confidence_regardless_of_markers() {
        assert_eq!(derive_root_confidence(true, 0, 0), RootConfidence::Default);
        assert_eq!(derive_root_confidence(true, 5, 5), RootConfidence::Default);
    }

    #[test]
    fn a_custom_root_needs_one_identifying_and_two_total_markers_for_high_confidence() {
        assert_eq!(derive_root_confidence(false, 1, 2), RootConfidence::High);
        assert_eq!(derive_root_confidence(false, 1, 1), RootConfidence::Low);
        assert_eq!(derive_root_confidence(false, 0, 2), RootConfidence::Low);
        assert_eq!(derive_root_confidence(false, 0, 0), RootConfidence::Unknown);
    }
}

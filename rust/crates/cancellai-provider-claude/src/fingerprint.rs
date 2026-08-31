//! Claude Code provider-root fingerprinting (ported from `cancellai.py`'s
//! `ROOT_MARKERS["claude"]`/`fingerprint_root`, E05-S03, SI-002 "Provider root must be
//! positively bounded", SI-004 "Unknown provider layout/version reduces capability").
//!
//! Unlike `cancellai.py`, `fingerprint_claude_root` takes `is_default_root` as an explicit
//! parameter rather than reading `CLAUDE_CONFIG_DIR`/`HOME` itself - resolving *which* path is
//! the OS-default Claude home is environment/config-resolution logic that belongs to a caller
//! (a future CLI/config story), not to a pure, synthetic-filesystem-testable fingerprinting
//! function. This is a deliberate, narrow improvement on the Python reference's shape, not a
//! behavioral divergence: given the same `is_default_root` answer, the confidence/marker
//! computation below matches `fingerprint_root` exactly.

use std::path::Path;

use cancellai_provider_api::{
    contains_uuid_named_jsonl, is_dir, is_json_object, is_jsonl_of_objects,
};

struct Marker {
    name: &'static str,
    probe: fn(&Path) -> bool,
    identifying: bool,
}

fn probe_projects(path: &Path) -> bool {
    contains_uuid_named_jsonl(path, "")
}

/// `cancellai.py`'s `ROOT_MARKERS["claude"]`, unchanged in name, probe, and
/// identifying/non-identifying weight.
const CLAUDE_ROOT_MARKERS: &[Marker] = &[
    Marker {
        name: "settings.json",
        probe: is_json_object,
        identifying: true,
    },
    Marker {
        name: "keybindings.json",
        probe: is_json_object,
        identifying: true,
    },
    Marker {
        name: "projects",
        probe: probe_projects,
        identifying: true,
    },
    Marker {
        name: "history.jsonl",
        probe: is_jsonl_of_objects,
        identifying: false,
    },
    Marker {
        name: "file-history",
        probe: is_dir,
        identifying: false,
    },
    Marker {
        name: "shell-snapshots",
        probe: is_dir,
        identifying: false,
    },
    Marker {
        name: "plugins",
        probe: is_dir,
        identifying: false,
    },
    Marker {
        name: "agent-memory",
        probe: is_dir,
        identifying: false,
    },
    Marker {
        name: "session-env",
        probe: is_dir,
        identifying: false,
    },
    Marker {
        name: "tasks",
        probe: is_dir,
        identifying: false,
    },
    Marker {
        name: "statsig",
        probe: is_dir,
        identifying: false,
    },
    Marker {
        name: "todos",
        probe: is_dir,
        identifying: false,
    },
];

/// Mirrors `cancellai.py`'s `RootAuthority.origin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootOrigin {
    Default,
    Custom,
}

/// Mirrors `cancellai.py`'s `RootAuthority.confidence` four-value vocabulary exactly - this
/// is the source vocabulary [`crate::ClaudeProvider::capability`] maps onto
/// `cancellai_provider_api::SupportState`/`cancellai_model::KnowledgeConfidence`, not a
/// replacement for either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootConfidence {
    /// The provider's own default directory - authoritative by definition, including when
    /// empty or absent on a fresh machine (`cancellai.py`'s own comment on this case).
    Default,
    /// A custom root with at least one identifying marker and at least two markers overall.
    High,
    /// A custom root with some marker evidence, but not enough to be `High`.
    Low,
    /// A custom root with no recognized marker at all.
    Unknown,
}

/// A Claude Code root's fingerprint: which markers were found (sorted, matching
/// `cancellai.py`'s `tuple(sorted(found))`) and the confidence/origin derived from them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeRootFingerprint {
    pub origin: RootOrigin,
    pub confidence: RootConfidence,
    pub markers: Vec<&'static str>,
}

/// Fingerprints `path` as a candidate Claude Code root. `is_default_root` is the caller's
/// answer to "is this the OS-default Claude home" (see the module doc for why that
/// determination is not made here).
pub fn fingerprint_claude_root(path: &Path, is_default_root: bool) -> ClaudeRootFingerprint {
    let mut found = Vec::new();
    let mut identifying = 0usize;
    for marker in CLAUDE_ROOT_MARKERS {
        if (marker.probe)(&path.join(marker.name)) {
            found.push(marker.name);
            if marker.identifying {
                identifying += 1;
            }
        }
    }
    found.sort_unstable();

    let confidence = if is_default_root {
        RootConfidence::Default
    } else if identifying >= 1 && found.len() >= 2 {
        RootConfidence::High
    } else if !found.is_empty() {
        RootConfidence::Low
    } else {
        RootConfidence::Unknown
    };

    ClaudeRootFingerprint {
        origin: if is_default_root {
            RootOrigin::Default
        } else {
            RootOrigin::Custom
        },
        confidence,
        markers: found,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-claude-fingerprint-test-{label}-{}",
                std::process::id()
            ));
            fs::remove_dir_all(&dir).ok();
            fs::create_dir_all(&dir).expect("create temp root");
            Self(dir)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn an_empty_directory_has_unknown_confidence_and_no_markers() {
        let tree = TempTree::new("empty");
        let fingerprint = fingerprint_claude_root(&tree.0, false);
        assert_eq!(fingerprint.confidence, RootConfidence::Unknown);
        assert!(fingerprint.markers.is_empty());
    }

    #[test]
    fn a_default_root_is_always_default_confidence_even_when_empty() {
        let tree = TempTree::new("default-empty");
        let fingerprint = fingerprint_claude_root(&tree.0, true);
        assert_eq!(fingerprint.origin, RootOrigin::Default);
        assert_eq!(fingerprint.confidence, RootConfidence::Default);
    }

    #[test]
    fn two_identifying_markers_on_a_custom_root_reach_high_confidence() {
        let tree = TempTree::new("high");
        fs::write(tree.0.join("settings.json"), "{}").unwrap();
        fs::write(tree.0.join("keybindings.json"), "{}").unwrap();
        let fingerprint = fingerprint_claude_root(&tree.0, false);
        assert_eq!(fingerprint.confidence, RootConfidence::High);
        assert_eq!(
            fingerprint.markers,
            vec!["keybindings.json", "settings.json"]
        );
    }

    #[test]
    fn a_single_non_identifying_marker_on_a_custom_root_is_only_low_confidence() {
        let tree = TempTree::new("low");
        fs::create_dir_all(tree.0.join("todos")).unwrap();
        let fingerprint = fingerprint_claude_root(&tree.0, false);
        assert_eq!(fingerprint.confidence, RootConfidence::Low);
        assert_eq!(fingerprint.markers, vec!["todos"]);
    }

    #[test]
    fn a_symlink_whose_target_is_not_a_directory_does_not_satisfy_a_directory_marker() {
        // The exact claude-symlink-protected-name fixture scenario: a case-variant symlink
        // named "Plugins", pointing at a plain *file* outside the root, must not satisfy the
        // "plugins" directory marker - `is_dir` follows the symlink and finds a file, not a
        // directory. (On a case-insensitive filesystem such as APFS, a real directory literally
        // named "Plugins" *would* satisfy a lookup for "plugins" - that is shared, correct
        // filesystem behavior on both the Python reference and this port, not a divergence to
        // guard against here.)
        let tree = TempTree::new("symlink-not-a-dir");
        let outside_file = tree.0.join("outside-payload.txt");
        fs::write(&outside_file, "synthetic content outside the approved root").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside_file, tree.0.join("Plugins")).unwrap();

        #[cfg(unix)]
        {
            let fingerprint = fingerprint_claude_root(&tree.0, false);
            assert!(!fingerprint.markers.contains(&"plugins"));
        }
    }
}

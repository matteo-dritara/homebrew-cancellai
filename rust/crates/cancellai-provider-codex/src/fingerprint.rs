//! Codex CLI provider-root fingerprinting (ported from `cancellai.py`'s
//! `ROOT_MARKERS["codex"]`/`fingerprint_root`, E05-S04, SI-002, SI-004). See
//! `cancellai-provider-claude::fingerprint` for the Claude counterpart and the shared
//! confidence-derivation rationale (`RootConfidence`'s four-value vocabulary, `is_default_root`
//! as an explicit caller-supplied argument rather than an ambient environment read).

use std::path::Path;

use cancellai_provider_api::{
    RootFingerprint, RootOrigin, contains_uuid_named_jsonl, derive_root_confidence, is_dir,
    is_json_object, is_jsonl_of_objects, is_nonempty_file,
};

struct Marker {
    name: &'static str,
    probe: fn(&Path) -> bool,
    identifying: bool,
}

fn probe_sessions(path: &Path) -> bool {
    contains_uuid_named_jsonl(path, "rollout-")
}

/// `cancellai.py`'s `ROOT_MARKERS["codex"]`, unchanged in name, probe, and
/// identifying/non-identifying weight.
const CODEX_ROOT_MARKERS: &[Marker] = &[
    Marker {
        name: "auth.json",
        probe: is_json_object,
        identifying: true,
    },
    Marker {
        name: "session_index.jsonl",
        probe: is_jsonl_of_objects,
        identifying: true,
    },
    Marker {
        name: "installation_id",
        probe: is_nonempty_file,
        identifying: true,
    },
    Marker {
        name: "sessions",
        probe: probe_sessions,
        identifying: true,
    },
    Marker {
        name: "config.toml",
        probe: is_nonempty_file,
        identifying: false,
    },
    Marker {
        name: "archived_sessions",
        probe: is_dir,
        identifying: false,
    },
    Marker {
        name: "history.jsonl",
        probe: is_jsonl_of_objects,
        identifying: false,
    },
    Marker {
        name: "skills",
        probe: is_dir,
        identifying: false,
    },
    Marker {
        name: "rules",
        probe: is_dir,
        identifying: false,
    },
    Marker {
        name: "memories",
        probe: is_dir,
        identifying: false,
    },
    Marker {
        name: "plugins",
        probe: is_dir,
        identifying: false,
    },
    Marker {
        name: "sqlite",
        probe: is_dir,
        identifying: false,
    },
];

/// Fingerprints `path` as a candidate Codex root. `is_default_root` is the caller's answer to
/// "is this the OS-default Codex home".
pub fn fingerprint_codex_root(path: &Path, is_default_root: bool) -> RootFingerprint {
    let mut found = Vec::new();
    let mut identifying = 0usize;
    for marker in CODEX_ROOT_MARKERS {
        if (marker.probe)(&path.join(marker.name)) {
            found.push(marker.name);
            if marker.identifying {
                identifying += 1;
            }
        }
    }
    found.sort_unstable();

    RootFingerprint {
        origin: if is_default_root {
            RootOrigin::Default
        } else {
            RootOrigin::Custom
        },
        confidence: derive_root_confidence(is_default_root, identifying, found.len()),
        markers: found,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_provider_api::RootConfidence;
    use std::fs;
    use std::path::PathBuf;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-codex-fingerprint-test-{label}-{}",
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
    fn an_empty_directory_has_unknown_confidence() {
        let tree = TempTree::new("empty");
        let fingerprint = fingerprint_codex_root(&tree.0, false);
        assert_eq!(fingerprint.confidence, RootConfidence::Unknown);
    }

    #[test]
    fn two_identifying_markers_reach_high_confidence() {
        let tree = TempTree::new("high");
        fs::write(tree.0.join("auth.json"), "{}").unwrap();
        fs::write(tree.0.join("installation_id"), "x").unwrap();
        let fingerprint = fingerprint_codex_root(&tree.0, false);
        assert_eq!(fingerprint.confidence, RootConfidence::High);
    }
}

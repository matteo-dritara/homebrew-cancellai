//! Tool-agnostic, read-only provider-root marker probes (ported from `cancellai.py`'s
//! `_is_json_object`/`_is_jsonl_of_objects`/`_is_nonempty_file`/`_contains_uuid_named_jsonl`/
//! `extract_uuid`).
//!
//! Every provider adapter's marker table (`cancellai.py`'s `ROOT_MARKERS[tool]`) names
//! *which* marker file/directory to check and *which* of these probes to run against it; the
//! probes themselves carry no provider-specific knowledge, matching how `cancellai.py` shares
//! them across `ROOT_MARKERS["codex"]` and `ROOT_MARKERS["claude"]`.
//!
//! Every probe here is a pure filesystem *read* - none of this module reaches
//! `cancellai-platform`'s mutation capability (`scripts/check_mutation_boundary.py` would
//! reject that anyway), and fingerprinting deliberately runs before any root capability is
//! established (SI-002), so these run against a raw, not-yet-approved path.

use std::fs;
use std::path::Path;

/// Mirrors `cancellai.py`'s `MAX_ROOT_PROBE_ENTRIES`: fingerprinting is a bounded probe of an
/// unapproved directory, not an unbounded walk (C-11's self-budget spirit, applied
/// pre-authority).
pub const MAX_ROOT_PROBE_ENTRIES: usize = 2000;

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn is_json_value_object(text: &str) -> bool {
    matches!(
        serde_json::from_str::<serde_json::Value>(text),
        Ok(serde_json::Value::Object(_))
    )
}

/// A UTF-8, non-symlink file, at most 8MiB, that fully parses as a single JSON object.
/// Strict UTF-8 (a decode failure is `false`), matching `cancellai.py`'s
/// `errors="strict"` for this specific probe (its JSON-Lines counterpart below uses
/// `errors="replace"` instead - the two intentionally differ in the Python reference).
pub fn is_json_object(path: &Path) -> bool {
    if is_symlink(path) {
        return false;
    }
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() > 8 * 1024 * 1024 {
        return false;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    is_json_value_object(&text)
}

/// A non-symlink file whose first 20 lines contain at least one non-blank line that parses as
/// a single JSON object (JSON Lines format, sniffed rather than fully validated). Invalid
/// UTF-8 bytes are lossily replaced rather than rejected, matching `cancellai.py`'s
/// `errors="replace"` for this probe.
pub fn is_jsonl_of_objects(path: &Path) -> bool {
    if is_symlink(path) {
        return false;
    }
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    let text = String::from_utf8_lossy(&bytes);
    for line in text.lines().take(20) {
        if line.trim().is_empty() {
            continue;
        }
        if is_json_value_object(line) {
            return true;
        }
    }
    false
}

/// A non-symlink, non-empty file.
pub fn is_nonempty_file(path: &Path) -> bool {
    if is_symlink(path) {
        return false;
    }
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

/// A non-symlink directory.
pub fn is_dir(path: &Path) -> bool {
    if is_symlink(path) {
        return false;
    }
    fs::metadata(path)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
}

/// Extracts the first `8-4-4-4-12` hyphenated hex-digit run found anywhere in `text` (a
/// filename-shaped UUID probe, not a validating UUID parser - it accepts any hex digits in
/// those positions, exactly like `cancellai.py`'s `UUID_RE.search`, which does not check
/// version/variant bits either).
pub fn extract_uuid(text: &str) -> Option<String> {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let bytes = text.as_bytes();
    let total_len: usize = GROUPS.iter().sum::<usize>() + (GROUPS.len() - 1);
    if bytes.len() < total_len {
        return None;
    }
    for start in 0..=(bytes.len() - total_len) {
        if let Some(end) = try_match_uuid(bytes, start, &GROUPS) {
            // GROUPS/hyphens are ASCII-only by construction, so this slice is always valid
            // UTF-8 even if `text` as a whole is not ASCII elsewhere.
            return std::str::from_utf8(&bytes[start..end])
                .ok()
                .map(str::to_string);
        }
    }
    None
}

fn try_match_uuid(bytes: &[u8], start: usize, groups: &[usize; 5]) -> Option<usize> {
    let mut pos = start;
    for (index, &len) in groups.iter().enumerate() {
        for _ in 0..len {
            if !bytes.get(pos).is_some_and(u8::is_ascii_hexdigit) {
                return None;
            }
            pos += 1;
        }
        if index < groups.len() - 1 {
            if bytes.get(pos) != Some(&b'-') {
                return None;
            }
            pos += 1;
        }
    }
    Some(pos)
}

/// Bounded probe for a provider-shaped transcript below `root`: does any file reachable from
/// `root` (without following symlinks, and giving up after [`MAX_ROOT_PROBE_ENTRIES`] files)
/// have a name ending in `.jsonl`, starting with `prefix`, and containing a UUID?
/// Fingerprinting runs before any root capability is granted and must not become an unbounded
/// walk of a directory this workspace has not yet trusted.
pub fn contains_uuid_named_jsonl(root: &Path, prefix: &str) -> bool {
    if is_symlink(root) {
        return false;
    }
    match fs::metadata(root) {
        Ok(metadata) if metadata.is_dir() => {}
        _ => return false,
    }

    let mut seen = 0usize;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if is_symlink(&path) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            seen += 1;
            if seen > MAX_ROOT_PROBE_ENTRIES {
                return false;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".jsonl") && name.starts_with(prefix) && extract_uuid(&name).is_some()
            {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cancellai-root-probe-test-{label}-{}",
                std::process::id()
            ));
            fs::remove_dir_all(&dir).ok();
            fs::create_dir_all(&dir).expect("create temp root");
            Self(dir)
        }

        fn path(&self, rel: &str) -> PathBuf {
            self.0.join(rel)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn extract_uuid_finds_a_uuid_anywhere_in_the_string() {
        let name = "rollout-2026-08-20T09-00-00-22222222-2222-4222-8222-222222222222.jsonl";
        assert_eq!(
            extract_uuid(name).as_deref(),
            Some("22222222-2222-4222-8222-222222222222")
        );
    }

    #[test]
    fn extract_uuid_returns_none_when_no_uuid_shaped_run_exists() {
        assert_eq!(extract_uuid("not-a-uuid-at-all.jsonl"), None);
        assert_eq!(extract_uuid(""), None);
    }

    #[test]
    fn extract_uuid_rejects_a_run_with_a_non_hex_character() {
        assert_eq!(extract_uuid("gggggggg-1111-1111-1111-111111111111"), None);
    }

    #[test]
    fn is_json_object_accepts_a_real_json_object_file() {
        let tree = TempTree::new("json-object");
        fs::write(tree.path("settings.json"), "{}").unwrap();
        assert!(is_json_object(&tree.path("settings.json")));
    }

    #[test]
    fn is_json_object_rejects_a_json_array() {
        let tree = TempTree::new("json-array");
        fs::write(tree.path("list.json"), "[]").unwrap();
        assert!(!is_json_object(&tree.path("list.json")));
    }

    #[test]
    fn is_json_object_rejects_a_missing_file() {
        let tree = TempTree::new("json-missing");
        assert!(!is_json_object(&tree.path("does-not-exist.json")));
    }

    #[test]
    fn is_jsonl_of_objects_accepts_a_file_with_one_object_line() {
        let tree = TempTree::new("jsonl-object");
        fs::write(tree.path("session.jsonl"), "{\"type\": \"user\"}\n").unwrap();
        assert!(is_jsonl_of_objects(&tree.path("session.jsonl")));
    }

    #[test]
    fn is_jsonl_of_objects_rejects_a_file_with_no_object_lines() {
        let tree = TempTree::new("jsonl-no-object");
        fs::write(tree.path("session.jsonl"), "\n\n[]\n").unwrap();
        assert!(!is_jsonl_of_objects(&tree.path("session.jsonl")));
    }

    #[test]
    fn is_nonempty_file_rejects_an_empty_file() {
        let tree = TempTree::new("nonempty");
        fs::write(tree.path("empty.txt"), "").unwrap();
        assert!(!is_nonempty_file(&tree.path("empty.txt")));
        fs::write(tree.path("full.txt"), "x").unwrap();
        assert!(is_nonempty_file(&tree.path("full.txt")));
    }

    #[test]
    fn contains_uuid_named_jsonl_finds_a_nested_match() {
        let tree = TempTree::new("uuid-nested");
        let nested = tree.path("sessions/2026/08/20");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            nested.join("rollout-2026-08-20T09-00-00-22222222-2222-4222-8222-222222222222.jsonl"),
            "{}\n",
        )
        .unwrap();
        assert!(contains_uuid_named_jsonl(&tree.0, ""));
        assert!(contains_uuid_named_jsonl(&tree.0, "rollout-"));
        assert!(!contains_uuid_named_jsonl(&tree.0, "not-a-real-prefix-"));
    }

    #[test]
    fn contains_uuid_named_jsonl_does_not_descend_into_a_symlinked_directory() {
        let tree = TempTree::new("uuid-symlink");
        let outside = tree.path("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(
            outside.join("11111111-1111-4111-8111-111111111111.jsonl"),
            "{}\n",
        )
        .unwrap();
        fs::create_dir_all(tree.path("root")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, tree.path("root/link")).unwrap();
        assert!(!contains_uuid_named_jsonl(&tree.path("root"), ""));
    }

    #[test]
    fn contains_uuid_named_jsonl_returns_false_for_a_nonexistent_root() {
        let tree = TempTree::new("uuid-missing-root");
        assert!(!contains_uuid_named_jsonl(&tree.path("does-not-exist"), ""));
    }
}

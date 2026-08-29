//! Shared synthetic-dataset generator for the E04-S04 performance tests
//! (`performance_micro.rs`, `performance_scheduled.rs`). Kept as its own module (not
//! `#[cfg(test)]` code in `src/`) since it is only needed by integration tests, never by the
//! library itself.

use std::path::{Path, PathBuf};

/// Builds a synthetic tree of exactly `total_files` regular files, spread across nested
/// directories of at most `files_per_dir` entries each - a single directory with, say,
/// 1,000,000 entries is not representative of a real provider layout and stresses the
/// filesystem's own directory format rather than this crate's traversal. Returns the number
/// of directories created (including `root` itself), for the caller to sanity-check against
/// `InventorySnapshot::directories_visited`.
pub fn build_synthetic_tree(root: &Path, total_files: usize, files_per_dir: usize) -> usize {
    std::fs::create_dir_all(root).expect("create synthetic tree root");
    let mut directories = 1; // root itself
    let mut created = 0;
    let mut dir_index = 0;
    let mut current_dir = root.to_path_buf();
    let mut current_dir_count = 0;

    while created < total_files {
        if current_dir_count == 0 {
            current_dir = if dir_index == 0 {
                root.to_path_buf()
            } else {
                let dir = root.join(format!("dir-{dir_index:06}"));
                std::fs::create_dir_all(&dir).expect("create synthetic subdirectory");
                directories += 1;
                dir
            };
        }
        let file = current_dir.join(format!("file-{created:08}.dat"));
        std::fs::write(&file, b"synthetic").expect("write synthetic file");
        created += 1;
        current_dir_count += 1;
        if current_dir_count >= files_per_dir {
            current_dir_count = 0;
            dir_index += 1;
        }
    }
    directories
}

pub fn temp_root(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cancellai-inventory-perf-{label}-{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(&dir).ok();
    dir
}

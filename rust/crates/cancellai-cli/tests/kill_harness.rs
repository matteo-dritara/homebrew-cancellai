//! Crash/recovery under a real process kill at every point on `clean`'s mutation path (E06-S09).
//!
//! Each case builds a synthetic `$HOME`, starts a real `clean --yes`, waits until the process
//! reports - through `src/kill_points.rs`'s marker file - that it reached the armed point, and
//! kills it (`SIGKILL` on Unix, `TerminateProcess` on Windows, both via `Child::kill`). It then
//! checks what the kill left behind and reruns `clean` to completion:
//!
//! - nothing outside the plan changed: recent sessions, `settings.json` and `history.jsonl` are
//!   byte-identical, and no file appeared that was not there before;
//! - every planned artifact is either intact or gone, and exactly as many are gone as the point
//!   implies (`before-delete#n` has removed `n`, `after-delete#n` has removed `n + 1`);
//! - the rerun succeeds, removes exactly the remainder, fails nothing, and acts on nothing twice.
//!
//! A point the process never reaches fails the case instead of counting as survived.
//!
//! Only built with `--features kill-points`; `rust.yml` runs it on macOS, Linux and Windows.
//! Every tree is synthetic and temporary - never the real `~/.claude` or `~/.codex`.

#![cfg(feature = "kill-points")]
// Integration tests live outside `#[cfg(test)]`, so clippy's `allow-*-in-tests` options in
// `clippy.toml` do not reach them. A test that unwraps is asserting.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Stale sessions per provider. With `--keep-latest 0` every one of them is planned.
const STALE_PER_PROVIDER: usize = 2;
const PLANNED: usize = 2 * STALE_PER_PROVIDER;
const REACH_TIMEOUT: Duration = Duration::from_secs(60);

struct Home(PathBuf);

impl Home {
    fn new(label: &str) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "cancellai-kill-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // See cli_behavior.rs: canonicalize the harness's own temp path so macOS's `/var ->
        // private/var` link does not make every default root look reached through a symlink.
        Self(std::fs::canonicalize(&dir).unwrap())
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn set_mtime(path: &Path, seconds_since_epoch: u64) {
    let when = std::time::UNIX_EPOCH + Duration::from_secs(seconds_since_epoch);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(when)
        .unwrap();
}

/// The tree every case starts from, and which files in it are planned for deletion.
struct Tree {
    planned: Vec<PathBuf>,
}

fn build_tree(home: &Path) -> Tree {
    let claude = home.join(".claude");
    let projects = claude.join("projects/proj-k");
    let codex = home.join(".codex/sessions/2020/01/01");
    std::fs::create_dir_all(&projects).unwrap();
    std::fs::create_dir_all(&codex).unwrap();
    std::fs::write(claude.join("settings.json"), "{\"theme\":\"dark\"}\n").unwrap();
    std::fs::write(claude.join("history.jsonl"), "{\"display\":\"kept\"}\n").unwrap();

    let mut planned = Vec::new();
    for index in 0..STALE_PER_PROVIDER {
        let id = format!("{index:08x}-1111-4111-8111-111111111111");
        let transcript = projects.join(format!("{id}.jsonl"));
        std::fs::write(&transcript, format!("{{\"session\":\"{id}\"}}\n")).unwrap();
        set_mtime(&transcript, 946_684_800);
        planned.push(transcript);

        let id = format!("{index:08x}-2222-4222-8222-222222222222");
        let rollout = codex.join(format!("rollout-2020-01-01T00-00-00-{id}.jsonl"));
        let meta =
            format!("{{\"type\":\"session_meta\",\"payload\":{{\"meta\":{{\"id\":\"{id}\"}}}}}}\n");
        std::fs::write(&rollout, meta).unwrap();
        set_mtime(&rollout, 946_684_800);
        planned.push(rollout);
    }
    // Recent sessions: never planned, and must survive every kill.
    let recent_claude = projects.join("ffffffff-1111-4111-8111-111111111111.jsonl");
    std::fs::write(&recent_claude, "{\"session\":\"recent\"}\n").unwrap();
    let recent_codex =
        codex.join("rollout-2020-01-01T00-00-00-ffffffff-2222-4222-8222-222222222222.jsonl");
    std::fs::write(
        &recent_codex,
        "{\"type\":\"session_meta\",\"payload\":{\"meta\":{\"id\":\"ffffffff-2222-4222-8222-222222222222\"}}}\n",
    )
    .unwrap();
    Tree { planned }
}

/// Every regular file under `root`, with its bytes.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                stack.push(entry.path());
            } else {
                files.insert(entry.path(), std::fs::read(entry.path()).unwrap());
            }
        }
    }
    files
}

/// Everything wrong with `after` given `before`, the planned set, and how many planned files
/// the kill point implies are gone. Empty means the kill left a safe state.
fn violations(
    before: &BTreeMap<PathBuf, Vec<u8>>,
    after: &BTreeMap<PathBuf, Vec<u8>>,
    planned: &[PathBuf],
    expected_gone: usize,
) -> Vec<String> {
    let mut problems = Vec::new();
    for (path, bytes) in after {
        match before.get(path) {
            None => problems.push(format!("{} appeared", path.display())),
            Some(original) if original != bytes => {
                problems.push(format!("{} changed without being removed", path.display()));
            }
            Some(_) => {}
        }
    }
    let mut gone = 0;
    for path in before.keys() {
        if !after.contains_key(path) {
            if planned.contains(path) {
                gone += 1;
            } else {
                problems.push(format!("{} was not planned but is gone", path.display()));
            }
        }
    }
    if gone != expected_gone {
        problems.push(format!(
            "{gone} planned artifact(s) gone, the kill point implies {expected_gone}"
        ));
    }
    problems
}

fn bin() -> String {
    std::env::var("CARGO_BIN_EXE_cancellai-cli").expect("cargo sets this for integration tests")
}

fn clean_command(home: &Path) -> Command {
    let mut command = Command::new(bin());
    command
        .args([
            "clean",
            "--yes",
            "--json",
            "--allow-running",
            "--keep-latest",
            "0",
        ])
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// Start `clean` armed at `point`, wait for its marker, kill it. Panics if the point is never
/// reached - an unreached point is a failed case, not a survived one.
fn kill_at(home: &Path, point: &str, plant_partial: Option<&Path>) {
    let marker = home.with_extension("marker");
    std::fs::remove_file(&marker).ok();
    let mut command = clean_command(home);
    command
        .env("CANCELLAI_KILL_POINT", point)
        .env("CANCELLAI_KILL_MARKER", &marker);
    if let Some(partial) = plant_partial {
        command.env("CANCELLAI_KILL_PLANT_PARTIAL", partial);
    }
    let mut child = command.spawn().unwrap();
    let start = Instant::now();
    loop {
        if marker.exists() {
            break;
        }
        if let Some(status) = child.try_wait().unwrap() {
            let output = child.wait_with_output().unwrap();
            panic!(
                "{point}: clean exited ({status}) without reaching the point; stdout: {} stderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if start.elapsed() > REACH_TIMEOUT {
            child.kill().ok();
            panic!("{point}: not reached within {REACH_TIMEOUT:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    std::fs::remove_file(&marker).ok();
}

/// Every point, and how many planned artifacts it implies are already gone when it is reached.
fn cases() -> Vec<(String, usize)> {
    let mut cases = vec![("roots-established".to_string(), 0)];
    for n in 0..PLANNED {
        cases.push((format!("before-delete#{n}"), n));
        cases.push((format!("after-delete#{n}"), n + 1));
    }
    cases.push(("before-report".to_string(), PLANNED));
    cases
}

// Real-deletion tests run on every platform since E06-S13, which let the safety executor admit a
// Windows plain file and gave the provider-layout check a handle-bound Windows observation.
#[test]
fn clean_killed_at_every_mutation_point_leaves_a_safe_state_and_a_rerun_finishes_it() {
    // E06-S07: a build without a stable channel compiled in cannot delete (SI-030), so there is
    // no mutation path to kill. `rust.yml`'s kill-harness job builds with CANCELLAI_CHANNEL=stable.
    if !matches!(
        option_env!("CANCELLAI_CHANNEL")
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("stable" | "beta")
    ) {
        eprintln!("skipped: needs a CANCELLAI_CHANNEL=stable build (E06-S07)");
        return;
    }
    for (point, expected_gone) in cases() {
        let home = Home::new("case");
        let tree = build_tree(home.path());
        let before = snapshot(home.path());

        kill_at(home.path(), &point, None);
        let after_kill = snapshot(home.path());
        let problems = violations(&before, &after_kill, &tree.planned, expected_gone);
        assert!(problems.is_empty(), "{point}: after the kill: {problems:?}");

        let rerun = clean_command(home.path()).output().unwrap();
        assert!(
            rerun.status.success(),
            "{point}: rerun failed: {}{}",
            String::from_utf8_lossy(&rerun.stdout),
            String::from_utf8_lossy(&rerun.stderr)
        );
        // With nothing left, the rerun reports every action safely skipped (E06-S11: `--json`
        // always prints a document); otherwise it removes exactly the remainder.
        let doc: serde_json::Value = serde_json::from_slice(&rerun.stdout).unwrap();
        assert_eq!(
            doc["summary"]["succeeded"].as_u64(),
            Some((PLANNED - expected_gone) as u64),
            "{point}: the rerun must remove exactly the remainder"
        );
        assert_eq!(doc["summary"]["failed"].as_u64(), Some(0), "{point}");

        let after_rerun = snapshot(home.path());
        let problems = violations(&before, &after_rerun, &tree.planned, PLANNED);
        assert!(
            problems.is_empty(),
            "{point}: after the rerun: {problems:?}"
        );
    }
}

/// The harness must catch the defect it exists for: a kill that leaves a half-written file
/// behind. The planted file is written by the armed point itself, just before it pauses.
#[test]
fn the_harness_detects_a_half_written_file_left_by_a_kill() {
    let home = Home::new("planted");
    let tree = build_tree(home.path());
    let before = snapshot(home.path());
    let partial = home.path().join(".claude/cancellai-state.json");

    kill_at(home.path(), "before-delete#1", Some(&partial));
    let after = snapshot(home.path());
    let problems = violations(&before, &after, &tree.planned, 1);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("cancellai-state.json appeared")),
        "the planted partial file must be reported: {problems:?}"
    );
}

#[test]
fn an_unreached_point_fails_instead_of_counting_as_survived() {
    let home = Home::new("unreached");
    build_tree(home.path());
    let result = std::panic::catch_unwind(|| kill_at(home.path(), "no-such-point", None));
    assert!(
        result.is_err(),
        "a point clean never reaches must fail the case"
    );
}

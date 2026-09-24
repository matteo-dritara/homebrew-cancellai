//! Signed incident containment on the live mutation path (E06-S07, ADR-0039), driven through the
//! real binary against synthetic `$HOME`/`$CANCELLAI_HOME` trees - never real provider data.
//!
//! Notices are signed with a key generated here and trusted only through a temporary owner trust
//! file, so no test key ships in any build. Tests that need a deletion to be *possible* run only
//! on a stable-channel build (`rust.yml` sets `CANCELLAI_CHANNEL=stable` for this crate's tests);
//! on a nightly build the release channel already withholds every deletion (SI-030).

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::path::PathBuf;
use std::process::{Command, Output};

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

const PUBLISHER: &str = "test-publisher";
const CLAUDE_SESSION: &str = "11111111-1111-4111-8111-111111111111";
const CODEX_SESSION: &str = "22222222-2222-4222-8222-222222222222";

fn stable_build() -> bool {
    matches!(
        option_env!("CANCELLAI_CHANNEL")
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("stable" | "beta")
    )
}

macro_rules! needs_stable_build {
    () => {
        if !stable_build() {
            eprintln!("skipped: needs a CANCELLAI_CHANNEL=stable build (E06-S07)");
            return;
        }
    };
}

struct Tree(PathBuf);

impl Tree {
    fn new(label: &str) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "cancellai-containment-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let tree = Self(std::fs::canonicalize(&dir).unwrap());
        tree.seed_sessions();
        tree
    }

    fn state(&self) -> PathBuf {
        self.0.join("cancellai-home/state")
    }

    fn log(&self) -> PathBuf {
        self.state().join("containment_log.jsonl")
    }

    fn seed_sessions(&self) {
        let project = self.0.join(".claude/projects/proj");
        std::fs::create_dir_all(&project).unwrap();
        let claude = project.join(format!("{CLAUDE_SESSION}.jsonl"));
        std::fs::write(&claude, "{}\n").unwrap();
        let codex_dir = self.0.join(".codex/sessions/2020/01/01");
        std::fs::create_dir_all(&codex_dir).unwrap();
        let codex = codex_dir.join(format!("rollout-2020-01-01T00-00-00-{CODEX_SESSION}.jsonl"));
        std::fs::write(
            &codex,
            format!("{{\"type\":\"session_meta\",\"payload\":{{\"meta\":{{\"id\":\"{CODEX_SESSION}\"}}}}}}\n"),
        )
        .unwrap();
        for path in [claude, codex] {
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(946_684_800))
                .unwrap();
        }
    }

    fn trust(&self, entries: &str) {
        std::fs::create_dir_all(self.state()).unwrap();
        std::fs::write(self.state().join("trusted_publishers.json"), entries).unwrap();
    }

    fn trust_test_publisher(&self) {
        let key = hex(&signing_key(7).verifying_key().to_bytes());
        self.trust(&format!(
            "[{{\"publisher_id\":\"{PUBLISHER}\",\"public_key\":\"{key}\"}}]"
        ));
    }

    fn claude_session(&self) -> PathBuf {
        self.0
            .join(format!(".claude/projects/proj/{CLAUDE_SESSION}.jsonl"))
    }

    fn codex_session(&self) -> PathBuf {
        self.0.join(format!(
            ".codex/sessions/2020/01/01/rollout-2020-01-01T00-00-00-{CODEX_SESSION}.jsonl"
        ))
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_with_path(args, None)
    }

    /// Runs with `CANCELLAI_TEST_CURL` pointing at `bin_dir`'s fake `curl`, when given (a
    /// `test-curl` build only; the release build runs the system curl at its fixed location).
    fn run_with_path(&self, args: &[&str], bin_dir: Option<&std::path::Path>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cancellai-cli"));
        if let Some(dir) = bin_dir {
            command.env("CANCELLAI_TEST_CURL", dir.join("curl"));
        }
        command
            .args(args)
            .env("HOME", &self.0)
            .env("CANCELLAI_HOME", self.0.join("cancellai-home"))
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("CODEX_HOME")
            .output()
            .unwrap()
    }

    fn write_notice(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    fn install(&self, name: &str, text: &str) -> Output {
        let path = self.write_notice(name, text);
        self.run(&["containment", "install", path.to_str().unwrap()])
    }

    fn plan(&self) -> serde_json::Value {
        let output = self.run(&["plan", "--json", "--allow-running", "--keep-latest", "0"]);
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Mirrors `KnowledgeBundle::signing_bytes` (private to the kernel); a mismatch fails every
/// install in this file rather than passing silently.
fn signed(seed: u8, publisher: &str, sequence: u64, expires: Option<u64>, payload: &str) -> String {
    let digest = hex(&Sha256::digest(payload.as_bytes()));
    let issued = now() - 10;
    let mut buf = Vec::new();
    let mut push = |field: &[u8]| {
        buf.extend_from_slice(&(field.len() as u64).to_be_bytes());
        buf.extend_from_slice(field);
    };
    push(&1u32.to_be_bytes());
    push(publisher.as_bytes());
    push(&sequence.to_be_bytes());
    push(&issued.to_be_bytes());
    push(&expires.unwrap_or(0).to_be_bytes());
    push(&[u8::from(expires.is_some())]);
    push(digest.as_bytes());
    push(payload.as_bytes());
    let signature = hex(&signing_key(seed).sign(&buf).to_bytes());
    serde_json::json!({
        "schema_version": 1,
        "publisher_id": publisher,
        "sequence": sequence,
        "issued_at": issued,
        "expires_at": expires,
        "content_digest": digest,
        "payload": payload,
        "signature": signature,
    })
    .to_string()
}

fn notice(incident: &str, provider: &str) -> String {
    serde_json::json!({
        "schema_version": 1,
        "kind": "capability_containment",
        "entries": [{
            "incident_id": incident,
            "severity": "S0",
            "provider_id": provider,
            "provider_versions": null,
            "action_classes": null,
            "platforms": null,
            "ceiling": "observe",
            "invariants": ["SI-022"],
            "affected_releases": ["1.21.0"]
        }]
    })
    .to_string()
}

fn deletes(plan: &serde_json::Value) -> Vec<String> {
    plan["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["action_class"] == "delete")
        .map(|a| a["target_artifact_ids"][0].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_contained_provider_is_withheld_in_plan_and_clean_and_an_uncontained_one_still_deletes() {
    needs_stable_build!();
    let tree = Tree::new("contain");
    tree.trust_test_publisher();
    let before = tree.plan();
    assert_eq!(deletes(&before).len(), 2, "{before}");

    let installed = tree.install(
        "n1.json",
        &signed(7, PUBLISHER, 1, None, &notice("INC-1", "claude-code")),
    );
    assert_eq!(
        installed.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );

    let after = tree.plan();
    assert_eq!(deletes(&after).len(), 1, "{after}");
    assert!(
        after.to_string().contains("INC-1"),
        "the incident must be named: {after}"
    );

    let clean = tree.run(&[
        "clean",
        "--yes",
        "--json",
        "--allow-running",
        "--keep-latest",
        "0",
    ]);
    assert_eq!(
        clean.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&clean.stdout)
    );
    assert!(
        tree.claude_session().exists(),
        "the contained provider must not be deleted"
    );
    assert!(
        !tree.codex_session().exists(),
        "the uncontained provider still deletes"
    );
}

#[test]
fn every_refused_notice_leaves_the_history_byte_identical() {
    let tree = Tree::new("refusals");
    tree.trust_test_publisher();
    let first = signed(7, PUBLISHER, 1, None, &notice("INC-1", "claude-code"));
    assert_eq!(tree.install("first.json", &first).status.code(), Some(0));
    let history = std::fs::read(tree.log()).unwrap();

    let mut forged: serde_json::Value = serde_json::from_str(&signed(
        7,
        PUBLISHER,
        2,
        None,
        &notice("INC-2", "codex-cli"),
    ))
    .unwrap();
    forged["payload"] = serde_json::Value::String(notice("INC-3", "codex-cli"));
    let oversized = format!("{}{}", " ".repeat(256 * 1024), first);
    let cases = [
        ("replay", first.clone()),
        (
            "untrusted",
            signed(9, PUBLISHER, 2, None, &notice("INC-2", "codex-cli")),
        ),
        (
            "unknown-publisher",
            signed(7, "someone-else", 1, None, &notice("INC-2", "codex-cli")),
        ),
        ("forged", forged.to_string()),
        (
            "expired",
            signed(
                7,
                PUBLISHER,
                2,
                Some(now() - 1),
                &notice("INC-2", "codex-cli"),
            ),
        ),
        ("malformed", "{\"not\":\"a bundle\"}".to_string()),
        ("oversized", oversized),
    ];
    for (name, text) in cases {
        let output = tree.install(&format!("{name}.json"), &text);
        assert_eq!(
            output.status.code(),
            Some(4),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read(tree.log()).unwrap(),
            history,
            "{name} changed the history"
        );
    }
}

#[test]
fn an_unreadable_or_unverifiable_history_caps_everything_at_recommend() {
    needs_stable_build!();
    for (label, content) in [
        ("truncated", "{\"install\":\"{\\\"schema"),
        ("garbage", "not json at all\n"),
        ("unsigned-install", "{\"install\":\"{}\"}\n"),
    ] {
        let tree = Tree::new(label);
        std::fs::create_dir_all(tree.state()).unwrap();
        std::fs::write(tree.log(), content).unwrap();
        let plan = tree.plan();
        assert!(deletes(&plan).is_empty(), "{label}: {plan}");
        assert!(
            plan.to_string().contains("containment_ledger_unknown"),
            "{label}: {plan}"
        );
        let clean = tree.run(&[
            "clean",
            "--yes",
            "--json",
            "--allow-running",
            "--keep-latest",
            "0",
        ]);
        assert_eq!(clean.status.code(), Some(4), "{label}");
        assert!(
            tree.claude_session().exists() && tree.codex_session().exists(),
            "{label}"
        );
    }
}

#[test]
fn a_malformed_or_reserved_owner_trust_file_is_unknown_not_ignored() {
    needs_stable_build!();
    let reserved = format!(
        "[{{\"publisher_id\":\"cancellai-incident\",\"public_key\":\"{}\"}}]",
        hex(&signing_key(7).verifying_key().to_bytes())
    );
    for (label, content) in [
        ("malformed", "[{\"publisher_id\":\"x\"}]".to_string()),
        (
            "bad-key",
            "[{\"publisher_id\":\"x\",\"public_key\":\"00\"}]".to_string(),
        ),
        ("reserved", reserved),
    ] {
        let tree = Tree::new(label);
        tree.trust(&content);
        let plan = tree.plan();
        assert!(deletes(&plan).is_empty(), "{label}: {plan}");
        assert!(
            plan.to_string().contains("containment_ledger_unknown"),
            "{label}"
        );
    }
}

#[test]
fn no_state_directory_is_an_empty_ledger_and_plan_creates_none() {
    needs_stable_build!();
    let tree = Tree::new("missing");
    assert_eq!(deletes(&tree.plan()).len(), 2);
    assert!(
        !tree.state().exists(),
        "a read-only plan must not create cancellAI's state directory"
    );
}

#[test]
fn an_expired_containment_still_binds_after_install() {
    needs_stable_build!();
    let tree = Tree::new("expiry");
    tree.trust_test_publisher();
    let expiring = signed(
        7,
        PUBLISHER,
        1,
        Some(now() + 2),
        &notice("INC-1", "claude-code"),
    );
    assert_eq!(
        tree.install("expiring.json", &expiring).status.code(),
        Some(0)
    );
    std::thread::sleep(std::time::Duration::from_secs(3));
    assert_eq!(
        deletes(&tree.plan()).len(),
        1,
        "expiry must not lift a containment (SI-029)"
    );
}

#[test]
fn only_a_confirmed_local_lift_removes_a_containment() {
    let tree = Tree::new("lift");
    tree.trust_test_publisher();
    assert_eq!(
        tree.install(
            "n.json",
            &signed(7, PUBLISHER, 1, None, &notice("INC-1", "claude-code"))
        )
        .status
        .code(),
        Some(0)
    );
    let unconfirmed = tree.run(&["containment", "lift", "INC-1"]);
    assert_eq!(unconfirmed.status.code(), Some(2));
    let listed = tree.run(&["containment", "list"]);
    assert!(String::from_utf8_lossy(&listed.stdout).contains("INC-1"));

    let unknown = tree.run(&["containment", "lift", "INC-9", "--confirm"]);
    assert_eq!(unknown.status.code(), Some(2));
    let lifted = tree.run(&["containment", "lift", "INC-1", "--confirm"]);
    assert_eq!(
        lifted.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&lifted.stderr)
    );
    let listed = tree.run(&["containment", "list"]);
    assert!(!String::from_utf8_lossy(&listed.stdout).contains("INC-1"));
    if stable_build() {
        assert_eq!(deletes(&tree.plan()).len(), 2);
    }
}

/// A fake `curl` for `containment refresh` (E33-S01): prints `body` to stdout and `status` to
/// stderr the way `--write-out '%{stderr}%{http_code}'` does, then exits `code`.
#[cfg(all(unix, feature = "test-curl"))]
fn fake_curl(tree: &Tree, body: &str, status: &str, code: i32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = tree.0.join(format!("fakebin-{status}-{code}"));
    std::fs::create_dir_all(&dir).unwrap();
    let body_file = dir.join("body");
    std::fs::write(&body_file, body).unwrap();
    let script = dir.join("curl");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\n/bin/cat '{}'\nprintf '{status}' >&2\nexit {code}\n",
            body_file.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

#[cfg(all(unix, feature = "test-curl"))]
#[test]
fn refresh_installs_a_published_notice_once_and_is_current_after() {
    let tree = Tree::new("refresh");
    tree.trust_test_publisher();
    let text = signed(7, PUBLISHER, 1, None, &notice("INC-1", "claude-code"));
    let bin = fake_curl(&tree, &text, "200", 0);
    let first = tree.run_with_path(&["containment", "refresh"], Some(&bin));
    assert_eq!(
        first.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let history = std::fs::read(tree.log()).unwrap();
    let again = tree.run_with_path(&["containment", "refresh"], Some(&bin));
    assert_eq!(again.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&again.stdout).contains("already current"));
    assert_eq!(std::fs::read(tree.log()).unwrap(), history);
    let listed = tree.run(&["containment", "list"]);
    assert!(String::from_utf8_lossy(&listed.stdout).contains("INC-1"));
}

#[cfg(all(unix, feature = "test-curl"))]
#[test]
fn every_unusable_feed_leaves_the_history_byte_identical() {
    let tree = Tree::new("refresh-refusals");
    tree.trust_test_publisher();
    let first = signed(7, PUBLISHER, 2, None, &notice("INC-1", "claude-code"));
    assert_eq!(tree.install("first.json", &first).status.code(), Some(0));
    let history = std::fs::read(tree.log()).unwrap();

    let rolled_back = signed(7, PUBLISHER, 1, None, &notice("INC-2", "codex-cli"));
    let untrusted = signed(9, PUBLISHER, 3, None, &notice("INC-2", "codex-cli"));
    let oversized = "x".repeat(256 * 1024 + 10);
    let cases: Vec<(&str, String, &str, i32, Option<i32>)> = vec![
        ("unreachable", String::new(), "000", 7, Some(4)),
        ("server-error", "oops".into(), "500", 0, Some(4)),
        ("oversized", oversized, "200", 0, Some(4)),
        (
            "malformed",
            "{\"not\":\"a bundle\"}".into(),
            "200",
            0,
            Some(4),
        ),
        ("rolled-back", rolled_back, "200", 0, Some(4)),
        ("untrusted", untrusted, "200", 0, Some(4)),
        ("nothing-published", String::new(), "404", 0, Some(0)),
    ];
    for (label, body, status, code, expected) in cases {
        let bin = fake_curl(&tree, &body, status, code);
        let output = tree.run_with_path(&["containment", "refresh"], Some(&bin));
        assert_eq!(
            output.status.code(),
            expected,
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read(tree.log()).unwrap(),
            history,
            "{label} changed the history"
        );
        if label == "oversized" {
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("larger than"),
                "the cap must refuse before parsing: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

/// E06 review round 6: a `curl` earlier on `PATH` must never run. With no test override, the
/// feed is pointed at an unreachable address so the system curl fails fast without network.
#[cfg(unix)]
#[test]
fn a_curl_on_path_is_never_executed() {
    use std::os::unix::fs::PermissionsExt;
    let tree = Tree::new("path-hijack");
    let bin = tree.0.join("hijack-bin");
    std::fs::create_dir_all(&bin).unwrap();
    let marker = tree.0.join("hijacked");
    let script = bin.join("curl");
    std::fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::create_dir_all(tree.state()).unwrap();
    std::fs::write(
        tree.state().join("containment_feed_url"),
        "https://127.0.0.1:9/notice.json",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cancellai-cli"))
        .args(["containment", "refresh"])
        .env("HOME", &tree.0)
        .env("CANCELLAI_HOME", tree.0.join("cancellai-home"))
        .env("PATH", &bin)
        .env_remove("CANCELLAI_TEST_CURL")
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists(), "the curl on PATH was executed");
}

/// E06 review round 6: "already current" is only said of a history that replays.
#[cfg(all(unix, feature = "test-curl"))]
#[test]
fn an_unverifiable_history_is_never_already_current() {
    let tree = Tree::new("stale-current");
    tree.trust_test_publisher();
    let text = "{\"not\":\"a bundle\"}".to_string();
    std::fs::create_dir_all(tree.state()).unwrap();
    let line = serde_json::json!({ "install": text }).to_string();
    std::fs::write(tree.log(), format!("{line}\n")).unwrap();
    let before = std::fs::read(tree.log()).unwrap();
    let bin = fake_curl(&tree, &text, "200", 0);
    let output = tree.run_with_path(&["containment", "refresh"], Some(&bin));
    assert_eq!(
        output.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("already current"));
    assert_eq!(std::fs::read(tree.log()).unwrap(), before);
}

#[test]
fn a_feed_override_that_is_not_https_is_refused() {
    let tree = Tree::new("feed-override");
    std::fs::create_dir_all(tree.state()).unwrap();
    std::fs::write(
        tree.state().join("containment_feed_url"),
        "http://example.invalid/x",
    )
    .unwrap();
    let output = tree.run(&["containment", "refresh"]);
    assert_eq!(output.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&output.stderr).contains("https"));
}

/// E06 review round 7: after sequences 1 and 2 are installed, a feed serving the exact older
/// sequence 1 is a rollback - refused, not "already current".
#[cfg(all(unix, feature = "test-curl"))]
#[test]
fn a_feed_serving_an_older_installed_notice_is_refused_as_a_rollback() {
    let tree = Tree::new("rollback-current");
    tree.trust_test_publisher();
    let first = signed(7, PUBLISHER, 1, None, &notice("INC-1", "claude-code"));
    let second = signed(7, PUBLISHER, 2, None, &notice("INC-1", "claude-code"));
    assert_eq!(tree.install("one.json", &first).status.code(), Some(0));
    assert_eq!(tree.install("two.json", &second).status.code(), Some(0));
    let before = std::fs::read(tree.log()).unwrap();
    let bin = fake_curl(&tree, &first, "200", 0);
    let output = tree.run_with_path(&["containment", "refresh"], Some(&bin));
    assert_eq!(
        output.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("older"));
    assert_eq!(std::fs::read(tree.log()).unwrap(), before);
    let bin = fake_curl(&tree, &second, "200", 1);
    let output = tree.run_with_path(&["containment", "refresh"], Some(&bin));
    assert_eq!(output.status.code(), Some(4), "curl failure is not current");
    let bin = fake_curl(&tree, &second, "200", 0);
    let output = tree.run_with_path(&["containment", "refresh"], Some(&bin));
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("already current"));
}

/// E06 review round 8: sixteen simultaneous refreshes of one valid notice. Exactly one install
/// is accepted, every process succeeds, and the history still replays.
#[cfg(all(unix, feature = "test-curl"))]
#[test]
fn concurrent_refreshes_of_one_notice_leave_one_accepted_install_and_a_replayable_history() {
    let tree = Tree::new("concurrent-refresh");
    tree.trust_test_publisher();
    let text = signed(7, PUBLISHER, 1, None, &notice("INC-1", "claude-code"));
    let bin = fake_curl(&tree, &text, "200", 0);
    let handles: Vec<_> = (0..16)
        .map(|_| {
            Command::new(env!("CARGO_BIN_EXE_cancellai-cli"))
                .args(["containment", "refresh"])
                .env("HOME", &tree.0)
                .env("CANCELLAI_HOME", tree.0.join("cancellai-home"))
                .env("CANCELLAI_TEST_CURL", bin.join("curl"))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in handles {
        let output = child.wait_with_output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let listed = tree.run(&["containment", "list"]);
    assert_eq!(
        listed.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&listed.stdout)
            .matches("INC-1")
            .count(),
        1
    );
}

/// E06 review round 8, the general case: many concurrent installs of different newer notices.
/// Whatever wins, the history replays, every accepted install is listed, and every loser was told
/// to retry rather than silently dropped.
#[test]
fn concurrent_installs_of_different_notices_never_break_the_history() {
    let tree = Tree::new("concurrent-install");
    tree.trust_test_publisher();
    let paths: Vec<_> = (1..=8)
        .map(|sequence| {
            tree.write_notice(
                &format!("n{sequence}.json"),
                &signed(
                    7,
                    PUBLISHER,
                    sequence,
                    None,
                    &notice(&format!("INC-{sequence}"), "claude-code"),
                ),
            )
        })
        .collect();
    let children: Vec<_> = paths
        .iter()
        .map(|path| {
            Command::new(env!("CARGO_BIN_EXE_cancellai-cli"))
                .args(["containment", "install", path.to_str().unwrap()])
                .env("HOME", &tree.0)
                .env("CANCELLAI_HOME", tree.0.join("cancellai-home"))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let mut accepted = 0;
    for child in children {
        let output = child.wait_with_output().unwrap();
        match output.status.code() {
            Some(0) => accepted += 1,
            Some(4) => {}
            other => panic!(
                "unexpected exit {other:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        }
    }
    let listed = tree.run(&["containment", "list"]);
    assert_eq!(
        listed.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    assert!(accepted >= 1);
    assert_eq!(
        String::from_utf8_lossy(&listed.stdout).lines().count(),
        accepted
    );
}

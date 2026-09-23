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
        Command::new(env!("CARGO_BIN_EXE_cancellai-cli"))
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

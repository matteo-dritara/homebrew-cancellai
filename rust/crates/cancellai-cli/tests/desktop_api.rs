//! The `desktop-api` command against a real synthetic provider tree (E19-S01).
//!
//! Two properties a desktop client depends on, proven end to end through the real binary:
//!
//! - **parity**: the documents the API serves are the documents `status --json` and
//!   `plan --json` print for the same tree and the same query, field for field apart from the
//!   generation timestamp - so a desktop view cannot show a different inventory or plan;
//! - **no bypass**: serving every document kind, including a plan full of delete candidates,
//!   leaves every provider file where it was;
//! - **view-model parity** (E19-S02): the dashboard view built from those documents shows the
//!   same per-provider lines and plan counts the CLI's human `status` and `plan` print.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use cancellai_desktop_api::{
    Client, ClientError, Descriptor, DocumentKind, ErrorCode, Query, ToolScope,
};

struct TempHome(PathBuf);

impl TempHome {
    fn new(label: &str) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "cancellai-desktop-api-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(std::fs::canonicalize(&dir).unwrap())
    }

    fn stale(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_modified(std::time::UNIX_EPOCH).unwrap();
        path
    }

    /// Two stale Claude sessions and one stale Codex rollout: enough for `plan` to propose
    /// deletions once `keep_latest` is 0.
    fn populated(label: &str) -> Self {
        let home = Self::new(label);
        home.stale(
            ".claude/projects/p/11111111-1111-4111-8111-111111111111.jsonl",
            "{}",
        );
        home.stale(
            ".claude/projects/p/22222222-2222-4222-8222-222222222222.jsonl",
            "{}",
        );
        let id = "33333333-3333-4333-8333-333333333333";
        let meta = serde_json::json!({"type": "session_meta", "payload": {"meta": {"id": id}}});
        home.stale(
            &format!(".codex/sessions/2020/01/01/rollout-{id}.jsonl"),
            &format!("{meta}\n"),
        );
        home
    }

    fn snapshot(&self) -> Vec<(PathBuf, u64)> {
        fn walk(dir: &Path, out: &mut Vec<(PathBuf, u64)>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let meta = entry.metadata().unwrap();
                if meta.is_dir() {
                    walk(&entry.path(), out);
                } else {
                    out.push((entry.path(), meta.len()));
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.0, &mut out);
        out.sort();
        out
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn bin() -> String {
    std::env::var("CARGO_BIN_EXE_cancellai-cli").unwrap()
}

fn command(home: &TempHome) -> Command {
    let mut command = Command::new(bin());
    command
        .env("HOME", &home.0)
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME");
    command
}

/// Starts `desktop-api --connections <n>` and reads its descriptor line.
fn start_api(home: &TempHome, connections: u32) -> (Child, Descriptor) {
    let mut child = command(home)
        .args(["desktop-api", "--connections", &connections.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    let descriptor: Descriptor = serde_json::from_str(line.trim()).unwrap();
    (child, descriptor)
}

fn cli_json(home: &TempHome, args: &[&str]) -> serde_json::Value {
    let output = command(home).args(args).output().unwrap();
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Removes what depends on the wall clock at the moment each document was built: the
/// generation timestamp, and the retention cutoff (seconds before "now") that activity
/// explanations quote. Everything else must match exactly.
fn without_timestamp(mut document: serde_json::Value) -> serde_json::Value {
    fn clock_free(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, inner) in map.iter_mut() {
                    if key == "explanation" {
                        if let serde_json::Value::String(text) = inner {
                            *text = text
                                .split(' ')
                                .map(|word| {
                                    if word.ends_with('s')
                                        && word[..word.len() - 1]
                                            .bytes()
                                            .all(|b| b.is_ascii_digit())
                                        && word.len() > 1
                                    {
                                        "<cutoff>s"
                                    } else {
                                        word
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(" ");
                        }
                    } else {
                        clock_free(inner);
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(clock_free),
            _ => {}
        }
    }
    document.as_object_mut().unwrap().remove("generated_at");
    clock_free(&mut document);
    document
}

#[test]
fn documents_match_the_cli_json_output_for_the_same_tree_and_query() {
    let home = TempHome::populated("parity");
    // Every query passes `allow_running`: without it both documents depend on which provider
    // processes happen to be alive at each of the two moments they are built, which is live
    // state, not a difference between the API and the CLI.
    let queries = [
        (
            Query {
                allow_running: true,
                ..Query::default()
            },
            vec!["--allow-running"],
        ),
        (
            Query {
                days: 1,
                keep_latest: 0,
                tool: ToolScope::All,
                allow_running: true,
            },
            vec!["--days", "1", "--keep-latest", "0", "--allow-running"],
        ),
        (
            Query {
                days: 1,
                keep_latest: 0,
                tool: ToolScope::Codex,
                allow_running: true,
            },
            vec![
                "--days",
                "1",
                "--keep-latest",
                "0",
                "--tool",
                "codex",
                "--allow-running",
            ],
        ),
    ];
    let (mut child, descriptor) = start_api(&home, 1);
    let mut client = Client::connect(&descriptor).unwrap();
    assert_eq!(client.engine_version(), env!("CARGO_PKG_VERSION"));
    for (query, flags) in &queries {
        for (kind, command) in [
            (DocumentKind::Status, "status"),
            (DocumentKind::Inspect, "inspect"),
            (DocumentKind::Plan, "plan"),
        ] {
            let envelope = client.document(kind, *query).unwrap();
            let mut args = vec![command, "--json"];
            if command == "inspect" {
                args.pop();
            }
            args.extend(flags.iter().copied());
            let expected = cli_json(&home, &args);
            assert_eq!(
                without_timestamp(envelope.document),
                without_timestamp(expected),
                "{kind:?} {flags:?}"
            );
            assert!(!envelope.scan_incomplete);
        }
    }
    client.close().unwrap();
    assert!(child.wait().unwrap().success());
}

#[test]
fn serving_a_plan_full_of_delete_candidates_mutates_nothing() {
    let home = TempHome::populated("no-bypass");
    let before = home.snapshot();
    let (mut child, descriptor) = start_api(&home, 1);
    let mut client = Client::connect(&descriptor).unwrap();
    let aggressive = Query {
        days: 0,
        keep_latest: 0,
        tool: ToolScope::All,
        allow_running: true,
    };
    let plan = client.document(DocumentKind::Plan, aggressive).unwrap();
    let actions = plan.document["actions"].as_array().unwrap();
    assert!(
        actions.iter().any(|a| a["action_class"] == "delete"),
        "the fixture must make the plan propose deletions, or this test proves nothing: {actions:?}"
    );
    for kind in [
        DocumentKind::Status,
        DocumentKind::Inspect,
        DocumentKind::Plan,
    ] {
        client.document(kind, aggressive).unwrap();
    }
    client.close().unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(before, home.snapshot());
}

#[test]
fn an_unauthorized_client_gets_nothing_and_the_server_keeps_serving() {
    let home = TempHome::populated("unauthorized");
    let (mut child, descriptor) = start_api(&home, 2);
    let forged = Descriptor {
        token: "0".repeat(descriptor.token.len()),
        ..descriptor.clone()
    };
    match Client::connect(&forged) {
        Err(ClientError::Refused { code, .. }) => assert_eq!(code, ErrorCode::Unauthorized),
        other => panic!("{other:?}"),
    }
    let mut client = Client::connect(&descriptor).unwrap();
    client
        .document(DocumentKind::Status, Query::default())
        .unwrap();
    client.close().unwrap();
    assert!(child.wait().unwrap().success());
}

#[test]
fn a_custom_root_plan_reports_the_same_withholding_the_cli_reports() {
    let home = TempHome::populated("withheld");
    let custom = TempHome::populated("withheld-custom");
    let mut child = Command::new(bin())
        .args(["desktop-api", "--connections", "1"])
        .env("HOME", &home.0)
        .env("CLAUDE_CONFIG_DIR", custom.0.join(".claude"))
        .env_remove("CODEX_HOME")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    let descriptor: Descriptor = serde_json::from_str(line.trim()).unwrap();
    let mut client = Client::connect(&descriptor).unwrap();
    let plan = client
        .document(
            DocumentKind::Plan,
            Query {
                days: 0,
                keep_latest: 0,
                tool: ToolScope::All,
                allow_running: true,
            },
        )
        .unwrap();
    client.close().unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(
        plan.withheld_by_root_authority,
        vec!["claude-code".to_string()]
    );
}

#[test]
fn desktop_api_refuses_a_zero_connection_limit() {
    let home = TempHome::new("zero");
    let output = command(&home)
        .args(["desktop-api", "--connections", "0"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

fn cli_text(home: &TempHome, args: &[&str]) -> String {
    String::from_utf8(command(home).args(args).output().unwrap().stdout).unwrap()
}

#[test]
fn the_dashboard_view_matches_the_cli_human_summaries() {
    let home = TempHome::populated("view-parity");
    // A second, fresh Claude session so the plan holds both deletions and observations.
    let fresh = home
        .0
        .join(".claude/projects/p/44444444-4444-4444-8444-444444444444.jsonl");
    std::fs::write(&fresh, "{}").unwrap();

    let cases = [
        (
            Query {
                days: 1,
                keep_latest: 0,
                tool: ToolScope::All,
                allow_running: true,
            },
            vec!["--days", "1", "--keep-latest", "0", "--allow-running"],
        ),
        (
            Query {
                days: 1,
                keep_latest: 1,
                tool: ToolScope::Claude,
                allow_running: true,
            },
            vec![
                "--days",
                "1",
                "--keep-latest",
                "1",
                "--tool",
                "claude",
                "--allow-running",
            ],
        ),
    ];
    let (mut child, descriptor) = start_api(&home, 1);
    let mut client = Client::connect(&descriptor).unwrap();
    let (mut seen_deletes, mut seen_observations) = (0, 0);
    for (query, flags) in &cases {
        let status = client.document(DocumentKind::Status, *query).unwrap();
        let plan = client.document(DocumentKind::Plan, *query).unwrap();
        let view = cancellai_desktop::build(client.engine_version(), &status, &plan).unwrap();
        seen_deletes += view.plan.delete_candidates;
        seen_observations += view.plan.observations;

        let mut status_args = vec!["status"];
        status_args.extend(flags.iter().copied());
        let expected_status: Vec<String> = cli_text(&home, &status_args)
            .lines()
            .map(str::to_string)
            .collect();
        let shown_status: Vec<String> = view
            .providers
            .iter()
            .map(|row| {
                format!(
                    "{}: {} artifact(s), {} bytes, scan_complete={}",
                    row.provider_id, row.artifacts, row.bytes, row.scan_complete
                )
            })
            .collect();
        assert_eq!(shown_status, expected_status, "{flags:?}");

        let mut plan_args = vec!["plan"];
        plan_args.extend(flags.iter().copied());
        let expected_plan = cli_text(&home, &plan_args);
        let headline = format!(
            "{} action(s) proposed: {} delete candidate(s), {} observation(s)",
            view.plan.total_actions, view.plan.delete_candidates, view.plan.observations
        );
        assert_eq!(
            expected_plan.lines().next(),
            Some(headline.as_str()),
            "{flags:?}"
        );
        assert_eq!(
            expected_plan.lines().count(),
            view.plan.actions.len() + 1,
            "one CLI line per action the view shows: {flags:?}"
        );
        for action in &view.plan.actions {
            assert!(
                expected_plan.contains(&action.reason),
                "the CLI does not print the reason the view shows: {}",
                action.reason
            );
        }
    }
    assert!(
        seen_deletes > 0 && seen_observations > 0,
        "the fixture must exercise both counts, or the headline comparison proves little: \
         {seen_deletes} deletes, {seen_observations} observations"
    );
    client.close().unwrap();
    assert!(child.wait().unwrap().success());
}

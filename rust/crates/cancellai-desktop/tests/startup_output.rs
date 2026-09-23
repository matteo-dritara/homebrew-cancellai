//! The real `cancellai-desktop` binary never writes its token to standard output or standard
//! error (E19 round 1: the tokenised URL used to be printed, so redirecting output to a log
//! captured the dashboard's only credential).
//!
//! The engine is stood in for by a shell script that prints the descriptor of a desktop API
//! server this test runs in-process, so the binary is exercised end to end - start the engine,
//! write the URL file, serve a page - without needing `cancellai-cli` built. Unix-only because the
//! stand-in is a shell script; the property itself is platform-independent and the pure startup
//! message is unit-tested everywhere (`launch::tests`).

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use cancellai_desktop_api::{
    DocumentEnvelope, DocumentKind, DocumentSource, ProviderSummary, Query, Server,
};

struct FakeEngine;

impl DocumentSource for FakeEngine {
    fn engine_version(&self) -> String {
        "fake-engine".to_string()
    }

    fn document(&self, kind: DocumentKind, query: Query) -> Result<DocumentEnvelope, String> {
        Ok(DocumentEnvelope {
            kind,
            query,
            document: serde_json::json!({"actions": [], "provider_roots": []}),
            scan_incomplete: false,
            withheld_by_root_authority: Vec::new(),
            provider_summaries: vec![ProviderSummary {
                provider_id: "codex-cli".to_string(),
                artifacts: 1,
                bytes: 1,
                scan_complete: true,
            }],
        })
    }
}

fn scratch(label: &str) -> PathBuf {
    let token = cancellai_desktop_api::SessionToken::generate().unwrap();
    let dir = std::env::temp_dir().join(format!(
        "cancellai-desktop-startup-{label}-{}",
        &token.as_str()[..12]
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A stand-in `cancellai-cli` that prints `descriptor_line` and then stays alive.
fn fake_cli(dir: &Path, descriptor_line: &str) -> PathBuf {
    let path = dir.join("fake-cancellai-cli");
    std::fs::write(
        &path,
        format!("#!/bin/sh\nprintf '%s\\n' '{descriptor_line}'\nexec sleep 30\n"),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn wait_for(path: &Path) -> String {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(text) = std::fs::read_to_string(path)
            && text.ends_with('\n')
        {
            return text.trim().to_string();
        }
        assert!(Instant::now() < deadline, "the URL file never appeared");
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_token_reaches_the_url_file_and_never_the_output() {
    let dir = scratch("run");
    let api = Server::bind().unwrap();
    let descriptor = serde_json::to_string(&api.descriptor().unwrap()).unwrap();
    let api_thread = thread::spawn(move || api.serve(&FakeEngine, Some(1)).unwrap());

    let url_file = dir.join("url.txt");
    let child = Command::new(env!("CARGO_BIN_EXE_cancellai-desktop"))
        .args(["--cli"])
        .arg(fake_cli(&dir, &descriptor))
        .args(["--no-open", "--url-file"])
        .arg(&url_file)
        .args(["--requests", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let url = wait_for(&url_file);
    let token = url.rsplit('/').next().unwrap().to_string();
    assert_eq!(token.len(), 64);
    assert_eq!(
        std::fs::metadata(&url_file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let authority = url
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap()
        .to_string();
    let mut stream = TcpStream::connect(&authority).unwrap();
    write!(stream, "GET /{token} HTTP/1.1\r\nHost: {authority}\r\n\r\n").unwrap();
    let mut page = String::new();
    stream.read_to_string(&mut page).unwrap();
    assert!(page.starts_with("HTTP/1.1 200 OK"), "{page}");
    assert!(page.contains("codex-cli"));

    let output = child.wait_with_output().unwrap();
    api_thread.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("listening on 127.0.0.1:"), "{stdout}");
    assert!(!stdout.contains(&token), "token on stdout: {stdout}");
    assert!(!stderr.contains(&token), "token on stderr: {stderr}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_existing_url_file_is_refused_without_printing_the_token() {
    let dir = scratch("exists");
    let api = Server::bind().unwrap();
    let descriptor = serde_json::to_string(&api.descriptor().unwrap()).unwrap();
    let url_file = dir.join("url.txt");
    std::fs::write(&url_file, "planted\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cancellai-desktop"))
        .args(["--cli"])
        .arg(fake_cli(&dir, &descriptor))
        .args(["--no-open", "--url-file"])
        .arg(&url_file)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(std::fs::read_to_string(&url_file).unwrap(), "planted\n");
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!all.contains("http://127.0.0.1"), "{all}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn no_open_without_a_url_file_is_a_usage_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_cancellai-desktop"))
        .arg("--no-open")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--no-open needs --url-file"));
}

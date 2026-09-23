//! The real `cancellai-desktop` binary never writes its token to standard output or standard
//! error (E19 round 1: the tokenised URL used to be printed, so redirecting output to a log
//! captured the dashboard's only credential).
//!
//! The engine is stood in for by a shell script that prints the descriptor of a desktop API
//! server this test runs in-process, so the binary is exercised end to end - start the engine,
//! write the URL file in a fresh private directory, serve a page - without needing
//! `cancellai-cli` built. Unix-only because the
//! stand-in is a shell script; the property itself is platform-independent and the pure startup
//! message is unit-tested everywhere (`launch::tests`).

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

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

/// Starts the real binary with `--no-open` against the stand-in engine and returns the child and
/// the startup line it printed.
fn start(dir: &Path, descriptor: &str) -> (std::process::Child, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cancellai-desktop"))
        .args(["--cli"])
        .arg(fake_cli(dir, descriptor))
        .args(["--no-open", "--requests", "1"])
        .env("TMPDIR", dir)
        .env_remove("XDG_RUNTIME_DIR")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    (child, line)
}

fn url_file_from(line: &str) -> PathBuf {
    let path = line
        .split("URL written to ")
        .nth(1)
        .unwrap_or_else(|| panic!("no URL file named in {line:?}"))
        .trim()
        .trim_end_matches('.');
    PathBuf::from(path)
}

#[test]
fn the_token_reaches_a_private_url_file_and_never_the_output() {
    let dir = scratch("run");
    let api = Server::bind().unwrap();
    let descriptor = serde_json::to_string(&api.descriptor().unwrap()).unwrap();
    let api_thread = thread::spawn(move || api.serve(&FakeEngine, Some(1)).unwrap());

    let (child, line) = start(&dir, &descriptor);
    assert!(line.contains("listening on 127.0.0.1:"), "{line}");
    let url_file = url_file_from(&line);
    assert!(url_file.starts_with(&dir), "{}", url_file.display());
    let url = std::fs::read_to_string(&url_file)
        .unwrap()
        .trim()
        .to_string();
    let token = url.rsplit('/').next().unwrap().to_string();
    assert_eq!(token.len(), 64);
    assert!(!line.contains(&token));
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(url_file.parent().unwrap()), 0o700);
    assert_eq!(mode(&url_file), 0o600);

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
    let rest = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{rest}\n{stderr}");
    assert!(!rest.contains(&token), "token on stdout: {rest}");
    assert!(!stderr.contains(&token), "token on stderr: {stderr}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_unknown_option_is_a_usage_error() {
    for bad in [&["--url-file", "/tmp/u"][..], &["--open"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_cancellai-desktop"))
            .args(bad)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{bad:?}");
    }
}

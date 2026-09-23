//! Unauthorized local clients and schema compatibility against a real loopback server
//! (E19-S01 verification: "Unauthorized local client and schema compatibility tests").
//!
//! Every hostile case is sent as raw bytes over a real socket rather than through `Client`, so
//! the test cannot inherit a client-side assumption the server does not enforce. Each asserts two
//! things: the refusal, and that the engine was never asked for a document.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use cancellai_desktop_api::{
    Client, ClientError, Descriptor, DocumentEnvelope, DocumentKind, DocumentSource, ErrorCode,
    MAX_REQUEST_BYTES, Query, Response, Server,
};

#[derive(Default)]
struct CountingEngine {
    calls: AtomicUsize,
    fail: bool,
}

impl DocumentSource for CountingEngine {
    fn engine_version(&self) -> String {
        "test-engine".to_string()
    }

    fn document(&self, kind: DocumentKind, query: Query) -> Result<DocumentEnvelope, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err("no provider root resolves".to_string());
        }
        Ok(DocumentEnvelope {
            kind,
            query,
            document: serde_json::json!({"secret": "engine-data"}),
            scan_incomplete: false,
            withheld_by_root_authority: Vec::new(),
            provider_summaries: Vec::new(),
        })
    }
}

/// Starts a server that serves `connections` connections on a background thread.
fn start(
    connections: usize,
    fail: bool,
) -> (Descriptor, Arc<CountingEngine>, thread::JoinHandle<()>) {
    let server = Server::bind().unwrap();
    let descriptor = server.descriptor().unwrap();
    let engine = Arc::new(CountingEngine {
        calls: AtomicUsize::new(0),
        fail,
    });
    let served = Arc::clone(&engine);
    let handle = thread::spawn(move || {
        server.serve(served.as_ref(), Some(connections)).unwrap();
    });
    (descriptor, engine, handle)
}

/// Sends raw lines, returns every response line until the server closes the connection.
fn exchange(descriptor: &Descriptor, lines: &[&str]) -> Vec<String> {
    let mut stream = TcpStream::connect(&descriptor.address).unwrap();
    for line in lines {
        stream.write_all(line.as_bytes()).unwrap();
        stream.write_all(b"\n").unwrap();
    }
    stream.flush().unwrap();
    // A server that closes with unsent request bytes still unread may reset the connection;
    // keep whatever arrived before that rather than treating the reset as a test failure.
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
        }
    }
    String::from_utf8(out)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

fn error_code(line: &str) -> ErrorCode {
    match serde_json::from_str::<Response>(line).unwrap() {
        Response::Error { code, .. } => code,
        other => panic!("expected an error, got {other:?}"),
    }
}

const DOCUMENT: &str =
    r#"{"request":"document","kind":"plan","query":{"days":7,"keep_latest":2,"tool":"all"}}"#;

fn hello(version: u32, token: &str) -> String {
    format!(r#"{{"request":"hello","api_version":{version},"token":"{token}"}}"#)
}

#[test]
fn a_document_request_before_hello_is_refused_and_the_connection_closed() {
    let (descriptor, engine, handle) = start(1, false);
    let responses = exchange(&descriptor, &[DOCUMENT, DOCUMENT]);
    handle.join().unwrap();
    assert_eq!(
        responses.len(),
        1,
        "the connection must close after the refusal"
    );
    assert_eq!(error_code(&responses[0]), ErrorCode::NotAuthenticated);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    assert!(!responses[0].contains("engine-data"));
}

#[test]
fn a_wrong_empty_or_truncated_token_is_refused() {
    for token in ["", "0000", "not-the-token"] {
        let (descriptor, engine, handle) = start(1, false);
        let responses = exchange(&descriptor, &[&hello(1, token), DOCUMENT]);
        handle.join().unwrap();
        assert_eq!(responses.len(), 1, "{token:?}");
        assert_eq!(error_code(&responses[0]), ErrorCode::Unauthorized);
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    }
    // One character short of the real token.
    let (descriptor, engine, handle) = start(1, false);
    let short = &descriptor.token[..descriptor.token.len() - 1];
    let responses = exchange(&descriptor, &[&hello(1, short), DOCUMENT]);
    handle.join().unwrap();
    assert_eq!(error_code(&responses[0]), ErrorCode::Unauthorized);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn a_token_from_another_server_is_refused() {
    let (first, _, first_handle) = start(1, false);
    let (second, engine, second_handle) = start(1, false);
    let responses = exchange(&second, &[&hello(1, &first.token), DOCUMENT]);
    second_handle.join().unwrap();
    exchange(&first, &[]);
    first_handle.join().unwrap();
    assert_eq!(error_code(&responses[0]), ErrorCode::Unauthorized);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn an_unsupported_version_is_refused_with_the_supported_list() {
    for version in [0, 2, u32::MAX] {
        let (descriptor, engine, handle) = start(1, false);
        let responses = exchange(&descriptor, &[&hello(version, &descriptor.token), DOCUMENT]);
        handle.join().unwrap();
        assert_eq!(responses.len(), 1);
        match serde_json::from_str::<Response>(&responses[0]).unwrap() {
            Response::Error {
                code,
                supported_versions,
                ..
            } => {
                assert_eq!(code, ErrorCode::UnsupportedVersion);
                assert_eq!(supported_versions, vec![1]);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn a_browser_style_http_request_is_refused_without_engine_data() {
    let (descriptor, engine, handle) = start(1, false);
    let responses = exchange(
        &descriptor,
        &["GET /document?kind=plan HTTP/1.1", "Host: 127.0.0.1", ""],
    );
    handle.join().unwrap();
    assert_eq!(responses.len(), 1);
    assert_eq!(error_code(&responses[0]), ErrorCode::MalformedRequest);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn a_mutating_request_is_not_part_of_the_protocol_even_when_authenticated() {
    let (descriptor, engine, handle) = start(1, false);
    let responses = exchange(
        &descriptor,
        &[
            &hello(1, &descriptor.token),
            r#"{"request":"clean","yes":true}"#,
            r#"{"request":"document","kind":"clean","query":{"days":0,"keep_latest":0,"tool":"all"}}"#,
            r#"{"request":"goodbye"}"#,
        ],
    );
    handle.join().unwrap();
    assert_eq!(responses.len(), 4);
    assert_eq!(error_code(&responses[1]), ErrorCode::MalformedRequest);
    assert_eq!(error_code(&responses[2]), ErrorCode::MalformedRequest);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn an_oversized_frame_is_refused_before_it_is_parsed() {
    let (descriptor, engine, handle) = start(1, false);
    let huge = format!(
        r#"{{"request":"hello","api_version":1,"token":"{}"}}"#,
        "a".repeat(MAX_REQUEST_BYTES)
    );
    let responses = exchange(&descriptor, &[&huge]);
    handle.join().unwrap();
    assert_eq!(responses.len(), 1);
    assert_eq!(error_code(&responses[0]), ErrorCode::FrameTooLarge);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn a_hostile_connection_does_not_stop_the_next_legitimate_one() {
    let (descriptor, engine, handle) = start(2, false);
    exchange(&descriptor, &["garbage"]);
    let mut client = Client::connect(&descriptor).unwrap();
    assert_eq!(client.engine_version(), "test-engine");
    let envelope = client
        .document(DocumentKind::Status, Query::default())
        .unwrap();
    assert_eq!(envelope.kind, DocumentKind::Status);
    assert_eq!(envelope.document["secret"], "engine-data");
    client.close().unwrap();
    handle.join().unwrap();
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn an_authenticated_client_gets_documents_and_engine_errors_keep_the_session() {
    let (descriptor, engine, handle) = start(1, true);
    let mut client = Client::connect(&descriptor).unwrap();
    match client.document(DocumentKind::Plan, Query::default()) {
        Err(ClientError::Refused { code, .. }) => assert_eq!(code, ErrorCode::EngineUnavailable),
        other => panic!("{other:?}"),
    }
    // The session is still usable after an engine error.
    assert!(
        client
            .document(DocumentKind::Inspect, Query::default())
            .is_err()
    );
    client.close().unwrap();
    handle.join().unwrap();
    assert_eq!(engine.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn a_second_hello_is_refused_but_does_not_end_the_session() {
    let (descriptor, _, handle) = start(1, false);
    let responses = exchange(
        &descriptor,
        &[
            &hello(1, &descriptor.token),
            &hello(1, &descriptor.token),
            DOCUMENT,
            r#"{"request":"goodbye"}"#,
        ],
    );
    handle.join().unwrap();
    assert_eq!(responses.len(), 4);
    assert_eq!(error_code(&responses[1]), ErrorCode::AlreadyAuthenticated);
    assert!(responses[2].contains(r#""response":"document""#));
    assert!(responses[3].contains("farewell"));
}

#[test]
fn the_client_reports_version_refusal_and_refuses_non_loopback_addresses() {
    let (descriptor, _, handle) = start(1, false);
    match Client::connect_with(&descriptor, 9) {
        Err(ClientError::Refused {
            code,
            supported_versions,
            ..
        }) => {
            assert_eq!(code, ErrorCode::UnsupportedVersion);
            assert_eq!(supported_versions, vec![1]);
        }
        other => panic!("{other:?}"),
    }
    handle.join().unwrap();
    let remote = Descriptor {
        address: "192.0.2.1:9".to_string(),
        ..descriptor.clone()
    };
    assert!(matches!(
        Client::connect(&remote),
        Err(ClientError::Protocol(_))
    ));
    let bad = Descriptor {
        address: "not an address".to_string(),
        ..descriptor
    };
    assert!(matches!(
        Client::connect(&bad),
        Err(ClientError::Protocol(_))
    ));
}

#[test]
fn the_descriptor_is_loopback_and_one_json_line() {
    let server = Server::bind().unwrap();
    let descriptor = server.descriptor().unwrap();
    assert!(descriptor.address.starts_with("127.0.0.1:"));
    let line = serde_json::to_string(&descriptor).unwrap();
    assert!(!line.contains('\n'));
    let round: Descriptor = serde_json::from_str(&line).unwrap();
    assert_eq!(round, descriptor);
}

#[test]
fn a_line_split_request_is_read_as_one_frame() {
    let (descriptor, _, handle) = start(1, false);
    let mut stream = TcpStream::connect(&descriptor.address).unwrap();
    let frame = hello(1, &descriptor.token);
    let (a, b) = frame.split_at(10);
    stream.write_all(a.as_bytes()).unwrap();
    stream.flush().unwrap();
    thread::sleep(std::time::Duration::from_millis(20));
    stream.write_all(b.as_bytes()).unwrap();
    stream.write_all(b"\r\n{\"request\":\"goodbye\"}").unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    let lines: Vec<String> = BufReader::new(stream).lines().map(Result::unwrap).collect();
    handle.join().unwrap();
    assert!(lines[0].contains("welcome"));
    assert!(lines[1].contains("farewell"));
}

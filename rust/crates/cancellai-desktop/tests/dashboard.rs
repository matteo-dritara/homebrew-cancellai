//! The dashboard server against real sockets (E19-S02).
//!
//! The refusal cases are the attacks a web page elsewhere in the same browser could mount
//! against a loopback port: guessing the URL, DNS rebinding, and non-GET requests. The last test
//! runs the whole chain in-process - a desktop API server with a fake engine, the real
//! `ApiViewSource`, and the real dashboard - to prove the page shows what the engine returned.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::thread;

use cancellai_desktop::{
    ApiViewSource, Dashboard, DashboardView, PlanView, ProviderRow, ViewSource,
};
use cancellai_desktop_api::{
    DocumentEnvelope, DocumentKind, DocumentSource, ProviderSummary, Query, Server, ToolScope,
};

struct Fixed {
    seen: Mutex<Vec<Query>>,
    fail: bool,
}

impl ViewSource for Fixed {
    fn view(&self, query: Query) -> Result<DashboardView, String> {
        self.seen.lock().unwrap().push(query);
        if self.fail {
            return Err("<engine> down".to_string());
        }
        Ok(DashboardView {
            engine_version: "9.9.9".to_string(),
            query,
            providers: vec![ProviderRow {
                provider_id: "codex-cli".to_string(),
                artifacts: 4,
                bytes: 400,
                scan_complete: true,
                root_origin: "default".to_string(),
                mutation_eligible: true,
            }],
            scan_incomplete: false,
            plan: PlanView {
                total_actions: 0,
                delete_candidates: 0,
                observations: 0,
                withheld_by_root_authority: Vec::new(),
                actions: Vec::new(),
            },
        })
    }
}

fn fixed(fail: bool) -> Fixed {
    Fixed {
        seen: Mutex::new(Vec::new()),
        fail,
    }
}

fn path_of(dashboard: &Dashboard) -> String {
    let url = dashboard.url();
    url[url.find('/').unwrap() + 2..]
        .split_once('/')
        .map(|(_, p)| format!("/{p}"))
        .unwrap()
}

fn head(method: &str, target: &str, host: &str) -> String {
    format!("{method} {target} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: test\r\n\r\n")
}

#[test]
fn only_the_tokenised_get_from_a_loopback_host_is_answered() {
    let dashboard = Dashboard::bind().unwrap();
    let source = fixed(false);
    let path = path_of(&dashboard);
    let host = format!("127.0.0.1:{}", dashboard.port());

    let ok = dashboard.respond(&head("GET", &path, &host), &source);
    assert_eq!(ok.status, 200);
    assert!(ok.body.contains("codex-cli"));

    let localhost = format!("localhost:{}", dashboard.port());
    assert_eq!(
        dashboard
            .respond(&head("GET", &path, &localhost), &source)
            .status,
        200
    );

    let cases = [
        (head("GET", "/", &host), 404),
        (head("GET", "/wrong-token", &host), 404),
        (head("GET", &format!("{path}x"), &host), 404),
        (head("GET", &path, "evil.example:80"), 421),
        (
            head(
                "GET",
                &path,
                &format!("127.0.0.1:{}", dashboard.port().wrapping_add(1)),
            ),
            421,
        ),
        (format!("GET {path} HTTP/1.1\r\n\r\n"), 421),
        (head("POST", &path, &host), 405),
        (head("DELETE", &path, &host), 405),
        (head("GET", &format!("{path}?days=x"), &host), 400),
        (head("GET", &format!("{path}?tool=everything"), &host), 400),
        (head("GET", &format!("{path}?clean=yes"), &host), 400),
        ("garbage".to_string(), 400),
        (format!("GET {path} SPDY/3\r\nHost: {host}\r\n\r\n"), 400),
    ];
    for (request, expected) in cases {
        let reply = dashboard.respond(&request, &source);
        assert_eq!(reply.status, expected, "{request:?}");
        assert!(
            !reply.body.contains("codex-cli"),
            "{request:?} leaked a view"
        );
    }
    // Only the two accepted requests ever reached the engine.
    assert_eq!(source.seen.lock().unwrap().len(), 2);
}

#[test]
fn the_query_string_reaches_the_engine_exactly() {
    let dashboard = Dashboard::bind().unwrap();
    let source = fixed(false);
    let path = path_of(&dashboard);
    let host = format!("127.0.0.1:{}", dashboard.port());
    let target = format!("{path}?days=30&keep_latest=0&tool=codex&allow_running=true");
    assert_eq!(
        dashboard
            .respond(&head("GET", &target, &host), &source)
            .status,
        200
    );
    assert_eq!(
        source.seen.lock().unwrap()[0],
        Query {
            days: 30,
            keep_latest: 0,
            tool: ToolScope::Codex,
            allow_running: true
        }
    );
}

#[test]
fn an_engine_failure_is_shown_escaped_not_as_an_empty_dashboard() {
    let dashboard = Dashboard::bind().unwrap();
    let path = path_of(&dashboard);
    let host = format!("127.0.0.1:{}", dashboard.port());
    let reply = dashboard.respond(&head("GET", &path, &host), &fixed(true));
    assert_eq!(reply.status, 502);
    assert!(reply.body.contains("&lt;engine&gt; down"));
    assert!(!reply.body.contains("action(s) proposed"));
}

fn fetch(port: u16, request: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut out = String::new();
    stream.read_to_string(&mut out).unwrap();
    out
}

#[test]
fn replies_carry_the_security_headers_and_oversized_heads_are_refused() {
    let dashboard = Dashboard::bind().unwrap();
    let port = dashboard.port();
    let path = path_of(&dashboard);
    let handle = thread::spawn(move || {
        dashboard.serve(&fixed(false), Some(2)).unwrap();
    });
    let reply = fetch(port, &head("GET", &path, &format!("127.0.0.1:{port}")));
    assert!(reply.starts_with("HTTP/1.1 200 OK\r\n"));
    for header in [
        "Content-Security-Policy: default-src 'none'",
        "frame-ancestors 'none'",
        "X-Frame-Options: DENY",
        "Referrer-Policy: no-referrer",
        "Cache-Control: no-store",
        "X-Content-Type-Options: nosniff",
    ] {
        assert!(reply.contains(header), "missing {header}");
    }
    let huge = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Pad: {}\r\n\r\n",
        "a".repeat(cancellai_desktop::server::MAX_HEAD_BYTES)
    );
    assert!(fetch(port, &huge).starts_with("HTTP/1.1 431 "));
    handle.join().unwrap();
}

struct FakeEngine;

impl DocumentSource for FakeEngine {
    fn engine_version(&self) -> String {
        "fake-engine".to_string()
    }

    fn document(&self, kind: DocumentKind, query: Query) -> Result<DocumentEnvelope, String> {
        let document = match kind {
            DocumentKind::Plan => serde_json::json!({"actions": [
                {"action_class": "delete", "target_artifact_ids": ["artifact-1"], "reason": "stale <old>"},
                {"action_class": "observe", "target_artifact_ids": ["artifact-2"], "reason": "kept"}
            ]}),
            _ => serde_json::json!({"provider_roots": [
                {"provider_id": "claude-code", "origin": "override", "mutation_eligible": false}
            ]}),
        };
        Ok(DocumentEnvelope {
            kind,
            query,
            document,
            scan_incomplete: false,
            withheld_by_root_authority: if kind == DocumentKind::Plan {
                vec!["claude-code".to_string()]
            } else {
                Vec::new()
            },
            provider_summaries: vec![ProviderSummary {
                provider_id: "claude-code".to_string(),
                artifacts: 2,
                bytes: 1234,
                scan_complete: true,
            }],
        })
    }
}

#[test]
fn the_dashboard_shows_what_the_desktop_api_returned() {
    let api = Server::bind().unwrap();
    let descriptor = api.descriptor().unwrap();
    let api_thread = thread::spawn(move || api.serve(&FakeEngine, Some(1)).unwrap());

    let dashboard = Dashboard::bind().unwrap();
    let port = dashboard.port();
    let path = path_of(&dashboard);
    let source = ApiViewSource::new(descriptor);
    let dashboard_thread = thread::spawn(move || dashboard.serve(&source, Some(1)).unwrap());

    let page = fetch(port, &head("GET", &path, &format!("127.0.0.1:{port}")));
    dashboard_thread.join().unwrap();
    api_thread.join().unwrap();

    assert!(page.starts_with("HTTP/1.1 200 OK"));
    assert!(page.contains("Engine fake-engine via the desktop API"));
    assert!(page.contains(
        "<td>claude-code</td><td>2</td><td>1234</td><td>yes</td><td>override</td><td>no</td>"
    ));
    assert!(page.contains("2 action(s) proposed: 1 delete candidate(s), 1 observation(s)."));
    assert!(page.contains("withheld for: claude-code"));
    assert!(page.contains("stale &lt;old&gt;"));
}

#[test]
fn an_unreachable_api_is_a_502_page() {
    let descriptor = Server::bind().unwrap().descriptor().unwrap(); // bound, then dropped
    let dashboard = Dashboard::bind().unwrap();
    let path = path_of(&dashboard);
    let host = format!("127.0.0.1:{}", dashboard.port());
    let reply = dashboard.respond(&head("GET", &path, &host), &ApiViewSource::new(descriptor));
    assert_eq!(reply.status, 502);
    assert!(reply.body.contains("desktop API unreachable"));
}

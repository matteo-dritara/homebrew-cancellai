//! A deliberately small HTTP/1.1 server for the dashboard page, on loopback only.
//!
//! It answers one route - `GET /<token>` with an optional query string - and nothing else.
//! Three checks run before any view is built, each closing an attack a local web page could
//! otherwise mount against a loopback port:
//!
//! - **the path token** (a fresh 256-bit value per process): a page elsewhere in the browser
//!   cannot guess the URL, so it cannot load the dashboard or read it cross-origin;
//! - **the `Host` header** must be `127.0.0.1:<port>` or `localhost:<port>`: a DNS-rebinding page
//!   whose hostname now resolves to 127.0.0.1 still sends its own hostname and is refused;
//! - **the method** must be `GET`: there is no state to change, so nothing else is accepted.
//!
//! Every response carries a `Content-Security-Policy` that forbids script and framing, and
//! `Referrer-Policy: no-referrer`, so the tokenised URL never leaves the page.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::time::Duration;

use cancellai_desktop_api::{Query, SessionToken, ToolScope};

use crate::render;
use crate::viewmodel::DashboardView;

/// The largest request head read before refusing.
pub const MAX_HEAD_BYTES: usize = 8 * 1024;

/// How long a browser connection may stay silent.
pub const READ_TIMEOUT: Duration = Duration::from_secs(10);

const SECURITY_HEADERS: &str = "Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; \
form-action 'self'; frame-ancestors 'none'; base-uri 'none'\r\n\
X-Content-Type-Options: nosniff\r\n\
X-Frame-Options: DENY\r\n\
Referrer-Policy: no-referrer\r\n\
Cache-Control: no-store\r\n\
Connection: close\r\n";

/// Where views come from. The real source asks the desktop API; tests supply fixed views.
pub trait ViewSource {
    fn view(&self, query: Query) -> Result<DashboardView, String>;
}

/// A response the dashboard sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub reason: &'static str,
    pub body: String,
}

impl Reply {
    fn html(status: u16, reason: &'static str, body: String) -> Self {
        Self {
            status,
            reason,
            body,
        }
    }

    fn refusal(status: u16, reason: &'static str) -> Self {
        Self::html(status, reason, render::error_page(reason))
    }
}

/// The bound dashboard server.
#[derive(Debug)]
pub struct Dashboard {
    listener: TcpListener,
    token: SessionToken,
    port: u16,
}

impl Dashboard {
    /// Binds an ephemeral port on 127.0.0.1 with a fresh path token.
    pub fn bind() -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let token = SessionToken::generate().map_err(io::Error::other)?;
        Ok(Self {
            listener,
            token,
            port,
        })
    }

    /// The URL to open. Contains the path token.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/{}", self.port, self.token.as_str())
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Serves requests one at a time, until `max_requests` have been answered (forever when
    /// `None`). A failed connection never stops the server.
    pub fn serve(&self, source: &dyn ViewSource, max_requests: Option<usize>) -> io::Result<()> {
        self.serve_observed(source, max_requests, &mut |_| {})
    }

    /// [`Dashboard::serve`], calling `observe` with every reply after it is sent - how the
    /// binary learns the page has been loaded and its launcher file can go.
    pub fn serve_observed(
        &self,
        source: &dyn ViewSource,
        max_requests: Option<usize>,
        observe: &mut dyn FnMut(&Reply),
    ) -> io::Result<()> {
        let mut served = 0usize;
        while max_requests.is_none_or(|max| served < max) {
            let (stream, peer) = self.listener.accept()?;
            served = served.saturating_add(1);
            if !peer.ip().is_loopback() {
                continue;
            }
            if let Ok(reply) = self.answer(stream, source) {
                observe(&reply);
            }
        }
        Ok(())
    }

    fn answer(&self, stream: TcpStream, source: &dyn ViewSource) -> io::Result<Reply> {
        stream.set_read_timeout(Some(READ_TIMEOUT))?;
        let mut reader = BufReader::new(stream.try_clone()?);
        let reply = match read_head(&mut reader) {
            Ok(head) => self.respond(&head, source),
            Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                Reply::refusal(431, "Request Header Fields Too Large")
            }
            Err(error) => return Err(error),
        };
        write_reply(stream, &reply)?;
        Ok(reply)
    }

    /// Decides the reply for one request head. Public so tests can drive every branch without
    /// a socket.
    pub fn respond(&self, head: &str, source: &dyn ViewSource) -> Reply {
        let mut lines = head.split("\r\n");
        let request_line = lines.next().unwrap_or_default();
        let mut parts = request_line.split(' ');
        let (method, target, version) = (parts.next(), parts.next(), parts.next());
        let (Some(method), Some(target), Some(version)) = (method, target, version) else {
            return Reply::refusal(400, "Bad Request");
        };
        if !version.starts_with("HTTP/1.") || parts.next().is_some() {
            return Reply::refusal(400, "Bad Request");
        }
        let host = lines
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.trim().eq_ignore_ascii_case("host"))
            .map(|(_, value)| value.trim().to_string());
        let allowed_hosts = [
            format!("127.0.0.1:{}", self.port),
            format!("localhost:{}", self.port),
        ];
        if !host.is_some_and(|host| {
            allowed_hosts
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(&host))
        }) {
            return Reply::refusal(421, "Misdirected Request");
        }
        if method != "GET" {
            return Reply::refusal(405, "Method Not Allowed");
        }
        let (path, query_string) = target.split_once('?').unwrap_or((target, ""));
        let presented = path.strip_prefix('/').unwrap_or_default();
        if !self.token.matches(presented) {
            return Reply::refusal(404, "Not Found");
        }
        let Some(query) = parse_query(query_string) else {
            return Reply::refusal(400, "Bad Request");
        };
        match source.view(query) {
            Ok(view) => Reply::html(200, "OK", render::page(&view, path)),
            Err(reason) => Reply::html(
                502,
                "Bad Gateway",
                render::error_page(&format!("The engine could not answer: {reason}")),
            ),
        }
    }
}

/// Reads a request head (through the blank line). `InvalidData` when it exceeds
/// [`MAX_HEAD_BYTES`] or is not UTF-8.
pub fn read_head(reader: &mut impl BufRead) -> io::Result<String> {
    let mut head = Vec::new();
    loop {
        let mut line = Vec::new();
        let limit = u64::try_from(MAX_HEAD_BYTES.saturating_sub(head.len()))
            .unwrap_or(0)
            .saturating_add(1);
        let read = io::Read::take(&mut *reader, limit).read_until(b'\n', &mut line)?;
        head.extend_from_slice(&line);
        if head.len() > MAX_HEAD_BYTES {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "head too large"));
        }
        if read == 0 || line == b"\r\n" || line == b"\n" {
            break;
        }
    }
    String::from_utf8(head).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not UTF-8"))
}

/// Parses the dashboard form's query string. `None` on anything it does not recognise: a
/// malformed query is refused, not guessed.
pub fn parse_query(query_string: &str) -> Option<Query> {
    let mut query = Query::default();
    for pair in query_string.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "days" => query.days = value.parse().ok()?,
            "keep_latest" => query.keep_latest = value.parse().ok()?,
            "tool" => {
                query.tool = match value {
                    "all" => ToolScope::All,
                    "claude" => ToolScope::Claude,
                    "codex" => ToolScope::Codex,
                    _ => return None,
                }
            }
            "allow_running" => {
                query.allow_running = match value {
                    "true" | "on" => true,
                    "false" | "" => false,
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
    Some(query)
}

fn write_reply(mut stream: TcpStream, reply: &Reply) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n{SECURITY_HEADERS}\r\n",
        reply.status,
        reply.reason,
        reply.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(reply.body.as_bytes())?;
    stream.flush()
}

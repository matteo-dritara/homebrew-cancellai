//! The wire protocol: newline-delimited JSON frames, one request per line, one response per line.
//!
//! The request vocabulary is the security property. It names three read-only documents and the
//! handshake around them, and nothing else: there is no `clean`, `configure`, `restore` or
//! `execute` variant for a client to send, so "the desktop cannot bypass the safety executor" does
//! not depend on a server remembering to refuse one. `deny_unknown_fields` on every shape means a
//! frame carrying an extra field is refused rather than half-read.

use serde::{Deserialize, Serialize};

/// The API version this build speaks. A client whose `hello` names another version is refused
/// with [`ErrorCode::UnsupportedVersion`], which lists [`SUPPORTED_VERSIONS`].
pub const API_VERSION: u32 = 1;

/// Every API version this build accepts, oldest first.
pub const SUPPORTED_VERSIONS: &[u32] = &[API_VERSION];

/// The largest request frame the server reads, newline included. Requests are small; anything
/// larger is refused before it is parsed.
pub const MAX_REQUEST_BYTES: usize = 16 * 1024;

/// Which provider(s) a document covers - the same choice as the CLI's `--tool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolScope {
    #[default]
    All,
    Claude,
    Codex,
}

/// The parameters of a document request - the read-only flags the CLI's `status`/`inspect`/`plan`
/// accept, and deliberately nothing that only `clean` takes (`--yes`, `--dry-run`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub days: u32,
    pub keep_latest: u32,
    pub tool: ToolScope,
    /// The CLI's `--allow-running` on a read-only command: preview what `clean` would propose
    /// even though a provider process appears to be running. It changes what a plan shows, never
    /// what anything does - no request can execute a plan. Absent means `false`.
    #[serde(default)]
    pub allow_running: bool,
}

impl Default for Query {
    /// The CLI's own defaults (`--days 7 --keep-latest 2 --tool all`).
    fn default() -> Self {
        Self {
            days: 7,
            keep_latest: 2,
            tool: ToolScope::All,
            allow_running: false,
        }
    }
}

/// The read-only documents a client may ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    /// The inventory document `cancellai-cli status --json` prints.
    Status,
    /// The same inventory document `cancellai-cli inspect` prints.
    Inspect,
    /// The plan document `cancellai-cli plan --json` prints: a preview, never an execution.
    Plan,
}

/// A client frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// Must be the first frame of every connection.
    Hello { api_version: u32, token: String },
    /// Ask for one read-only document.
    Document { kind: DocumentKind, query: Query },
    /// End the session.
    Goodbye,
}

/// A document together with the conditions the CLI reports on stderr and in its exit code, so a
/// desktop client sees what a CLI user sees rather than only the JSON body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentEnvelope {
    pub kind: DocumentKind,
    pub query: Query,
    /// The document itself, exactly as the CLI's JSON output renders it.
    pub document: serde_json::Value,
    /// True when any provider scan was incomplete - the CLI's `IncompleteInventory` exit.
    pub scan_incomplete: bool,
    /// Providers whose destructive work a plan withheld because their root is not the default
    /// root - the CLI's `SafetyBlock` exit. Always empty for inventory documents.
    pub withheld_by_root_authority: Vec<String>,
}

/// Why the server refused a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// A request arrived before a successful `hello`.
    NotAuthenticated,
    /// The `hello` token did not match this session's token.
    Unauthorized,
    /// The `hello` named an API version this build does not speak.
    UnsupportedVersion,
    /// The frame was not a request this protocol defines.
    MalformedRequest,
    /// The frame exceeded [`MAX_REQUEST_BYTES`].
    FrameTooLarge,
    /// The engine could not produce the document (for example, no provider root resolves).
    EngineUnavailable,
    /// A second `hello` on an already-authenticated connection.
    AlreadyAuthenticated,
}

/// A server frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case", deny_unknown_fields)]
pub enum Response {
    Welcome {
        api_version: u32,
        engine_version: String,
    },
    Document {
        envelope: DocumentEnvelope,
    },
    Error {
        code: ErrorCode,
        message: String,
        /// Filled only for [`ErrorCode::UnsupportedVersion`].
        supported_versions: Vec<u32>,
    },
    Farewell,
}

impl Response {
    pub(crate) fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Error {
            code,
            message: message.into(),
            supported_versions: if code == ErrorCode::UnsupportedVersion {
                SUPPORTED_VERSIONS.to_vec()
            } else {
                Vec::new()
            },
        }
    }
}

/// How a client finds a server: printed once, as a single JSON line, on the serving process's
/// standard output, so only the process that started the server learns the token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub api_version: u32,
    /// `127.0.0.1:<port>` - always loopback.
    pub address: String,
    pub token: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Schema compatibility: these literal frames are the version-1 wire format. A change that
    // breaks one of them breaks every client built against version 1 and must bump
    // API_VERSION instead.
    #[test]
    fn version_one_request_frames_are_stable() {
        let hello = r#"{"request":"hello","api_version":1,"token":"abc"}"#;
        assert_eq!(
            serde_json::from_str::<Request>(hello).expect("hello parses"),
            Request::Hello {
                api_version: 1,
                token: "abc".to_string()
            }
        );
        let document = r#"{"request":"document","kind":"plan","query":{"days":7,"keep_latest":2,"tool":"codex"}}"#;
        assert_eq!(
            serde_json::from_str::<Request>(document).expect("document parses"),
            Request::Document {
                kind: DocumentKind::Plan,
                query: Query {
                    days: 7,
                    keep_latest: 2,
                    tool: ToolScope::Codex,
                    allow_running: false,
                }
            }
        );
        assert_eq!(
            serde_json::from_str::<Request>(r#"{"request":"goodbye"}"#).expect("goodbye parses"),
            Request::Goodbye
        );
    }

    #[test]
    fn version_one_response_frames_are_stable() {
        let welcome = Response::Welcome {
            api_version: 1,
            engine_version: "0.1.0".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&welcome).expect("serializes"),
            r#"{"response":"welcome","api_version":1,"engine_version":"0.1.0"}"#
        );
        assert_eq!(
            serde_json::to_string(&Response::error(ErrorCode::UnsupportedVersion, "no"))
                .expect("serializes"),
            r#"{"response":"error","code":"unsupported_version","message":"no","supported_versions":[1]}"#
        );
        assert_eq!(
            serde_json::to_string(&Response::error(ErrorCode::Unauthorized, "no"))
                .expect("serializes"),
            r#"{"response":"error","code":"unauthorized","message":"no","supported_versions":[]}"#
        );
    }

    #[test]
    fn no_mutating_request_is_expressible() {
        for frame in [
            r#"{"request":"clean"}"#,
            r#"{"request":"configure","days":1}"#,
            r#"{"request":"execute","plan":"plan-1"}"#,
            r#"{"request":"restore"}"#,
            r#"{"request":"document","kind":"clean","query":{"days":7,"keep_latest":2,"tool":"all"}}"#,
            r#"{"request":"document","kind":"plan","query":{"days":7,"keep_latest":2,"tool":"all","yes":true}}"#,
            r#"{"request":"document","kind":"plan","query":{"days":7,"keep_latest":2,"tool":"all"},"execute":true}"#,
        ] {
            assert!(serde_json::from_str::<Request>(frame).is_err(), "{frame}");
        }
    }

    #[test]
    fn the_default_query_matches_the_cli_defaults() {
        assert_eq!(
            Query::default(),
            Query {
                days: 7,
                keep_latest: 2,
                tool: ToolScope::All,
                allow_running: false,
            }
        );
    }
}

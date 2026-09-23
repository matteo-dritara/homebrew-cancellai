//! The desktop API boundary (E19-S01, `docs/architecture/TARGET.md` "Desktop API boundary").
//!
//! A narrow, versioned, locally-authenticated channel between the engine and a desktop client.
//! The engine side is hosted by `cancellai-cli desktop-api`; the client side is
//! `cancellai-desktop` (E19-S02). What travels over it is the engine's own read-only documents -
//! the inventory document `status --json` prints and the plan document `plan --json` prints -
//! so a desktop client shows the plans and explanations the engine produced rather than
//! re-deriving them.
//!
//! Three properties, each held by structure rather than by a check someone must remember:
//!
//! - **No bypass of the safety executor.** The request vocabulary ([`Request`]) has no mutating
//!   variant, and this crate depends on no crate that can reach a provider root or the mutation
//!   executor (see `Cargo.toml`).
//! - **Local authentication.** Every connection must open with the per-process
//!   [`SessionToken`], which only the process that started the server ever sees; the listener
//!   binds loopback only.
//! - **Versioning.** Every connection negotiates [`API_VERSION`]; an unsupported version is
//!   refused with the list of supported ones.

pub mod auth;
pub mod protocol;
pub mod session;
pub mod transport;

pub use auth::SessionToken;
pub use protocol::{
    API_VERSION, Descriptor, DocumentEnvelope, DocumentKind, ErrorCode, MAX_REQUEST_BYTES, Query,
    Request, Response, SUPPORTED_VERSIONS, ToolScope,
};
pub use session::{DocumentSource, Next, Session, read_frame, serve_connection};
pub use transport::{Client, ClientError, IDLE_TIMEOUT, Server};

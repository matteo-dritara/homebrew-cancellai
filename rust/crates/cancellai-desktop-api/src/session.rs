//! One connection's state machine, independent of any transport.
//!
//! A connection starts unauthenticated. Its first frame must be a `hello` carrying the session
//! token and a supported API version; anything else ends the connection with an error that
//! discloses no engine data. Once authenticated, the only thing a client can obtain is a
//! read-only document from the [`DocumentSource`] the host supplied.

use std::io::{self, BufRead, Read, Write};

use crate::auth::SessionToken;
use crate::protocol::{
    DocumentEnvelope, DocumentKind, ErrorCode, MAX_REQUEST_BYTES, Query, Request, Response,
    SUPPORTED_VERSIONS,
};

/// What the engine side supplies. The host (the CLI's `desktop-api` command) implements this
/// with the same code paths as `status --json` and `plan --json`.
pub trait DocumentSource {
    fn engine_version(&self) -> String;
    fn document(&self, kind: DocumentKind, query: Query) -> Result<DocumentEnvelope, String>;
}

/// Whether the connection continues after a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    Continue,
    Close,
}

/// The per-connection state.
#[derive(Debug)]
pub struct Session<'a> {
    token: &'a SessionToken,
    authenticated: bool,
}

impl<'a> Session<'a> {
    pub fn new(token: &'a SessionToken) -> Self {
        Self {
            token,
            authenticated: false,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.authenticated
    }

    /// Handles one frame (without its trailing newline).
    pub fn handle(&mut self, frame: &[u8], engine: &dyn DocumentSource) -> (Response, Next) {
        let request = match serde_json::from_slice::<Request>(frame) {
            Ok(request) => request,
            Err(_) => {
                let next = if self.authenticated {
                    Next::Continue
                } else {
                    Next::Close
                };
                return (
                    Response::error(ErrorCode::MalformedRequest, "not a desktop API request"),
                    next,
                );
            }
        };
        match (self.authenticated, request) {
            (false, Request::Hello { api_version, token }) => {
                if !self.token.matches(&token) {
                    return (
                        Response::error(ErrorCode::Unauthorized, "session token rejected"),
                        Next::Close,
                    );
                }
                if !SUPPORTED_VERSIONS.contains(&api_version) {
                    return (
                        Response::error(
                            ErrorCode::UnsupportedVersion,
                            format!("API version {api_version} is not supported"),
                        ),
                        Next::Close,
                    );
                }
                self.authenticated = true;
                (
                    Response::Welcome {
                        api_version,
                        engine_version: engine.engine_version(),
                    },
                    Next::Continue,
                )
            }
            (false, Request::Goodbye) => (Response::Farewell, Next::Close),
            (false, Request::Document { .. }) => (
                Response::error(ErrorCode::NotAuthenticated, "send hello first"),
                Next::Close,
            ),
            (true, Request::Hello { .. }) => (
                Response::error(ErrorCode::AlreadyAuthenticated, "already authenticated"),
                Next::Continue,
            ),
            (true, Request::Document { kind, query }) => match engine.document(kind, query) {
                Ok(envelope) => (Response::Document { envelope }, Next::Continue),
                Err(reason) => (
                    Response::error(ErrorCode::EngineUnavailable, reason),
                    Next::Continue,
                ),
            },
            (true, Request::Goodbye) => (Response::Farewell, Next::Close),
        }
    }
}

/// Reads one newline-terminated frame of at most `max` bytes. `Ok(None)` at a clean end of
/// stream; `Err` of kind `InvalidData` when the frame is too long.
pub fn read_frame(reader: &mut impl BufRead, max: usize) -> io::Result<Option<Vec<u8>>> {
    let mut frame = Vec::new();
    let limit = u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1);
    let read = reader.by_ref().take(limit).read_until(b'\n', &mut frame)?;
    if read == 0 {
        return Ok(None);
    }
    if frame.last() == Some(&b'\n') {
        frame.pop();
        if frame.last() == Some(&b'\r') {
            frame.pop();
        }
        return Ok(Some(frame));
    }
    if frame.len() > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    // A final frame with no trailing newline, cut short by end of stream.
    Ok(Some(frame))
}

pub(crate) fn write_frame(writer: &mut impl Write, response: &Response) -> io::Result<()> {
    let mut line = serde_json::to_vec(response).map_err(io::Error::other)?;
    line.push(b'\n');
    writer.write_all(&line)?;
    writer.flush()
}

/// Serves one connection until the client leaves, the session closes it, or I/O fails.
pub fn serve_connection(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    token: &SessionToken,
    engine: &dyn DocumentSource,
) -> io::Result<()> {
    let mut session = Session::new(token);
    loop {
        let frame = match read_frame(reader, MAX_REQUEST_BYTES) {
            Ok(Some(frame)) => frame,
            Ok(None) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                let response = Response::error(ErrorCode::FrameTooLarge, "request frame too large");
                return write_frame(writer, &response);
            }
            Err(error) => return Err(error),
        };
        let (response, next) = session.handle(&frame, engine);
        write_frame(writer, &response)?;
        if next == Next::Close {
            return Ok(());
        }
    }
}

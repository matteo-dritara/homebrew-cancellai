//! Loopback TCP transport and a client for it.
//!
//! Loopback TCP rather than a Unix socket or named pipe because it is the one local IPC std
//! offers identically on all three tier-1 platforms. It is reachable by every local process, so
//! it is never the authentication: the session token is. The listener binds `127.0.0.1` only,
//! and a connection whose peer is not loopback is dropped before a byte is read.

use std::io::{self, BufReader, BufWriter};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use crate::auth::SessionToken;
use crate::protocol::{
    API_VERSION, Descriptor, DocumentEnvelope, DocumentKind, ErrorCode, Query, Request, Response,
};
use crate::session::{DocumentSource, read_frame, serve_connection};

/// How long a connection may stay silent before the server drops it, so one idle or hostile
/// local client cannot hold a single-threaded server.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// The largest response frame a client reads. Documents for large inventories are big; this is a
/// ceiling against a runaway server, not a tuning value.
pub const MAX_RESPONSE_BYTES: usize = 256 * 1024 * 1024;

/// A bound, not yet serving, loopback server.
#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
    token: SessionToken,
}

impl Server {
    /// Binds an ephemeral port on 127.0.0.1 and draws a fresh session token.
    pub fn bind() -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let token = SessionToken::generate().map_err(io::Error::other)?;
        Ok(Self { listener, token })
    }

    /// What a client needs to connect. The host prints this once to its standard output.
    pub fn descriptor(&self) -> io::Result<Descriptor> {
        Ok(Descriptor {
            api_version: API_VERSION,
            address: self.listener.local_addr()?.to_string(),
            token: self.token.as_str().to_string(),
        })
    }

    /// Serves connections one at a time, until `max_connections` have been served (or forever
    /// when `None`). A connection that fails is logged by the caller's choice and never stops
    /// the server.
    pub fn serve(
        &self,
        engine: &dyn DocumentSource,
        max_connections: Option<usize>,
    ) -> io::Result<()> {
        let mut served = 0usize;
        while max_connections.is_none_or(|max| served < max) {
            let (stream, peer) = self.listener.accept()?;
            served = served.saturating_add(1);
            if !peer.ip().is_loopback() {
                continue;
            }
            // A connection-level failure (timeout, reset) ends that connection only.
            let _ = self.serve_stream(stream, engine);
        }
        Ok(())
    }

    fn serve_stream(&self, stream: TcpStream, engine: &dyn DocumentSource) -> io::Result<()> {
        stream.set_read_timeout(Some(IDLE_TIMEOUT))?;
        stream.set_write_timeout(Some(IDLE_TIMEOUT))?;
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut writer = BufWriter::new(stream);
        serve_connection(&mut reader, &mut writer, &self.token, engine)
    }
}

/// Why a client call failed.
#[derive(Debug)]
pub enum ClientError {
    Io(io::Error),
    /// The server refused the request.
    Refused {
        code: ErrorCode,
        message: String,
        supported_versions: Vec<u32>,
    },
    /// The server sent something this client does not understand.
    Protocol(String),
}

impl From<io::Error> for ClientError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// An authenticated connection to a desktop API server.
#[derive(Debug)]
pub struct Client {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
    engine_version: String,
}

impl Client {
    /// Connects to `descriptor.address` and authenticates with its token.
    pub fn connect(descriptor: &Descriptor) -> Result<Self, ClientError> {
        Self::connect_with(descriptor, API_VERSION)
    }

    /// As [`Client::connect`], announcing `api_version` - for compatibility tests.
    pub fn connect_with(descriptor: &Descriptor, api_version: u32) -> Result<Self, ClientError> {
        let address: SocketAddr = descriptor
            .address
            .parse()
            .map_err(|_| ClientError::Protocol(format!("bad address {}", descriptor.address)))?;
        if !address.ip().is_loopback() {
            return Err(ClientError::Protocol(
                "refusing a non-loopback desktop API address".to_string(),
            ));
        }
        let stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(IDLE_TIMEOUT))?;
        let mut client = Self {
            reader: BufReader::new(stream.try_clone()?),
            writer: BufWriter::new(stream),
            engine_version: String::new(),
        };
        match client.call(&Request::Hello {
            api_version,
            token: descriptor.token.clone(),
        })? {
            Response::Welcome { engine_version, .. } => {
                client.engine_version = engine_version;
                Ok(client)
            }
            other => Err(unexpected(other)),
        }
    }

    pub fn engine_version(&self) -> &str {
        &self.engine_version
    }

    /// Fetches one read-only document.
    pub fn document(
        &mut self,
        kind: DocumentKind,
        query: Query,
    ) -> Result<DocumentEnvelope, ClientError> {
        match self.call(&Request::Document { kind, query })? {
            Response::Document { envelope } => Ok(envelope),
            other => Err(unexpected(other)),
        }
    }

    /// Ends the session politely.
    pub fn close(mut self) -> Result<(), ClientError> {
        match self.call(&Request::Goodbye)? {
            Response::Farewell => Ok(()),
            other => Err(unexpected(other)),
        }
    }

    fn call(&mut self, request: &Request) -> Result<Response, ClientError> {
        use std::io::Write;
        let mut line = serde_json::to_vec(request).map_err(io::Error::other)?;
        line.push(b'\n');
        self.writer.write_all(&line)?;
        self.writer.flush()?;
        let frame = read_frame(&mut self.reader, MAX_RESPONSE_BYTES)?
            .ok_or_else(|| ClientError::Protocol("server closed the connection".to_string()))?;
        serde_json::from_slice(&frame).map_err(|error| ClientError::Protocol(error.to_string()))
    }
}

fn unexpected(response: Response) -> ClientError {
    match response {
        Response::Error {
            code,
            message,
            supported_versions,
        } => ClientError::Refused {
            code,
            message,
            supported_versions,
        },
        other => ClientError::Protocol(format!("unexpected response {other:?}")),
    }
}

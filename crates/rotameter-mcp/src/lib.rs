#![forbid(unsafe_code)]
//! Read-only `stdio` Model Context Protocol (MCP) server for Rotameter.

mod parse;
mod protocol;
mod schema;
mod tools;

use std::io::{self, BufRead, BufReader, BufWriter, Write};

use rotameter_client::types::{
    QueryRequest, QueryResponse, RealtimeResponse, SiteRef, SiteResponse,
};
use rotameter_client::{BlockingRotameterClient, ClientError, ComponentInfo};
use thiserror::Error;

/// Error returned by the MCP `stdio` transport loop.
#[derive(Debug, Error)]
pub enum McpError {
    /// Reading `stdin` or writing `stdout` failed.
    #[error("MCP stdio I/O failure: {0}")]
    Io(#[from] io::Error),
}

/// Strictly read-only analytics operations exposed to MCP tools.
pub trait AnalyticsBackend {
    /// Lists accessible sites (`GET /v1/admin/sites`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network or authorization errors.
    fn list_sites(&self) -> Result<Vec<SiteResponse>, ClientError>;

    /// Executes an analytics query (`POST /v1/query`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network, auth, or query validation errors.
    fn query(&self, req: &QueryRequest) -> Result<QueryResponse, ClientError>;

    /// Fetches trailing realtime visitor activity (`GET /v1/sites/{site}/realtime`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network, auth, or missing-site errors.
    fn realtime(&self, site: &SiteRef) -> Result<RealtimeResponse, ClientError>;

    /// Returns the optional default site configured on the client profile.
    fn default_site(&self) -> Option<&str>;
}

impl AnalyticsBackend for BlockingRotameterClient {
    fn list_sites(&self) -> Result<Vec<SiteResponse>, ClientError> {
        Self::list_sites(self)
    }

    fn query(&self, req: &QueryRequest) -> Result<QueryResponse, ClientError> {
        Self::query(self, req)
    }

    fn realtime(&self, site: &SiteRef) -> Result<RealtimeResponse, ClientError> {
        Self::realtime(self, site)
    }

    fn default_site(&self) -> Option<&str> {
        self.config().default_site.as_deref()
    }
}

/// Serves newline-delimited JSON-RPC 2.0 MCP requests from `reader` onto `writer`.
///
/// # Errors
///
/// Returns [`McpError::Io`] if reading input lines or flushing responses fails.
pub fn serve_stdio<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    backend: &dyn AnalyticsBackend,
) -> Result<(), McpError> {
    for line_res in reader.lines() {
        let line = line_res?;
        if let Some(reply) = protocol::handle_line(&line, backend) {
            writeln!(writer, "{reply}")?;
            writer.flush()?;
        }
    }
    Ok(())
}

/// Runs the production MCP `stdio` server over process `stdin`/`stdout`.
///
/// # Errors
///
/// Returns [`McpError::Io`] on stream read/write failure.
pub fn run_stdio(client: &BlockingRotameterClient) -> Result<(), McpError> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve_stdio(
        BufReader::new(stdin.lock()),
        BufWriter::new(stdout.lock()),
        client,
    )
}

/// Returns static metadata describing `rotameter-mcp`.
pub const fn component_info() -> ComponentInfo {
    ComponentInfo {
        name: "rotameter-mcp",
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Returns the underlying `rotameter-client` metadata linked by this crate.
pub const fn client_info() -> ComponentInfo {
    rotameter_client::component_info()
}

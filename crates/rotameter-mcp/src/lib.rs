#![forbid(unsafe_code)]
//! Model Context Protocol (MCP) server for Rotameter (`stdio`, read-only by default).

mod admin_tools;
mod backend;
mod parse;
mod protocol;
mod schema;
mod tools;

use std::io::{self, BufRead, BufReader, BufWriter, Write};

use rotameter_client::{BlockingRotameterClient, ComponentInfo};
use thiserror::Error;

pub use backend::AnalyticsBackend;

/// Runtime options controlling MCP tool exposure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct McpOptions {
    /// When `true`, exposes guarded site/key admin tools in addition to read-only analytics tools.
    pub allow_admin: bool,
}

/// Error returned by the MCP `stdio` transport loop.
#[derive(Debug, Error)]
pub enum McpError {
    /// Reading `stdin` or writing `stdout` failed.
    #[error("MCP stdio I/O failure: {0}")]
    Io(#[from] io::Error),
}

/// Serves newline-delimited JSON-RPC 2.0 MCP requests with default read-only options.
///
/// # Errors
///
/// Returns [`McpError::Io`] if reading input lines or flushing responses fails.
pub fn serve_stdio<R: BufRead, W: Write>(
    reader: R,
    writer: W,
    backend: &dyn AnalyticsBackend,
) -> Result<(), McpError> {
    serve_stdio_with_options(reader, writer, backend, McpOptions::default())
}

/// Serves newline-delimited JSON-RPC 2.0 MCP requests with explicit [`McpOptions`].
///
/// # Errors
///
/// Returns [`McpError::Io`] if reading input lines or flushing responses fails.
pub fn serve_stdio_with_options<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    backend: &dyn AnalyticsBackend,
    options: McpOptions,
) -> Result<(), McpError> {
    for line_res in reader.lines() {
        let line = line_res?;
        if let Some(reply) = protocol::handle_line(&line, backend, options) {
            writeln!(writer, "{reply}")?;
            writer.flush()?;
        }
    }
    Ok(())
}

/// Runs the production MCP `stdio` server over process `stdin`/`stdout` (read-only).
///
/// # Errors
///
/// Returns [`McpError::Io`] on stream read/write failure.
pub fn run_stdio(client: &BlockingRotameterClient) -> Result<(), McpError> {
    run_stdio_with_options(client, McpOptions::default())
}

/// Runs the production MCP `stdio` server over `stdin`/`stdout` with explicit [`McpOptions`].
///
/// # Errors
///
/// Returns [`McpError::Io`] on stream read/write failure.
pub fn run_stdio_with_options(
    client: &BlockingRotameterClient,
    options: McpOptions,
) -> Result<(), McpError> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve_stdio_with_options(
        BufReader::new(stdin.lock()),
        BufWriter::new(stdout.lock()),
        client,
        options,
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

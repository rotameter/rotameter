#![forbid(unsafe_code)]
//! Interactive `ratatui` flight-instrument terminal UI for Rotameter.

pub mod chart;
pub mod fmt;
pub mod input;
mod modals;
mod panels;
pub mod render;
mod runtime;
pub mod state;

use std::io;

use rotameter_client::types::{QueryRequest, QueryResponse, RealtimeResponse, SiteRef};
use rotameter_client::{BlockingRotameterClient, ComponentInfo};
use thiserror::Error;

use runtime::{TuiBackend, run_event_loop};

/// Error returned when entering raw terminal mode or rendering the TUI fails.
#[derive(Debug, Error)]
pub enum TuiError {
    /// Terminal control or stdout I/O error.
    #[error("terminal I/O error: {0}")]
    Io(#[from] io::Error),
}

struct ClientBackend(BlockingRotameterClient);

impl TuiBackend for ClientBackend {
    fn list_site_slugs(&self) -> Vec<String> {
        match self.0.list_sites() {
            Ok(sites) if !sites.is_empty() => sites.into_iter().map(|s| s.slug).collect(),
            _ => self.0.config().default_site.clone().into_iter().collect(),
        }
    }

    fn fetch_query(&self, req: &QueryRequest) -> Result<QueryResponse, String> {
        self.0.query(req).map_err(|err| err.to_string())
    }

    fn fetch_realtime(&self, site: &SiteRef) -> Result<RealtimeResponse, String> {
        self.0.realtime(site).map_err(|err| err.to_string())
    }

    fn default_site(&self) -> Option<String> {
        self.0.config().default_site.clone()
    }
}

/// Launches the interactive `ratatui` flight-instrument dashboard backed by `client`.
///
/// # Errors
///
/// Returns [`TuiError::Io`] if terminal raw mode setup, drawing, or cleanup fails.
pub fn run_tui(client: &BlockingRotameterClient) -> Result<(), TuiError> {
    run_event_loop(ClientBackend(client.clone()))
}

/// Returns static metadata describing `rotameter-tui`.
pub const fn component_info() -> ComponentInfo {
    ComponentInfo {
        name: "rotameter-tui",
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Returns the underlying `rotameter-client` metadata linked by this crate.
pub const fn client_info() -> ComponentInfo {
    rotameter_client::component_info()
}

#![forbid(unsafe_code)]
//! Runtime configuration and startup error types for `rotameter-server`.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// HTTP server and bounded ingestion pipeline configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// Socket address to bind the HTTP listener (`127.0.0.1:8000` by default).
    pub bind_addr: SocketAddr,
    /// Optional plaintext admin token (`rm_adm_...`) idempotently seeded on startup.
    pub bootstrap_admin_key: Option<String>,
    /// Capacity of the bounded in-memory ingestion channel (`16,384` events).
    pub queue_capacity: usize,
    /// Maximum wait before flushing a partial batch to storage (`1,000 ms`).
    pub flush_interval_ms: u64,
    /// Maximum queued events folded into a single `WriteBatch` transaction (`512`).
    pub max_batch_size: usize,
    /// Graceful `SIGTERM` / `SIGINT` drain timeout in seconds (`10 s`).
    pub shutdown_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000),
            bootstrap_admin_key: None,
            queue_capacity: 16_384,
            flush_interval_ms: 1_000,
            max_batch_size: 512,
            shutdown_timeout_secs: 10,
        }
    }
}

/// Startup or listener failure surfaced by [`crate::serve`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerError {
    message: String,
}

impl ServerError {
    /// Creates a new [`ServerError`] from a descriptive message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ServerError {}

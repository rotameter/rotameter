#![forbid(unsafe_code)]
//! Tracing setup, `TcpListener` lifecycle, and graceful `SIGTERM`/`SIGINT` drain.

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use crate::config::{ServerConfig, ServerError};
use crate::handlers::build_router;
use crate::state::{AppState, ServerStore};

/// Initializes `RUST_LOG`-configurable structured tracing subscriber for CLI/server runs.
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

/// Starts the HTTP server on `config.bind_addr` and blocks until `SIGINT` or `SIGTERM`,
/// draining queued batches and checkpointing WAL storage before returning.
///
/// # Errors
///
/// Returns [`ServerError`] if initialization, socket binding, serving, or WAL checkpoint fails.
pub async fn serve(config: ServerConfig, store: Arc<dyn ServerStore>) -> Result<(), ServerError> {
    let state = AppState::new(store.clone(), &config)?;
    let router = build_router(state.clone());

    let listener = TcpListener::bind(config.bind_addr)
        .await
        .map_err(|err| ServerError::new(format!("cannot bind {}: {err}", config.bind_addr)))?;
    tracing::info!(bind = %config.bind_addr, "rotameter HTTP server listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|err| ServerError::new(format!("HTTP server error: {err}")))?;

    let drain_timeout = Duration::from_secs(config.shutdown_timeout_secs.max(1));
    state.ingest.shutdown(drain_timeout).await;
    store
        .checkpoint_truncate()
        .map_err(|err| ServerError::new(format!("final WAL checkpoint failed: {err}")))?;
    tracing::info!("rotameter server drained and stopped cleanly");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                let _ = sigterm.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

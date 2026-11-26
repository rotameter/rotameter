#![forbid(unsafe_code)]
//! HTTP ingest, query, and admin server for Rotameter.

mod auth;
pub mod config;
mod error;
mod geoip;
pub mod handlers;
pub mod ingest;
mod rate_limit;
mod salt_store;
mod self_site;
pub mod serve;
pub mod state;

use rotameter_core::ComponentInfo;

pub use config::{
    GeoIpMode, RateLimitConfig, SaltStoreMode, ServerConfig, ServerError, ServerRole,
};
pub use handlers::build_router;
pub use serve::{init_tracing, serve};
pub use state::{AppState, ServerStores};

/// Returns static metadata describing `rotameter-server`.
pub const fn component_info() -> ComponentInfo {
    ComponentInfo {
        name: "rotameter-server",
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Returns the underlying `rotameter-core` metadata linked by this crate.
pub const fn core_info() -> ComponentInfo {
    rotameter_core::component_info()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_core() {
        assert_eq!(component_info().name, "rotameter-server");
        assert_eq!(component_info().version, core_info().version);
    }
}

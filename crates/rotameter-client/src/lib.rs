#![forbid(unsafe_code)]
//! Typed HTTP client for the Rotameter `/v1/*` API.

#[cfg(feature = "blocking")]
pub mod blocking;
pub mod client;
pub mod config;
pub mod error;
mod headers;
pub mod types;

pub use rotameter_core::ComponentInfo;

#[cfg(feature = "blocking")]
pub use blocking::BlockingRotameterClient;
pub use client::RotameterClient;
pub use config::{ClientConfig, ConfigFile, ConfigOverrides, ProfileConfig};
pub use error::ClientError;

/// Returns static metadata describing `rotameter-client`.
pub const fn component_info() -> ComponentInfo {
    ComponentInfo {
        name: "rotameter-client",
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
        assert_eq!(component_info().name, "rotameter-client");
        assert_eq!(component_info().version, core_info().version);
    }
}

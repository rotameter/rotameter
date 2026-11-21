#![forbid(unsafe_code)]
//! HTTP ingest, query, and admin server for Rotameter.

use rotameter_core::ComponentInfo;

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

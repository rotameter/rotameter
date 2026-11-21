#![forbid(unsafe_code)]
//! Bundled `SQLite` WAL storage backend for Rotameter.

use rotameter_core::ComponentInfo;

/// Returns static metadata describing `rotameter-store-sqlite`.
pub const fn component_info() -> ComponentInfo {
    ComponentInfo {
        name: "rotameter-store-sqlite",
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
        assert_eq!(component_info().name, "rotameter-store-sqlite");
        assert_eq!(component_info().version, core_info().version);
    }
}

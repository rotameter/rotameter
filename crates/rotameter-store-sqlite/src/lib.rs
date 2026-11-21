#![forbid(unsafe_code)]
//! Bundled `SQLite` WAL storage backend for Rotameter.

pub mod backup;
pub mod config;
mod control_plane;
mod ingest;
mod intern;
pub mod migrations;
pub mod query;
mod realtime;
mod retention;
mod row_decode;
pub mod store;

use rotameter_core::ComponentInfo;

pub use config::{SqliteConfig, SqliteHealthSnapshot};
pub use query::format_query_result_csv;
pub use store::SqliteStore;

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

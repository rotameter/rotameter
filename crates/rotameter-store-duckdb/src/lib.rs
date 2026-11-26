#![forbid(unsafe_code)]
//! Opt-in `DuckDB` OLAP [`rotameter_core::storage::AnalyticsDataPlane`] storage backend (`RM-34`).

mod ingest;
mod ingest_rollups;
mod query;
mod realtime;
mod retention;
mod schema;
pub mod store;

use rotameter_core::ComponentInfo;

pub use store::{DEFAULT_MEMORY_LIMIT, DEFAULT_THREADS, DuckDbConfig, DuckDbStore};

/// Returns static metadata describing `rotameter-store-duckdb`.
pub const fn component_info() -> ComponentInfo {
    ComponentInfo {
        name: "rotameter-store-duckdb",
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
        assert_eq!(component_info().name, "rotameter-store-duckdb");
        assert_eq!(component_info().version, core_info().version);
    }
}

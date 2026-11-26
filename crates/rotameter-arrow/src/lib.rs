#![forbid(unsafe_code)]
//! Canonical Arrow Schema v1, Parquet Zstd codecs, SHA-256 manifest, and `.tar.zst` archive exchange.

pub mod archive;
pub mod batch_codec;
mod col_ext;
pub mod events_codec;
pub mod manifest;
pub mod parquet_io;
pub mod query_codec;
pub mod rollups_codec;
pub mod schema;
pub mod sessions_codec;

use rotameter_core::ComponentInfo;
use thiserror::Error;

pub use archive::{pack_tar_zst, unpack_tar_zst};
pub use batch_codec::{WriteBatchArrow, arrow_to_write_batch, write_batch_to_arrow};
pub use events_codec::{events_to_record_batch, record_batch_to_events};
pub use manifest::{
    PORTABLE_FORMAT_VERSION, PortableManifest, PortableRowCounts, sha256_file, verify_manifest,
    write_manifest,
};
pub use parquet_io::{PartitionedParquetWriter, stream_parquet_batches, write_parquet_batches};
pub use query_codec::{QueryResultArrow, arrow_to_query_result, query_result_to_arrow};
pub use rollups_codec::{
    record_batch_to_rollups_daily, record_batch_to_rollups_hourly,
    record_batch_to_rollups_path_daily, rollups_daily_to_record_batch,
    rollups_hourly_to_record_batch, rollups_path_daily_to_record_batch,
};
pub use schema::{
    ARROW_SCHEMA_VERSION, events_schema, query_rows_schema, query_timeseries_schema,
    rollups_daily_schema, rollups_hourly_schema, rollups_path_daily_schema, sessions_schema,
};
pub use sessions_codec::{record_batch_to_sessions, sessions_to_record_batch};

/// Unified error type returned by `rotameter-arrow` codecs, Parquet I/O, and archive verification.
#[derive(Debug, Error)]
pub enum ArrowError {
    /// Apache Arrow schema or array conversion error.
    #[error("arrow codec error: {0}")]
    Arrow(#[from] arrow_schema::ArrowError),
    /// Apache Parquet reader or writer error.
    #[error("parquet I/O error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),
    /// Filesystem or archive compression error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Manifest or property JSON error.
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    /// Core domain validation error while decoding exchange records.
    #[error("domain error: {0}")]
    Core(#[from] rotameter_core::error::CoreError),
    /// Schema mismatch, corrupt checksum, or unsafe archive entry.
    #[error("exchange integrity error: {0}")]
    InvalidExchange(String),
}

/// Returns static metadata describing `rotameter-arrow`.
pub const fn component_info() -> ComponentInfo {
    ComponentInfo {
        name: "rotameter-arrow",
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Returns the underlying `rotameter-core` metadata linked by this crate.
pub const fn core_info() -> ComponentInfo {
    rotameter_core::component_info()
}

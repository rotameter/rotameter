#![forbid(unsafe_code)]
//! Typed `/v1/*` request and response DTOs plus re-exported domain types.

pub mod admin;
pub mod events;
pub mod query;
pub mod realtime;

pub use admin::{
    CreateKeyRequest, CreatedKeyResponse, DeleteSiteResponse, HealthResponse, KeyMetadataResponse,
    RevokeKeyResponse, SiteResponse, UpsertSiteRequest,
};
pub use events::{
    BatchEventItem, BatchEventsRequest, BatchEventsResponse, EventAcceptedResponse, EventPayload,
    MAX_BATCH_EVENTS,
};
pub use query::{QueryRequest, QueryResponse, QueryWindow, TimeseriesRow, WireTimeRange};
pub use realtime::{RealtimePulsePoint, RealtimeResponse};
pub use rotameter_core::id::{KeyId, OrgId, SiteId, SiteRef};
pub use rotameter_core::query::{
    Dimension, FilterOp, FilterValue, Granularity, Metric, QueryFilter, QueryFormat, QuerySort,
    RangePreset, SortDir,
};
pub use rotameter_core::storage::{BreakdownRow, KpiSummary, QueryMeta, RealtimePage};
pub use rotameter_core::types::{KeyScope, SaltRotation};

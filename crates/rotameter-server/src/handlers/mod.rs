#![forbid(unsafe_code)]
//! Axum `/v1/*` route registration for ingest, query, admin, collection, and health.

pub(crate) mod admin_backup;
pub(crate) mod admin_keys;
pub(crate) mod admin_sites;
pub(crate) mod admin_wire;
pub(crate) mod batch_wire;
pub(crate) mod collect;
pub(crate) mod events;
pub(crate) mod events_batch;
pub(crate) mod headers;
pub(crate) mod health;
pub(crate) mod mcp;
pub(crate) mod mcp_admin;
pub(crate) mod mcp_parse;
pub(crate) mod mcp_schema;
pub(crate) mod mcp_tools;
pub(crate) mod query;
pub(crate) mod query_wire;

use axum::Router;
use axum::routing::{delete, get, post};

use crate::state::AppState;

/// Builds the `/v1/*` HTTP [`Router`] filtered by `state.role`.
pub fn build_router(state: AppState) -> Router {
    let mut router = Router::new().route("/v1/health", get(health::get_health));
    if state.role.allows_ingest() {
        router = router
            .route("/v1/collect.js", get(collect::get_collect_js))
            .route("/v1/pixel.gif", get(collect::get_pixel_gif))
            .route("/v1/events", post(events::post_event))
            .route("/v1/events/batch", post(events_batch::post_events_batch));
    }
    if state.role.allows_query() {
        if state.mcp_http_enabled {
            router = router.route(
                "/mcp",
                post(mcp::post_mcp)
                    .get(mcp::get_mcp)
                    .delete(mcp::delete_mcp),
            );
        }
        router = router
            .route("/v1/query", post(query::post_query))
            .route("/v1/sites/{site}/realtime", get(query::get_realtime))
            .route(
                "/v1/admin/sites",
                post(admin_sites::create_site).get(admin_sites::list_sites),
            )
            .route(
                "/v1/admin/sites/{site}",
                get(admin_sites::get_site)
                    .put(admin_sites::put_site)
                    .delete(admin_sites::delete_site),
            )
            .route(
                "/v1/admin/keys",
                post(admin_keys::create_key).get(admin_keys::list_keys),
            )
            .route("/v1/admin/keys/{key_id}", delete(admin_keys::revoke_key))
            .route("/v1/admin/backup", post(admin_backup::post_backup));
    }
    router.with_state(state)
}
